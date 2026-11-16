// Textos que o CodeMirror escreve sozinho (painel do Ctrl+F, ir para a linha, dobra de código).
// A chave é a frase em inglês que o CodeMirror pede; o valor é a chave de pt-BR.json.
import { EditorState } from '@codemirror/state';
import { t } from '$lib/i18n';

type Chave = Parameters<typeof t>[0];

export const FRASES: Record<string, Chave> = {
  'Find': 'editor.busca.find',
  'Replace': 'editor.busca.replace',
  'next': 'editor.busca.next',
  'previous': 'editor.busca.previous',
  'all': 'editor.busca.all',
  'match case': 'editor.busca.match-case',
  'regexp': 'editor.busca.regexp',
  'by word': 'editor.busca.by-word',
  'replace': 'editor.busca.replace-one',
  'replace all': 'editor.busca.replace-all',
  'close': 'editor.busca.close',
  'Go to line': 'editor.busca.go-to-line',
  'go': 'editor.busca.go',
  'current match': 'editor.busca.current-match',
  'on line': 'editor.busca.on-line',
  'replaced match on line $': 'editor.busca.replaced-match-on-line',
  'replaced $ matches': 'editor.busca.replaced-matches',
  'Folded lines': 'editor.busca.folded-lines',
  'Unfolded lines': 'editor.busca.unfolded-lines',
  'to': 'editor.busca.to',
  'folded code': 'editor.busca.folded-code',
  'unfold': 'editor.busca.unfold',
  'Fold line': 'editor.busca.fold-line',
  'Unfold line': 'editor.busca.unfold-line',
  'Selection deleted': 'editor.busca.selection-deleted',
  'Completions': 'editor.busca.completions',
};

/** Extensão que põe as frases em português. */
export function frasesDoEditor() {
  const mapa: Record<string, string> = {};
  for (const [ingles, chave] of Object.entries(FRASES)) mapa[ingles] = t(chave);
  return EditorState.phrases.of(mapa);
}
