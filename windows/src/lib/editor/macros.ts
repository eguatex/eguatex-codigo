// Macros: ao digitar o texto de uma macro e depois um espaco, o texto vira o codigo.
// `substituirMacro` e pura e recebe o texto da linha antes do cursor (sem o espaco digitado).

import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';

export interface MacroEditor {
  texto: string;
  codigo: string;
}

export interface SubstituicaoMacro {
  /** Quantos caracteres antes do cursor saem (o texto da macro). */
  apagar: number;
  /** O que entra no lugar. O espaco digitado e consumido. */
  inserir: string;
}

const ESPACO = /\s/;

/**
 * Devolve a substituicao se `antes` termina com o texto de alguma macro inteira (comeca no inicio da linha
 * ou depois de um espaco). Texto parcial, macro sem texto e macro sem codigo nao substituem.
 * Com duas macros possiveis, vale a de texto mais longo.
 */
export function substituirMacro(antes: string, macros: readonly MacroEditor[]): SubstituicaoMacro | null {
  let melhor: MacroEditor | null = null;
  for (const m of macros) {
    if (!m.texto || !m.codigo || ESPACO.test(m.texto)) continue;
    if (!antes.endsWith(m.texto)) continue;
    const inicio = antes.length - m.texto.length;
    if (inicio > 0 && !ESPACO.test(antes[inicio - 1])) continue;
    if (!melhor || m.texto.length > melhor.texto.length) melhor = m;
  }
  return melhor ? { apagar: melhor.texto.length, inserir: melhor.codigo } : null;
}

/** Extensao do CodeMirror: espaco digitado depois de uma macro troca o texto pelo codigo. `obter` le a lista de agora. */
export function extensaoMacros(obter: () => readonly MacroEditor[]): Extension {
  return EditorView.inputHandler.of((view, from, to, texto) => {
    if (texto !== ' ' || from !== to) return false;
    const macros = obter();
    if (macros.length === 0) return false;
    const linha = view.state.doc.lineAt(from);
    const troca = substituirMacro(view.state.sliceDoc(linha.from, from), macros);
    if (!troca) return false;
    view.dispatch({
      changes: { from: from - troca.apagar, to: from, insert: troca.inserir },
      selection: { anchor: from - troca.apagar + troca.inserir.length },
      scrollIntoView: true,
      userEvent: 'input.type',
    });
    return true;
  });
}
