// Abas do editor: varios arquivos abertos ao mesmo tempo, num unico EditorView.
// As funcoes puras (abrir, fechar, renomearPrefixo, removerPrefixo, proxima, anterior) recebem um
// EstadoAbas e devolvem outro; so o estado reativo e as ajudas do fim do arquivo mexem em $state.
// Arquivo .svelte.ts: usa runes fora de componentes. Nunca reatribua `abasEstado`; use aplicarEstadoAbas.

import type { EditorState } from '@codemirror/state';
import { SvelteSet } from 'svelte/reactivity';

export type Aba = { tipo: 'texto'; caminho: string } | { tipo: 'imagem'; caminho: string };

export interface EstadoAbas {
  /** Abas na ordem em que aparecem na barra. */
  abas: Aba[];
  /** Caminho da aba ativa; '' se nao ha nenhuma. */
  ativa: string;
  /** Caminhos do menos para o mais usado (a ultima e a ativa). Vale para o LRU. */
  recentes: string[];
}

/** Maximo de abas abertas. Acima disso fecha a menos usada que estiver salva. */
export const LIMITE_ABAS = 12;

export const estadoVazio = (): EstadoAbas => ({ abas: [], ativa: '', recentes: [] });

const SEM_SUJAS: ReadonlySet<string> = new Set();

function usar(recentes: readonly string[], caminho: string): string[] {
  return [...recentes.filter((c) => c !== caminho), caminho];
}

/** O caminho e a pasta `prefixo` ou fica dentro dela ("/a" nao pega "/ab"). */
export function dentroDe(caminho: string, prefixo: string): boolean {
  return caminho === prefixo || caminho.startsWith(prefixo + '/');
}

/** Se passar do limite, fecha a menos usada que nao seja a ativa nem esteja suja (nunca perde edicao). */
function podar(estado: EstadoAbas, sujas: ReadonlySet<string>, limite: number): EstadoAbas {
  let { abas, recentes } = estado;
  while (abas.length > limite) {
    // as que nunca foram usadas nesta sessao (abas reabertas) saem primeiro
    const semUso = abas.map((a) => a.caminho).filter((c) => !recentes.includes(c));
    const ordem = [...semUso, ...recentes.filter((c) => abas.some((a) => a.caminho === c))];
    const alvo = ordem.find((c) => c !== estado.ativa && !sujas.has(c));
    if (alvo === undefined) break;
    abas = abas.filter((a) => a.caminho !== alvo);
    recentes = recentes.filter((c) => c !== alvo);
  }
  return { abas, ativa: estado.ativa, recentes };
}

/** Abre a aba (ou so ativa, se ja existe: nunca duplica) e aplica o limite de abas. */
export function abrir(estado: EstadoAbas, aba: Aba, sujas: ReadonlySet<string> = SEM_SUJAS, limite = LIMITE_ABAS): EstadoAbas {
  const existe = estado.abas.some((a) => a.caminho === aba.caminho);
  const abas = existe ? [...estado.abas] : [...estado.abas, aba];
  return podar({ abas, ativa: aba.caminho, recentes: usar(estado.recentes, aba.caminho) }, sujas, limite);
}

/** Ativa uma aba que ja esta aberta. */
export function ativar(estado: EstadoAbas, caminho: string): EstadoAbas {
  if (!estado.abas.some((a) => a.caminho === caminho)) return estado;
  return { ...estado, ativa: caminho, recentes: usar(estado.recentes, caminho) };
}

/** Escolhe a vizinha de quem saiu: a da direita; sem ela, a da esquerda. `indice` e a posicao antiga. */
function vizinha(abas: readonly Aba[], indice: number): string {
  return (abas[indice] ?? abas[indice - 1])?.caminho ?? '';
}

/** Fecha a aba. Se era a ativa, a vizinha passa a ser a ativa. Fechar a ultima deixa `ativa` vazia. */
export function fechar(estado: EstadoAbas, caminho: string): EstadoAbas {
  const indice = estado.abas.findIndex((a) => a.caminho === caminho);
  if (indice < 0) return estado;
  const abas = estado.abas.filter((a) => a.caminho !== caminho);
  const recentes = estado.recentes.filter((c) => c !== caminho);
  if (estado.ativa !== caminho) return { abas, ativa: estado.ativa, recentes };
  const ativa = vizinha(abas, indice);
  return { abas, ativa, recentes: ativa ? usar(recentes, ativa) : recentes };
}

/** Renomear ou mover: troca o prefixo `antigo` por `novo` em todas as abas (um arquivo ou uma pasta). */
export function renomearPrefixo(estado: EstadoAbas, antigo: string, novo: string): EstadoAbas {
  const trocar = (c: string) => (dentroDe(c, antigo) ? novo + c.slice(antigo.length) : c);
  const vistos = new Set<string>();
  const abas: Aba[] = [];
  for (const a of estado.abas) {
    const caminho = trocar(a.caminho);
    if (vistos.has(caminho)) continue;
    vistos.add(caminho);
    abas.push({ ...a, caminho });
  }
  const recentes = [...new Set(estado.recentes.map(trocar))];
  return { abas, ativa: trocar(estado.ativa), recentes };
}

/** Apagar: fecha todas as abas do arquivo ou da pasta. Se a ativa saiu, a vizinha mais proxima assume. */
export function removerPrefixo(estado: EstadoAbas, prefixo: string): EstadoAbas {
  const abas = estado.abas.filter((a) => !dentroDe(a.caminho, prefixo));
  const recentes = estado.recentes.filter((c) => !dentroDe(c, prefixo));
  if (!dentroDe(estado.ativa, prefixo)) return { abas, ativa: estado.ativa, recentes };
  // a mais proxima da antiga posicao da ativa: a primeira que sobrou depois dela, senao a ultima antes
  const posicao = estado.abas.findIndex((a) => a.caminho === estado.ativa);
  const sobraram = new Set(abas.map((a) => a.caminho));
  const depois = estado.abas.slice(posicao + 1).find((a) => sobraram.has(a.caminho));
  const antes = [...estado.abas.slice(0, Math.max(posicao, 0))].reverse().find((a) => sobraram.has(a.caminho));
  const ativa = (depois ?? antes)?.caminho ?? '';
  return { abas, ativa, recentes: ativa ? usar(recentes, ativa) : recentes };
}

/**
 * Arrastar uma aba: tira a aba da posicao `de` e a poe na posicao `para` da lista ja sem ela
 * (`para` entre 0 e abas.length - 1). A aba ativa e as recentes nao mudam; so a ordem.
 */
export function mover(estado: EstadoAbas, de: number, para: number): EstadoAbas {
  const n = estado.abas.length;
  if (!Number.isInteger(de) || !Number.isInteger(para) || de < 0 || de >= n) return estado;
  const destino = Math.min(n - 1, Math.max(0, para));
  if (destino === de) return estado;
  const abas = [...estado.abas];
  const [aba] = abas.splice(de, 1);
  abas.splice(destino, 0, aba);
  return { ...estado, abas };
}

/**
 * Indice final de uma aba solta ao lado de outra: `lado` diz se ela cai antes ou depois da aba alvo.
 * `de` e `alvo` sao posicoes na lista atual. Devolve o `para` de `mover`, ou -1 se a soltura nao muda nada.
 */
export function destinoDaSoltura(total: number, de: number, alvo: number, lado: 'esquerda' | 'direita'): number {
  if (!Number.isInteger(de) || !Number.isInteger(alvo) || !Number.isInteger(total)) return -1;
  if (de === alvo || de < 0 || alvo < 0 || de >= total || alvo >= total) return -1;
  // depois de tirar a aba de `de`, o alvo anda uma casa para tras se ele vinha depois
  const alvoSemOrigem = alvo > de ? alvo - 1 : alvo;
  const para = alvoSemOrigem + (lado === 'direita' ? 1 : 0);
  return para === de ? -1 : para;
}

function andar(estado: EstadoAbas, passo: 1 | -1): EstadoAbas {
  const n = estado.abas.length;
  if (n === 0) return estado;
  const atual = estado.abas.findIndex((a) => a.caminho === estado.ativa);
  // sem ativa: a proxima e a primeira e a anterior e a ultima
  const alvo = atual < 0 ? (passo === 1 ? 0 : n - 1) : (atual + passo + n) % n;
  return ativar(estado, estado.abas[alvo].caminho);
}

/** Proxima aba; depois da ultima volta a primeira. */
export const proxima = (estado: EstadoAbas): EstadoAbas => andar(estado, 1);

/** Aba anterior; antes da primeira vai para a ultima. */
export const anterior = (estado: EstadoAbas): EstadoAbas => andar(estado, -1);

/** Atalhos de aba: Ctrl+Tab e Ctrl+PageDown (proxima), Ctrl+Shift+Tab e Ctrl+PageUp (anterior), Ctrl+W (fechar). */
export function atalhoDeAba(e: { key: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey?: boolean }): 'proxima' | 'anterior' | 'fechar' | null {
  if (!e.ctrlKey || e.altKey || e.metaKey) return null;
  if (e.key === 'Tab') return e.shiftKey ? 'anterior' : 'proxima';
  if (e.shiftKey) return null;
  if (e.key === 'PageDown') return 'proxima';
  if (e.key === 'PageUp') return 'anterior';
  if (e.key.toLowerCase() === 'w') return 'fechar';
  return null;
}

/**
 * O que gravar antes de compilar, sincronizar ou fechar a janela: todas as abas de texto sujas, estejam
 * ativas ou nao. Imagem nunca entra; caminho sujo sem aba aberta tambem nao.
 */
export function abasParaSalvar(abas: readonly Aba[], sujas: Iterable<string>): string[] {
  const conjunto = new Set(sujas);
  return abas.filter((a) => a.tipo === 'texto' && conjunto.has(a.caminho)).map((a) => a.caminho);
}

/** Nome do arquivo para o rotulo da aba. */
export const nomeDaAba = (caminho: string): string => caminho.split('/').pop() || caminho;

// ---------- persistencia por projeto (localStorage) ----------

/** Chave do localStorage com as abas de um projeto (a chave leva o caminho do projeto, como o principal). */
export function chaveAbasDoProjeto(caminhoProjeto: string): string {
  return `eguatex.abas.${caminhoProjeto.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()}`;
}

type Leitor = Pick<Storage, 'getItem'>;
type Gravador = Pick<Storage, 'setItem'>;

const armazenamentoPadrao = (): Storage | undefined => {
  try {
    return typeof localStorage === 'undefined' ? undefined : localStorage;
  } catch {
    return undefined;
  }
};

/**
 * As abas guardadas do projeto, so com os caminhos que `existe` confirma (arquivo apagado ou
 * renomeado fora do app some). Dado ilegivel ou sem localStorage vale como "sem abas".
 */
export function lerAbasSalvas(caminhoProjeto: string, existe: (caminho: string) => boolean, armazenamento: Leitor | undefined = armazenamentoPadrao()): EstadoAbas {
  try {
    const bruto = armazenamento?.getItem(chaveAbasDoProjeto(caminhoProjeto));
    if (!bruto) return estadoVazio();
    const salvo: unknown = JSON.parse(bruto);
    if (typeof salvo !== 'object' || salvo === null) return estadoVazio();
    const { abas: principal, ativa, direita } = salvo as { abas?: unknown; ativa?: unknown; direita?: { abas?: unknown } | null };
    if (!Array.isArray(principal)) return estadoVazio();
    // formato da fase 1c: as imagens ficavam em `direita`; voltam para o grupo unico, depois das abas de texto
    const extras = typeof direita === 'object' && direita !== null && Array.isArray(direita.abas) ? direita.abas : [];
    const lista: unknown[] = [...principal, ...extras];
    const vistos = new Set<string>();
    const abas: Aba[] = [];
    for (const item of lista) {
      if (typeof item !== 'object' || item === null) continue;
      const { tipo, caminho } = item as { tipo?: unknown; caminho?: unknown };
      if ((tipo !== 'texto' && tipo !== 'imagem') || typeof caminho !== 'string') continue;
      if (vistos.has(caminho) || !existe(caminho)) continue;
      vistos.add(caminho);
      abas.push({ tipo, caminho });
      if (abas.length >= LIMITE_ABAS) break;
    }
    const ativaValida = typeof ativa === 'string' && vistos.has(ativa) ? ativa : '';
    return { abas, ativa: ativaValida, recentes: ativaValida ? [ativaValida] : [] };
  } catch {
    return estadoVazio();
  }
}

/** Guarda as abas do projeto. Sem localStorage, as abas valem so nesta sessao. */
export function gravarAbasSalvas(caminhoProjeto: string, estado: EstadoAbas, armazenamento: Gravador | undefined = armazenamentoPadrao()): void {
  try {
    armazenamento?.setItem(chaveAbasDoProjeto(caminhoProjeto), JSON.stringify({ abas: estado.abas, ativa: estado.ativa }));
  } catch {
    // sem armazenamento: as abas nao voltam na proxima abertura
  }
}

// ---------- estado reativo e dados por aba ----------

/** Estado das abas na tela. Altere so por aplicarEstadoAbas. */
export const abasEstado: EstadoAbas = $state(estadoVazio());

export function aplicarEstadoAbas(novo: EstadoAbas): void {
  abasEstado.abas = novo.abas.map((a) => ({ ...a }));
  abasEstado.ativa = novo.ativa;
  abasEstado.recentes = [...novo.recentes];
}

/** Foto do estado reativo, sem proxy, para as funcoes puras. */
export function fotoDasAbas(): EstadoAbas {
  return { abas: abasEstado.abas.map((a) => ({ ...a })), ativa: abasEstado.ativa, recentes: [...abasEstado.recentes] };
}

/** Aba com texto ainda nao confirmado pelo disco. Reativo: a barra mostra o ponto no lugar do x. */
export const sujas = new SvelteSet<string>();

/** O que cada aba de texto guarda enquanto nao e a ativa. FORA do $state: EditorState e imutavel e grande. */
export interface DadosAba {
  estado: EditorState;
  rolagem: number;
  sujo: boolean;
}
export const dadosDasAbas = new Map<string, DadosAba>();

export function marcarSuja(caminho: string, suja: boolean): void {
  if (suja) sujas.add(caminho);
  else sujas.delete(caminho);
  const dados = dadosDasAbas.get(caminho);
  if (dados) dados.sujo = suja;
}

export const estaSuja = (caminho: string): boolean => sujas.has(caminho);

/** Renomear ou mover: as chaves do Map e do conjunto de sujas acompanham o novo caminho. */
export function renomearDados(antigo: string, novo: string): void {
  const trocar = (c: string) => (dentroDe(c, antigo) ? novo + c.slice(antigo.length) : c);
  for (const [chave, dados] of [...dadosDasAbas]) {
    const nova = trocar(chave);
    if (nova !== chave) {
      dadosDasAbas.delete(chave);
      dadosDasAbas.set(nova, dados);
    }
  }
  for (const c of [...sujas]) {
    const nova = trocar(c);
    if (nova !== c) {
      sujas.delete(c);
      sujas.add(nova);
    }
  }
}

/** Apagar ou fechar: o estado guardado e a marca de suja somem junto com as abas. */
export function removerDados(prefixo: string): void {
  for (const chave of [...dadosDasAbas.keys()]) if (dentroDe(chave, prefixo)) dadosDasAbas.delete(chave);
  for (const c of [...sujas]) if (dentroDe(c, prefixo)) sujas.delete(c);
}

/** Troca de projeto: nada das abas do projeto anterior sobra. */
export function limparDadosDasAbas(): void {
  dadosDasAbas.clear();
  sujas.clear();
}
