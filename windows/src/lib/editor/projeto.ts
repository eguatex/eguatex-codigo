// Indice do projeto aberto: os \label de todos os .tex e as entradas de todos os .bib.
// A pagina atualiza o indice ao abrir o projeto e ao salvar; o arquivo aberto no editor
// e lido direto do editor, para os avisos acompanharem a digitacao.
import { StateEffect, type EditorState, type Text } from '@codemirror/state';
import { extrairBib, extrairBibitems, extrairRotulos, type EntradaBib, type Rotulo } from './analise-latex';

interface DadosArquivo {
  rotulos: Rotulo[];
  bib: EntradaBib[];
}

const porArquivo = new Map<string, DadosArquivo>();
let raiz = '';
let arquivoAberto = '';

/** Avisa o editor que o indice mudou (os avisos sao refeitos). */
export const indiceMudou = StateEffect.define<null>();

export function definirProjeto(caminhoRaiz: string) {
  if (caminhoRaiz !== raiz) porArquivo.clear();
  raiz = caminhoRaiz;
}

export function definirArquivoAberto(caminho: string) {
  arquivoAberto = caminho;
}

export const raizDoProjeto = () => raiz;
export const arquivoDoEditor = () => arquivoAberto;

export function atualizarArquivo(caminho: string, texto: string) {
  const minusculo = caminho.toLowerCase();
  if (minusculo.endsWith('.bib')) porArquivo.set(caminho, { rotulos: [], bib: extrairBib(texto, caminho) });
  else if (minusculo.endsWith('.tex')) {
    porArquivo.set(caminho, { rotulos: extrairRotulos(texto, caminho), bib: extrairBibitems(texto, caminho) });
  }
}

/** O arquivo ja esta no indice? Quem indexa o projeto so le do disco os que faltam. */
export const arquivoIndexado = (caminho: string) => porArquivo.has(caminho);

/** Tira um arquivo do indice (mudou fora do editor, ou foi renomeado); a proxima indexacao le de novo. */
export function esquecerArquivo(caminho: string) {
  porArquivo.delete(caminho);
}

/** Esquece os arquivos que sairam do projeto (apagados ou renomeados). */
export function manterSomente(caminhos: string[]) {
  const ficam = new Set(caminhos);
  for (const c of [...porArquivo.keys()]) if (!ficam.has(c)) porArquivo.delete(c);
}

// O documento do editor muda a cada tecla; a leitura dele e guardada por versao do texto.
const doEditor = new WeakMap<Text, DadosArquivo>();
function dadosDoEditor(doc: Text): DadosArquivo {
  let d = doEditor.get(doc);
  if (!d) {
    const texto = doc.toString();
    d = { rotulos: extrairRotulos(texto, arquivoAberto), bib: extrairBibitems(texto, arquivoAberto) };
    doEditor.set(doc, d);
  }
  return d;
}

function todos(state: EditorState): DadosArquivo[] {
  const lista: DadosArquivo[] = [];
  for (const [caminho, dados] of porArquivo) if (caminho !== arquivoAberto) lista.push(dados);
  lista.push(dadosDoEditor(state.doc));
  return lista;
}

/** Todos os rotulos do projeto, agrupados pelo nome (mais de um = rotulo repetido). */
export function rotulosDoProjeto(state: EditorState): Map<string, Rotulo[]> {
  const mapa = new Map<string, Rotulo[]>();
  for (const d of todos(state)) {
    for (const r of d.rotulos) {
      const lista = mapa.get(r.nome);
      if (lista) lista.push(r);
      else mapa.set(r.nome, [r]);
    }
  }
  return mapa;
}

/** Entradas de bibliografia do projeto, pela chave. */
export function bibliografiaDoProjeto(state: EditorState): Map<string, EntradaBib> {
  const mapa = new Map<string, EntradaBib>();
  for (const d of todos(state)) for (const e of d.bib) if (!mapa.has(e.chave)) mapa.set(e.chave, e);
  return mapa;
}

/** O projeto tem algum .bib ou \bibitem? Sem nenhum, as citacoes nao sao conferidas. */
export function temBibliografia(state: EditorState): boolean {
  for (const [caminho, d] of porArquivo) {
    if (caminho.toLowerCase().endsWith('.bib') || d.bib.length > 0) return true;
  }
  return dadosDoEditor(state.doc).bib.length > 0;
}

/** Caminho absoluto de um caminho relativo ao projeto (com barras do Windows). */
export function absoluto(relativo: string): string {
  const base = raiz.replace(/[\\/]+$/, '');
  return `${base}\\${relativo.replace(/^[\\/]+/, '').replace(/\//g, '\\')}`;
}

/** Nomes dos tipos de rotulo, para a lista de sugestoes e a previa. */
export const NOME_DO_TIPO: Record<string, string> = {
  figura: 'Figura', tabela: 'Tabela', equacao: 'Equação', secao: 'Seção', capitulo: 'Capítulo',
  parte: 'Parte', item: 'Item', teorema: 'Teorema', lema: 'Lema', definicao: 'Definição',
  codigo: 'Código', texto: 'Rótulo',
};
