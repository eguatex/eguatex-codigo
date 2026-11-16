// Autocompletar de \ref{ com os \label do projeto e de \cite{ com as chaves dos .bib, e previa da figura
// de um \includegraphics. Versao do Android da fonte do EguaTex para Windows (src/lib/editor/referencias.ts):
// sem os avisos de referencia nem a previa de formulas, so as sugestoes e a previa de figuras.
import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import type { Extension } from '@codemirror/state';
import { EditorView, hoverTooltip, type Tooltip } from '@codemirror/view';
import { invoke } from '@tauri-apps/api/core';
import { ehComandoCitacao, ehComandoReferencia } from './analise-latex';
import { arquivoDoEditor, bibliografiaDoProjeto, NOME_DO_TIPO, raizDoProjeto, rotulosDoProjeto } from './projeto';

const RE_ARGUMENTO_ABERTO = /\\([a-zA-Z]+)\*?\s*(?:\[[^\]\n]*\]\s*){0,2}\{([^}\n]*)$/;

function resumoAutor(autor: string): string {
  const primeiro = autor.split(/\s+and\s+/i)[0] ?? '';
  const sobrenome = primeiro.includes(',') ? primeiro.split(',')[0] : primeiro.split(' ').at(-1) ?? '';
  return sobrenome + (autor.split(/\s+and\s+/i).length > 1 ? ' et al.' : '');
}

function infoTexto(linhas: string[]): () => Node {
  return () => {
    const raiz = document.createElement('div');
    raiz.className = 'cm-eguatex-info';
    for (const l of linhas.filter(Boolean)) {
      const p = document.createElement('p');
      p.textContent = l;
      raiz.appendChild(p);
    }
    return raiz;
  };
}

/** Sugestoes dentro de \ref{...} (rotulos) e de \cite{...} (chaves da bibliografia). */
export function fonteReferencias(contexto: CompletionContext): CompletionResult | null {
  const linha = contexto.state.doc.lineAt(contexto.pos);
  const antes = linha.text.slice(0, contexto.pos - linha.from);
  if (/(^|[^\\])%/.test(antes)) return null;
  const m = RE_ARGUMENTO_ABERTO.exec(antes);
  if (!m) return null;
  const comando = m[1];
  const digitado = m[2].slice(m[2].lastIndexOf(',') + 1).replace(/^\s+/, '');
  const from = contexto.pos - digitado.length;

  if (ehComandoReferencia(comando)) {
    const opcoes: Completion[] = [];
    for (const [nome, lista] of rotulosDoProjeto(contexto.state)) {
      const r = lista[0];
      const equacao = r.tipo === 'equacao';
      opcoes.push({
        label: nome,
        detail: NOME_DO_TIPO[r.tipo] ?? '',
        type: 'variable',
        boost: comando === 'eqref' ? (equacao ? 10 : -10) : 0,
        info: infoTexto([r.descricao, `${r.arquivo}, linha ${r.linha}`]),
      });
    }
    return { from, options: opcoes, validFor: /^[^,}\s]*$/ };
  }
  if (ehComandoCitacao(comando)) {
    const opcoes: Completion[] = [];
    for (const e of bibliografiaDoProjeto(contexto.state).values()) {
      opcoes.push({
        label: e.chave,
        detail: [resumoAutor(e.autor), e.ano].filter(Boolean).join(', '),
        type: 'interface',
        info: infoTexto([e.titulo, e.autor, [e.onde, e.ano].filter(Boolean).join(', '), `${e.arquivo}, linha ${e.linha}`]),
      });
    }
    return { from, options: opcoes, validFor: /^[^,}\s]*$/ };
  }
  return null;
}

// ---------- previa da figura do \includegraphics ----------

const EXTENSOES = ['', '.pdf', '.png', '.jpg', '.jpeg'];
const PASTAS_COMUNS = ['', 'figuras/', 'imagens/', 'figures/', 'images/', 'img/', 'fig/'];
const imagens = new Map<string, Promise<HTMLElement | null>>();
// blob URL de cada previa guardada: sai junto com ela, senao a imagem fica na memoria ate fechar o app
const urlsDasImagens = new Map<string, string>();

function esquecerImagem(chave: string) {
  imagens.delete(chave);
  const url = urlsDasImagens.get(chave);
  if (url) URL.revokeObjectURL(url);
  urlsDasImagens.delete(chave);
}

function pastasDeImagem(doc: string): string[] {
  const m = /\\graphicspath\s*\{((?:\s*\{[^}]*\})+)\s*\}/.exec(doc);
  const declaradas = m ? [...m[1].matchAll(/\{([^}]*)\}/g)].map((x) => x[1]) : [];
  return [...declaradas, ...PASTAS_COMUNS];
}

async function lerBytes(relativo: string): Promise<{ caminho: string; bytes: ArrayBuffer } | null> {
  try {
    // cmd_ler_bytes_projeto confere o caminho dentro do projeto e o limite de 25 MB
    const bytes = await invoke<ArrayBuffer>('cmd_ler_bytes_projeto', { caminhoProjeto: raizDoProjeto(), caminhoRelativo: relativo });
    return { caminho: relativo, bytes };
  } catch {
    return null;
  }
}

async function acharImagem(nome: string, doc: string): Promise<{ caminho: string; bytes: ArrayBuffer } | null> {
  const atual = arquivoDoEditor();
  const pastaAtual = atual.includes('/') ? atual.slice(0, atual.lastIndexOf('/') + 1) : '';
  const limpo = nome.replace(/^\.\//, '');
  for (const pasta of pastasDeImagem(doc)) {
    for (const base of new Set([pastaAtual + pasta, pasta])) {
      for (const ext of EXTENSOES) {
        const achou = await lerBytes(base + limpo + ext);
        if (achou) return achou;
      }
    }
  }
  return null;
}

function caixa(classe = ''): HTMLDivElement {
  const el = document.createElement('div');
  el.className = 'cm-eguatex-previa ' + classe;
  return el;
}

function paragrafo(pai: HTMLElement, texto: string, classe = '') {
  if (!texto) return;
  const p = document.createElement('p');
  p.textContent = texto;
  if (classe) p.className = classe;
  pai.appendChild(p);
}

async function desenharPdf(bytes: ArrayBuffer): Promise<HTMLElement> {
  const pdfjs = await import('pdfjs-dist');
  pdfjs.GlobalWorkerOptions.workerSrc = new URL('pdfjs-dist/build/pdf.worker.mjs', import.meta.url).toString();
  const pdf = await pdfjs.getDocument({ data: new Uint8Array(bytes) }).promise;
  const pagina = await pdf.getPage(1);
  const base = pagina.getViewport({ scale: 1 });
  const escala = Math.min(320 / base.width, 240 / base.height) * (window.devicePixelRatio || 1);
  const vp = pagina.getViewport({ scale: escala });
  const canvas = document.createElement('canvas');
  canvas.width = vp.width;
  canvas.height = vp.height;
  canvas.style.width = `${vp.width / (window.devicePixelRatio || 1)}px`;
  await pagina.render({ canvasContext: canvas.getContext('2d')!, viewport: vp }).promise;
  void pdf.destroy();
  return canvas;
}

function previaImagem(nome: string, doc: string): Promise<HTMLElement | null> {
  const chave = arquivoDoEditor() + '|' + nome;
  let p = imagens.get(chave);
  if (!p) {
    p = (async () => {
      const el = caixa('imagem');
      const achou = await acharImagem(nome, doc);
      if (!achou) {
        paragrafo(el, `A imagem "${nome}" não foi encontrada no projeto.`);
        return el;
      }
      try {
        if (achou.caminho.toLowerCase().endsWith('.pdf')) el.appendChild(await desenharPdf(achou.bytes));
        else {
          const img = document.createElement('img');
          const url = URL.createObjectURL(new Blob([achou.bytes]));
          urlsDasImagens.set(chave, url);
          img.src = url;
          el.appendChild(img);
        }
      } catch {
        paragrafo(el, 'Não foi possível mostrar esta imagem.');
      }
      paragrafo(el, achou.caminho, 'origem');
      return el;
    })();
    imagens.set(chave, p);
    // a imagem pode mudar no disco: a previa guardada vale por 30 s
    setTimeout(() => esquecerImagem(chave), 30_000);
  }
  return p;
}

const RE_INCLUDEGRAPHICS = /\\includegraphics\*?\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}/g;

const previa = hoverTooltip(async (view, pos): Promise<Tooltip | null> => {
  const state = view.state;
  const linha = state.doc.lineAt(pos);
  RE_INCLUDEGRAPHICS.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = RE_INCLUDEGRAPHICS.exec(linha.text))) {
    const de = linha.from + m.index;
    const ate = de + m[0].length;
    if (pos < de || pos > ate) continue;
    const dom = await previaImagem(m[1].trim(), state.doc.toString());
    if (dom) return { pos: de, end: ate, above: true, create: () => ({ dom }) };
  }
  return null;
}, { hoverTime: 350 });

const temaPrevia = EditorView.theme({
  '.cm-tooltip:has(> .cm-eguatex-previa)': {
    background: 'var(--superficie-0)',
    color: 'var(--texto)',
    border: '1px solid var(--borda)',
    borderRadius: '6px',
    boxShadow: 'var(--sombra-2)',
  },
  '.cm-eguatex-previa': {
    maxWidth: '380px',
    padding: '6px 10px',
    fontFamily: "'Segoe UI', Inter, system-ui, sans-serif",
    fontSize: '0.8125rem',
    lineHeight: '1.4',
  },
  '.cm-eguatex-previa p': { margin: '2px 0' },
  '.cm-eguatex-previa .origem': { color: 'var(--texto-terciario)', fontSize: '0.75rem' },
  '.cm-eguatex-previa img, .cm-eguatex-previa canvas': {
    display: 'block',
    maxWidth: '320px',
    maxHeight: '240px',
    background: '#fff',
    borderRadius: '4px',
  },
});

export const previaFiguras: Extension = [previa, temaPrevia];
