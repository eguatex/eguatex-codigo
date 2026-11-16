// Registro unico das esperas do app (compilar, abrir projeto, zip, Drive...).
// Arquivo .svelte.ts: usa runes fora de componentes. Nunca reatribua `tarefas`;
// o registro so muda por iniciarTarefa, comTarefa e aplicarProgressoRust.

import { untrack } from 'svelte';

export interface Tarefa {
  id: number;
  rotulo: string;
  feito?: number;
  total?: number;
  /** Date.now() de quando a tarefa comecou. */
  inicio: number;
  cancelar?: () => void;
  /** Nome do canal do evento `tarefa-progresso` do Rust que alimenta esta tarefa. */
  canal?: string;
}

export interface OpcoesTarefa {
  cancelar?: () => void;
  canal?: string;
}

export interface ControleTarefa {
  id: number;
  /** Informa quanto ja foi feito; `rotulo` troca o texto da tarefa. Depois de terminar, nao faz nada. */
  atualizar: (feito: number, total?: number, rotulo?: string) => void;
  /** Encerra a tarefa. Chamar de novo nao faz nada. */
  terminar: () => void;
}

/** A barra so aparece depois deste tempo, para nao piscar em esperas curtas. */
export const ATRASO_BARRA_MS = 300;

export const tarefas: { lista: Tarefa[] } = $state({ lista: [] });

/** Uma tarefa que comeca ate este tempo depois do fim da anterior herda o `inicio` dela: a barra nao pisca entre etapas encadeadas. */
export const FOLGA_ENCADEADA_MS = 200;

let proximoId = 1;
/** Inicio e fim da ultima sequencia que esvaziou a lista. */
let ultimaSequencia: { inicio: number; fim: number } | null = null;

export function iniciarTarefa(rotulo: string, opcoes: OpcoesTarefa = {}): ControleTarefa {
  const id = proximoId++;
  const agora = Date.now();
  const encadeada = tarefas.lista.length === 0 && ultimaSequencia !== null && agora >= ultimaSequencia.fim && agora - ultimaSequencia.fim <= FOLGA_ENCADEADA_MS;
  const inicio = encadeada && ultimaSequencia ? ultimaSequencia.inicio : agora;
  tarefas.lista.push({ id, rotulo, inicio, cancelar: opcoes.cancelar, canal: opcoes.canal });
  let ativa = true;

  const achar = (): Tarefa | undefined => tarefas.lista.find((x) => x.id === id);

  return {
    id,
    atualizar(feito, total, novoRotulo) {
      if (!ativa) return;
      const tarefa = achar();
      if (!tarefa) return;
      tarefa.feito = feito;
      if (total !== undefined) tarefa.total = total;
      if (novoRotulo !== undefined) tarefa.rotulo = novoRotulo;
    },
    terminar() {
      if (!ativa) return;
      ativa = false;
      const i = tarefas.lista.findIndex((x) => x.id === id);
      if (i >= 0) {
        const [saiu] = tarefas.lista.splice(i, 1);
        if (tarefas.lista.length === 0) ultimaSequencia = { inicio: saiu.inicio, fim: Date.now() };
      }
    },
  };
}

/** Roda `fn` dentro de uma tarefa e a encerra em `finally`, mesmo se `fn` lancar. */
export async function comTarefa<T>(
  rotulo: string,
  fn: (atualizar: ControleTarefa['atualizar']) => Promise<T> | T,
  opcoes: OpcoesTarefa = {},
): Promise<T> {
  const controle = iniciarTarefa(rotulo, opcoes);
  try {
    return await fn(controle.atualizar);
  } finally {
    controle.terminar();
  }
}

/** Aplica um evento `tarefa-progresso` do Rust na tarefa mais recente daquele canal. */
export function aplicarProgressoRust(evento: { id: string; feito: number; total: number }): void {
  for (let i = tarefas.lista.length - 1; i >= 0; i--) {
    const tarefa = tarefas.lista[i];
    if (tarefa.canal === evento.id) {
      tarefa.feito = evento.feito;
      tarefa.total = evento.total;
      return;
    }
  }
}

/** Porcentagem inteira de 0 a 100, ou null quando nao ha total (barra sem numero). */
export function porcentagem(tarefa: Pick<Tarefa, 'feito' | 'total'>): number | null {
  const { feito, total } = tarefa;
  if (feito === undefined || total === undefined || !(total > 0)) return null;
  return Math.max(0, Math.min(100, Math.round((feito / total) * 100)));
}

/** A tarefa mais recente que ja passou do atraso de 300 ms, ou undefined. */
export function tarefaVisivel(lista: readonly Tarefa[], agora: number): Tarefa | undefined {
  for (let i = lista.length - 1; i >= 0; i--) {
    if (agora - lista[i].inicio >= ATRASO_BARRA_MS) return lista[i];
  }
  return undefined;
}

/** Liga o evento `tarefa-progresso` do Rust ao registro. Devolve a funcao que desliga. */
export async function escutarProgressoRust(): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<{ id: string; feito: number; total: number }>('tarefa-progresso', (e) => aplicarProgressoRust(e.payload));
}

/**
 * Mantem uma tarefa "compilando" enquanto `status()` for 'compilando'. Chamar dentro de um componente (ou de um
 * `$effect.root`). O efeito le so o status: criar e encerrar a tarefa mexe na lista, e sem `untrack` o efeito
 * passaria a depender dela e rodaria sem parar. A limpeza encerra a tarefa ao mudar o status e ao desmontar.
 */
export function acompanharCompilacao(status: () => string, rotulo: string): void {
  $effect(() => {
    if (status() !== 'compilando') return;
    const tarefa = untrack(() => iniciarTarefa(rotulo, { canal: 'compilar' }));
    return () => untrack(() => tarefa.terminar());
  });
}
