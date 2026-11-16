// "Salvar como Word": converte o PDF compilado (pdf-para-word.ts) e devolve o .docx em
// base64, pronto para o comando cmd_gravar_bytes. O mesmo arquivo serve ao Windows e ao Android.

import { pdfParaWord, idiomaDoDocumento, estruturaDoTex } from './pdf-para-word';
import { equacoesDoTex } from './equacoes-word';

/** Quantas palavras vao por vez ao dicionario do app (o mesmo lote da correcao ortografica). */
const LOTE_DICIONARIO = 3000;

/**
 * Funcao "existe" do conversor, montada sobre o verificador do app (que devolve as palavras ERRADAS).
 * Recebe palavras e devolve as que o dicionario conhece, em lotes. Funcao pura: `verificar` entra
 * como parametro, para testar sem o Tauri.
 */
export function dicionarioPelasErradas(
  verificar: (palavras: string[]) => Promise<string[]>,
): (palavras: string[]) => Promise<string[]> {
  return async (palavras) => {
    const existentes: string[] = [];
    for (let i = 0; i < palavras.length; i += LOTE_DICIONARIO) {
      const lote = palavras.slice(i, i + LOTE_DICIONARIO);
      const erradas = new Set(await verificar(lote));
      for (const p of lote) if (!erradas.has(p)) existentes.push(p);
    }
    return existentes;
  };
}

/**
 * Dicionario pt-BR do app para o Word (decide hifens de fim de linha). No Windows usa o comando
 * de ortografia do Rust. Onde ele nao existe (o Android nao leva dicionario), devolve undefined
 * e o conversor segue so com o documento e a lista de prefixos.
 */
export async function dicionarioDoApp(): Promise<((palavras: string[]) => Promise<string[]>) | undefined> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const verificar = (palavras: string[]) => invoke<string[]>('cmd_ortografia_verificar', { palavras });
    await verificar([]);
    return dicionarioPelasErradas(verificar);
  } catch {
    return undefined;
  }
}

function paraBase64(dados: Uint8Array): string {
  let binario = '';
  const passo = 0x8000;
  for (let i = 0; i < dados.length; i += passo) {
    binario += String.fromCharCode(...dados.subarray(i, i + passo));
  }
  return btoa(binario);
}

/** Junta ao texto os arquivos de \input e \include (as equacoes dos capitulos). */
export async function expandirInputs(tex: string, ler: (caminho: string) => Promise<string>, nivel = 0): Promise<string> {
  if (nivel > 5) return tex;
  const re = /(^|[^\\%])\\(input|include|subfile)\s*\{([^}]+)\}/g;
  const partes: string[] = [];
  let ultimo = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(tex))) {
    // linha comentada fica como esta
    const inicioLinha = tex.lastIndexOf('\n', m.index) + 1;
    if (/(^|[^\\])%/.test(tex.slice(inicioLinha, m.index + m[1].length))) continue;
    const nome = m[3].trim();
    const caminho = /\.\w+$/.test(nome) ? nome : nome + '.tex';
    const conteudo = await ler(caminho).catch(() => '');
    partes.push(tex.slice(ultimo, m.index + m[1].length), await expandirInputs(conteudo, ler, nivel + 1));
    ultimo = m.index + m[0].length;
  }
  partes.push(tex.slice(ultimo));
  return partes.join('');
}

/** Titulos, notas e sumario do .tex; se a leitura falhar, o Word sai sem a conferencia. */
function estruturaOuNada(tex: string): ReturnType<typeof estruturaDoTex> | undefined {
  try {
    return estruturaDoTex(tex);
  } catch {
    return undefined;
  }
}

/** Gera o .docx do PDF e devolve os bytes em base64.
 * `tex` e o texto do documento principal (idioma da hifenizacao e equacoes do Word);
 * `lerArquivo` le os arquivos do projeto incluidos por \input. Se o .tex nao for lido ou
 * as equacoes nao forem convertidas, o Word sai so com o texto do PDF. */
export async function gerarWord(
  pdf: Uint8Array,
  tex: string,
  progresso?: (pagina: number, total: number) => void,
  lerArquivo?: (caminho: string) => Promise<string>,
): Promise<string> {
  const completo = lerArquivo ? await expandirInputs(tex, lerArquivo).catch(() => tex) : tex;
  let equacoes: ReturnType<typeof equacoesDoTex> = [];
  try {
    equacoes = equacoesDoTex(completo);
  } catch {
    equacoes = [];
  }
  const pdfjs = await import('pdfjs-dist');
  pdfjs.GlobalWorkerOptions.workerSrc = new URL('pdfjs-dist/build/pdf.worker.mjs', import.meta.url).toString();
  const dicionario = await dicionarioDoApp();
  const docx = await pdfParaWord(pdfjs, pdf, {
    idioma: idiomaDoDocumento(tex),
    dicionario,
    fonte: async (arquivo) => {
      const r = await fetch(`/fontes-word/${arquivo}`);
      return r.ok ? new Uint8Array(await r.arrayBuffer()) : null;
    },
    progresso,
    equacoes,
    estrutura: estruturaOuNada(completo),
  });
  return paraBase64(docx);
}
