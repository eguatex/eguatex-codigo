// Barra do painel do PDF: decide o que cabe quando o painel fica estreito. Funcao pura; as larguras vem
// do DOM (getBoundingClientRect), nunca de numeros fixos, para valer em qualquer escala do Windows (100, 125, 150%).
// Ordem de corte: primeiro somem os rotulos "Recompilar" e "Baixar" (fica o icone), depois o lado direito
// esconde a pagina, os botoes de zoom e a apresentacao, nessa ordem, e por ultimo some o tempo da compilacao.

import { calcularTransbordo } from './transbordo';

export type GrupoDireito = 'pagina' | 'zoom' | 'apresentacao';

/** Ordem em que o lado direito perde grupos: primeiro a pagina, depois o zoom, por fim a apresentacao. */
export const ORDEM_CORTE_DIREITO: readonly GrupoDireito[] = ['pagina', 'zoom', 'apresentacao'];

export interface MedidasBarra {
  /** Largura util da barra, sem o padding. */
  disponivel: number;
  /** Espaco entre o lado esquerdo e o direito. */
  gap: number;
  /** Lado esquerdo com os rotulos (Recompilar, Baixar) e o tempo. */
  esquerdaCheia: number;
  /** Lado esquerdo so com icones, com o tempo. */
  esquerdaCompacta: number;
  /** Largura do tempo de compilacao dentro da esquerda compacta (0 se nao aparece). */
  tempo: number;
  /** Lado direito que nunca sai (PDF escuro e menu de zoom). 0 se nao ha lado direito. */
  direitaFixa: number;
  /** Largura de cada grupo do lado direito, ja com o espaco entre itens. */
  grupos: Record<GrupoDireito, number>;
  /** Sobra de seguranca contra arredondamento de subpixel. */
  folga?: number;
}

export interface PlanoBarra {
  /** false: Recompilar e Baixar mostram so o icone. */
  rotulos: boolean;
  /** Grupos do lado direito que saem da barra. */
  ocultos: GrupoDireito[];
  /** true: o tempo da compilacao tambem sai. */
  semTempo: boolean;
}

export function planejarBarraPdf(m: MedidasBarra): PlanoBarra {
  const folga = m.folga ?? 2;
  const disponivel = m.disponivel - folga;
  const somaGrupos = ORDEM_CORTE_DIREITO.reduce((s, g) => s + m.grupos[g], 0);
  const direitaTudo = m.direitaFixa + somaGrupos;
  const entre = (direita: number) => (direita > 0 ? m.gap : 0);
  const cabe = (esquerda: number, direita: number) => esquerda + entre(direita) + direita <= disponivel;

  if (cabe(m.esquerdaCheia, direitaTudo)) return { rotulos: true, ocultos: [], semTempo: false };
  if (cabe(m.esquerdaCompacta, direitaTudo)) return { rotulos: false, ocultos: [], semTempo: false };

  // calcularTransbordo tira o ultimo da lista primeiro: a lista vai da apresentacao (sai por ultimo) a pagina (sai primeiro).
  const lista = [...ORDEM_CORTE_DIREITO].reverse();
  const espaco = disponivel - m.esquerdaCompacta - m.gap;
  const saem = calcularTransbordo(lista.map((g) => m.grupos[g]), espaco, { fixo: m.direitaFixa });
  const ocultos = ORDEM_CORTE_DIREITO.filter((g) => saem.includes(lista.indexOf(g)));
  const restante = m.direitaFixa + ORDEM_CORTE_DIREITO.filter((g) => !ocultos.includes(g)).reduce((s, g) => s + m.grupos[g], 0);
  if (cabe(m.esquerdaCompacta, restante)) return { rotulos: false, ocultos, semTempo: false };
  return { rotulos: false, ocultos, semTempo: m.tempo > 0 };
}

/** Abaixo desta largura (em px) a barra deixa so o PDF escuro, o menu de zoom e o "...", que guarda a pagina e o zoom. */
export const LARGURA_BARRA_MINIMA = 320;

export function barraMinima(largura: number): boolean {
  return largura > 0 && largura < LARGURA_BARRA_MINIMA;
}
