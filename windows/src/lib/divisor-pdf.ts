// Limites do divisor entre o editor e o PDF. A area dos dois e a janela menos a barra de icones
// e o painel lateral aberto. O PDF ocupa de 25% a 75% dessa area, entao o editor tambem pode ir
// de 25% a 75%, para um lado ou para o outro (como no Overleaf).
export const FRACAO_MINIMA = 0.25;
export const FRACAO_MAXIMA = 0.75;

/** Largura do PDF dentro dos limites. `outros` e a largura das barras e paineis ao lado do editor. */
export function limitarLarguraPdf(desejada: number, janela: number, outros: number): number {
  const area = Math.max(0, janela - outros);
  return Math.round(Math.max(area * FRACAO_MINIMA, Math.min(area * FRACAO_MAXIMA, desejada)));
}
