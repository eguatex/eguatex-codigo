// Itens do menu Ajuda que falam com o sistema: "Enviar feedback" e "Procurar atualização".
// Os dois só agem quando a pessoa clica. Nenhum roda sozinho.

import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { avisoDeAtualizacao, type ResultadoAtualizacao } from './atualizacao';
import { sistemaDoUserAgent } from './feedback';
import { abrirFeedbackModal } from './feedback-modal.svelte';
import { t } from './i18n';

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
  if (aviso.link) {
    if (confirm(`${aviso.texto}\n\n${t('atualizacao.abrir-pergunta')}`)) {
      openUrl(aviso.link).catch((e) => console.error('Erro ao abrir o link:', e));
    }
  } else {
    alert(aviso.texto);
  }
}
