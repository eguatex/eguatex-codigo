// @ts-nocheck: usa modulos do Node (fs); o projeto nao tem @types/node
// Ferramentas dos testes de prova (artigo, TCC, beamer): palavras do PDF e do .docx na ordem,
// paginas, imagens e estilos. Os PDFs ficam em fixtures/ e saem do .tex ao lado.
// @ts-expect-error sem @types/node no projeto
import { readFileSync, existsSync } from 'node:fs';
// @ts-expect-error sem @types/node no projeto
import { createRequire } from 'node:module';
// @ts-expect-error sem @types/node no projeto
import { dirname, join } from 'node:path';
// @ts-expect-error sem @types/node no projeto
import { fileURLToPath } from 'node:url';
import { estruturaDoTex, pdfParaWord } from '../pdf-para-word';
import { equacoesDoTex } from '../equacoes-word';
import { descompactar } from './zip-leitura';

const aqui = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

/** Faz o node-canvas valer como document.createElement('canvas') do navegador. */
export function instalarCanvas() {
  const { createCanvas } = require('@napi-rs/canvas');
  (globalThis as any).FontFace = class {
    constructor(public family: string) {}
    load() {
      return Promise.resolve(this);
    }
  };
  (globalThis as any).document = {
    fonts: { add() {}, delete() {}, clear() {} },
    createElement: (tag: string) => {
      if (tag !== 'canvas') throw new Error('so canvas');
      const c = createCanvas(1, 1);
      c.toBlob = (cb: (b: Blob) => void) => cb(new Blob([c.toBuffer('image/png')], { type: 'image/png' }));
      return c;
    },
  };
}

const SEP = /[^\p{L}\p{N}\u0001-]+/u;
const SEM_TRACO = (w: string) => w.replace(/^-+|-+$/g, '');

/** Palavras com 2 letras ou mais; o hifen no meio da palavra faz parte dela. */
export const palavras = (s: string): string[] =>
  s
    .normalize('NFKC')
    .replace(/[\u00ad\u200b]/g, '')
    .split(SEP)
    .map(SEM_TRACO)
    .filter((w) => w.replace(/-/g, '').length >= 2);

export interface ProvaPdf {
  /** Palavras na ordem de leitura do PDF; hifen de fim de linha vira lista com as duas formas. */
  palavras: (string | string[])[];
  paginas: number;
  /** Imagens raster desenhadas nas paginas (figuras vetoriais nao contam). */
  imagens: number;
}

export async function lerPdf(pdfjs: any, pdf: Uint8Array): Promise<ProvaPdf> {
  const doc = await pdfjs.getDocument({ data: pdf.slice(), isEvalSupported: false }).promise;
  const saida: (string | string[])[] = [];
  let imagens = 0;
  for (let i = 1; i <= doc.numPages; i++) {
    const pagina = await doc.getPage(i);
    const c = await pagina.getTextContent();
    const texto = (c.items as any[]).map((it) => it.str + (it.hasEOL ? '\n' : '')).join('');
    const marcado = texto.replace(/(\p{L})-\s*\n\s*(\p{Ll})/gu, '$1\u0001$2');
    for (const w0 of palavras(marcado)) {
      // o numero sobrescrito de uma nota de rodape vem colado ao texto ("1Texto"): no Word e a referencia da nota
      const w = w0.replace(/^\d{1,2}(?=\p{L}{2})/u, '');
      saida.push(w.includes('\u0001') ? [w.replace('\u0001', ''), w.replace('\u0001', '-')] : w);
    }
    const ops = await pagina.getOperatorList();
    const { OPS } = pdfjs;
    imagens += ops.fnArray.filter((f: number) => f === OPS.paintImageXObject || f === OPS.paintInlineImageXObject).length;
  }
  return { palavras: saida, paginas: doc.numPages, imagens };
}

export interface ProvaDocx {
  palavras: string[];
  /** Quebras de pagina explicitas mais 1. */
  paginas: number;
  imagens: number;
  /** Estilos de paragrafo usados no corpo, com a contagem de cada um. */
  estilos: Record<string, number>;
  campoToc: number;
  notas: number;
  tabelas: number;
  equacoes: number;
  /** paragrafos que usam numeracao do Word (itens de lista) */
  itensDeLista: number;
}

export function lerDocx(docx: Uint8Array): ProvaDocx {
  const z = descompactar(docx);
  const xml = new TextDecoder().decode(z['word/document.xml']);
  const cortado = xml.replace(/<\/w:p>|<w:tab\/>|<w:br[^>]*\/>|<\/w:txbxContent>/g, '<w:t> </w:t>');
  const semOmml = cortado.replace(/<m:oMathPara>.*?<\/m:oMathPara>/gs, '<w:t> </w:t>').replace(/<m:oMath>.*?<\/m:oMath>/gs, '<w:t> </w:t>');
  const textos = [...semOmml.matchAll(/<w:t(?: [^>]*)?>([^<]*)<\/w:t>/g)].map((m) => m[1]);
  const estilos: Record<string, number> = {};
  for (const m of xml.matchAll(/<w:pStyle w:val="([^"]+)"/g)) estilos[m[1]] = (estilos[m[1]] ?? 0) + 1;
  return {
    palavras: palavras(
      textos.join('\u0001').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/\u0001/g, ''),
    ),
    paginas: (xml.match(/<w:pageBreakBefore\/>/g)?.length ?? 0) + 1,
    imagens: xml.match(/<pic:pic /g)?.length ?? 0,
    estilos,
    campoToc: (xml.match(/<w:instrText[^>]*>\s*TOC\b/g)?.length ?? 0) + (xml.match(/<w:fldSimple[^>]*w:instr="\s*TOC\b/g)?.length ?? 0),
    notas: (xml.match(/<w:footnoteReference /g)?.length ?? 0),
    tabelas: xml.match(/<w:tbl>/g)?.length ?? 0,
    equacoes: xml.match(/<m:oMath>|<m:oMathPara>/g)?.length ?? 0,
    itensDeLista: xml.match(/<w:numPr>/g)?.length ?? 0,
  };
}

/** Palavras do PDF que nao aparecem no .docx, respeitando a ordem (subsequencia comum mais longa). */
export function faltandoNaOrdem(esperado: (string | string[])[], obtido: string[]): string[] {
  const n = esperado.length;
  const m = obtido.length;
  // a tabela da subsequencia ocupa n*m inteiros de 4 bytes: acima de 30 milhoes de celulas (120 MB) recusa
  if (n * m > 30_000_000) throw new Error(`documento grande demais para comparar a ordem (${n} x ${m} palavras)`);
  const casa = (e: string | string[], o: string) => (Array.isArray(e) ? e.includes(o) : e === o);
  const dp: Uint32Array[] = [];
  for (let i = 0; i <= n; i++) dp.push(new Uint32Array(m + 1));
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      dp[i][j] = casa(esperado[i - 1], obtido[j - 1]) ? dp[i - 1][j - 1] + 1 : Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }
  const faltando: string[] = [];
  let i = n;
  let j = m;
  while (i > 0) {
    if (j > 0 && casa(esperado[i - 1], obtido[j - 1]) && dp[i][j] === dp[i - 1][j - 1] + 1) {
      i--;
      j--;
    } else if (j > 0 && dp[i][j - 1] >= dp[i - 1][j]) j--;
    else {
      const e = esperado[i - 1];
      faltando.unshift(Array.isArray(e) ? e.join('|') : e);
      i--;
    }
  }
  return faltando;
}

/** Texto de cada \footnote{...} do .tex (sem chaves aninhadas). */
export function notasDoTex(tex: string): string[] {
  return [...tex.matchAll(/\\footnote\{([^}]*)\}/g)].map((m) => m[1]);
}

/** Tira do PDF as palavras de cada nota (elas ficam no pe da pagina; no Word vao em footnotes.xml). */
export function semAsNotas(esperado: (string | string[])[], notas: string[]): (string | string[])[] {
  let saida = esperado;
  for (const nota of notas) {
    const ws = palavras(nota);
    const i = saida.findIndex((_, k) => ws.every((w, j) => (Array.isArray(saida[k + j]) ? (saida[k + j] as string[]).includes(w) : saida[k + j] === w)));
    if (i >= 0) saida = [...saida.slice(0, i), ...saida.slice(i + ws.length)];
  }
  return saida;
}

/** Palavras de footnotes.xml na ordem, uma lista por nota. */
export function palavrasDasNotas(docx: Uint8Array): string[][] {
  const bruto = descompactar(docx)['word/footnotes.xml'];
  if (!bruto) return [];
  const xml = new TextDecoder().decode(bruto);
  return [...xml.matchAll(/<w:footnote w:id="(\d+)">(.*?)<\/w:footnote>/gs)].map((m) =>
    palavras([...m[2].matchAll(/<w:t(?: [^>]*)?>([^<]*)<\/w:t>/g)].map((x) => x[1]).join(' ')),
  );
}

/** Texto do .tex com os \input e \include trocados pelo conteudo (so um nivel, o bastante para as provas). */
export function texCompleto(nome: string): string {
  const base = join(aqui, nome + '.tex');
  const tex = readFileSync(base, 'utf8');
  return tex.replace(/\\(?:input|include)\{([^}]+)\}/g, (_m, arq) => {
    const caminho = join(aqui, arq.endsWith('.tex') ? arq : arq + '.tex');
    return existsSync(caminho) ? readFileSync(caminho, 'utf8') : '';
  });
}

export async function converterProva(pdfjs: any, nome: string): Promise<{ pdf: Uint8Array; docx: Uint8Array; tex: string }> {
  const pdf = new Uint8Array(readFileSync(join(aqui, nome + '.pdf')));
  const tex = texCompleto(nome);
  const comFontes = { ...pdfjs, getDocument: (o: any) => pdfjs.getDocument({ ...o, disableFontFace: false }) };
  const docx = await pdfParaWord(comFontes, pdf, {
    idioma: 'pt-BR',
    fonte: async (arq) => {
      const caminho = join(aqui, '../../../static/fontes-word', arq);
      return existsSync(caminho) ? new Uint8Array(readFileSync(caminho)) : null;
    },
    equacoes: equacoesDoTex(tex),
    estrutura: estruturaDoTex(tex),
  });
  return { pdf, docx, tex };
}
