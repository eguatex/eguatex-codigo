// Modo Visualizando: o texto so pode ser lido. `EditorState.readOnly` sozinho so trava os comandos
// do CodeMirror; as acoes do EguaTex (negrito, colar, corretor) enviam transacoes direto. Este filtro
// descarta toda transacao de edicao do usuario com o estado em somente leitura.
// Ficam de fora: a troca de arquivo ("eguatex.trocar") e aceitar ou rejeitar sugestoes ("revisao"),
// para o Visualizando nao bloquear a revisao.
import { EditorState, type Extension } from '@codemirror/state';

const EVENTOS_DE_EDICAO = ['input', 'delete', 'undo', 'redo'];

export const bloqueioDeEdicao: Extension = EditorState.transactionFilter.of((tr) =>
  tr.docChanged && tr.startState.readOnly && EVENTOS_DE_EDICAO.some((e) => tr.isUserEvent(e)) ? [] : tr,
);
