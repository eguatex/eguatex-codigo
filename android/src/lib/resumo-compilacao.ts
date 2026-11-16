// Resumo da ultima compilacao para a barra de status: "2,5 s, 2 erros, 1 aviso". Sem tempo (ainda nao
// compilou) devolve vazio; sem erros e sem avisos so o tempo.

import { t, tf } from './i18n';
import { formatarTempo } from './formatar-tempo';

export function resumoCompilacao(tempoMs: number | null, erros: number, avisos: number): string {
  if (tempoMs === null) return '';
  const tempo = formatarTempo(tempoMs);
  if (!tempo) return '';
  const partes = [tempo];
  if (erros > 0) partes.push(erros === 1 ? t('status.resumo-erro') : tf('status.resumo-erros', { n: erros }));
  if (avisos > 0) partes.push(avisos === 1 ? t('status.resumo-aviso') : tf('status.resumo-avisos', { n: avisos }));
  return partes.join(', ');
}
