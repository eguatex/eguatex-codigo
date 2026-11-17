// Limites do divisor entre o editor e o PDF: o PDF ocupa de 300 px ate 70% da janela,
// e o editor fica sempre com pelo menos 360 px.
export const PDF_MINIMO = 300;
export const EDITOR_MINIMO = 360;
export const FRACAO_MAXIMA = 0.7;

/** Largura do PDF dentro dos limites. `outros` e a largura das barras e paineis ao lado do editor. */
export function limitarLarguraPdf(desejada: number, janela: number, outros: number): number {
  const teto = Math.min(janela * FRACAO_MAXIMA, janela - EDITOR_MINIMO - outros);
  return Math.round(Math.max(PDF_MINIMO, Math.min(teto, desejada)));
}
