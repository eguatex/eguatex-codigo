// Converte o PDF compilado em .docx no proprio aparelho, em TypeScript. Windows e Android
// usam este arquivo. O PDF.js, que ja mostra o PDF, le o texto com posicao, fonte, tamanho e cor.
//
// O que sai igual ao PDF:
// - todo o texto, palavra por palavra, com fonte, tamanho, negrito, italico, cor,
//   sobrescrito e subscrito;
// - paragrafos com o mesmo alinhamento, recuo, entrelinha e espaco entre eles;
// - uma quebra de pagina por pagina do PDF, entao as paginas batem uma a uma;
// - duas colunas viram secao de duas colunas do Word;
// - equacoes destacadas viram equacoes editaveis do Word (OMML), tiradas do .tex
//   (equacoes-word.ts), no lugar das letras do PDF;
// - figuras, linhas de tabela, caixas e fundos viram imagens no mesmo lugar da pagina,
//   atras do texto;
// - Latin Modern vai embutido no .docx, como no Windows.
// - texto girado vira caixa de texto girada, na mesma posicao;
// - links do PDF viram links do Word (enderecos e referencias internas, com marcadores);
// - hifen de fim de linha: so se junta quando e quebra de palavra (guarda-chuva mantem o hifen).
// - cabecalho, rodape e numero de pagina que repetem viram as partes header e footer do Word,
//   com o numero como campo PAGE; a entrelinha e "pelo menos" e as margens sao as do bloco de texto,
//   entao o Word repagina sem cortar nem sobrepor texto se a fonte for outra.
// - estrutura: titulo e Titulo 1 a 3 (conferidos com os titulos do .tex, ou pelo padrao "1.2 Texto"),
//   sumario como campo TOC, notas de rodape como w:footnote e listas como numeracao do Word.
// Ainda como texto solto: tabelas (texto com tabulacao, linhas como imagem).

import type * as PdfJs from 'pdfjs-dist';

import { comTamanho, type EquacaoTex } from './equacoes-word';

type Pdfjs = typeof PdfJs;

interface Trecho {
  texto: string;
  x: number;
  x1: number;
  base: number;
  tam: number;
  fonte: string;
  negrito: boolean;
  italico: boolean;
  cor: string;
  vert?: 'superscript' | 'subscript';
  /** quanto o trecho sobe (positivo) ou desce em relacao a linha, em pt */
  desloc?: number;
  /** link do PDF: endereco externo ou nome de um destino dentro do documento */
  ligacao?: { url?: string; destino?: string };
  /** a referencia de uma nota de rodape: numero da nota no .docx (w:footnote) */
  nota?: number;
}

interface Linha {
  trechos: Trecho[];
  x0: number;
  x1: number;
  base: number;
  tam: number;
  /** a linha tem um espaco grande no meio (tabela, sumario): vira paragrafo com tabulacao */
  tabulada: boolean;
}

interface Paragrafo {
  linhas: Linha[];
  alinhamento: 'both' | 'center' | 'right' | 'left';
  esquerda: number;
  primeira: number;
  entrelinha: number;
  antes: number;
  /** sumario e listas: o numero no fim encostado na margem, com pontilhado ou nao */
  tabDireita?: { pos: number; pontilhado: boolean };
  /** quanto o texto passa da margem direita (vira recuo direito negativo) */
  excesso: number;
  /** linhas empilhadas de uma formula (fracao, raiz): vao em caixas no lugar exato */
  aglomerado?: boolean;
  /** equacao destacada trocada pela equacao editavel do Word (OMML) e os numeros dela */
  equacao?: { omml: string; numeros: Linha[] };
  /** marcadores (bookmarks) dos destinos de link que ficam neste paragrafo */
  marcas?: string;
  /** estilo do Word: Title, Heading1 a Heading3, TOCHeading, TOC1 a TOC3 */
  estilo?: string;
  /** item de lista: numero da instancia (w:numId) em numbering.xml */
  numeracao?: number;
  /** entrada do sumario, e a primeira delas (abre o campo TOC do Word) */
  toc?: boolean;
  tocInicio?: boolean;
}

interface Bloco {
  colunas: boolean;
  esq: Linha[];
  dir: Linha[];
  /** limites do texto do bloco (ou da coluna esquerda e direita) */
  xa: number;
  xb: number;
  xc: number;
  xd: number;
}

interface Imagem {
  id: number;
  png: Uint8Array;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Texto girado (margem de arXiv, rotulo de eixo, \rotatebox): vira caixa de texto girada. */
interface Girado {
  texto: string;
  /** origem da linha de base (inicio do texto), em pt a partir do canto de cima */
  x: number;
  y: number;
  /** comprimento do texto, em pt */
  w: number;
  tam: number;
  /** angulo no sentido horario, em graus (0 a 360), como o Word conta */
  graus: number;
  fonte: string;
  negrito: boolean;
  italico: boolean;
  cor: string;
}

interface Pagina {
  largura: number;
  altura: number;
  linhas: Linha[];
  imagens: Imagem[];
  girados: Girado[];
  /** destinos de link que caem nesta pagina (alvo dos links internos), por altura */
  destinos: { nome: string; y: number }[];
}

const ESCALA = 2; // 144 dpi para as figuras e para ler as cores

// ---------- fontes ----------

// Fontes. O que vai embutido no .docx: so a Latin Modern (licenca GUST/LPPL, que permite
// embutir), em TrueType, que e o unico formato que o Word embute. As fontes do PDF vem como
// Type 1 ou CFF (cm-super, TeX Gyre), que o Word nao embute; elas viram a fonte equivalente:
//   Latin Modern, CM, cm-super, EC          -> Latin Modern Roman/Sans/Mono (embutida)
//   Times, Nimbus Roman, TeX Gyre Termes    -> Times New Roman (alternativa: Liberation Serif)
//   Helvetica, Arial, Nimbus Sans, Heros    -> Arial (alternativa: Liberation Sans)
//   Courier New                             -> Courier New (alternativa: Liberation Mono)
//   Palatino, Pagella, Bookman, Schola      -> Palatino Linotype, Bookman Old Style, Century Schoolbook
//   fontes de matematica                    -> Cambria Math
//   nome desconhecido                       -> o mesmo nome, que o Word substitui pela mais proxima
// (nome da fonte no PDF, familia no Word): a mesma tabela do pdf-para-word do Windows
const FAMILIAS: [RegExp, string][] = [
  [/^(couriernew|liberationmono|dejavusansmono)/, 'Courier New'],
  [/^(texgyreadventor|urwgothic|avantgarde|pagk)/, 'Century Gothic'],
  [/^(lmmono|lmtypewriter|cmtt|cmitt|cmsltt|cmvtt|(?:sf|ec|tc)(?:tt|it|st|tl|vt|ti\d)|cmutypewriter|nimbusmono|courier|texgyrecursor|pcrr)/, 'Latin Modern Mono'],
  [/^(lmsans|cmss|(?:sf|ec|tc)(?:ss|sx|si|so|qb|qi)|cmusans|latinmodernsans|firasans|fira-sans)/, 'Latin Modern Sans'],
  [/^(cmmi|cmbsy|cmsy|cmex|lmmath|msam|msbm|eufm|eurm|eusm|stixmath|firamath|texgyre\w*math|xitsmath|latinmodernmath|cambriamath|rsfs|wasy|stmary|esint)/, 'Cambria Math'],
  [/^(lmroman|cmr|cmbx|cmb\d|cmti|cmsl|cmcsc|cmu|cmdunh|cmfib|(?:sf|ec|tc)[a-z]{2}\d|latinmodernroman|lmr)/, 'Latin Modern Roman'],
  [/^(nimbussan|helvetica|arial|texgyreheros|uhvr|phvr|liberationsans|freesans)/, 'Arial'],
  [/^(nimbusrom|times|texgyretermes|utmr|ptmr|stix|liberationserif|freeserif)/, 'Times New Roman'],
  [/^(texgyrepagella|palatino|urwpalladio|pplr|tgpagella)/, 'Palatino Linotype'],
  [/^(texgyrebonum|bookman|urwbookman|pbkl)/, 'Bookman Old Style'],
  [/^(texgyreschola|centuryschoolbook|ncr|pncr)/, 'Century Schoolbook'],
  [/^(dejavusans)/, 'DejaVu Sans'],
];
export const FAMILIAS_EMBUTIDAS = ['Latin Modern Roman', 'Latin Modern Sans', 'Latin Modern Mono'];
/** Fonte parecida e com as mesmas medidas, que o Word, o LibreOffice e o Android usam se faltar a original. */
const ALTERNATIVAS: Record<string, { alt: string; tipo: 'roman' | 'swiss' | 'modern' }> = {
  'Times New Roman': { alt: 'Liberation Serif', tipo: 'roman' },
  Arial: { alt: 'Liberation Sans', tipo: 'swiss' },
  'Courier New': { alt: 'Liberation Mono', tipo: 'modern' },
  'Palatino Linotype': { alt: 'TeX Gyre Pagella', tipo: 'roman' },
  'Bookman Old Style': { alt: 'TeX Gyre Bonum', tipo: 'roman' },
  'Century Schoolbook': { alt: 'TeX Gyre Schola', tipo: 'roman' },
  'Century Gothic': { alt: 'TeX Gyre Adventor', tipo: 'swiss' },
  'DejaVu Sans': { alt: 'Verdana', tipo: 'swiss' },
  'Cambria Math': { alt: 'Latin Modern Math', tipo: 'roman' },
};
const ESTILOS: [string, string][] = [
  ['regular', 'embedRegular'],
  ['bold', 'embedBold'],
  ['italic', 'embedItalic'],
  ['bolditalic', 'embedBoldItalic'],
];

export function familiaWord(nome: string): string {
  const limpo = nome.replace(/^[A-Z]{6}\+/, '');
  const chave = limpo.replace(/[\s,]/g, '').replace(/-(regular|bold|italic|oblique|roman|medium|light).*$/i, (m) => m).toLowerCase();
  for (const [padrao, familia] of FAMILIAS) if (padrao.test(chave)) return familia;
  return limpo.replace(/[-,](Regular|Bold|Italic|Oblique|BoldItalic|Roman|Medium).*$/i, '') || 'Times New Roman';
}

function ehNegrito(nome: string, objeto: any): boolean {
  if (objeto?.bold || objeto?.black) return true;
  const n = nome.replace(/^[A-Z]{6}\+/, '').toLowerCase();
  return /bold|black|heavy|semibold|demi|-medi|cmbx|cmb\d|(?:sf|ec|tc)(?:bx|sx|bi|bl|rb|qb)|lmmonolt10-bold/.test(n);
}

function ehItalico(nome: string, objeto: any): boolean {
  if (objeto?.italic) return true;
  const n = nome.replace(/^[A-Z]{6}\+/, '').toLowerCase();
  return /italic|ital|-mediital|oblique|slant|cmti|cmsl|cmmi|cmitt|(?:sf|ec|tc)(?:ti|bi|it|sl|si|so|st|qi)\d/.test(n);
}

// ---------- leitura de uma pagina ----------

async function lerPagina(pdfjs: Pdfjs, doc: PdfJs.PDFDocumentProxy, pagina: PdfJs.PDFPageProxy, proximaImagem: () => number): Promise<Pagina> {
  const vp = pagina.getViewport({ scale: 1 });
  const largura = vp.width;
  const altura = vp.height;
  // carrega as fontes da pagina em commonObjs (nome, negrito, italico)
  await pagina.getOperatorList();
  const conteudo = await pagina.getTextContent();

  // duas renderizacoes: a normal e uma sem texto. A diferenca entre elas da a cor de cada
  // trecho; a sem texto da as figuras, linhas e fundos.
  const vpg = pagina.getViewport({ scale: ESCALA });
  const W = Math.ceil(vpg.width);
  const H = Math.ceil(vpg.height);
  const comTexto = await renderizar(pagina, vpg, W, H, false);
  const semTexto = await renderizar(pagina, vpg, W, H, true);

  const trechos: Trecho[] = [];
  const girados: Girado[] = [];
  const vistos = new Set<string>();
  for (const item of conteudo.items as any[]) {
    if (typeof item.str !== 'string' || !item.str.trim()) continue;
    const t = item.transform as number[];
    // o Beamer desenha o rodape varias vezes no mesmo lugar: so a primeira copia conta
    const chave = `${item.str}|${Math.round(t[4] * 2)}|${Math.round(t[5] * 2)}`;
    if (vistos.has(chave)) continue;
    vistos.add(chave);
    const rotacionado = Math.abs(t[1]) > 0.01 || Math.abs(t[2]) > 0.01 || t[0] <= 0;
    const tam = rotacionado ? Math.hypot(t[0], t[1]) : Math.hypot(t[2], t[3]);
    if (tam < 1) continue;
    let objeto: any = null;
    try {
      if (pagina.commonObjs.has(item.fontName)) objeto = pagina.commonObjs.get(item.fontName);
    } catch {
      objeto = null;
    }
    const nome: string = objeto?.name || (conteudo.styles as any)[item.fontName]?.fontFamily || 'Times';
    if (rotacionado) {
      // texto girado: uma caixa de texto girada no Word, no lugar do texto no PDF
      const w = item.width || tam * item.str.length * 0.5;
      const graus = (((-Math.atan2(t[1], t[0]) * 180) / Math.PI) % 360 + 360) % 360;
      const g: Girado = {
        texto: item.str,
        x: t[4],
        y: altura - t[5],
        w,
        tam,
        graus,
        fonte: familiaWord(nome),
        negrito: ehNegrito(nome, objeto),
        italico: ehItalico(nome, objeto),
        cor: '000000',
      };
      const c = centroGirado(g);
      // fora da folha nao aparece no PDF, entao tambem nao vai para o Word
      if (c.x < 0 || c.x > vp.width || c.y < 0 || c.y > altura) continue;
      const ultimo = girados[girados.length - 1];
      if (ultimo && juntaGirado(ultimo, g)) continue;
      girados.push(g);
      continue;
    }
    const x = t[4];
    const base = altura - t[5];
    const largura = item.width || tam * item.str.length * 0.5;
    // fora da folha nao aparece no PDF, entao tambem nao vai para o Word
    if (base < tam * 0.3 || base > altura + tam * 0.2 || x >= vp.width || x + largura <= 0) continue;
    const fonte = familiaWord(nome);
    const negrito = ehNegrito(nome, objeto);
    const italico = ehItalico(nome, objeto);
    // um item do PDF pode ter cores diferentes (no sumario o titulo e azul e os pontos
    // pretos): a cor e lida palavra por palavra e o item e partido onde ela muda
    const texto: string = item.str;
    const pedacos = texto.match(/\S+\s*|\s+/g) ?? [texto];
    let pos = 0;
    for (const p of pedacos) {
      const xa = x + (largura * pos) / texto.length;
      const xb = x + (largura * (pos + p.length)) / texto.length;
      pos += p.length;
      const cor = p.trim() ? corDoTrecho(comTexto, semTexto, W, H, xa, xb, base, tam) : '';
      const ultimo = trechos[trechos.length - 1];
      if (ultimo && ultimo.base === base && Math.abs(ultimo.x1 - xa) < 0.01 && (cor === '' || ultimo.cor === cor)) {
        ultimo.texto += p;
        ultimo.x1 = xb;
      } else {
        trechos.push({ texto: p, x: xa, x1: xb, base, tam, fonte, negrito, italico, cor: cor || '000000' });
      }
    }
  }

  for (const g of girados) {
    const [x0, y0, x1, y1] = limitesGirado(g);
    g.cor = corNaCaixa(comTexto, semTexto, W, H, x0, y0, x1, y1);
  }
  const ligacoes = await ligacoesDaPagina(doc, pagina, vp).catch(() => []);
  const comLigacoes = ligacoes.length ? aplicarLigacoes(trechos, ligacoes, comTexto, semTexto, W, H) : trechos;
  const imagens = acharImagens(semTexto, W, H, proximaImagem);
  return { largura, altura, linhas: montarLinhas(comLigacoes), imagens: await imagens, girados, destinos: [] };
}

async function renderizar(
  pagina: PdfJs.PDFPageProxy,
  vp: PdfJs.PageViewport,
  W: number,
  H: number,
  semTexto: boolean,
): Promise<{ canvas: HTMLCanvasElement; dados: Uint8ClampedArray }> {
  const canvas = document.createElement('canvas');
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, W, H);
  if (semTexto) {
    // o PDF.js desenha as letras com fillText/strokeText: sem elas sobra so o desenho.
    // O texto girado tambem sai da imagem: vai para caixas de texto giradas.
    (ctx as any).fillText = () => {};
    (ctx as any).strokeText = () => {};
  }
  await pagina.render({ canvasContext: ctx, viewport: vp, background: '#ffffff' } as any).promise;
  return { canvas, dados: ctx.getImageData(0, 0, W, H).data };
}

/** Cor do pixel que mais muda entre a pagina com texto e sem texto, dentro do trecho. */
function corDoTrecho(
  com: { dados: Uint8ClampedArray },
  sem: { dados: Uint8ClampedArray },
  W: number,
  H: number,
  x: number,
  x1: number,
  base: number,
  tam: number,
): string {
  return corNaCaixa(com, sem, W, H, x, base - tam * 0.75, x1, base + tam * 0.1);
}

/** O mesmo, numa caixa qualquer da pagina (pt a partir do canto de cima). */
function corNaCaixa(
  com: { dados: Uint8ClampedArray },
  sem: { dados: Uint8ClampedArray },
  W: number,
  H: number,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
): string {
  const xa = Math.max(0, Math.floor(x0 * ESCALA));
  const xb = Math.min(W - 1, Math.ceil(x1 * ESCALA));
  const ya = Math.max(0, Math.floor(y0 * ESCALA));
  const yb = Math.min(H - 1, Math.ceil(y1 * ESCALA));
  let melhor = 0;
  let cor: [number, number, number] = [0, 0, 0];
  const passo = xb - xa > 200 ? 2 : 1;
  for (let y = ya; y <= yb; y++) {
    for (let xx = xa; xx <= xb; xx += passo) {
      const i = (y * W + xx) * 4;
      const d = Math.abs(com.dados[i] - sem.dados[i]) + Math.abs(com.dados[i + 1] - sem.dados[i + 1]) + Math.abs(com.dados[i + 2] - sem.dados[i + 2]);
      if (d > melhor) {
        melhor = d;
        cor = [com.dados[i], com.dados[i + 1], com.dados[i + 2]];
      }
    }
  }
  if (melhor < 90) return '000000';
  // o miolo da letra e a cor pura; a borda suavizada puxa para o fundo
  if (cor[0] < 60 && cor[1] < 60 && cor[2] < 60) return '000000';
  return cor.map((c) => c.toString(16).padStart(2, '0')).join('').toUpperCase();
}

/** Figuras, linhas e fundos: regioes com tinta na pagina sem texto. */
async function acharImagens(
  sem: { canvas: HTMLCanvasElement; dados: Uint8ClampedArray },
  W: number,
  H: number,
  proximaImagem: () => number,
): Promise<Imagem[]> {
  const C = 4; // celula de 4 px (2 pt)
  const gw = Math.ceil(W / C);
  const gh = Math.ceil(H / C);
  const tinta = new Uint8Array(gw * gh);
  const d = sem.dados;
  for (let y = 0; y < H; y++) {
    const linha = y * W * 4;
    const gy = ((y / C) | 0) * gw;
    for (let x = 0; x < W; x++) {
      const i = linha + x * 4;
      if (d[i] < 245 || d[i + 1] < 245 || d[i + 2] < 245) tinta[gy + ((x / C) | 0)] = 1;
    }
  }
  // regioes ligadas, juntando pedacos a ate 3 celulas (6 pt) de distancia
  const R = 3;
  const visto = new Uint8Array(gw * gh);
  const caixas: [number, number, number, number][] = [];
  const fila: number[] = [];
  for (let k = 0; k < tinta.length; k++) {
    if (!tinta[k] || visto[k]) continue;
    let x0 = gw;
    let y0 = gh;
    let x1 = 0;
    let y1 = 0;
    fila.length = 0;
    fila.push(k);
    visto[k] = 1;
    while (fila.length) {
      const c = fila.pop()!;
      const cx = c % gw;
      const cy = (c / gw) | 0;
      if (cx < x0) x0 = cx;
      if (cx > x1) x1 = cx;
      if (cy < y0) y0 = cy;
      if (cy > y1) y1 = cy;
      for (let dy = -R; dy <= R; dy++) {
        const ny = cy + dy;
        if (ny < 0 || ny >= gh) continue;
        for (let dx = -R; dx <= R; dx++) {
          const nx = cx + dx;
          if (nx < 0 || nx >= gw) continue;
          const n = ny * gw + nx;
          if (tinta[n] && !visto[n]) {
            visto[n] = 1;
            fila.push(n);
          }
        }
      }
    }
    caixas.push([x0, y0, x1, y1]);
  }
  // muitas pecas soltas (graficos com pontos): uma imagem so com todas
  let finais = caixas;
  if (caixas.length > 60) {
    finais = [[
      Math.min(...caixas.map((c) => c[0])),
      Math.min(...caixas.map((c) => c[1])),
      Math.max(...caixas.map((c) => c[2])),
      Math.max(...caixas.map((c) => c[3])),
    ]];
  }
  const imagens: Imagem[] = [];
  for (const [x0, y0, x1, y1] of finais) {
    const px = x0 * C;
    const py = y0 * C;
    const pw = Math.min(W, (x1 + 1) * C) - px;
    const ph = Math.min(H, (y1 + 1) * C) - py;
    if (pw < 2 || ph < 2) continue;
    const recorte = document.createElement('canvas');
    recorte.width = pw;
    recorte.height = ph;
    recorte.getContext('2d')!.drawImage(sem.canvas, px, py, pw, ph, 0, 0, pw, ph);
    const blob: Blob = await new Promise((ok, erro) =>
      recorte.toBlob((b) => (b ? ok(b) : erro(new Error('falha ao gerar a imagem'))), 'image/png'),
    );
    imagens.push({
      id: proximaImagem(),
      png: new Uint8Array(await blob.arrayBuffer()),
      x: px / ESCALA,
      y: py / ESCALA,
      w: pw / ESCALA,
      h: ph / ESCALA,
    });
  }
  return imagens;
}

// ---------- texto girado ----------

const ALTURA_CAIXA = 1.2; // altura da caixa girada, em tamanhos de letra; a linha de base fica a 0,8 dela

/** Vetores unitarios do texto (u, ao longo da linha) e do alto das letras (v), em pt de pagina. */
function eixosGirado(graus: number): { u: [number, number]; v: [number, number] } {
  const f = (graus * Math.PI) / 180;
  return { u: [Math.cos(f), Math.sin(f)], v: [Math.sin(f), -Math.cos(f)] };
}

/** Centro da caixa de um texto girado. */
function centroGirado(g: Girado): { x: number; y: number } {
  const { u, v } = eixosGirado(g.graus);
  const h = ALTURA_CAIXA * g.tam;
  return { x: g.x + (u[0] * g.w) / 2 + v[0] * 0.3 * h, y: g.y + (u[1] * g.w) / 2 + v[1] * 0.3 * h };
}

/** Retangulo reto que cobre a caixa girada: x0, y0, x1, y1. */
function limitesGirado(g: Girado): [number, number, number, number] {
  const { u, v } = eixosGirado(g.graus);
  const h = ALTURA_CAIXA * g.tam;
  const cantos = [0, g.w].flatMap((a) => [-0.2 * h, 0.8 * h].map((b) => [g.x + u[0] * a + v[0] * b, g.y + u[1] * a + v[1] * b]));
  return [Math.min(...cantos.map((c) => c[0])), Math.min(...cantos.map((c) => c[1])), Math.max(...cantos.map((c) => c[0])), Math.max(...cantos.map((c) => c[1]))];
}

/** Junta `b` ao fim de `a` se continuam a mesma linha girada (palavras do mesmo texto). */
function juntaGirado(a: Girado, b: Girado): boolean {
  const dif = Math.abs(((a.graus - b.graus + 540) % 360) - 180);
  if (dif > 0.5 || Math.abs(a.tam - b.tam) > 0.2) return false;
  const { u } = eixosGirado(a.graus);
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const ao = dx * u[0] + dy * u[1];
  const lado = Math.abs(dx * u[1] - dy * u[0]);
  const vao = ao - a.w;
  if (lado > 0.3 * a.tam || vao < -0.3 * a.tam || vao > 1.2 * a.tam) return false;
  if (vao > 0.15 * a.tam && !/\s$/.test(a.texto) && !/^\s/.test(b.texto)) a.texto += ' ';
  a.texto += b.texto;
  a.w = ao + b.w;
  return true;
}

// ---------- linhas e paragrafos ----------

function montarLinhas(trechos: Trecho[]): Linha[] {
  // os trechos grandes formam as linhas primeiro; indices e expoentes entram depois
  const ordem = [...trechos].sort((a, b) => b.tam - a.tam || a.base - b.base);
  const linhas: Linha[] = [];
  for (const t of ordem) {
    let alvo: Linha | null = null;
    let menor = Infinity;
    for (const l of linhas) {
      const d = Math.abs(t.base - l.base);
      // letra do mesmo tamanho um pouco abaixo ou acima, colada na linha (o "E" do LaTeX)
      const vizinha = d <= 0.4 * l.tam && Math.abs(t.tam - l.tam) < 0.15 * l.tam && t.x <= l.x1 + 0.5 * l.tam && t.x1 >= l.x0 - 0.5 * l.tam;
      const mesmaAltura = d <= 0.25 * l.tam || vizinha;
      // expoente ou indice e curto: um trecho de varias palavras logo abaixo de um titulo e texto, nao indice
      const indice =
        t.tam < 0.8 * l.tam &&
        t.texto.trim().length <= 12 &&
        !/\s/.test(t.texto.trim()) &&
        t.base >= l.base - 0.75 * l.tam &&
        t.base <= l.base + 0.45 * l.tam &&
        t.x <= l.x1 + 2 * l.tam &&
        t.x1 >= l.x0 - 2 * l.tam;
      // entre as linhas possiveis, a mais perto na horizontal (outra coluna na mesma altura perde)
      const longe = Math.max(0, t.x - l.x1, l.x0 - t.x1);
      const nota = d + 0.1 * longe;
      if ((mesmaAltura || indice) && nota < menor) {
        menor = nota;
        alvo = l;
      }
    }
    if (!alvo) {
      linhas.push({ trechos: [t], x0: t.x, x1: t.x1, base: t.base, tam: t.tam, tabulada: false });
      continue;
    }
    if (Math.abs(t.base - alvo.base) > 0.2 * alvo.tam && t.tam < 0.8 * alvo.tam) {
      t.vert = t.base < alvo.base ? 'superscript' : 'subscript';
    }
    // subida ou descida exata (expoentes, o "E" do logo do LaTeX), em pt
    if (Math.abs(t.base - alvo.base) > 0.3) t.desloc = alvo.base - t.base;
    alvo.trechos.push(t);
    alvo.x0 = Math.min(alvo.x0, t.x);
    alvo.x1 = Math.max(alvo.x1, t.x1);
  }
  for (const l of linhas) {
    l.trechos.sort((a, b) => a.x - b.x);
    // tamanho dominante: o que cobre mais largura
    const peso = new Map<number, number>();
    for (const t of l.trechos) if (!t.vert) peso.set(t.tam, (peso.get(t.tam) ?? 0) + (t.x1 - t.x));
    let melhor = l.tam;
    let maior = -1;
    for (const [tam, p] of peso) if (p > maior) [maior, melhor] = [p, tam];
    l.tam = melhor;
  }
  return linhas.sort((a, b) => a.base - b.base || a.x0 - b.x0);
}

/** Separa a linha onde ha um vao grande (colunas, tabelas, sumario). Com `meio`, um vao
 * menor tambem separa quando cruza o meio da pagina (o numero "(1)" de uma equacao colado
 * no texto da outra coluna). */
function pedacos(l: Linha, meio?: number): Linha[] {
  const partes: Linha[] = [];
  let atual: Trecho[] = [];
  for (const t of l.trechos) {
    const ultimo = atual[atual.length - 1];
    const vao = ultimo ? t.x - ultimo.x1 : 0;
    const noMeio = meio !== undefined && ultimo && ultimo.x1 <= meio + 2 && t.x >= meio - 2 && vao > 0.8 * l.tam;
    if (ultimo && (vao > 2.5 * l.tam || noMeio)) {
      partes.push(novaLinha(atual, l));
      atual = [];
    }
    atual.push(t);
  }
  if (atual.length) partes.push(novaLinha(atual, l));
  if (meio !== undefined && partes.length > 1) {
    // linhas de colunas diferentes na mesma altura: cada pedaco volta a ter a propria base
    for (const parte of partes) {
      const normais = parte.trechos.filter((t) => t.tam >= 0.8 * l.tam);
      if (!normais.length) continue;
      const conta = new Map<number, number>();
      for (const t of normais) conta.set(t.base, (conta.get(t.base) ?? 0) + t.texto.length);
      parte.base = [...conta.entries()].sort((a, b) => b[1] - a[1])[0][0];
      for (const t of parte.trechos) {
        const d = parte.base - t.base;
        t.desloc = Math.abs(d) > 0.3 ? d : undefined;
        if (t.tam >= 0.8 * l.tam) t.vert = undefined;
      }
    }
  }
  return partes;
}

function novaLinha(trechos: Trecho[], origem: Linha): Linha {
  return {
    trechos,
    x0: Math.min(...trechos.map((t) => t.x)),
    x1: Math.max(...trechos.map((t) => t.x1)),
    base: origem.base,
    tam: origem.tam,
    tabulada: false,
  };
}

/** Pagina em uma ou duas colunas, em blocos de cima para baixo. */
function montarBlocos(p: Pagina): Bloco[] {
  const meio = p.largura / 2;
  const partes = p.linhas.flatMap((l) => pedacos(l, meio).map((x) => ({ l, x })));
  // so linhas de texto de verdade contam (rotulos curtos nas bordas nao fazem colunas)
  const longa = ({ x }: { x: Linha }) => x.x1 - x.x0 > 0.2 * p.largura;
  const esq = partes.filter((y) => y.x.x1 < meio && longa(y));
  const dir = partes.filter((y) => y.x.x0 > meio && longa(y));
  const duas = esq.length >= 6 && dir.length >= 6;
  const unir = (ls: Linha[]): Linha[] => ls;
  if (!duas) {
    const linhas = p.linhas.map((l) => {
      const ps = pedacos(l);
      if (ps.length > 1) l.tabulada = true;
      return l;
    });
    return [{ colunas: false, esq: unir(linhas), dir: [], ...limites(linhas) }];
  }
  // linhas que cruzam o meio da pagina (titulo, resumo, rodape) quebram as colunas
  const blocos: Bloco[] = [];
  let atual: { tipo: 'cheio' | 'colunas'; esq: Linha[]; dir: Linha[] } | null = null;
  for (const l of p.linhas) {
    const ps = pedacos(l, meio);
    const cruza = ps.some((x) => x.x0 < meio - 2 && x.x1 > meio + 2);
    const tipo = cruza ? 'cheio' : 'colunas';
    if (!atual || atual.tipo !== tipo) {
      if (atual) blocos.push(fecharBloco(atual));
      atual = { tipo, esq: [], dir: [] };
    }
    if (cruza) {
      if (ps.length > 1) l.tabulada = true;
      atual.esq.push(l);
    } else {
      for (const x of ps) (x.x1 < meio ? atual.esq : atual.dir).push(x);
    }
  }
  if (atual) blocos.push(fecharBloco(atual));
  return blocos;
}

function fecharBloco(b: { tipo: 'cheio' | 'colunas'; esq: Linha[]; dir: Linha[] }): Bloco {
  if (b.tipo === 'cheio' || !b.dir.length || !b.esq.length) {
    const todas = [...b.esq, ...b.dir].sort((a, c) => a.base - c.base);
    return { colunas: false, esq: todas, dir: [], ...limites(todas) };
  }
  const e = limites(b.esq);
  const d = limites(b.dir);
  return { colunas: true, esq: b.esq, dir: b.dir, xa: e.xa, xb: e.xb, xc: d.xa, xd: d.xb };
}

function limites(linhas: Linha[]): { xa: number; xb: number; xc: number; xd: number } {
  if (!linhas.length) return { xa: 0, xb: 0, xc: 0, xd: 0 };
  // a margem e a mais comum entre as linhas (um titulo centralizado nao a desloca)
  const xa = Math.min(...linhas.map((l) => l.x0));
  const xb = Math.max(...linhas.map((l) => l.x1));
  return { xa, xb, xc: 0, xd: 0 };
}

const ROTULO = /^([•·◦▪▸►‣\-–—*∙○■□✓]|\d{1,3}[.)]|[a-zA-Z][.)]|\([a-zA-Z0-9ivx]{1,5}\)|[ivxIVX]{1,5}[.)])$/;

/** Junta as linhas de uma coluna em paragrafos. xa e xb sao as bordas do texto. */
function montarParagrafos(linhas: Linha[], xa: number, xb: number): Paragrafo[] {
  const paragrafos: Paragrafo[] = [];
  let atual: Linha[] = [];
  // x das linhas de continuacao; null ate a segunda linha dizer (recuo de primeira linha)
  let corpo: number | null = null;
  const fechar = () => {
    if (atual.length) paragrafos.push(formatar(atual, xa, xb, corpo ?? atual[0].x0));
    atual = [];
  };
  // linhas empilhadas (mais perto que uma entrelinha e uma em cima da outra): formula
  const grupo = new Array<number>(linhas.length).fill(-1);
  for (let i = 1; i < linhas.length; i++) {
    const a = linhas[i - 1];
    const c = linhas[i];
    const perto = c.base - a.base < 0.85 * Math.max(a.tam, c.tam);
    const sobre = c.x0 < a.x1 && c.x1 > a.x0;
    if (perto && sobre) grupo[i] = grupo[i - 1] = grupo[i - 1] >= 0 ? grupo[i - 1] : i - 1;
  }
  for (let i = 0; i < linhas.length; i++) {
    const l = linhas[i];
    if (grupo[i] >= 0) {
      if (grupo[i] === i) {
        fechar();
        const juntas = linhas.filter((_, k) => grupo[k] === i);
        paragrafos.push({ ...formatar(juntas, xa, xb, juntas[0].x0), aglomerado: true });
      }
      continue;
    }
    const ant = atual[atual.length - 1];
    if (ant) {
      const vao = l.base - ant.base;
      const alinhada = corpo !== null ? Math.abs(l.x0 - corpo) < 1.5 : l.x0 <= ant.x0 + 1.5 && l.x0 >= xa - 1.5;
      const continua =
        !l.tabulada &&
        !ant.tabulada &&
        Math.abs(l.tam - ant.tam) < 0.6 &&
        vao > 0.5 * l.tam &&
        vao < 1.9 * l.tam &&
        ant.x1 >= xb - 1.2 * l.tam &&
        alinhada &&
        mesmaFonte(ant, l);
      if (continua) {
        atual.push(l);
        if (corpo === null) corpo = l.x0;
        continue;
      }
      fechar();
    }
    atual = [l];
    // lista: o texto depois do rotulo marca onde as linhas seguintes comecam
    const [r, segundo] = l.trechos;
    corpo = r && segundo && ROTULO.test(r.texto.trim()) ? segundo.x : null;
  }
  fechar();
  return paragrafos;
}

function mesmaFonte(a: Linha, b: Linha): boolean {
  const fa = a.trechos.find((t) => !t.vert)?.fonte;
  const fb = b.trechos.find((t) => !t.vert)?.fonte;
  return !fa || !fb || fa === fb || fa === 'Cambria Math' || fb === 'Cambria Math';
}

function formatar(linhas: Linha[], xa: number, xb: number, corpo: number): Paragrafo {
  const primeira = linhas[0];
  const vaos = linhas.slice(1).map((l, i) => l.base - linhas[i].base).sort((a, b) => a - b);
  const entrelinha = vaos.length ? vaos[(vaos.length / 2) | 0] : primeira.tam * 1.2;
  let alinhamento: Paragrafo['alinhamento'] = 'left';
  const centro = (xa + xb) / 2;
  const centrada = (l: Linha) => Math.abs((l.x0 + l.x1) / 2 - centro) < 2.5 && l.x0 > xa + 4;
  if (linhas.length > 1) {
    const cheias = linhas.slice(0, -1).every((l) => l.x1 >= xb - 1.2 * l.tam);
    if (linhas.every(centrada)) alinhamento = 'center';
    else if (cheias) alinhamento = 'both';
  } else if (!primeira.tabulada) {
    if (centrada(primeira)) alinhamento = 'center';
    else if (Math.abs(primeira.x1 - xb) < 2 && primeira.x0 > xa + 20) alinhamento = 'right';
  }
  const esquerda = alinhamento === 'center' || alinhamento === 'right' ? 0 : corpo - xa;
  const prim = alinhamento === 'center' || alinhamento === 'right' ? 0 : primeira.x0 - xa - esquerda;
  // linha de sumario: texto, pontos e o numero da pagina na margem direita
  let tabDireita: Paragrafo['tabDireita'];
  const ultimaLinha = linhas[linhas.length - 1];
  const fim = ultimaLinha.trechos[ultimaLinha.trechos.length - 1];
  if (
    alinhamento === 'left' &&
    ultimaLinha.trechos.length >= 2 &&
    // 3 pt de folga: o numero fica alinhado a direita, mas o 1 e o 3 terminam em pontos diferentes
    fim.x1 >= xb - 3 &&
    /^\s*[\divxlcIVXLC]{1,5}\s*$/.test(fim.texto) &&
    ultimaLinha.x0 < xb - 8 * ultimaLinha.tam
  ) {
    const antes = ultimaLinha.trechos.slice(0, -1).map((t) => t.texto).join('');
    // no Word a posicao da tabulacao conta da margem (ou da borda da coluna), sem o recuo
    tabDireita = { pos: xb - xa, pontilhado: /(\.\s*){3,}$/.test(antes) };
    if (!tabDireita.pontilhado && fim.x - ultimaLinha.trechos[ultimaLinha.trechos.length - 2].x1 < 1.5 * ultimaLinha.tam) {
      tabDireita = undefined; // numero colado no texto: nao e coluna de pagina
    }
  }
  const excesso = Math.max(0, Math.max(...linhas.map((l) => l.x1)) - xb);
  return { linhas, alinhamento, esquerda, primeira: prim, entrelinha, antes: 0, tabDireita, excesso };
}

// ---------- XML do Word ----------

const tw = (pt: number) => Math.round(pt * 20);
const emu = (pt: number) => Math.round(pt * 12700);
const esc = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
    // caracteres de controle nao valem em XML
    .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F￾￿]/g, '');

function run(t: Trecho, texto: string): string {
  if (t.nota !== undefined) {
    return `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="${t.nota}"/></w:r>`;
  }
  const props: string[] = [];
  props.push(`<w:rFonts w:ascii="${esc(t.fonte)}" w:hAnsi="${esc(t.fonte)}" w:cs="${esc(t.fonte)}" w:eastAsia="${esc(t.fonte)}"/>`);
  if (t.negrito) props.push('<w:b/><w:bCs/>');
  if (t.italico) props.push('<w:i/><w:iCs/>');
  if (t.cor !== '000000') props.push(`<w:color w:val="${t.cor}"/>`);
  // tamanho real e deslocamento exato (w:position, em meios pontos): o vertAlign do Word
  // encolhe e desloca do jeito dele, diferente do LaTeX
  if (t.desloc) props.push(`<w:position w:val="${Math.round(t.desloc * 2)}"/>`);
  const sz = Math.max(2, Math.round(t.tam * 2));
  props.push(`<w:sz w:val="${sz}"/><w:szCs w:val="${sz}"/>`);
  return `<w:r><w:rPr>${props.join('')}</w:rPr><w:t xml:space="preserve">${esc(texto)}</w:t></w:r>`;
}

// ---------- links ----------

/** Destinos de link do documento: nome -> pagina (0 a n-1) e altura em pt a partir do topo. */
let destinosDoDocumento = new Map<string, { pagina: number; y: number }>();
/** Enderecos externos dos links, na ordem em que aparecem (a relacao e rIdLink + indice). */
let urlsDasLigacoes: string[] = [];
/** Nome do marcador do Word de cada destino. */
let marcadoresDestino = new Map<string, { nome: string; id: number }>();

interface LigacaoPdf {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  url?: string;
  destino?: string;
}

/** Links da pagina (anotacoes Link do PDF): retangulo em pt a partir do topo e o alvo. */
async function ligacoesDaPagina(doc: PdfJs.PDFDocumentProxy, pagina: PdfJs.PDFPageProxy, vp: PdfJs.PageViewport): Promise<LigacaoPdf[]> {
  const notas: any[] = await pagina.getAnnotations({ intent: 'display' });
  const saida: LigacaoPdf[] = [];
  for (const n of notas) {
    if (n.subtype !== 'Link' || !n.rect) continue;
    const [ax, ay, bx, by] = vp.convertToViewportRectangle(n.rect);
    const lig: LigacaoPdf = { x0: Math.min(ax, bx), y0: Math.min(ay, by), x1: Math.max(ax, bx), y1: Math.max(ay, by) };
    const url: string | undefined = n.url || n.unsafeUrl;
    if (url && /^(https?|mailto|ftp):/i.test(url)) lig.url = url;
    else if (n.dest) {
      const nome = typeof n.dest === 'string' ? n.dest : JSON.stringify(n.dest);
      if (!destinosDoDocumento.has(nome)) {
        try {
          const d = typeof n.dest === 'string' ? await doc.getDestination(n.dest) : n.dest;
          if (Array.isArray(d)) {
            const alvo = d[0];
            const indice = typeof alvo === 'number' ? alvo : await doc.getPageIndex(alvo);
            const alvoPagina = await doc.getPage(indice + 1);
            const v2 = alvoPagina.getViewport({ scale: 1 });
            const y = typeof d[3] === 'number' ? v2.convertToViewportPoint(typeof d[2] === 'number' ? d[2] : 0, d[3])[1] : 0;
            destinosDoDocumento.set(nome, { pagina: indice, y });
          }
        } catch {
          // destino que o PDF.js nao resolve: o link fica so como texto
        }
      }
      if (destinosDoDocumento.has(nome)) lig.destino = nome;
    }
    if (lig.url || lig.destino) saida.push(lig);
  }
  return saida;
}

interface Palavra {
  /** posicao das letras no texto do trecho */
  a: number;
  b: number;
  /** onde a palavra fica na pagina, em pt */
  x0: number;
  x1: number;
}

/** Palavras de um trecho com a posicao de cada uma. Um trecho do PDF e uma linha inteira e a
 * largura das letras varia, entao a posicao vem da tinta da pagina (diferenca entre a pagina
 * com e sem texto): vaos largos separam as palavras. Se a conta nao fecha, divide pela
 * quantidade de letras. */
function palavrasDoTrecho(
  t: Trecho,
  com: { dados: Uint8ClampedArray },
  sem: { dados: Uint8ClampedArray },
  W: number,
  H: number,
): Palavra[] {
  const n = t.texto.length;
  const larg = (t.x1 - t.x) / n;
  const saida: Palavra[] = [];
  for (const m of t.texto.matchAll(/\S+/g)) {
    saida.push({ a: m.index!, b: m.index! + m[0].length, x0: t.x + m.index! * larg, x1: t.x + (m.index! + m[0].length) * larg });
  }
  if (!saida.length) return saida;
  const xa = Math.max(0, Math.floor(t.x * ESCALA));
  const xb = Math.min(W - 1, Math.ceil(t.x1 * ESCALA));
  const ya = Math.max(0, Math.floor((t.base - t.tam * 0.75) * ESCALA));
  const yb = Math.min(H - 1, Math.ceil((t.base + t.tam * 0.1) * ESCALA));
  const tinta: boolean[] = [];
  for (let x = xa; x <= xb; x++) {
    let achou = false;
    for (let y = ya; y <= yb && !achou; y++) {
      const i = (y * W + x) * 4;
      achou = Math.abs(com.dados[i] - sem.dados[i]) + Math.abs(com.dados[i + 1] - sem.dados[i + 1]) + Math.abs(com.dados[i + 2] - sem.dados[i + 2]) > 60;
    }
    tinta.push(achou);
  }
  const vaoMinimo = Math.max(2, 0.18 * t.tam * ESCALA);
  const grupos: [number, number][] = [];
  let ini = -1;
  let vazio = 0;
  tinta.forEach((c, i) => {
    if (c) {
      if (ini < 0) ini = i;
      else if (vazio >= vaoMinimo) {
        grupos.push([ini, i - vazio - 1]);
        ini = i;
      }
      vazio = 0;
    } else vazio++;
  });
  if (ini >= 0) grupos.push([ini, tinta.length - 1 - vazio]);
  if (grupos.length !== saida.length) return saida;
  grupos.forEach(([g0, g1], k) => {
    saida[k].x0 = (xa + g0) / ESCALA;
    saida[k].x1 = (xa + g1 + 1) / ESCALA;
  });
  return saida;
}

/** Marca nos trechos as palavras que ficam dentro de cada link, partindo o trecho onde preciso. */
function aplicarLigacoes(
  trechos: Trecho[],
  ligacoes: LigacaoPdf[],
  com: { dados: Uint8ClampedArray },
  sem: { dados: Uint8ClampedArray },
  W: number,
  H: number,
): Trecho[] {
  let atual = trechos;
  for (const lig of ligacoes) {
    const proximo: Trecho[] = [];
    for (const t of atual) {
      const meioY = t.base - t.tam * 0.275;
      if (t.ligacao || !t.texto.trim() || meioY < lig.y0 - 1 || meioY > lig.y1 + 1 || t.x1 < lig.x0 || t.x > lig.x1) {
        proximo.push(t);
        continue;
      }
      const palavras = palavrasDoTrecho(t, com, sem, W, H);
      // palavra dentro do link: metade ou mais dela fica entre as bordas do retangulo
      const dentro = palavras.map((p) => Math.min(p.x1, lig.x1) - Math.max(p.x0, lig.x0) >= 0.5 * (p.x1 - p.x0));
      if (!dentro.some(Boolean)) {
        proximo.push(t);
        continue;
      }
      // sequencias de palavras dentro do link; o resto do texto fica como estava
      let cursor = 0;
      let xCursor = t.x;
      const pedaco = (a: number, b: number, x0: number, x1: number, ligacao?: Trecho['ligacao']): Trecho => ({ ...t, texto: t.texto.slice(a, b), x: x0, x1, ligacao });
      let k = 0;
      while (k < palavras.length) {
        if (!dentro[k]) {
          k++;
          continue;
        }
        let f = k;
        while (f + 1 < palavras.length && dentro[f + 1]) f++;
        if (palavras[k].a > cursor) proximo.push(pedaco(cursor, palavras[k].a, xCursor, palavras[k].x0));
        proximo.push(pedaco(palavras[k].a, palavras[f].b, palavras[k].x0, palavras[f].x1, { url: lig.url, destino: lig.destino }));
        cursor = palavras[f].b;
        xCursor = palavras[f].x1;
        k = f + 1;
      }
      if (cursor < t.texto.length) proximo.push(pedaco(cursor, t.texto.length, xCursor, t.x1));
    }
    atual = proximo;
  }
  return atual;
}

/** Nome do marcador do Word para um destino (so letras, algarismos e sublinhado). */
function marcadorDoDestino(destino: string): { nome: string; id: number } {
  let m = marcadoresDestino.get(destino);
  if (!m) {
    m = { nome: `_Ref_destino_${marcadoresDestino.size + 1}`, id: 1000 + marcadoresDestino.size };
    marcadoresDestino.set(destino, m);
  }
  return m;
}

/** Envolve o run num link do Word (externo por relacao, interno por marcador). */
function envolverLigacao(t: Trecho, xml: string): string {
  const lig = t.ligacao;
  if (!lig) return xml;
  if (lig.url) {
    let i = urlsDasLigacoes.indexOf(lig.url);
    if (i < 0) i = urlsDasLigacoes.push(lig.url) - 1;
    return `<w:hyperlink r:id="rIdLink${i}" w:history="1">${xml}</w:hyperlink>`;
  }
  if (lig.destino && destinosDoDocumento.has(lig.destino)) {
    return `<w:hyperlink w:anchor="${marcadorDoDestino(lig.destino).nome}" w:history="1">${xml}</w:hyperlink>`;
  }
  return xml;
}

// ---------- hifen de fim de linha ----------

/** Palavras do documento e palavras que o dicionario do app conhece (preenchidas antes de
 * montar o XML). Sem dicionario, `dicionario` fica vazio e valem o documento e a lista abaixo. */
let contextoHifen: { documento: Set<string>; dicionario: Set<string> } = { documento: new Set(), dicionario: new Set() };

/** Primeiros elementos de palavras compostas do portugues, que mantem o hifen ("guarda-chuva",
 * "ex-aluno", "recem-nascido"). So entram aqui quando o documento e o dicionario nao decidem. */
const PREFIXOS_COM_HIFEN = new Set([
  'guarda', 'quebra', 'porta', 'abre', 'beija', 'arranha', 'saca', 'lava', 'papa', 'bem', 'alem', 'aquem', 'recem', 'ex', 'vice', 'sota', 'soto',
]);

const semAcento = (x: string) => x.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();

/** O hifen no fim da linha e so quebra de palavra (juntar) ou faz parte da palavra (manter)?
 * Ordem: o proprio documento ("guarda-chuva" escrito inteiro em outro lugar), o dicionario
 * pt-BR do app (se a palavra junta existe, e quebra), os primeiros elementos de composta. */
export function hifenDeQuebra(anterior: string, seguinte: string): boolean {
  const a = anterior.match(/([A-Za-zÀ-ÿ]+)-$/)?.[1];
  const b = seguinte.match(/^([a-zà-ÿ]+)/)?.[1];
  if (!a || !b) return true;
  const junta = (a + b).toLowerCase();
  const composta = `${a}-${b}`.toLowerCase();
  const { documento, dicionario } = contextoHifen;
  if (documento.has(composta)) return false;
  if (documento.has(junta)) return true;
  if (dicionario.has(junta)) return true;
  if (dicionario.has(composta)) return false;
  return !PREFIXOS_COM_HIFEN.has(semAcento(a));
}

/** Palavras do texto do PDF e, se houver dicionario, as que ele conhece entre as palavras
 * formadas pelos hifens de fim de linha. */
async function montarContextoHifen(
  paginas: Pagina[],
  dicionario?: (palavras: string[]) => Promise<string[]>,
): Promise<{ documento: Set<string>; dicionario: Set<string> }> {
  const documento = new Set<string>();
  const candidatas = new Set<string>();
  for (const p of paginas) {
    const linhas = [...p.linhas].sort((x, y) => x.base - y.base || x.x0 - y.x0);
    linhas.forEach((l, i) => {
      const texto = l.trechos.map((t) => t.texto).join('');
      for (const w of texto.match(/[A-Za-zÀ-ÿ]+(?:-[A-Za-zÀ-ÿ]+)+/g) ?? []) documento.add(w.toLowerCase());
      for (const w of texto.match(/[A-Za-zÀ-ÿ]+/g) ?? []) documento.add(w.toLowerCase());
      const fim = texto.match(/([A-Za-zÀ-ÿ]+)-\s*$/);
      const prox = linhas[i + 1]?.trechos.map((t) => t.texto).join('').match(/^\s*([a-zà-ÿ]+)/);
      if (fim && prox) {
        candidatas.add((fim[1] + prox[1]).toLowerCase());
        candidatas.add(`${fim[1]}-${prox[1]}`.toLowerCase());
      }
    });
  }
  let conhecidas: string[] = [];
  if (dicionario && candidatas.size) conhecidas = await dicionario([...candidatas]).catch(() => []);
  return { documento, dicionario: new Set(conhecidas.map((w) => w.toLowerCase())) };
}

/** Os runs de um paragrafo: espacos entre trechos, hifen de fim de linha desfeito, tabs. */
function runsDoParagrafo(p: Paragrafo, xa: number): { xml: string; tabs: number[] } {
  const partes: string[] = [];
  const tabs: number[] = [];
  let anterior: Trecho | null = null;
  let textoAnterior = '';
  let numeroFinal: Trecho | null = null;
  if (p.tabDireita) {
    // tira os pontos do PDF: o Word desenha os seus com o pontilhado da tabulacao
    const l = p.linhas[p.linhas.length - 1];
    numeroFinal = l.trechos[l.trechos.length - 1];
    if (p.tabDireita.pontilhado) {
      const resto = l.trechos.slice(0, -1);
      while (resto.length > 1 && /^[\s.·]*$/.test(resto[resto.length - 1].texto)) resto.pop();
      const ult = resto[resto.length - 1];
      if (ult) ult.texto = ult.texto.replace(/[\s.·]+$/, '') || ult.texto;
      l.trechos = [...resto, numeroFinal];
    }
  }
  p.linhas.forEach((l, nl) => {
    l.trechos.forEach((t, i) => {
      let texto = t.texto;
      if (t === numeroFinal) {
        partes.push('<w:r><w:tab/></w:r>');
        partes.push(envolverLigacao(t, run(t, texto.trim())));
        anterior = t;
        textoAnterior = texto;
        return;
      }
      if (i === 0 && nl > 0 && anterior) {
        // "compu-" + "tacional" vira "computacional" (o Word hifeniza de novo); "guarda-" +
        // "chuva" mantem o hifen, porque a palavra e composta
        if (/[A-Za-zÀ-ÿ]-$/.test(textoAnterior) && /^[a-zà-ÿ]/.test(texto) && hifenDeQuebra(textoAnterior, texto)) {
          const ultimo = partes.pop()!;
          partes.push(ultimo.replace(/-<\/w:t><\/w:r>$/, '</w:t></w:r>'));
        } else if (!/\s$/.test(textoAnterior) && !(/[A-Za-zÀ-ÿ]-$/.test(textoAnterior) && /^[a-zà-ÿ]/.test(texto))) {
          texto = ' ' + texto; // hifen que fica na palavra composta: "guarda-" + "chuva" sem espaco
        }
      } else if (anterior && i > 0) {
        const vao = t.x - anterior.x1;
        if (l.tabulada && vao > 2.5 * l.tam) {
          tabs.push(t.x - xa);
          partes.push('<w:r><w:tab/></w:r>');
        } else if (vao > 0.12 * l.tam && !/\s$/.test(textoAnterior) && !/^\s/.test(texto)) {
          texto = ' ' + texto;
        }
      }
      partes.push(envolverLigacao(t, run(t, texto)));
      anterior = t;
      textoAnterior = texto;
    });
  });
  return { xml: partes.join(''), tabs };
}

/** Equacao numerada: o numero "(1)" entra dentro do proprio OMML, no formato "formula#(1)" do Word
 * (matriz com maxDist), alinhado a direita. Nao usa caixa flutuante, que o Google Docs descarta. */
function ommlComNumero(eq: NonNullable<Paragrafo['equacao']>): string {
  if (!eq.numeros.length) return eq.omml;
  const textos = [...eq.numeros].sort((a, b) => a.base - b.base).map((l) => l.trechos.map((x) => x.texto.trim()).join(''));
  const interno = eq.omml.replace(/^<m:oMath>/, '').replace(/<\/m:oMath>$/, '');
  const marca = (n: string) => `<m:r><m:t>#</m:t></m:r><m:r><m:t>${esc(n)}</m:t></m:r>`;
  // align com varios numeros: cada numero vai no fim da linha dele ("(2)" e "(3)" em linhas separadas)
  if (textos.length > 1 && interno.startsWith('<m:eqArr>')) {
    const partes: string[] = [];
    let nivel = 0;
    let linha = 0;
    let ultimo = 0;
    for (const m of interno.matchAll(/<m:e>|<\/m:e>/g)) {
      if (m[0] === '<m:e>') nivel++;
      else if (--nivel === 0) {
        partes.push(interno.slice(ultimo, m.index), marca(textos[linha] ?? ''));
        ultimo = m.index!;
        linha++;
      }
    }
    if (linha === textos.length) {
      partes.push(interno.slice(ultimo));
      return `<m:oMath>${partes.join('')}</m:oMath>`;
    }
  }
  return `<m:oMath><m:eqArr><m:eqArrPr><m:maxDist m:val="1"/></m:eqArrPr><m:e>${interno}${marca(textos.join(' '))}</m:e></m:eqArr></m:oMath>`;
}

function paragrafoXml(p: Paragrafo, xa: number, quebraPagina: boolean, extra = '', sectPr = ''): string {
  if (p.equacao) {
    // equacao do Word centrada na largura do texto (o numero vai antes, numa caixa)
    return (
      `<w:p><w:pPr>${quebraPagina ? '<w:pageBreakBefore/>' : ''}<w:widowControl w:val="0"/>` +
      // altura minima igual a do PDF: o texto seguinte fica no mesmo lugar, e uma equacao
      // mais alta no Word cresce sem ser cortada
      `<w:spacing w:before="${tw(Math.max(0, p.antes))}" w:after="0" w:line="${tw(p.entrelinha)}" w:lineRule="atLeast"/>` +
      `<w:ind w:left="0" w:right="0"/><w:jc w:val="center"/>${sectPr}</w:pPr>${extra}` +
      `<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr>${ommlComNumero(p.equacao)}</m:oMathPara></w:p>`.replace('</w:pPr>', `</w:pPr>${p.marcas ?? ''}`)
    );
  }
  const { xml, tabs } = runsDoParagrafo(p, xa);
  const pPr: string[] = [];
  if (p.estilo) pPr.push(`<w:pStyle w:val="${p.estilo}"/>`);
  if (quebraPagina) pPr.push('<w:pageBreakBefore/>');
  pPr.push('<w:widowControl w:val="0"/>');
  if (p.numeracao) pPr.push(`<w:numPr><w:ilvl w:val="0"/><w:numId w:val="${p.numeracao}"/></w:numPr>`);
  const paradas = tabs.map((x) => `<w:tab w:val="left" w:pos="${tw(Math.max(0, x))}"/>`);
  if (p.tabDireita) {
    paradas.push(`<w:tab w:val="right"${p.tabDireita.pontilhado ? ' w:leader="dot"' : ''} w:pos="${tw(p.tabDireita.pos)}"/>`);
  }
  if (paradas.length) pPr.push('<w:tabs>' + paradas.join('') + '</w:tabs>');
  pPr.push(`<w:spacing w:before="${tw(Math.max(0, p.antes))}" w:after="0" w:line="${tw(p.entrelinha)}" w:lineRule="atLeast"/>`);
  const primeira = p.primeira >= 0 ? `w:firstLine="${tw(p.primeira)}"` : `w:hanging="${tw(-p.primeira)}"`;
  // paragrafo de uma linha (titulo, rodape, sumario) pode avancar na margem: se o Word
  // medir o texto um pouco mais largo, a linha nao quebra em duas e a pagina nao transborda
  let esquerda = p.esquerda;
  let direita = -p.excesso;
  if (p.linhas.length === 1 && !p.tabDireita) {
    if (p.alinhamento === 'center') {
      const folga = Math.min(folgaMargens.esquerda, folgaMargens.direita);
      esquerda = -folga;
      direita = -folga;
    } else if (p.alinhamento === 'right') {
      esquerda = Math.min(p.esquerda, 0) - folgaMargens.esquerda;
    } else {
      direita = Math.min(direita, -folgaMargens.direita);
    }
  }
  pPr.push(`<w:ind w:left="${tw(esquerda)}" w:right="${tw(direita)}" ${primeira}/>`);
  pPr.push(`<w:jc w:val="${p.alinhamento}"/>`);
  return `<w:p><w:pPr>${pPr.join('')}${sectPr}</w:pPr>${extra}${p.marcas ?? ''}${p.tocInicio ? CAMPO_TOC_ABRE : ''}${xml}</w:p>`;
}

function ancora(img: Imagem): string {
  const id = img.id;
  return (
    `<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="${id}" ` +
    `behindDoc="1" locked="0" layoutInCell="1" allowOverlap="1">` +
    `<wp:simplePos x="0" y="0"/>` +
    `<wp:positionH relativeFrom="page"><wp:posOffset>${emu(img.x)}</wp:posOffset></wp:positionH>` +
    `<wp:positionV relativeFrom="page"><wp:posOffset>${emu(img.y)}</wp:posOffset></wp:positionV>` +
    `<wp:extent cx="${emu(img.w)}" cy="${emu(img.h)}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/>` +
    `<wp:docPr id="${id}" name="Figura ${id}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">` +
    `<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">` +
    `<pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">` +
    `<pic:nvPicPr><pic:cNvPr id="${id}" name="figura${id}.png"/><pic:cNvPicPr/></pic:nvPicPr>` +
    `<pic:blipFill><a:blip r:embed="rIdImg${id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>` +
    `<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${emu(img.w)}" cy="${emu(img.h)}"/></a:xfrm>` +
    `<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>` +
    `</wp:anchor></w:drawing></w:r>`
  );
}

// ---------- equacoes ----------

/** Equacoes do .tex ainda nao achadas no PDF, na ordem do texto. */
let filaEquacoes: EquacaoTex[] = [];
/** Retangulos das equacoes trocadas na pagina atual (pt, a partir do topo). */
let areasEquacao: { x0: number; x1: number; y0: number; y1: number }[] = [];

const NUMERO_EQ = /^\(\s*[\w.\-–]{1,8}\s*\)$/;
const FUNCOES = /^(sin|cos|tan|cot|sec|csc|log|exp|lim|max|min|det|sup|inf|arg|mod|sinh|cosh|tanh|arcsin|arccos|arctan|lg|ker|dim|deg|gcd|Pr|hom)$/i;

const letrasPdf = (ts: Trecho[]) => ts.map((t) => t.texto).join('').normalize('NFKD').replace(/[^\p{L}\p{N}]/gu, '');

/** Trechos da formula e numeros "(1)" encostados a direita. */
function partesDaEquacao(p: Paragrafo, xa: number, xb: number): { formula: Trecho[]; numeros: Linha[] } {
  const formula: Trecho[] = [];
  const numeros: Linha[] = [];
  for (const l of p.linhas) {
    l.trechos.forEach((t, i) => {
      const ant = l.trechos[i - 1];
      const solto = !ant || t.x - ant.x1 > 1.5 * l.tam;
      if (NUMERO_EQ.test(t.texto.trim()) && solto && t.x > (xa + xb) / 2 && t.x1 > xb - 3 * l.tam) {
        numeros.push(novaLinha([t], { ...l, base: t.base }));
      } else formula.push(t);
    });
  }
  return { formula, numeros };
}

function pareceEquacao(p: Paragrafo, xa: number, xb: number): boolean {
  if (p.linhas.length > 12) return false;
  const { formula, numeros } = partesDaEquacao(p, xa, xb);
  if (!formula.length) return false;
  const total = formula.reduce((n, t) => n + t.texto.replace(/\s/g, '').length, 0);
  const mat = formula.filter((t) => t.fonte === 'Cambria Math').reduce((n, t) => n + t.texto.replace(/\s/g, '').length, 0);
  // texto corrido com formula no meio tem palavras; a equacao destacada quase nao tem
  const palavras = formula
    .filter((t) => t.fonte !== 'Cambria Math')
    .flatMap((t) => t.texto.match(/[A-Za-zÀ-ÿ]{3,}/g) ?? [])
    .filter((w) => !FUNCOES.test(w));
  if (palavras.length > 3) return false;
  const tam = Math.max(...p.linhas.map((l) => l.tam));
  const x0 = Math.min(...formula.map((t) => t.x));
  const recuada = x0 > xa + 2 * tam;
  return (recuada || !!p.aglomerado || numeros.length > 0) && (mat >= 0.2 * total || !!p.aglomerado || numeros.length > 0);
}

function parecido(a: string, b: string): number {
  if (!a || !b) return 0;
  const conta = new Map<string, number>();
  for (const c of a) conta.set(c, (conta.get(c) ?? 0) + 1);
  let comum = 0;
  for (const c of b) {
    const n = conta.get(c) ?? 0;
    if (n > 0) {
      comum++;
      conta.set(c, n - 1);
    }
  }
  return comum / Math.max([...a].length, [...b].length);
}

/** Troca os grupos de paragrafos que sao equacoes destacadas pela equacao do Word. */
function trocarEquacoes(ps: Paragrafo[], xa: number, xb: number): Paragrafo[] {
  if (!filaEquacoes.length) return ps;
  const saida: Paragrafo[] = [];
  for (let i = 0; i < ps.length; i++) {
    if (!pareceEquacao(ps[i], xa, xb)) {
      saida.push(ps[i]);
      continue;
    }
    // linhas de uma mesma equacao (fracao, align): paragrafos seguidos e perto. Um pedaco
    // so com algarismos ("2a" de um denominador) entra se estiver debaixo da equacao.
    const faixa = (q: Paragrafo) => ({ x0: Math.min(...q.linhas.map((l) => l.x0)), x1: Math.max(...q.linhas.map((l) => l.x1)) });
    // o grupo inteiro (largura e tamanho da letra) decide se um pedaco vizinho faz parte
    const continua = (q: Paragrafo, grupo: Paragrafo[], acima: boolean) => {
      const tam = Math.max(...grupo.flatMap((g) => g.linhas.map((l) => l.tam)));
      const vao = acima
        ? grupo[0].linhas[0].base - q.linhas[q.linhas.length - 1].base
        : q.linhas[0].base - Math.max(...grupo.flatMap((g) => g.linhas.map((l) => l.base)));
      if (vao >= 3 * tam) return false;
      if (pareceEquacao(q, xa, xb)) return true;
      // numero "(2)" sozinho (em duas colunas ele vira um pedaco a parte)
      const { formula, numeros } = partesDaEquacao(q, xa, xb);
      if (!formula.length && numeros.length) return true;
      // pedaco sem palavras colado na equacao: denominador, "lim", "= 1"
      const palavras = formula.flatMap((t) => t.texto.match(/[A-Za-zÀ-ÿ]{3,}/g) ?? []).filter((w) => !FUNCOES.test(w));
      return vao < 1.6 * tam && q.linhas.length <= 2 && palavras.length === 0 && faixa(q).x0 > xa + tam;
    };
    let j = i;
    while (j + 1 < ps.length && continua(ps[j + 1], ps.slice(i, j + 1), false)) j++;
    let inicio = i;
    while (inicio > 0 && saida.length && saida[saida.length - 1] === ps[inicio - 1] && continua(ps[inicio - 1], ps.slice(inicio, j + 1), true)) {
      saida.pop();
      inicio--;
    }
    const grupoPs = ps.slice(inicio, j + 1);
    const partes = grupoPs.map((p) => partesDaEquacao(p, xa, xb));
    const formula = partes.flatMap((x) => x.formula);
    const letras = letrasPdf(formula);
    // a mais parecida; entre as quase iguais, a primeira do texto. A fila toda conta porque
    // em duas colunas a ordem do PDF pode nao ser a do .tex.
    const notas = filaEquacoes.map((e) => parecido(letras, e.letras));
    const nota = Math.max(0, ...notas);
    const melhor = nota > 0.5 ? notas.findIndex((n) => n >= nota - 0.05) : -1;
    if (melhor < 0) {
      saida.push(...grupoPs);
      i = j;
      continue;
    }
    const eq = filaEquacoes[melhor];
    filaEquacoes.splice(melhor, 1);
    const linhas = grupoPs.flatMap((p) => p.linhas);
    const tam = Math.max(...linhas.map((l) => l.tam));
    const topo = Math.min(...linhas.map((l) => l.base - 0.8 * l.tam));
    const fundo = Math.max(...linhas.map((l) => l.base + 0.3 * l.tam));
    const altura = Math.max(tam * 1.2, fundo - topo);
    areasEquacao.push({ x0: Math.min(...linhas.map((l) => l.x0)), x1: Math.max(...linhas.map((l) => l.x1)), y0: topo - 0.3 * tam, y1: fundo + 0.2 * tam });
    const linha: Linha = {
      trechos: formula,
      x0: Math.min(...formula.map((t) => t.x)),
      x1: Math.max(...formula.map((t) => t.x1)),
      base: topo + 0.8 * altura,
      tam,
      tabulada: false,
    };
    saida.push({
      linhas: [linha],
      alinhamento: 'center',
      esquerda: 0,
      primeira: 0,
      entrelinha: altura,
      antes: 0,
      excesso: 0,
      equacao: { omml: comTamanho(eq.omml, tam), numeros: partes.flatMap((x) => x.numeros) },
    });
    i = j;
  }
  return saida;
}

// ---------- estrutura: titulos, sumario, notas de rodape e listas ----------

/** O que o .tex diz da estrutura do documento, para conferir o que o PDF mostra. */
export interface EstruturaTex {
  /** texto do \title, sem comandos */
  titulo?: string;
  /** titulos de \chapter, \section, \subsection e \subsubsection na ordem do texto */
  titulos: { nivel: 1 | 2 | 3; texto: string; numerado: boolean }[];
  /** quantidade de \footnote */
  notas: number;
  /** o documento tem \tableofcontents */
  sumario: boolean;
  /** o documento tem \bibliography ou \printbibliography (gera o titulo "Referencias") */
  referencias: boolean;
}

/** Texto entre chaves a partir de `i` (que aponta para a chave de abertura), com chaves aninhadas. */
function argumentoEntreChaves(s: string, i: number): { texto: string; fim: number } | null {
  if (s[i] !== '{') return null;
  let nivel = 0;
  for (let k = i; k < s.length; k++) {
    if (s[k] === '\\') {
      k++;
      continue;
    }
    if (s[k] === '{') nivel++;
    else if (s[k] === '}' && --nivel === 0) return { texto: s.slice(i + 1, k), fim: k + 1 };
  }
  return null;
}

/** Tira os comandos de um titulo: "\textbf{Meu} t\'itulo\label{a}" vira "Meu titulo". */
function textoSemComandos(s: string): string {
  return s
    .replace(/\\(?:label|index|cite\w*|ref|footnote)\s*\{[^}]*\}/g, '')
    .replace(/\\\\|~/g, ' ')
    .replace(/\\[a-zA-Z]+\*?\s*/g, '')
    .replace(/[{}$]/g, '')
    .replace(/\\(.)/g, '$1')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Titulos, notas e sumario do .tex (com os \input ja juntados). Com \chapter, o capitulo e o
 * Titulo 1 e a secao o Titulo 2; sem ele, a secao e o Titulo 1. */
export function estruturaDoTex(tex: string): EstruturaTex {
  const limpo = tex
    .split('\n')
    .map((l) => l.replace(/\r$/, '').replace(/(^|[^\\])%.*$/, '$1'))
    .join('\n');
  const corpo = limpo.includes('\\begin{document}') ? limpo.slice(limpo.indexOf('\\begin{document}')) : limpo;
  const brutos: { cmd: string; texto: string; numerado: boolean; pos: number }[] = [];
  const re = /\\(chapter|section|subsection|subsubsection|part)(\*?)\s*(?:\[[^\]]*\])?\s*\{/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(corpo))) {
    const arg = argumentoEntreChaves(corpo, m.index + m[0].length - 1);
    if (!arg) continue;
    brutos.push({ cmd: m[1], texto: textoSemComandos(arg.texto), numerado: m[2] !== '*', pos: m.index });
  }
  const temCapitulo = brutos.some((b) => b.cmd === 'chapter');
  const nivelDe = (cmd: string): 1 | 2 | 3 => {
    const ordem = temCapitulo ? ['chapter', 'section', 'subsection', 'subsubsection'] : ['section', 'subsection', 'subsubsection'];
    return Math.min(3, Math.max(1, ordem.indexOf(cmd) + 1)) as 1 | 2 | 3;
  };
  const titulos = brutos.filter((b) => b.cmd !== 'part').map((b) => ({ nivel: nivelDe(b.cmd), texto: b.texto, numerado: b.numerado }));
  let titulo: string | undefined;
  const t = /\\title\s*(?:\[[^\]]*\])?\s*\{/.exec(limpo) ?? /\\titulo\s*\{/.exec(limpo);
  if (t) {
    const arg = argumentoEntreChaves(limpo, t.index + t[0].length - 1);
    if (arg) titulo = textoSemComandos(arg.texto);
  }
  return {
    titulo,
    titulos,
    notas: (corpo.match(/\\footnote\s*(?:\[[^\]]*\])?\s*\{/g) ?? []).length,
    sumario: /\\tableofcontents/.test(corpo),
    referencias: /\\(?:bibliography|printbibliography)\b/.test(corpo),
  };
}

const normalizarTitulo = (s: string) =>
  s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '');

const NOMES_SUMARIO = ['sumario', 'contents', 'tableofcontents', 'indice', 'listadefiguras', 'listadetabelas'];
const NOMES_REFERENCIAS = ['referencias', 'bibliografia', 'references', 'bibliography'];

interface EstadoEstrutura {
  estrutura?: EstruturaTex;
  /** titulos do .tex ja ligados a um paragrafo do PDF */
  usados: Set<number>;
  tituloVisto: boolean;
  emSumario: boolean;
  /** a entrada de sumario que abre o campo TOC ja foi marcada */
  tocIniciado: boolean;
  proximaNota: number;
  notas: { id: number; xml: string }[];
  /** definicoes de lista (bullet ou numerada) e as instancias (w:num) */
  abstratos: { chave: string; xml: string }[];
  instancias: { abstrato: number; inicio: number }[];
  tamCorpo: number;
}

let estr: EstadoEstrutura = novoEstadoEstrutura();

function novoEstadoEstrutura(estrutura?: EstruturaTex): EstadoEstrutura {
  return { estrutura, usados: new Set(), tituloVisto: false, emSumario: false, tocIniciado: false, proximaNota: 1, notas: [], abstratos: [], instancias: [], tamCorpo: 12 };
}

function textoDoParagrafo(p: Paragrafo): string {
  return p.linhas
    .map((l) => l.trechos.map((t) => t.texto).join(' '))
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const RE_NUMERO_TITULO = /^\d+(?:\.\d+)*\.?\s+/;

/** Tamanho de letra do corpo do texto: o que cobre mais caracteres. */
function tamanhoDoCorpo(paginas: Pagina[]): number {
  const peso = new Map<number, number>();
  for (const p of paginas) for (const l of p.linhas) for (const t of l.trechos) {
    if (t.vert || t.fonte === 'Cambria Math') continue;
    const tam = Math.round(t.tam * 10) / 10;
    peso.set(tam, (peso.get(tam) ?? 0) + t.texto.length);
  }
  let melhor = 12;
  let maior = -1;
  for (const [tam, n] of peso) if (n > maior) [maior, melhor] = [n, tam];
  return melhor;
}

function definirLista(chave: string, xml: (id: number) => string): number {
  let i = estr.abstratos.findIndex((a) => a.chave === chave);
  if (i < 0) {
    i = estr.abstratos.length;
    estr.abstratos.push({ chave, xml: xml(i) });
  }
  return i;
}

function nivelLista(id: number, fmt: string, texto: string, fonte?: string): string {
  const rpr = fonte ? `<w:rPr><w:rFonts w:ascii="${esc(fonte)}" w:hAnsi="${esc(fonte)}" w:cs="${esc(fonte)}"/></w:rPr>` : '';
  return (
    `<w:abstractNum w:abstractNumId="${id}"><w:multiLevelType w:val="singleLevel"/>` +
    `<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="${fmt}"/><w:lvlText w:val="${esc(texto)}"/><w:lvlJc w:val="left"/>` +
    `<w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr>${rpr}</w:lvl></w:abstractNum>`
  );
}

const RE_PONTO = /^[•▪▸►‣∙◦▶▷➢■○–—-]$/;

/** Marca titulos, sumario e listas nos paragrafos de uma coluna, na ordem do documento. */
function classificarParagrafos(ps: Paragrafo[]): void {
  const corpo = estr.tamCorpo;
  const est = estr.estrutura;
  // 1. titulos e sumario
  for (const p of ps) {
    if (p.equacao || p.aglomerado) {
      estr.emSumario = false;
      estr.tocIniciado = false;
      continue;
    }
    const texto = textoDoParagrafo(p);
    if (!texto) continue;
    // entrada de sumario: texto, pontos e o numero da pagina na margem
    if (p.tabDireita && (estr.emSumario || p.tabDireita.pontilhado)) {
      estr.emSumario = true;
      p.toc = true;
      if (!estr.tocIniciado) p.tocInicio = true;
      estr.tocIniciado = true;
      const profundidade = (/^(\d+(?:\.\d+)*)/.exec(texto)?.[1].split('.').length ?? 1);
      p.estilo = `TOC${Math.min(3, profundidade)}`;
      continue;
    }
    estr.emSumario = false;
    estr.tocIniciado = false;
    if (p.tabDireita || p.linhas.length > 3 || texto.length > 100) continue;
    const tam = p.linhas[0].tam;
    const trechosTexto = p.linhas.flatMap((l) => l.trechos).filter((t) => t.fonte !== 'Cambria Math' && t.texto.trim());
    const negrito = trechosTexto.length > 0 && trechosTexto.every((t) => t.negrito);
    const grande = tam >= 1.15 * corpo;
    const chave = normalizarTitulo(texto.replace(RE_NUMERO_TITULO, ''));
    if (!chave) continue;
    if (est?.titulo && !estr.tituloVisto && grande && tam >= 1.3 * corpo && normalizarTitulo(texto) === normalizarTitulo(est.titulo)) {
      p.estilo = 'Title';
      estr.tituloVisto = true;
      continue;
    }
    if (!grande && !negrito) continue;
    if (p.linhas.length === 1 && NOMES_SUMARIO.includes(chave)) {
      p.estilo = 'TOCHeading';
      estr.emSumario = true;
      continue;
    }
    const idx = est ? est.titulos.findIndex((x, i) => !estr.usados.has(i) && normalizarTitulo(x.texto) === chave) : -1;
    let nivel = 0;
    if (idx >= 0) {
      estr.usados.add(idx);
      nivel = est!.titulos[idx].nivel;
    } else if (NOMES_REFERENCIAS.includes(chave) && p.linhas.length === 1) {
      nivel = 1;
    } else if (RE_NUMERO_TITULO.test(texto) && p.linhas.length <= 2 && texto.length <= 90) {
      nivel = Math.min(3, /^(\d+(?:\.\d+)*)/.exec(texto)![1].split('.').length);
    }
    if (nivel) p.estilo = `Heading${nivel}`;
  }
  // 2. listas: o marcador (bolinha, "1.", "(a)") vira a numeracao do Word e sai do texto
  type Item = { i: number; formato: string; valor: number; texto: string };
  const itens: Item[] = [];
  ps.forEach((p, i) => {
    if (p.estilo || p.equacao || p.aglomerado || p.tabDireita) return;
    const l0 = p.linhas[0];
    if (l0.tabulada || l0.trechos.length < 2) return;
    const r = l0.trechos[0].texto.trim();
    if (RE_PONTO.test(r)) {
      itens.push({ i, formato: `ponto:${r}`, valor: 0, texto: r });
      return;
    }
    let m: RegExpExecArray | null;
    if ((m = /^(\d{1,3})([.)])$/.exec(r))) itens.push({ i, formato: `decimal:%1${m[2]}`, valor: Number(m[1]), texto: r });
    else if ((m = /^\((\d{1,3})\)$/.exec(r))) itens.push({ i, formato: 'decimal:(%1)', valor: Number(m[1]), texto: r });
    else if ((m = /^\(([a-z])\)$/.exec(r))) itens.push({ i, formato: 'lowerLetter:(%1)', valor: m[1].charCodeAt(0) - 96, texto: r });
    else if ((m = /^([a-z])([.)])$/.exec(r))) itens.push({ i, formato: `lowerLetter:%1${m[2]}`, valor: m[1].charCodeAt(0) - 96, texto: r });
  });
  let k = 0;
  while (k < itens.length) {
    const it = itens[k];
    if (it.formato.startsWith('ponto:')) {
      const fonte = ps[it.i].linhas[0].trechos[0].fonte;
      const abs = definirLista(`${it.formato}:${fonte}`, (id) => nivelLista(id, 'bullet', it.texto, fonte));
      estr.instancias.push({ abstrato: abs, inicio: 1 });
      const numId = estr.instancias.length;
      // itens seguidos da mesma lista dividem a instancia
      let j = k;
      while (j < itens.length && itens[j].formato === it.formato && (j === k || itens[j].i - itens[j - 1].i <= 2)) {
        aplicarLista(ps[itens[j].i], numId);
        j++;
      }
      k = j;
      continue;
    }
    // numerada: so vale com 2 itens seguidos (1, 2, ...); um "1." sozinho e texto
    let j = k + 1;
    while (j < itens.length && itens[j].formato === it.formato && itens[j].valor === itens[j - 1].valor + 1 && itens[j].i - itens[j - 1].i <= 2) j++;
    if (j - k >= 2) {
      const [fmt, texto] = it.formato.split(':');
      const abs = definirLista(it.formato, (id) => nivelLista(id, fmt, texto));
      estr.instancias.push({ abstrato: abs, inicio: it.valor });
      const numId = estr.instancias.length;
      for (let q = k; q < j; q++) aplicarLista(ps[itens[q].i], numId);
    }
    k = j;
  }
}

function aplicarLista(p: Paragrafo, numId: number): void {
  const l0 = p.linhas[0];
  l0.trechos = l0.trechos.slice(1);
  l0.x0 = Math.min(...l0.trechos.map((t) => t.x));
  // o numero fica no recuo suspenso: sem ele o Word poe o numero dentro do texto
  if (p.primeira > -0.5) p.primeira = -Math.max(6, Math.min(p.esquerda, 18));
  p.numeracao = numId;
}

/** Notas de rodape da pagina: acha o filete, o numero e o texto da nota no pe da coluna e o
 * numero sobrescrito no texto; tira o filete e as linhas da nota e deixa a referencia no lugar. */
function extrairNotas(pagina: Pagina, blocos: Bloco[]): void {
  const corpo = estr.tamCorpo;
  const listas = blocos.flatMap((b) => (b.colunas ? [b.esq, b.dir] : [b.esq]));
  const ehNumero = (t: Trecho) => /^\d{1,2}$/.test(t.texto.trim());
  interface Candidata {
    lista: Linha[];
    ini: number;
    fim: number;
    marcador: string;
    regua: Imagem;
  }
  const candidatas: Candidata[] = [];
  for (const lista of listas) {
    for (let i = 0; i < lista.length; i++) {
      const l = lista[i];
      const m = l.trechos[0];
      if (!m || l.trechos.length < 2 || !ehNumero(m) || l.base < 0.5 * pagina.altura) continue;
      const sobe = m.vert === 'superscript' || (m.desloc ?? 0) > 0 || m.tam < 0.8 * corpo;
      if (!sobe || !l.trechos.slice(1).every((t) => t.tam <= 0.92 * corpo)) continue;
      const regua = pagina.imagens.find((r) => r.h <= 4 && r.w >= 20 && r.w < 0.6 * pagina.largura && r.y <= l.base && l.base - r.y < 40 && r.x <= l.x0 + 2);
      if (!regua) continue;
      let fim = i + 1;
      while (fim < lista.length && lista[fim].trechos.every((t) => t.tam <= 0.92 * corpo) && !(ehNumero(lista[fim].trechos[0]) && lista[fim].trechos[0].tam < 0.8 * corpo)) fim++;
      candidatas.push({ lista, ini: i, fim, marcador: m.texto.trim(), regua });
      i = fim - 1;
    }
  }
  if (!candidatas.length) return;
  const emNota = new Set<Linha>();
  for (const c of candidatas) for (const l of c.lista.slice(c.ini, c.fim)) emNota.add(l);
  const removidas = new Set<Linha>();
  for (const c of candidatas) {
    // a referencia no texto: o primeiro numero sobrescrito igual, fora de formulas e de outras notas
    let ref: Trecho | undefined;
    busca: for (const lista of listas) {
      for (const l of lista) {
        if (emNota.has(l) || l.trechos.some((t) => t.fonte === 'Cambria Math')) continue;
        for (const t of l.trechos) {
          if (t.nota === undefined && ehNumero(t) && t.texto.trim() === c.marcador && (t.vert === 'superscript' || (t.desloc ?? 0) > 1) && t.tam < 0.85 * corpo) {
            ref = t;
            break busca;
          }
        }
      }
    }
    if (!ref) continue; // sem referencia na pagina: a nota continua no corpo, para nao perder texto
    const id = estr.proximaNota++;
    ref.nota = id;
    const originais = c.lista.slice(c.ini, c.fim);
    const copia: Linha[] = originais.map((l, n) => {
      const trechos = n === 0 ? l.trechos.slice(1) : l.trechos;
      return { ...l, trechos, x0: Math.min(...trechos.map((t) => t.x)) };
    });
    const xa = Math.min(...copia.map((l) => l.x0));
    const xb = Math.max(...copia.map((l) => l.x1));
    const p = formatar(copia, xa, xb, copia.length > 1 ? copia[1].x0 : xa);
    // footnotes.xml nao tem arquivo de relacoes: o link da nota vira so o texto dele
    const runs = runsDoParagrafo(p, xa).xml.replace(/<w:hyperlink[^>]*>(.*?)<\/w:hyperlink>/gs, '$1');
    estr.notas.push({
      id,
      xml:
        `<w:footnote w:id="${id}"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr>` +
        '<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r>' +
        `<w:r><w:t xml:space="preserve"> </w:t></w:r>${runs}</w:p></w:footnote>`,
    });
    for (const l of originais) removidas.add(l);
    pagina.imagens = pagina.imagens.filter((r) => r !== c.regua);
  }
  for (const lista of listas) {
    for (let i = lista.length - 1; i >= 0; i--) if (removidas.has(lista[i])) lista.splice(i, 1);
  }
}

/** Estilos de paragrafo e de caractere da estrutura: so a identidade (nome, nivel de topico);
 * o aspecto continua o do PDF, que vai direto em cada paragrafo e em cada trecho. */
function estilosDaEstrutura(): string {
  const par = (id: string, nome: string, pPr = '', extra = '') =>
    `<w:style w:type="paragraph" w:styleId="${id}"><w:name w:val="${nome}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/>${extra}<w:qFormat/>` +
    (pPr ? `<w:pPr>${pPr}</w:pPr>` : '') +
    '</w:style>';
  const toc = (n: number) =>
    `<w:style w:type="paragraph" w:styleId="TOC${n}"><w:name w:val="toc ${n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="39"/>` +
    `<w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9350"/></w:tabs><w:ind w:left="${(n - 1) * 220}"/></w:pPr></w:style>`;
  return (
    par('Title', 'Title', '<w:keepNext/>') +
    par('Heading1', 'heading 1', '<w:keepNext/><w:keepLines/><w:outlineLvl w:val="0"/>') +
    par('Heading2', 'heading 2', '<w:keepNext/><w:keepLines/><w:outlineLvl w:val="1"/>') +
    par('Heading3', 'heading 3', '<w:keepNext/><w:keepLines/><w:outlineLvl w:val="2"/>') +
    par('TOCHeading', 'TOC Heading', '<w:keepNext/>') +
    toc(1) + toc(2) + toc(3) +
    '<w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="18"/><w:szCs w:val="18"/></w:rPr></w:style>' +
    '<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>'
  );
}

function numeracaoXml(W: string): string {
  return (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<w:numbering xmlns:w="${W}">` +
    estr.abstratos.map((a) => a.xml).join('') +
    estr.instancias
      .map(
        (n, i) =>
          `<w:num w:numId="${i + 1}"><w:abstractNumId w:val="${n.abstrato}"/>` +
          `<w:lvlOverride w:ilvl="0"><w:startOverride w:val="${n.inicio}"/></w:lvlOverride></w:num>`,
      )
      .join('') +
    '</w:numbering>'
  );
}

function notasXml(W: string, R: string): string {
  return (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<w:footnotes xmlns:w="${W}" xmlns:r="${R}">` +
    '<w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>' +
    '<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>' +
    estr.notas.map((n) => n.xml).join('') +
    '</w:footnotes>'
  );
}

/** Campo TOC do Word: abre no primeiro paragrafo do sumario e fecha num paragrafo vazio logo depois do ultimo. */
const CAMPO_TOC_ABRE =
  '<w:r><w:fldChar w:fldCharType="begin"/></w:r>' +
  '<w:r><w:instrText xml:space="preserve"> TOC \\o "1-3" \\h \\z \\u </w:instrText></w:r>' +
  '<w:r><w:fldChar w:fldCharType="separate"/></w:r>';
const CAMPO_TOC_FECHA =
  '<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="2" w:lineRule="exact"/></w:pPr><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>';

/** Estrutura do .tex do documento em conversao (opcoes.estrutura). */
let estruturaDoDocumento: EstruturaTex | undefined;

// ---------- montagem do documento ----------

const ESPACO_MAXIMO = 79; // pt: o Word aceita ate 1584 twips de "espaco antes"

/** Margens esquerda e direita do documento em montagem (folga das linhas unicas). */
let folgaMargens = { esquerda: 0, direita: 0 };

let proximaCaixa = 100000;

/** Paragrafo dentro de uma caixa de texto presa a pagina, no lugar exato do PDF. Com
 * `topoParagrafo`, a altura conta do topo do paragrafo que segura a caixa (que fica no PDF
 * nessa altura): a caixa acompanha o texto se o Word deslocar o paragrafo. */
function caixaDeTexto(p: Paragrafo, xa: number, topoParagrafo?: number): string {
  const x0 = Math.min(...p.linhas.map((l) => l.x0));
  const x1 = Math.max(...p.linhas.map((l) => l.x1));
  const varias = p.linhas.length > 1;
  const largura = varias ? x1 - x0 + 1 : x1 - x0 + 2 * p.linhas[0].tam;
  const y = p.linhas[0].base - p.entrelinha * 0.8;
  const altura = p.linhas[p.linhas.length - 1].base - p.linhas[0].base + p.entrelinha;
  const dentro: Paragrafo = { ...p, antes: 0, esquerda: Math.max(0, p.esquerda + xa - x0), tabDireita: undefined, excesso: 0 };
  if (dentro.alinhamento === 'center' || dentro.alinhamento === 'right') dentro.alinhamento = 'left';
  const id = proximaCaixa++;
  const xml = paragrafoXml(dentro, x0, false).replace(/<w:ind [^>]*\/>/, `<w:ind w:left="${tw(dentro.esquerda)}" w:right="0" w:firstLine="${tw(Math.max(0, dentro.primeira))}"/>`);
  return (
    `<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="${id}" ` +
    `behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/>` +
    `<wp:positionH relativeFrom="page"><wp:posOffset>${emu(x0)}</wp:posOffset></wp:positionH>` +
    (topoParagrafo === undefined
      ? `<wp:positionV relativeFrom="page"><wp:posOffset>${emu(y)}</wp:posOffset></wp:positionV>`
      : `<wp:positionV relativeFrom="paragraph"><wp:posOffset>${emu(y - topoParagrafo)}</wp:posOffset></wp:positionV>`) +
    `<wp:extent cx="${emu(largura)}" cy="${emu(altura)}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/>` +
    `<wp:docPr id="${id}" name="Texto ${id}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">` +
    `<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp>` +
    `<wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${emu(largura)}" cy="${emu(altura)}"/></a:xfrm>` +
    `<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></wps:spPr>` +
    `<wps:txbx><w:txbxContent>${xml}</w:txbxContent></wps:txbx>` +
    `<wps:bodyPr rot="0" vert="horz" wrap="${varias ? 'square' : 'none'}" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t" anchorCtr="0"><a:noAutofit/></wps:bodyPr>` +
    `</wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>`
  );
}

/** Texto girado: caixa de texto presa a pagina com a mesma rotacao, no lugar do PDF. */
function caixaGirada(g: Girado): string {
  const h = ALTURA_CAIXA * g.tam;
  const w = g.w + 1; // folga de 1 pt; a caixa nao quebra linha
  const c = centroGirado({ ...g, w });
  const x = c.x - w / 2;
  const y = c.y - h / 2;
  const trecho: Trecho = { texto: g.texto, x: 0, x1: w, base: 0.8 * h, tam: g.tam, fonte: g.fonte, negrito: g.negrito, italico: g.italico, cor: g.cor };
  const linha: Linha = { trechos: [trecho], x0: 0, x1: w, base: 0.8 * h, tam: g.tam, tabulada: false };
  const par: Paragrafo = { ...formatar([linha], 0, w, 0), antes: 0, excesso: 0, alinhamento: 'left', esquerda: 0, primeira: 0 };
  const xml = paragrafoXml(par, 0, false).replace(/<w:ind [^>]*\/>/, '<w:ind w:left="0" w:right="0" w:firstLine="0"/>');
  const id = proximaCaixa++;
  return (
    `<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="${id}" ` +
    `behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/>` +
    `<wp:positionH relativeFrom="page"><wp:posOffset>${emu(x)}</wp:posOffset></wp:positionH>` +
    `<wp:positionV relativeFrom="page"><wp:posOffset>${emu(y)}</wp:posOffset></wp:positionV>` +
    `<wp:extent cx="${emu(w)}" cy="${emu(h)}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/>` +
    `<wp:docPr id="${id}" name="Texto girado ${id}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">` +
    `<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp>` +
    `<wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm rot="${Math.round(g.graus * 60000)}"><a:off x="0" y="0"/><a:ext cx="${emu(w)}" cy="${emu(h)}"/></a:xfrm>` +
    `<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></wps:spPr>` +
    `<wps:txbx><w:txbxContent>${xml}</w:txbxContent></wps:txbx>` +
    `<wps:bodyPr rot="0" vert="horz" wrap="none" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t" anchorCtr="0"><a:noAutofit/></wps:bodyPr>` +
    `</wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>`
  );
}

/** Paragrafo vazio com a altura exata do vao. */
function espacador(altura: number, quebraPagina: boolean, extra: string): string {
  return (
    `<w:p><w:pPr>${quebraPagina ? '<w:pageBreakBefore/>' : ''}<w:widowControl w:val="0"/>` +
    `<w:spacing w:before="0" w:after="0" w:line="${tw(altura)}" w:lineRule="exact"/></w:pPr>${extra}</w:p>`
  );
}

interface Margens {
  esquerda: number;
  direita: number;
  topo: number;
  base: number;
  /** distancia da borda da pagina ate o cabecalho e ate o rodape */
  cabecalho?: number;
  rodape?: number;
}

// ---------- cabecalho e rodape ----------

/** Fracao da altura da pagina, no topo e na base, onde uma linha pode ser cabecalho ou rodape. */
const BANDA_FAIXA = 0.12;

/** Cabecalho ou rodape achado no PDF: uma linha que repete nas paginas, com o numero da pagina. */
interface Faixa {
  /** linha da primeira pagina em que aparece, usada como modelo */
  modelo: Linha;
  /** altura da pagina do modelo */
  altura: number;
  largura: number;
  /** paginas (0 a n-1) em que a faixa aparece no PDF */
  paginas: Set<number>;
}

interface Faixas {
  cabecalho?: Faixa;
  rodape?: Faixa;
}

/** Texto da linha com o numero da pagina trocado por # e o total de paginas por $, para
 * comparar a mesma faixa entre paginas ("3" na pagina 3 e "4" na pagina 4 viram "#"). */
function chaveDaFaixa(l: Linha, pagina: number, total: number): string {
  let achouPagina = false;
  return l.trechos
    .map((t) => t.texto)
    .join(' ')
    .replace(/\d+/g, (d) => {
      if (!achouPagina && Number(d) === pagina) {
        achouPagina = true;
        return '#';
      }
      return Number(d) === total && total > 1 ? '$' : d;
    })
    .replace(/\s+/g, ' ')
    .trim();
}

/** Acha o cabecalho e o rodape que repetem e os tira das linhas das paginas: eles voltam como as
 * partes header e footer do Word, com o numero da pagina como campo. Uma linha por faixa; linhas
 * de faixa que nao repetem ficam no corpo. */
function separarFaixas(paginas: Pagina[]): Faixas {
  const n = paginas.length;
  const saida: Faixas = {};
  for (const topo of [true, false]) {
    const candidatas = paginas.map((p, i) => {
      const dentro = p.linhas.filter((l) => (topo ? l.base < BANDA_FAIXA * p.altura : l.base > (1 - BANDA_FAIXA) * p.altura));
      if (!dentro.length) return null;
      const linha = dentro.reduce((a, b) => ((topo ? b.base < a.base : b.base > a.base) ? b : a));
      return { linha, chave: chaveDaFaixa(linha, i + 1, n) };
    });
    const contagem = new Map<string, number>();
    for (const c of candidatas) if (c) contagem.set(c.chave, (contagem.get(c.chave) ?? 0) + 1);
    let melhor = '';
    let maior = 0;
    for (const [chave, k] of contagem) {
      // numero de pagina puro pode ocorrer numa pagina so; com texto junto (nota de rodape "1 Texto") so conta se repete
      const variavel = chave.includes('#') && /^[#$\s/.-]+$/.test(chave);
      const repete = k >= 2 && k >= n / 2;
      if ((variavel && k >= Math.min(2, n)) || repete) {
        if (k > maior) [melhor, maior] = [chave, k];
      }
    }
    if (!maior || !melhor) continue;
    // o Word poe a faixa em todas as paginas (a 1a pode ser diferente: capa sem numero); se o PDF a
    // omite em outra pagina, ela fica no corpo para nao aparecer onde o PDF nao a mostra
    const cobertura = candidatas.map((c) => c?.chave === melhor);
    if (cobertura.some((ok, i) => i > 0 && !ok)) continue;
    const achadas = new Set<number>();
    let modelo: Linha | undefined;
    let altura = 0;
    let largura = 0;
    candidatas.forEach((c, i) => {
      if (!c || c.chave !== melhor) return;
      achadas.add(i);
      paginas[i].linhas = paginas[i].linhas.filter((l) => l !== c.linha);
      if (!modelo) {
        modelo = c.linha;
        altura = paginas[i].altura;
        largura = paginas[i].largura;
      }
    });
    if (modelo) saida[topo ? 'cabecalho' : 'rodape'] = { modelo, altura, largura, paginas: achadas };
  }
  return saida;
}

/** Paragrafo do cabecalho ou do rodape: o texto da linha modelo, com o numero da pagina como
 * campo PAGE e o total como NUMPAGES, e os grupos de texto separados por tabulacao. */
function paragrafoDaFaixa(f: Faixa, m: Margens, paginaDoModelo: number, total: number): string {
  const l = f.modelo;
  const tam = Math.max(...l.trechos.map((t) => t.tam));
  // grupos: trechos separados por um vao grande ficam em colunas de tabulacao
  const grupos: Trecho[][] = [];
  for (const t of l.trechos) {
    const g = grupos[grupos.length - 1];
    if (g && t.x - g[g.length - 1].x1 < 2 * tam) g.push(t);
    else grupos.push([t]);
  }
  let achouPagina = false;
  const runsDe = (t: Trecho): string =>
    t.texto
      .split(/(\d+)/)
      .filter((s) => s !== '')
      .map((s) => {
        if (!/^\d+$/.test(s)) return run(t, s);
        if (!achouPagina && Number(s) === paginaDoModelo) {
          achouPagina = true;
          return `<w:fldSimple w:instr=" PAGE ">${run(t, s)}</w:fldSimple>`;
        }
        if (Number(s) === total && total > 1) return `<w:fldSimple w:instr=" NUMPAGES ">${run(t, s)}</w:fldSimple>`;
        return run(t, s);
      })
      .join('');
  const esq = m.esquerda;
  const dirX = f.largura - m.direita;
  const paradas: string[] = [];
  let corpo = '';
  let alinhamento = 'left';
  let recuo = Math.max(0, grupos[0][0].x - esq);
  if (grupos.length === 1) {
    const centro = (l.x0 + l.x1) / 2;
    if (Math.abs(centro - f.largura / 2) < 0.04 * f.largura) {
      alinhamento = 'center';
      recuo = 0;
    } else if (f.largura - l.x1 < 0.6 * l.x0 && l.x0 > 0.4 * f.largura) {
      alinhamento = 'right';
      recuo = 0;
    }
    corpo = grupos[0].map(runsDe).join('');
  } else {
    grupos.forEach((g, k) => {
      if (k > 0) {
        const ultimo = g[g.length - 1];
        paradas.push(
          ultimo.x1 >= dirX - 4 ? `<w:tab w:val="right" w:pos="${tw(Math.max(0, dirX - esq))}"/>` : `<w:tab w:val="left" w:pos="${tw(Math.max(0, g[0].x - esq))}"/>`,
        );
        corpo += '<w:r><w:tab/></w:r>';
      }
      corpo += g.map(runsDe).join('');
    });
  }
  return (
    `<w:p><w:pPr>${paradas.length ? `<w:tabs>${paradas.join('')}</w:tabs>` : ''}` +
    `<w:spacing w:before="0" w:after="0" w:line="${tw(tam * 1.2)}" w:lineRule="atLeast"/>` +
    `<w:ind w:left="${tw(recuo)}" w:right="0"/><w:jc w:val="${alinhamento}"/></w:pPr>${corpo}</w:p>`
  );
}

/** Documento das partes header e footer, e a distancia da borda da pagina ate a linha. */
let faixasDoDocumento: {
  cabecalho?: string;
  rodape?: string;
  /** a pagina 1 do PDF nao tem a faixa (capa sem numero): o Word usa a primeira pagina diferente */
  capaSemFaixa: boolean;
  capaSemCabecalho: boolean;
  capaSemRodape: boolean;
  primeiraSecao: boolean;
} = { capaSemFaixa: false, capaSemCabecalho: false, capaSemRodape: false, primeiraSecao: true };

function sectPr(p: Pagina, m: Margens, colunas?: { espaco: number; larguras: [number, number] }, continua = false): string {
  const cols = colunas
    ? `<w:cols w:num="2" w:space="${tw(colunas.espaco)}" w:equalWidth="0">` +
      `<w:col w:w="${tw(colunas.larguras[0])}" w:space="${tw(colunas.espaco)}"/><w:col w:w="${tw(colunas.larguras[1])}"/></w:cols>`
    : '<w:cols w:space="720"/>';
  const f = faixasDoDocumento;
  const titulo = f.primeiraSecao && f.capaSemFaixa;
  f.primeiraSecao = false;
  return (
    '<w:sectPr>' +
    (f.cabecalho
      ? '<w:headerReference w:type="default" r:id="rIdCab"/>' +
        (f.capaSemFaixa ? `<w:headerReference w:type="first" r:id="${f.capaSemCabecalho ? 'rIdCabVazio' : 'rIdCab'}"/>` : '')
      : '') +
    (f.rodape
      ? '<w:footerReference w:type="default" r:id="rIdRod"/>' +
        (f.capaSemFaixa ? `<w:footerReference w:type="first" r:id="${f.capaSemRodape ? 'rIdRodVazio' : 'rIdRod'}"/>` : '')
      : '') +
    (continua ? '<w:type w:val="continuous"/>' : '') +
    `<w:pgSz w:w="${tw(p.largura)}" w:h="${tw(p.altura)}"${p.largura > p.altura ? ' w:orient="landscape"' : ''}/>` +
    `<w:pgMar w:top="${tw(m.topo)}" w:right="${tw(m.direita)}" w:bottom="${tw(m.base)}" w:left="${tw(m.esquerda)}" w:header="${tw(m.cabecalho ?? 0)}" w:footer="${tw(m.rodape ?? 0)}" w:gutter="0"/>` +
    cols +
    (titulo ? '<w:titlePg/>' : '') +
    '</w:sectPr>'
  );
}

function corpoDoDocumento(paginas: Pagina[]): string {
  // cabecalho e rodape que repetem saem do corpo e vao para as partes header e footer
  const faixas = separarFaixas(paginas);
  estr = novoEstadoEstrutura(estruturaDoDocumento);
  estr.tamCorpo = tamanhoDoCorpo(paginas);
  // margens comuns: a esquerda e a direita do texto mais largo, topo e base do bloco de texto
  // (o espaco de cima vem do "espaco antes" de cada paragrafo)
  const todas = paginas.flatMap((p) => p.linhas);
  const largura = paginas[0]?.largura ?? 595;
  // pelas linhas longas (corpo do texto): um rotulo solto na borda nao muda a margem
  const longas = todas.filter((l) => l.x1 - l.x0 > 0.3 * largura);
  const base = longas.length >= 3 ? longas : todas;
  const xs0 = base.map((l) => l.x0).sort((a, b) => a - b);
  const xs1 = base.map((l) => l.x1).sort((a, b) => a - b);
  const pct = (v: number[], q: number) => v[Math.min(v.length - 1, Math.max(0, Math.round(q * (v.length - 1))))];
  const esquerda = todas.length ? Math.max(10, pct(xs0, 0.05) - 0.5) : 72;
  const direita = todas.length ? Math.max(10, largura - pct(xs1, 0.95) - 2) : 72;
  // topo e base reais do bloco de texto: a margem de cima fica acima da primeira linha de todas as
  // paginas e a de baixo abaixo da ultima, menos uma folga de uma linha e meia (18 pt) para que uma
  // fonte um pouco mais alta no Word nao empurre a ultima linha para a pagina seguinte
  const comTexto = paginas.filter((p) => p.linhas.length);
  const FOLGA_BASE = 18;
  const margemTopo = comTexto.length ? Math.max(0, Math.floor(Math.min(...comTexto.map((p) => Math.min(...p.linhas.map((l) => l.base - 1.2 * l.tam)))))) : 0;
  const margemBase = comTexto.length
    ? Math.max(0, Math.floor(Math.min(...comTexto.map((p) => p.altura - Math.max(...p.linhas.map((l) => l.base + 0.3 * l.tam))))) - FOLGA_BASE)
    : 0;
  const m: Margens = { esquerda, direita, topo: margemTopo, base: margemBase };
  faixasDoDocumento = { capaSemFaixa: false, capaSemCabecalho: false, capaSemRodape: false, primeiraSecao: true };
  if (faixas.cabecalho) {
    const f = faixas.cabecalho;
    m.cabecalho = Math.max(0, f.modelo.base - f.modelo.tam);
    faixasDoDocumento.cabecalho = paragrafoDaFaixa(f, m, Math.min(...f.paginas) + 1, paginas.length);
    faixasDoDocumento.capaSemCabecalho = !f.paginas.has(0);
  }
  if (faixas.rodape) {
    const f = faixas.rodape;
    m.rodape = Math.max(0, f.altura - f.modelo.base - 0.25 * f.modelo.tam);
    faixasDoDocumento.rodape = paragrafoDaFaixa(f, m, Math.min(...f.paginas) + 1, paginas.length);
    faixasDoDocumento.capaSemRodape = !f.paginas.has(0);
  }
  faixasDoDocumento.capaSemFaixa = faixasDoDocumento.capaSemCabecalho || faixasDoDocumento.capaSemRodape;
  folgaMargens = { esquerda: Math.max(0, esquerda - 2), direita: Math.max(0, direita - 2) };

  const saida: string[] = [];
  let tocAberto = false;
  paginas.forEach((pagina, np) => {
    // texto dentro de figura, caixa colorida ou painel (rotulos de grafico, blocos do
    // Beamer): vai em caixas de texto no lugar exato, fora do fluxo da pagina
    const areaPagina = pagina.largura * pagina.altura;
    const regioes = pagina.imagens.filter((r) => r.w * r.h < 0.6 * areaPagina);
    const regiaoDa = (l: Linha) =>
      regioes.find((r) => l.x0 >= r.x - 1 && l.x1 <= r.x + r.w + 1 && l.base - l.tam * 0.7 >= r.y - 1 && l.base <= r.y + r.h + 1);
    const naFigura = new Map<Imagem, Linha[]>();
    pagina.linhas = pagina.linhas.filter((l) => {
      const r = regiaoDa(l);
      if (!r) return true;
      naFigura.set(r, [...(naFigura.get(r) ?? []), l]);
      return false;
    });
    const blocos = montarBlocos(pagina);
    extrairNotas(pagina, blocos);
    // paragrafos de cada bloco, com as equacoes destacadas ja trocadas pelas do Word
    areasEquacao = [];
    const paragrafosDoBloco = blocos.map((b) =>
      b.colunas
        ? { esq: trocarEquacoes(montarParagrafos(b.esq, b.xa, b.xb), b.xa, b.xb), dir: trocarEquacoes(montarParagrafos(b.dir, b.xc, b.xd), b.xc, b.xd) }
        : { esq: trocarEquacoes(montarParagrafos(b.esq, m.esquerda, largura - m.direita), m.esquerda, largura - m.direita), dir: [] },
    );
    for (const g of paragrafosDoBloco) {
      classificarParagrafos(g.esq);
      classificarParagrafos(g.dir);
    }
    // barras de fracao e de raiz do PDF: a equacao do Word desenha as suas
    const imagensDaPagina = pagina.imagens.filter(
      (r) => !areasEquacao.some((a) => r.x >= a.x0 - 3 && r.x + r.w <= a.x1 + 3 && r.y >= a.y0 - 3 && r.y + r.h <= a.y1 + 3),
    );
    let cursor = m.topo; // onde a ultima linha posta termina, em pt a partir do topo
    let primeiroDaPagina = true;
    pagina.imagens = imagensDaPagina;
    let imagensPendentes = imagensDaPagina.map(ancora).join('');
    let caixasPendentes = '';
    for (const [r, linhas] of naFigura) {
      // uma caixa por linha: cada rotulo fica exatamente onde estava
      for (const l of linhas) caixasPendentes += caixaDeTexto(formatar([l], l.x0, l.x1, l.x0), l.x0);
    }
    for (const g of pagina.girados) caixasPendentes += caixaGirada(g);
    // destinos de link da pagina: o marcador entra no primeiro paragrafo que passa da altura dele
    const destinosAbertos = [...pagina.destinos].sort((a, b) => a.y - b.y);
    const marcas = (ate: number): string => {
      let xml = '';
      while (destinosAbertos.length && destinosAbertos[0].y <= ate) {
        const m = marcadorDoDestino(destinosAbertos.shift()!.nome);
        xml += `<w:bookmarkStart w:id="${m.id}" w:name="${m.nome}"/><w:bookmarkEnd w:id="${m.id}"/>`;
      }
      return xml;
    };
    const emitir = (p: Paragrafo, xa: number, prefixo = '') => {
      // o campo TOC fecha num paragrafo vazio logo depois da ultima entrada do sumario
      if (tocAberto && !p.toc) {
        saida.push(CAMPO_TOC_FECHA);
        tocAberto = false;
      }
      if (p.tocInicio) tocAberto = true;
      p.marcas = marcas(p.linhas[p.linhas.length - 1].base + 0.5 * p.linhas[p.linhas.length - 1].tam);
      if (p.equacao) {
        // o paragrafo da equacao so pode ter a equacao: com outro conteudo o Word a desenha
        // pequena, como formula no meio do texto. Figuras, caixas e o numero "(1)" vao num
        // paragrafo vazio logo antes, que tambem faz o espaco de cima.
        const topo = p.linhas[0].base - p.entrelinha * 0.8;
        const numeros = '';
        saida.push(espacador(Math.max(0.1, topo - cursor), primeiroDaPagina && np > 0, prefixo + (primeiroDaPagina ? imagensPendentes : '') + caixasPendentes + numeros));
        p.antes = 0;
        saida.push(paragrafoXml(p, xa, false));
        cursor = Math.max(cursor, p.linhas[0].base + p.entrelinha * 0.2);
        if (primeiroDaPagina) imagensPendentes = '';
        caixasPendentes = '';
        primeiroDaPagina = false;
        return;
      }
      if (p.aglomerado) {
        // formula com fracao, raiz, indices empilhados: cada linha numa caixa no lugar
        // exato e um paragrafo vazio reserva a altura do conjunto
        const fundo = Math.max(...p.linhas.map((l) => l.base + 0.3 * l.tam));
        const caixas = p.linhas.map((l) => caixaDeTexto(formatar([l], l.x0, l.x1, l.x0), l.x0)).join('');
        const altura = Math.max(1, fundo - cursor);
        saida.push(espacador(altura, primeiroDaPagina && np > 0, prefixo + (primeiroDaPagina ? imagensPendentes : '') + caixasPendentes + caixas + (p.marcas ?? '')));
        cursor = Math.max(cursor, fundo);
        if (primeiroDaPagina) imagensPendentes = '';
        caixasPendentes = '';
        primeiroDaPagina = false;
        return;
      }
      // espaco antes: da linha de base anterior ate o topo desta linha
      let topo = p.linhas[0].base - p.entrelinha * 0.8;
      if (topo < cursor && p.linhas.length === 1) {
        // linha unica encostada na anterior: a entrelinha encolhe ate caber, sem empurrar
        const cabe = (p.linhas[0].base - cursor) / 0.8;
        if (cabe >= 0.9 * p.linhas[0].tam) {
          p.entrelinha = cabe;
          topo = cursor;
        }
      }
      if (!primeiroDaPagina && !prefixo && topo - cursor < -0.5) {
        // texto sobreposto ao anterior (rotulos de grafico, notas na margem): caixa de
        // texto no lugar exato, fora do fluxo, para nao empurrar o resto da pagina
        caixasPendentes += caixaDeTexto(p, xa);
        return;
      }
      p.antes = Math.max(0, topo - cursor);
      cursor = p.linhas[p.linhas.length - 1].base + p.entrelinha * 0.2;
      if (p.antes > ESPACO_MAXIMO || (primeiroDaPagina && p.antes > 0.5)) {
        // o Word limita o "espaco antes" a 79,2 pt e o ignora no topo da pagina depois de
        // uma quebra: um paragrafo vazio com a altura do vao resolve os dois casos
        saida.push(espacador(p.antes, primeiroDaPagina && np > 0, prefixo + (primeiroDaPagina ? imagensPendentes : '')));
        p.antes = 0;
        prefixo = '';
        if (primeiroDaPagina) imagensPendentes = '';
        primeiroDaPagina = false;
      }
      saida.push(paragrafoXml(p, xa, primeiroDaPagina && np > 0, prefixo + (primeiroDaPagina ? imagensPendentes : '') + caixasPendentes));
      if (primeiroDaPagina) imagensPendentes = '';
      caixasPendentes = '';
      primeiroDaPagina = false;
    };
    if (!blocos.some((b) => b.esq.length || b.dir.length)) {
      // pagina so com figura
      saida.push(
        `<w:p><w:pPr>${np > 0 ? '<w:pageBreakBefore/>' : ''}<w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/></w:pPr>${imagensPendentes}${caixasPendentes}${marcas(Infinity)}</w:p>`,
      );
      return;
    }
    for (const [nb, b] of blocos.entries()) {
      if (!b.colunas) {
        for (const p of paragrafosDoBloco[nb].esq) emitir(p, m.esquerda);
        continue;
      }
      // fecha a secao de uma coluna que vinha antes
      if (saida.length) {
        saida.push(`<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="2" w:lineRule="exact"/>${sectPr(pagina, m, undefined, true)}</w:pPr></w:p>`);
      }
      const esqP = paragrafosDoBloco[nb].esq;
      const dirP = paragrafosDoBloco[nb].dir;
      const inicio = cursor;
      for (const p of esqP) emitir(p, b.xa);
      const fimEsq = cursor;
      cursor = inicio;
      // a coluna da direita comeca com uma quebra de coluna no primeiro paragrafo dela
      dirP.forEach((p, i) => emitir(p, b.xc, i === 0 ? '<w:r><w:br w:type="column"/></w:r>' : ''));
      cursor = Math.max(cursor, fimEsq);
      const larguras: [number, number] = [b.xb - m.esquerda, largura - m.direita - b.xc];
      const espaco = Math.max(6, b.xc - b.xb);
      saida.push(
        `<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="2" w:lineRule="exact"/>${sectPr(pagina, m, { espaco, larguras }, true)}</w:pPr></w:p>`,
      );
    }
    // caixas e marcadores que sobraram no fim da pagina: um paragrafo de altura quase zero os segura
    const sobras = caixasPendentes + marcas(Infinity);
    if (sobras) {
      saida.push(`<w:p><w:pPr><w:widowControl w:val="0"/><w:spacing w:before="0" w:after="0" w:line="2" w:lineRule="exact"/></w:pPr>${sobras}</w:p>`);
    }
  });
  if (tocAberto) saida.push(CAMPO_TOC_FECHA);
  const ultima = paginas[paginas.length - 1] ?? { largura: 595, altura: 842, linhas: [], imagens: [], girados: [], destinos: [] };
  // pagina que termina em duas colunas: a ultima secao fica vazia e precisa ser continua,
  // senao o Word abre uma pagina em branco no fim
  const terminaEmColunas = saida[saida.length - 1]?.includes('<w:sectPr>') ?? false;
  const final = sectPr(ultima, m, undefined, true);
  return saida.join('') + (terminaEmColunas ? final : final.replace('<w:type w:val="continuous"/>', ''));
}

// ---------- fontes embutidas ----------

function ofuscar(fonte: Uint8Array, chave: string): Uint8Array {
  // Fonte embutida do Word (ECMA-376, 17.8.1): os 32 primeiros bytes passam por XOR com o GUID
  const hexa = chave.replace(/[{}-]/g, '');
  const k: number[] = [];
  for (let i = 0; i < 32; i += 2) k.push(parseInt(hexa.slice(i, i + 2), 16));
  k.reverse();
  const saida = new Uint8Array(fonte);
  for (let i = 0; i < 32 && i < saida.length; i++) saida[i] ^= k[i % 16];
  return saida;
}

function guid(): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  const h = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('').toUpperCase();
  return `{${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}}`;
}

// ---------- zip ----------

const TABELA_CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(d: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < d.length; i++) c = TABELA_CRC[(c ^ d[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** Zip sem compressao (o .docx aceita), com [Content_Types].xml primeiro. */
export function zipar(arquivos: [string, Uint8Array][]): Uint8Array {
  const enc = new TextEncoder();
  const locais: Uint8Array[] = [];
  const centrais: Uint8Array[] = [];
  let pos = 0;
  for (const [nome, dados] of arquivos) {
    const n = enc.encode(nome);
    const crc = crc32(dados);
    const loc = new Uint8Array(30 + n.length);
    const v = new DataView(loc.buffer);
    v.setUint32(0, 0x04034b50, true);
    v.setUint16(4, 20, true);
    v.setUint16(6, 0x0800, true); // nomes em UTF-8
    v.setUint16(8, 0, true);
    v.setUint32(14, crc, true);
    v.setUint32(18, dados.length, true);
    v.setUint32(22, dados.length, true);
    v.setUint16(26, n.length, true);
    loc.set(n, 30);
    const cen = new Uint8Array(46 + n.length);
    const c = new DataView(cen.buffer);
    c.setUint32(0, 0x02014b50, true);
    c.setUint16(4, 20, true);
    c.setUint16(6, 20, true);
    c.setUint16(8, 0x0800, true);
    c.setUint32(16, crc, true);
    c.setUint32(20, dados.length, true);
    c.setUint32(24, dados.length, true);
    c.setUint16(28, n.length, true);
    c.setUint32(42, pos, true);
    cen.set(n, 46);
    locais.push(loc, dados);
    centrais.push(cen);
    pos += loc.length + dados.length;
  }
  const tamCentral = centrais.reduce((s, x) => s + x.length, 0);
  const fim = new Uint8Array(22);
  const f = new DataView(fim.buffer);
  f.setUint32(0, 0x06054b50, true);
  f.setUint16(8, arquivos.length, true);
  f.setUint16(10, arquivos.length, true);
  f.setUint32(12, tamCentral, true);
  f.setUint32(16, pos, true);
  const total = new Uint8Array(pos + tamCentral + 22);
  let o = 0;
  for (const p of [...locais, ...centrais, fim]) {
    total.set(p, o);
    o += p.length;
  }
  return total;
}

// ---------- entrada ----------

export interface OpcoesWord {
  /** idioma da hifenizacao, ex.: pt-BR */
  idioma: string;
  /** devolve o .ttf de uma familia embutida ("LatinModernRoman-bold.ttf") ou null */
  fonte: (arquivo: string) => Promise<Uint8Array | null>;
  /** andamento: pagina atual e total */
  progresso?: (pagina: number, total: number) => void;
  /** equacoes destacadas do .tex (equacoes-word.ts), na ordem do texto */
  equacoes?: EquacaoTex[];
  /** dicionario pt-BR do app: recebe palavras (minusculas) e devolve as que existem. Decide se
   * um hifen de fim de linha e so quebra ("desen-volvimento") ou faz parte da palavra
   * ("guarda-chuva"). Sem ele valem o proprio texto do PDF e uma lista de compostas. */
  dicionario?: (palavras: string[]) => Promise<string[]>;
  /** titulos, notas e sumario do .tex (estruturaDoTex): confere e nomeia o que o PDF mostra */
  estrutura?: EstruturaTex;
}

export async function pdfParaWord(pdfjs: Pdfjs, pdf: Uint8Array, opcoes: OpcoesWord): Promise<Uint8Array> {
  const doc = await pdfjs.getDocument({ data: pdf.slice(), isEvalSupported: false }).promise;
  const paginas: Pagina[] = [];
  let contador = 1;
  destinosDoDocumento = new Map();
  urlsDasLigacoes = [];
  marcadoresDestino = new Map();
  try {
    for (let i = 1; i <= doc.numPages; i++) {
      opcoes.progresso?.(i, doc.numPages);
      const pagina = await doc.getPage(i);
      paginas.push(await lerPagina(pdfjs, doc, pagina, () => contador++));
      pagina.cleanup();
    }
  } finally {
    await doc.destroy();
  }

  const enc = new TextEncoder();
  const W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
  const R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
  filaEquacoes = [...(opcoes.equacoes ?? [])];
  for (const [nome, d] of destinosDoDocumento) paginas[d.pagina]?.destinos.push({ nome, y: d.y });
  contextoHifen = await montarContextoHifen(paginas, opcoes.dicionario);
  estruturaDoDocumento = opcoes.estrutura;
  const corpo = corpoDoDocumento(paginas);
  const faixasXml = faixasDoDocumento;
  const temNotas = estr.notas.length > 0;
  const temListas = estr.instancias.length > 0;
  const documento =
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<w:document xmlns:w="${W}" xmlns:r="${R}" ` +
    'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" ' +
    'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" ' +
    'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" ' +
    'xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" ' +
    'xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">' +
    `<w:body>${corpo}</w:body></w:document>`;

  // fonte mais usada vira a padrao (texto novo digitado no Word)
  const contagem = new Map<string, number>();
  for (const p of paginas) for (const l of p.linhas) for (const t of l.trechos) {
    if (t.fonte !== 'Cambria Math') contagem.set(t.fonte, (contagem.get(t.fonte) ?? 0) + t.texto.length);
  }
  let principal = 'Times New Roman';
  let maior = -1;
  for (const [f, n] of contagem) if (n > maior) [maior, principal] = [n, f];
  const idioma = esc(opcoes.idioma);
  const estilos =
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<w:styles xmlns:w="${W}"><w:docDefaults><w:rPrDefault><w:rPr>` +
    `<w:rFonts w:ascii="${esc(principal)}" w:hAnsi="${esc(principal)}" w:cs="${esc(principal)}" w:eastAsia="${esc(principal)}"/>` +
    `<w:sz w:val="24"/><w:szCs w:val="24"/><w:lang w:val="${idioma}" w:eastAsia="${idioma}" w:bidi="ar-SA"/>` +
    '</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:widowControl w:val="0"/><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>' +
    '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>' +
    estilosDaEstrutura() +
    '</w:styles>';
  const config =
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<w:settings xmlns:w="${W}"><w:embedTrueTypeFonts/><w:autoHyphenation/><w:consecutiveHyphenLimit w:val="0"/>` +
    '<w:defaultTabStop w:val="708"/>' +
    (temNotas ? '<w:footnotePr><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr>' : '') +
    '<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>';

  const arquivos: [string, Uint8Array][] = [];
  const relsDoc: string[] = [
    '<Relationship Id="rIdEstilos" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>',
    '<Relationship Id="rIdConfig" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>',
    '<Relationship Id="rIdFontes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/>',
  ];
  const REL = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
  const partesFaixa: [string, string][] = [];
  const faixaXml = (tag: string, corpoXml: string) =>
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' + `<w:${tag} xmlns:w="${W}" xmlns:r="${R}">${corpoXml}</w:${tag}>`;
  if (faixasXml.cabecalho) {
    partesFaixa.push(['header1', faixaXml('hdr', faixasXml.cabecalho)]);
    relsDoc.push(`<Relationship Id="rIdCab" Type="${REL}/header" Target="header1.xml"/>`);
    if (faixasXml.capaSemFaixa && faixasXml.capaSemCabecalho) {
      partesFaixa.push(['header2', faixaXml('hdr', '<w:p/>')]);
      relsDoc.push(`<Relationship Id="rIdCabVazio" Type="${REL}/header" Target="header2.xml"/>`);
    }
  }
  if (faixasXml.rodape) {
    partesFaixa.push(['footer1', faixaXml('ftr', faixasXml.rodape)]);
    relsDoc.push(`<Relationship Id="rIdRod" Type="${REL}/footer" Target="footer1.xml"/>`);
    if (faixasXml.capaSemFaixa && faixasXml.capaSemRodape) {
      partesFaixa.push(['footer2', faixaXml('ftr', '<w:p/>')]);
      relsDoc.push(`<Relationship Id="rIdRodVazio" Type="${REL}/footer" Target="footer2.xml"/>`);
    }
  }
  for (const [nome, xml] of partesFaixa) arquivos.push([`word/${nome}.xml`, enc.encode(xml)]);
  if (temNotas) {
    arquivos.push(['word/footnotes.xml', enc.encode(notasXml(W, R))]);
    relsDoc.push(`<Relationship Id="rIdNotas" Type="${REL}/footnotes" Target="footnotes.xml"/>`);
  }
  if (temListas) {
    arquivos.push(['word/numbering.xml', enc.encode(numeracaoXml(W))]);
    relsDoc.push(`<Relationship Id="rIdNumeracao" Type="${REL}/numbering" Target="numbering.xml"/>`);
  }
  urlsDasLigacoes.forEach((url, i) => {
    relsDoc.push(`<Relationship Id="rIdLink${i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="${esc(url)}" TargetMode="External"/>`);
  });
  for (const p of paginas) for (const img of p.imagens) {
    relsDoc.push(`<Relationship Id="rIdImg${img.id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/figura${img.id}.png"/>`);
    arquivos.push([`word/media/figura${img.id}.png`, img.png]);
  }

  // fontes embutidas: so as familias Latin Modern que aparecem no texto
  const usadas = new Set(paginas.flatMap((p) => p.linhas.flatMap((l) => l.trechos.map((t) => t.fonte))));
  const fontesXml: string[] = [];
  const relsFontes: string[] = [];
  let nf = 0;
  for (const familia of FAMILIAS_EMBUTIDAS) {
    if (!usadas.has(familia)) continue;
    const entradas: string[] = [];
    for (const [estilo, elemento] of ESTILOS) {
      const dados = await opcoes.fonte(`${familia.replace(/ /g, '')}-${estilo}.ttf`).catch(() => null);
      if (!dados) continue;
      const chave = guid();
      const rid = `rIdFonte${nf}`;
      arquivos.push([`word/fonts/fonte${nf}.odttf`, ofuscar(dados, chave)]);
      relsFontes.push(`<Relationship Id="${rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/font" Target="fonts/fonte${nf}.odttf"/>`);
      entradas.push(`<w:${elemento} r:id="${rid}" w:fontKey="${chave}"/>`);
      nf++;
    }
    const tipo = familia.includes('Mono') ? 'modern' : familia.includes('Sans') ? 'swiss' : 'roman';
    fontesXml.push(
      `<w:font w:name="${familia}"><w:charset w:val="00"/><w:family w:val="${tipo}"/>` +
        `<w:pitch w:val="${familia.includes('Mono') ? 'fixed' : 'variable'}"/>${entradas.join('')}</w:font>`,
    );
  }
  // fontes que nao vao embutidas: a alternativa de mesmas medidas fica registrada na tabela
  for (const familia of usadas) {
    const alt = ALTERNATIVAS[familia];
    if (!alt || FAMILIAS_EMBUTIDAS.includes(familia)) continue;
    fontesXml.push(
      `<w:font w:name="${esc(familia)}"><w:altName w:val="${esc(alt.alt)}"/><w:charset w:val="00"/><w:family w:val="${alt.tipo}"/>` +
        `<w:pitch w:val="${alt.tipo === 'modern' ? 'fixed' : 'variable'}"/></w:font>`,
    );
  }
  const tabelaFontes =
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<w:fonts xmlns:w="${W}" xmlns:r="${R}">${fontesXml.join('')}</w:fonts>`;
  const rels = (itens: string[]) =>
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${itens.join('')}</Relationships>`;
  const tipos =
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">' +
    '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>' +
    '<Default Extension="xml" ContentType="application/xml"/>' +
    '<Default Extension="png" ContentType="image/png"/>' +
    '<Default Extension="odttf" ContentType="application/vnd.openxmlformats-officedocument.obfuscatedFont"/>' +
    '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>' +
    '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>' +
    '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>' +
    '<Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>' +
    partesFaixa
      .map(([nome]) => `<Override PartName="/word/${nome}.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.${nome.startsWith('header') ? 'header' : 'footer'}+xml"/>`)
      .join('') +
    (temNotas ? '<Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/>' : '') +
    (temListas ? '<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>' : '') +
    '</Types>';

  return zipar([
    ['[Content_Types].xml', enc.encode(tipos)],
    ['_rels/.rels', enc.encode(rels(['<Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>']))],
    ['word/document.xml', enc.encode(documento)],
    ['word/styles.xml', enc.encode(estilos)],
    ['word/settings.xml', enc.encode(config)],
    ['word/fontTable.xml', enc.encode(tabelaFontes)],
    ['word/_rels/document.xml.rels', enc.encode(rels(relsDoc))],
    ['word/_rels/fontTable.xml.rels', enc.encode(rels(relsFontes))],
    ...arquivos,
  ]);
}

/** Idioma do Word tirado do babel/polyglossia (a mesma regra do Windows). */
export function idiomaDoDocumento(tex: string): string {
  const MAPA: Record<string, string> = {
    brazil: 'pt-BR', brazilian: 'pt-BR', portuguese: 'pt-PT', portuges: 'pt-PT',
    english: 'en-US', american: 'en-US', USenglish: 'en-US', british: 'en-GB', UKenglish: 'en-GB',
    spanish: 'es-ES', french: 'fr-FR', francais: 'fr-FR', german: 'de-DE', ngerman: 'de-DE', italian: 'it-IT',
  };
  let idioma = 'pt-BR';
  for (const bruta of tex.split('\n')) {
    const linha = bruta.split('%')[0];
    if (linha.includes('\\begin{document}')) break;
    let opcoes: string | null = null;
    const b = linha.indexOf('{babel}');
    if (b >= 0) opcoes = linha.slice(0, b).split('[').pop()?.replace(/\]$/, '') ?? null;
    else if (linha.includes('\\setmainlanguage')) opcoes = linha.slice(linha.indexOf('{'));
    else if (linha.includes('\\documentclass[')) opcoes = linha.slice(linha.indexOf('[')).split(']')[0];
    if (!opcoes) continue;
    for (const op of opcoes.split(/[,{}[\]]/).map((s) => s.trim())) if (MAPA[op]) idioma = MAPA[op];
  }
  return idioma;
}
