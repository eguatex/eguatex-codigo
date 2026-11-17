// Aviso de versao nova: desligado de fabrica. Ligado, consulta o GitHub no maximo uma vez por dia,
// ao abrir, e mostra uma faixa discreta no alto da tela. Sem internet, nao mostra nada.
// Na primeira abertura, a faixa pergunta uma vez se a pessoa quer o aviso.

import { invoke } from '@tauri-apps/api/core';
import { avisoDeAtualizacao, type ResultadoAtualizacao } from './atualizacao';
import { preferencias, definirPreferencia } from './preferencias.svelte';

export type Faixa =
  | { tipo: 'oferta' }
  | { tipo: 'pronto' }
  | { tipo: 'resultado'; texto: string; link: string | null; oferecerAviso: boolean };

export const faixaVersao = $state<{ atual: Faixa | null }>({ atual: null });

/** Dia local no formato AAAA-MM-DD. */
export function diaLocal(d = new Date()): string {
  const doisDigitos = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${doisDigitos(d.getMonth() + 1)}-${doisDigitos(d.getDate())}`;
}

/** A consulta automatica so roda com o aviso ligado e uma vez por dia. */
export function deveConsultar(p: { avisarVersaoNova: boolean; ultimaConsultaVersao: string }, hoje: string): boolean {
  return p.avisarVersaoNova && p.ultimaConsultaVersao !== hoje;
}

/** Na abertura do app. */
export async function iniciarAvisoDeVersao(): Promise<void> {
  try {
    // caixa marcada no instalador do Windows: ja abre com o aviso ligado e sem a faixa
    if (await invoke<boolean>('cmd_aviso_do_instalador')) {
      definirPreferencia('avisarVersaoNova', true);
      definirPreferencia('faixaAvisoVista', true);
    }
  } catch {
    // sem o comando (testes, navegador), segue sem ele
  }
  if (preferencias.avisarVersaoNova && !preferencias.faixaAvisoVista) definirPreferencia('faixaAvisoVista', true);
  if (!preferencias.faixaAvisoVista) {
    faixaVersao.atual = { tipo: 'oferta' };
    return;
  }
  const hoje = diaLocal();
  if (!deveConsultar(preferencias, hoje)) return;
  definirPreferencia('ultimaConsultaVersao', hoje);
  try {
    const r = await invoke<ResultadoAtualizacao>('cmd_procurar_atualizacao');
    if (r.estado !== 'nova') return;
    const a = avisoDeAtualizacao(r);
    faixaVersao.atual = { tipo: 'resultado', texto: a.texto, link: a.link, oferecerAviso: false };
  } catch {
    // sem internet ou outro erro: nada aparece
  }
}

export function ligarAviso(): void {
  definirPreferencia('avisarVersaoNova', true);
  definirPreferencia('faixaAvisoVista', true);
  faixaVersao.atual = { tipo: 'pronto' };
  setTimeout(() => {
    if (faixaVersao.atual?.tipo === 'pronto') faixaVersao.atual = null;
  }, 4000);
}

export function agoraNao(): void {
  definirPreferencia('faixaAvisoVista', true);
  faixaVersao.atual = null;
}

export function fecharFaixa(): void {
  faixaVersao.atual = null;
}

/** Resultado do "Procurar atualizacao" do menu. Com o aviso desligado, oferece liga-lo. */
export function mostrarResultadoManual(texto: string, link: string | null): void {
  faixaVersao.atual = { tipo: 'resultado', texto, link, oferecerAviso: !preferencias.avisarVersaoNova };
}
