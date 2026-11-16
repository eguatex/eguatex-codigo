/**
 * Tempo de compilacao no formato pt-BR: segundos com 1 casa e virgula decimal.
 * 2800 ms vira "2,8 s" e 450 ms vira "0,5 s". Conta em inteiros para nao
 * depender de arredondamento de ponto flutuante nem do ICU do sistema.
 */
export function formatarTempo(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "";
  const decimos = Math.round(ms / 100);
  return `${Math.floor(decimos / 10)},${decimos % 10} s`;
}
