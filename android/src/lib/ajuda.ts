// Itens do menu Ajuda que falam com o sistema: "Enviar feedback" e "Procurar atualização".
// Os dois só agem quando a pessoa clica. Nenhum roda sozinho.

import { invoke } from '@tauri-apps/api/core';
import { avisoDeAtualizacao, type ResultadoAtualizacao } from './atualizacao';
import { mostrarResultadoManual } from './aviso-versao.svelte';
import { sistemaDoUserAgent } from './feedback';
import { abrirFeedbackModal } from './feedback-modal.svelte';

/** Abre o modal "Enviar feedback" com a versão do app e o sistema já lidos. */
export async function enviarFeedback(): Promise<void> {
  let versao = '?';
  try {
    versao = await invoke<string>('cmd_versao');
  } catch {
    // sem a versão o feedback sai igual, com "?" no lugar
  }
  abrirFeedbackModal(versao, sistemaDoUserAgent(navigator.userAgent));
}

/** Pergunta ao GitHub se há versão mais nova. Sem release ou sem internet, só avisa. */
export async function procurarAtualizacao(): Promise<void> {
  let aviso;
  try {
    aviso = avisoDeAtualizacao(await invoke<ResultadoAtualizacao>('cmd_procurar_atualizacao'));
  } catch {
    aviso = avisoDeAtualizacao({ estado: 'erro', versao_atual: '' });
  }
  // resultado na faixa do alto da tela, com o botao de download quando ha versao nova
  mostrarResultadoManual(aviso.texto, aviso.link);
}
