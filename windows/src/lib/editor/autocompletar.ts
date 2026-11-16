// Autocompletar de comandos LaTeX para o CodeMirror 6.
// Os comandos e as explicacoes (em portugues) vem de dados/comandos-latex.json.
// Ideia e comportamento inspirados em services/web/frontend/js/features/source-editor/extensions/auto-complete.ts (Overleaf, AGPL-3.0).
import {
  acceptCompletion,
  autocompletion,
  snippet,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete';
import { Prec, type Extension } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import comandosBrutos from '../../../dados/comandos-latex.json';
import { fonteReferencias } from './referencias';

interface Entrada {
  comando: string;
  categoria: string;
  explicacao: string;
  exemplo: string;
}

// Aceita tanto a lista direta quanto { comandos: [...] }, e ignora itens malformados.
function lerEntradas(dados: unknown): Entrada[] {
  const lista: unknown = Array.isArray(dados) ? dados : (dados as { comandos?: unknown } | null)?.comandos;
  if (!Array.isArray(lista)) return [];
  const saida: Entrada[] = [];
  for (const item of lista) {
    if (typeof item !== 'object' || item === null) continue;
    const r = item as Record<string, unknown>;
    if (typeof r.comando !== 'string' || !r.comando.startsWith('\\')) continue;
    saida.push({
      comando: r.comando,
      categoria: typeof r.categoria === 'string' ? r.categoria : '',
      explicacao: typeof r.explicacao === 'string' ? r.explicacao : '',
      exemplo: typeof r.exemplo === 'string' ? r.exemplo : '',
    });
  }
  return saida;
}

const ENTRADAS = lerEntradas(comandosBrutos);

// Icone de cada categoria na lista de sugestoes (tipos que o CodeMirror ja conhece).
const TIPOS: Record<string, string> = {
  estrutura: 'keyword',
  preambulo: 'namespace',
  matematica: 'function',
  ambientes: 'class',
  formatacao: 'text',
  referencias: 'variable',
  bibliografia: 'interface',
  figuras: 'constant',
  tabelas: 'enum',
  listas: 'enum',
  geometria: 'property',
};

// Ambientes sugeridos depois de "\begin{", mesmo que o JSON nao os traga.
const AMBIENTES_BASE = [
  'itemize',
  'enumerate',
  'figure',
  'table',
  'tabular',
  'equation',
  'align',
  'center',
  'abstract',
  'quote',
];

// Modelos que nao se deduzem do texto do JSON.
const MODELOS_ESPECIAIS: Record<string, string> = {
  '\\left': '\\left( ${} \\right)',
  '\\verb': '\\verb|${texto}|',
};

const RE_AMBIENTE = /^\\begin\{([^}]+)\}([\s\S]*?)\.\.\.\\end\{\1\}$/;
const RE_NOME = /^\\[a-zA-Z]+\*?/;

// Conteudo entre chaves que deve ficar como esta (nao vira campo editavel).
function ehLiteral(miolo: string): boolean {
  if (miolo === '\\nome') return false;
  return miolo.startsWith('\\') || !/[a-zA-Z]/.test(miolo);
}

// Transforma "{titulo}[opc]{x}" em "{${titulo}}{${x}}": argumentos obrigatorios viram campos
// (Tab pula de um para outro) e os opcionais entre colchetes ficam de fora.
function gerarCampos(resto: string): string {
  let saida = '';
  let i = 0;
  while (i < resto.length) {
    const c = resto[i];
    if (c !== '{' && c !== '[') {
      saida += c;
      i++;
      continue;
    }
    const fecha = c === '{' ? '}' : ']';
    let nivel = 0;
    let j = i;
    for (; j < resto.length; j++) {
      if (resto[j] === c) nivel++;
      else if (resto[j] === fecha && --nivel === 0) break;
    }
    if (j >= resto.length) {
      saida += resto.slice(i);
      break;
    }
    const miolo = resto.slice(i + 1, j);
    if (c === '{') saida += ehLiteral(miolo) ? '{' + miolo + '}' : '{${' + miolo + '}}';
    i = j + 1;
  }
  return saida;
}

function modeloDoAmbiente(nome: string, argumentos: string): string {
  const recuo = nome === 'document' ? '' : '\t';
  return '\\begin{' + nome + '}' + gerarCampos(argumentos) + '\n' + recuo + '${}\n\\end{' + nome + '}';
}

function criarInfo(e: Entrada): () => Node {
  return () => {
    const raiz = document.createElement('div');
    raiz.className = 'cm-eguatex-info';
    const texto = document.createElement('p');
    texto.textContent = e.explicacao;
    raiz.appendChild(texto);
    if (e.exemplo) {
      const exemplo = document.createElement('pre');
      exemplo.textContent = e.exemplo;
      raiz.appendChild(exemplo);
    }
    return raiz;
  };
}

// Casamentos de igual qualidade seguem a ordem do JSON (e a lista de AMBIENTES_BASE), nao a ordem alfabetica.
const ORDEM = new WeakMap<Completion, number>();

function montarOpcoes(): { comandos: Completion[]; ambientes: Completion[] } {
  const comandos: Completion[] = [];
  const ambientes = new Map<string, Completion>();

  for (const e of ENTRADAS) {
    const amb = RE_AMBIENTE.exec(e.comando);
    if (amb) {
      const nome = amb[1];
      const argumentos = amb[2];
      comandos.push({
        label: '\\begin{' + nome + '}',
        detail: argumentos.trim(),
        type: 'class',
        info: criarInfo(e),
        apply: snippet(modeloDoAmbiente(nome, argumentos)),
      });
      ambientes.set(nome, aplicavelDepoisDeBegin(nome, argumentos, e));
      continue;
    }
    const m = RE_NOME.exec(e.comando);
    if (!m) continue;
    const nome = m[0];
    const resto = e.comando.slice(nome.length);
    const modelo = MODELOS_ESPECIAIS[nome] ?? nome + gerarCampos(resto);
    comandos.push({
      label: nome,
      detail: resto.trim(),
      type: TIPOS[e.categoria] ?? 'text',
      info: criarInfo(e),
      apply: modelo === nome ? undefined : snippet(modelo),
    });
  }

  // Os dez ambientes comuns vem primeiro (na ordem de AMBIENTES_BASE) e sempre aparecem.
  const ordenados: Completion[] = [];
  for (const nome of AMBIENTES_BASE) {
    ordenados.push(ambientes.get(nome) ?? aplicavelDepoisDeBegin(nome, '', null));
    ambientes.delete(nome);
  }
  ordenados.push(...ambientes.values());
  comandos.forEach((c, i) => ORDEM.set(c, i));
  ordenados.forEach((c, i) => ORDEM.set(c, i));
  return { comandos, ambientes: ordenados };
}

// Depois de "\begin{" o usuario ja digitou o comeco; ao aceitar, entram o nome, "}", a linha vazia e o \end.
function aplicavelDepoisDeBegin(nome: string, argumentos: string, entrada: Entrada | null): Completion {
  const aplicar = snippet(nome + '}' + modeloDoAmbiente(nome, argumentos).slice(('\\begin{' + nome + '}').length));
  return {
    label: nome,
    detail: argumentos.trim(),
    type: 'class',
    info: entrada ? criarInfo(entrada) : undefined,
    apply: (view: EditorView, completion: Completion, from: number, to: number) => {
      // o fecha-chaves automatico pode ja estar logo depois do cursor
      const fim = view.state.sliceDoc(to, to + 1) === '}' ? to + 1 : to;
      aplicar(view, completion, from, fim);
    },
  };
}

const OPCOES = montarOpcoes();

function dentroDeComentario(contexto: CompletionContext): boolean {
  const linha = contexto.state.doc.lineAt(contexto.pos);
  const antes = linha.text.slice(0, contexto.pos - linha.from);
  return /(^|[^\\])%/.test(antes);
}

function fonteComandos(contexto: CompletionContext): CompletionResult | null {
  const achou = contexto.matchBefore(/\\[a-zA-Z]*$/);
  if (!achou) return null;
  // "\\" e quebra de linha, nao comeco de comando
  if (achou.from > 0 && contexto.state.sliceDoc(achou.from - 1, achou.from) === '\\') return null;
  if (dentroDeComentario(contexto)) return null;
  return { from: achou.from, options: OPCOES.comandos, validFor: /^\\[a-zA-Z]*$/ };
}

function fonteAmbientes(contexto: CompletionContext): CompletionResult | null {
  const achou = contexto.matchBefore(/\\begin\{[a-zA-Z*]*$/);
  if (!achou || dentroDeComentario(contexto)) return null;
  return {
    from: achou.from + '\\begin{'.length,
    options: OPCOES.ambientes,
    validFor: /^[a-zA-Z*]*$/,
  };
}

// Visual da lista de sugestoes, so com os tokens de cor do app.
const temaAutocompletar = EditorView.theme({
  '.cm-tooltip.cm-tooltip-autocomplete': {
    background: 'var(--superficie-0)',
    color: 'var(--texto)',
    border: '1px solid var(--borda)',
    borderRadius: '6px',
    boxShadow: 'var(--sombra-2)',
    overflow: 'hidden',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': {
    fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
    fontSize: '0.8125rem',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
    padding: '2px 8px',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    background: 'var(--primaria)',
    color: '#fff',
  },
  '.cm-tooltip.cm-tooltip-autocomplete .cm-completionDetail': {
    color: 'var(--texto-terciario)',
    marginLeft: '0.6em',
    fontStyle: 'normal',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected] .cm-completionDetail': {
    color: 'inherit',
    opacity: '0.85',
  },
  '.cm-tooltip.cm-completionInfo': {
    background: 'var(--superficie-1)',
    color: 'var(--texto)',
    border: '1px solid var(--borda)',
    borderRadius: '6px',
    maxWidth: '340px',
    padding: '6px 10px',
  },
  '.cm-eguatex-info p': {
    margin: '0 0 6px',
    fontFamily: "'Segoe UI', Inter, system-ui, sans-serif",
    fontSize: '0.8125rem',
    lineHeight: '1.4',
  },
  '.cm-eguatex-info pre': {
    margin: '0',
    whiteSpace: 'pre-wrap',
    fontSize: '0.75rem',
    color: 'var(--texto-secundario)',
  },
});

export const autocompletarLatex: Extension = [
  autocompletion({
    override: [fonteReferencias, fonteComandos, fonteAmbientes],
    activateOnTyping: true,
    maxRenderedOptions: 60,
    compareCompletions: (a, b) => (ORDEM.get(a) ?? 0) - (ORDEM.get(b) ?? 0),
  }),
  // Tab tambem aceita a sugestao; sem lista aberta, a tecla segue para os outros atalhos
  Prec.highest(keymap.of([{ key: 'Tab', run: acceptCompletion }])),
  temaAutocompletar,
];
