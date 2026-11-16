// Corretor ortografico no editor: sublinha em vermelho as palavras que o dicionario
// pt-BR (VERO, no Rust) nao conhece. Comandos, matematica, rotulos e citacoes ficam de fora.
// O dicionario pessoal (palavras adicionadas pelo usuario) fica no localStorage.
import { Facet, StateEffect, StateField, type EditorState, type Extension, type Range } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from '@codemirror/view';
import { invoke } from '@tauri-apps/api/core';
import { palavrasParaVerificar, type Palavra } from './analise-latex';

/** Liga o corretor. */
export const ortografiaLigada = Facet.define<boolean, boolean>({ combine: (v) => v.some(Boolean) });
/** Idioma do dicionario: 'pt' (padrao) ou 'en'. */
export const idiomaOrtografia = Facet.define<string, string>({ combine: (v) => v[0] ?? 'pt' });

const CHAVE_PESSOAL = 'eguatex.dicionario-pessoal';
/** Documento maior que isso nao e conferido (evita travar em arquivos enormes). */
const LIMITE_CARACTERES = 600_000;
const ESPERA_MS = 500;

function lerPessoal(): Set<string> {
  try {
    const lista: unknown = JSON.parse(localStorage.getItem(CHAVE_PESSOAL) ?? '[]');
    return new Set(Array.isArray(lista) ? lista.filter((p): p is string => typeof p === 'string') : []);
  } catch {
    return new Set();
  }
}

const pessoal = lerPessoal();
/** Resposta do dicionario por idioma e palavra ("en:word"): true = certa. */
const conhecidas = new Map<string, boolean>();
const chave = (idioma: string, palavra: string) => idioma + ':' + palavra;
/** Falha ao carregar o dicionario: o corretor fica quieto. */
let indisponivel = false;

const definirErros = StateEffect.define<Palavra[]>();

const marca = Decoration.mark({ class: 'cm-erro-ortografia' });

const campoErros = StateField.define<{ lista: Palavra[]; deco: DecorationSet }>({
  create: () => ({ lista: [], deco: Decoration.none }),
  update(valor, tr) {
    for (const e of tr.effects) {
      if (e.is(definirErros)) {
        const enfeites: Range<Decoration>[] = e.value.map((p) => marca.range(p.de, p.ate));
        return { lista: e.value, deco: Decoration.set(enfeites, true) };
      }
    }
    if (!tr.docChanged) return valor;
    // enquanto a nova conferencia nao chega, as marcas acompanham o texto
    const lista = valor.lista
      .map((p) => ({ ...p, de: tr.changes.mapPos(p.de, 1), ate: tr.changes.mapPos(p.ate, -1) }))
      .filter((p) => p.ate > p.de && tr.state.sliceDoc(p.de, p.ate) === p.palavra);
    return { lista, deco: valor.deco.map(tr.changes) };
  },
  provide: (f) => EditorView.decorations.from(f, (v) => v.deco),
});

function erradas(palavras: Palavra[], idioma: string): Palavra[] {
  return palavras.filter((p) => conhecidas.get(chave(idioma, p.palavra)) === false && !pessoal.has(p.palavra));
}

const conferidor = ViewPlugin.fromClass(
  class {
    timer: ReturnType<typeof setTimeout> | undefined;
    destruido = false;

    constructor(readonly view: EditorView) {
      this.agendar();
    }

    update(u: ViewUpdate) {
      const ligou =
        u.startState.facet(ortografiaLigada) !== u.state.facet(ortografiaLigada) ||
        u.startState.facet(idiomaOrtografia) !== u.state.facet(idiomaOrtografia);
      if (u.docChanged || ligou) this.agendar();
    }

    agendar() {
      clearTimeout(this.timer);
      this.timer = setTimeout(() => void this.conferir(), ESPERA_MS);
    }

    async conferir() {
      const view = this.view;
      if (this.destruido) return;
      if (!view.state.facet(ortografiaLigada) || indisponivel || view.state.doc.length > LIMITE_CARACTERES) {
        if (view.state.field(campoErros).lista.length) view.dispatch({ effects: definirErros.of([]) });
        return;
      }
      const doc = view.state.doc;
      const idioma = view.state.facet(idiomaOrtografia);
      const palavras = palavrasParaVerificar(doc.toString());
      const novas = [...new Set(palavras.map((p) => p.palavra))].filter((p) => !conhecidas.has(chave(idioma, p)) && !pessoal.has(p));
      try {
        for (let i = 0; i < novas.length; i += 3000) {
          const lote = novas.slice(i, i + 3000);
          const ruins = new Set(await invoke<string[]>('cmd_ortografia_verificar', { palavras: lote, idioma }));
          for (const p of lote) conhecidas.set(chave(idioma, p), !ruins.has(p));
        }
      } catch {
        indisponivel = true;
        return;
      }
      // o texto mudou durante a conferencia: a proxima rodada cuida dele
      if (this.destruido || view.state.doc !== doc || view.state.facet(idiomaOrtografia) !== idioma) return;
      view.dispatch({ effects: definirErros.of(erradas(palavras, idioma)) });
    }

    destroy() {
      this.destruido = true;
      clearTimeout(this.timer);
    }
  },
);

/** A palavra marcada como errada na posicao, se houver. */
export function palavraErradaEm(state: EditorState, pos: number): Palavra | null {
  return state.field(campoErros, false)?.lista.find((p) => p.de <= pos && pos <= p.ate) ?? null;
}

export async function sugestoesPara(palavra: string, idioma = 'pt'): Promise<string[]> {
  try {
    return await invoke<string[]>('cmd_ortografia_sugerir', { palavra, idioma });
  } catch {
    return [];
  }
}

/** Guarda a palavra no dicionario pessoal e tira a marca dela em todo o texto. */
export function adicionarAoDicionario(view: EditorView, palavra: string) {
  pessoal.add(palavra);
  try {
    localStorage.setItem(CHAVE_PESSOAL, JSON.stringify([...pessoal].sort()));
  } catch {
    // sem localStorage a palavra vale ate fechar o app
  }
  const lista = view.state.field(campoErros).lista.filter((p) => p.palavra !== palavra);
  view.dispatch({ effects: definirErros.of(lista) });
}

const temaOrtografia = EditorView.baseTheme({
  '.cm-erro-ortografia': {
    textDecoration: 'underline wavy #d32f2f',
    textDecorationSkipInk: 'none',
    textUnderlineOffset: '3px',
  },
});

export const ortografia: Extension = [campoErros, conferidor, temaOrtografia];
