// Lista de projetos da tela inicial, como a do Overleaf: tempo relativo da ultima
// modificacao, local encurtado e ordenacao pela data.

import { t, tf } from './i18n';

const SEGUNDO = 1000;
const MINUTO = 60 * SEGUNDO;
const HORA = 60 * MINUTO;
const DIA = 24 * HORA;
const MES = 30 * DIA;
const ANO = 365 * DIA;

type Unidade = 'minuto' | 'hora' | 'dia' | 'mes' | 'ano';
const PLURAL = { minuto: 'lista.minutos', hora: 'lista.horas', dia: 'lista.dias', mes: 'lista.meses', ano: 'lista.anos' } as const;
const SINGULAR = { minuto: 'lista.minuto', hora: 'lista.hora', dia: 'lista.dia', mes: 'lista.mes', ano: 'lista.ano' } as const;

function plural(n: number, unidade: Unidade): string {
  return n === 1 ? t(SINGULAR[unidade]) : tf(PLURAL[unidade], { n });
}

/**
 * "há poucos segundos", "há 17 minutos", "há 6 dias" (textos do pt-BR.json). Arredonda como o
 * moment.js do Overleaf: 45 s ja e 1 minuto, 45 min ja e 1 hora, 22 h ja e 1 dia, 26 dias ja e
 * 1 mes e 11 meses ja e 1 ano. Data no futuro (relogio de outro computador) conta como agora.
 */
export function tempoRelativo(quando: number, agora: number = Date.now()): string {
  if (!Number.isFinite(quando)) return '';
  const diff = Math.max(0, agora - quando);
  if (diff < 45 * SEGUNDO) return t('lista.segundos');
  if (diff < 45 * MINUTO) return plural(Math.max(1, Math.round(diff / MINUTO)), 'minuto');
  if (diff < 22 * HORA) return plural(Math.max(1, Math.round(diff / HORA)), 'hora');
  if (diff < 26 * DIA) return plural(Math.max(1, Math.round(diff / DIA)), 'dia');
  if (diff < 320 * DIA) return plural(Math.max(1, Math.round(diff / MES)), 'mes');
  return plural(Math.max(1, Math.round(diff / ANO)), 'ano');
}

/** "Mostrando 1 projeto" ou "Mostrando N projetos". */
export function textoMostrando(n: number): string {
  return n === 1 ? t('lista.mostrando-um') : tf('lista.mostrando', { n });
}

/** Pasta onde o projeto esta (o caminho sem o nome do projeto), com as barras do Windows. */
export function pastaDoProjeto(caminho: string): string {
  const limpo = caminho.replace(/[\\/]+$/, '');
  const i = Math.max(limpo.lastIndexOf('\\'), limpo.lastIndexOf('/'));
  return i > 0 ? limpo.slice(0, i) : limpo;
}

/**
 * Local curto: as duas ultimas pastas, como "…\Documents\EguaTex". Caminho com ate duas pastas
 * fica inteiro. Usa o separador que o caminho ja usa.
 */
export function localCurto(caminho: string, pastas = 2): string {
  const pasta = pastaDoProjeto(caminho);
  const separador = pasta.includes('\\') || !pasta.includes('/') ? '\\' : '/';
  const partes = pasta.split(/[\\/]+/).filter((p) => p !== '');
  if (partes.length <= pastas + 1) return pasta.replace(/[\\/]+/g, separador);
  return `…${separador}${partes.slice(-pastas).join(separador)}`;
}

export type Ordem = 'desc' | 'asc';

/** Ordena pela data (ms); sem data vai para o fim. Empate: nome em ordem alfabetica. */
export function ordenarPorData<T extends { nome: string }>(lista: readonly T[], data: (item: T) => number, ordem: Ordem): T[] {
  return [...lista].sort((a, b) => {
    const da = data(a);
    const db = data(b);
    const va = Number.isFinite(da);
    const vb = Number.isFinite(db);
    if (va !== vb) return va ? -1 : 1;
    if (va && vb && da !== db) return ordem === 'desc' ? db - da : da - db;
    return a.nome.localeCompare(b.nome, 'pt-BR');
  });
}
