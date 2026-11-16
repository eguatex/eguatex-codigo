// Decide se o corretor deve sublinhar o arquivo aberto e em qual idioma (portugues ou ingles).

export type ModoOrtografia = 'desligada' | 'portugues' | 'ingles' | 'automatico';

/** Idiomas com dicionario no app. */
export type IdiomaOrtografia = 'pt' | 'en';

export type EstadoOrtografia =
  | { ligada: true; idioma?: IdiomaOrtografia }
  | { ligada: false; motivo: 'preferencia' }
  | { ligada: false; motivo: 'idioma'; codigo: string }
  | { ligada: false; motivo: 'nao-portugues' };

const LINHAS_MAGICAS = 20;
const AMOSTRA = 2000;
const LIMIAR_DESCONHECIDAS = 0.6;
const MINIMO_PALAVRAS = 8;

/** `% !TeX spellcheck = en_US` nas primeiras 20 linhas; null sem o comentario. */
export function codigoSpellcheck(texto: string): string | null {
  const linhas = texto.split(/\r?\n/, LINHAS_MAGICAS);
  for (const linha of linhas) {
    const m = /^\s*%\s*!TeX\s+spellcheck\s*=\s*([A-Za-z]{2,3}(?:[-_][A-Za-z]{2,4})?)\s*$/i.exec(linha);
    if (m) return m[1];
  }
  return null;
}

export function ehPortugues(codigo: string): boolean {
  return /^pt/i.test(codigo);
}

export function ehIngles(codigo: string): boolean {
  return /^en/i.test(codigo);
}

/** `\usepackage[english]{babel}` (ou american, british, USenglish, UKenglish) e o ultimo idioma do babel ou o polyglossia em ingles. */
export function inglesPeloBabel(texto: string): boolean {
  const trecho = texto.slice(0, 20000).replace(/(^|[^\\])%.*$/gm, '$1');
  const babel = /\\usepackage\s*\[([^\]]*)\]\s*\{babel\}/.exec(trecho);
  if (babel) {
    const opcoes = babel[1].split(',').map((o) => o.trim()).filter((o) => o && !o.includes('='));
    const principal = opcoes[opcoes.length - 1] ?? '';
    return /^(english|american|british|usenglish|ukenglish|canadian|australian|newzealand)$/i.test(principal);
  }
  return /\\setdefaultlanguage(?:\[[^\]]*\])?\{english\}|\\setmainlanguage(?:\[[^\]]*\])?\{english\}/.test(trecho);
}

/** Palavras com 4 ou mais letras dos primeiros 2000 caracteres, sem comandos LaTeX. */
export function palavrasAmostra(texto: string): string[] {
  const trecho = texto.slice(0, AMOSTRA).replace(/\\[A-Za-z]+\*?/g, ' ').replace(/%[^\n]*/g, ' ');
  const vistas = new Set<string>();
  for (const m of trecho.matchAll(/[A-Za-zÀ-ÿ]{4,}/g)) vistas.add(m[0].toLowerCase());
  return [...vistas];
}

/** Fracao de palavras da amostra que o dicionario nao conhece; null com amostra curta. */
export function fracaoDesconhecida(palavras: string[], desconhecidas: Iterable<string>): number | null {
  if (palavras.length < MINIMO_PALAVRAS) return null;
  const ruins = new Set([...desconhecidas].map((p) => p.toLowerCase()));
  const n = palavras.filter((p) => ruins.has(p.toLowerCase())).length;
  return n / palavras.length;
}

export function pareceOutroIdioma(fracao: number | null): boolean {
  return fracao !== null && fracao > LIMIAR_DESCONHECIDAS;
}

/**
 * `fracao`: parte da amostra que o dicionario pt-BR nao conhece. `fracaoIngles`: o mesmo no en-US.
 * `babelIngles`: o documento pede ingles no babel ou no polyglossia.
 */
export function decidirOrtografia(
  modo: ModoOrtografia,
  codigo: string | null,
  fracao: number | null,
  fracaoIngles: number | null = null,
  babelIngles = false,
): EstadoOrtografia {
  if (modo === 'desligada') return { ligada: false, motivo: 'preferencia' };
  if (modo === 'ingles') return { ligada: true, idioma: 'en' };
  if (codigo !== null) {
    if (ehPortugues(codigo)) return { ligada: true, idioma: 'pt' };
    if (ehIngles(codigo)) return { ligada: true, idioma: 'en' };
    return { ligada: false, motivo: 'idioma', codigo };
  }
  if (modo === 'automatico') {
    if (babelIngles && !(fracao !== null && fracaoIngles !== null && fracao < fracaoIngles)) return { ligada: true, idioma: 'en' };
    if (pareceOutroIdioma(fracao)) {
      // texto que o portugues nao reconhece e o ingles reconhece: corretor em ingles
      if (fracaoIngles !== null && !pareceOutroIdioma(fracaoIngles) && fracaoIngles < (fracao ?? 1)) return { ligada: true, idioma: 'en' };
      return { ligada: false, motivo: 'nao-portugues' };
    }
  }
  return { ligada: true, idioma: 'pt' };
}

/** Preferencia antiga era booleana: true vira automatico, false vira desligada. */
export function normalizarModo(valor: unknown): ModoOrtografia | undefined {
  if (valor === true) return 'automatico';
  if (valor === false) return 'desligada';
  return valor === 'desligada' || valor === 'portugues' || valor === 'ingles' || valor === 'automatico' ? valor : undefined;
}
