// Estado do modal "Enviar feedback". Um único FeedbackModal fica montado em +page.svelte
// e qualquer ponto do app o abre por abrirFeedbackModal().

export const feedbackModal = $state({ aberto: false, versao: '?', sistema: '' });

export function abrirFeedbackModal(versao: string, sistema: string) {
  feedbackModal.versao = versao;
  feedbackModal.sistema = sistema;
  feedbackModal.aberto = true;
}

export function fecharFeedbackModal() {
  feedbackModal.aberto = false;
}
