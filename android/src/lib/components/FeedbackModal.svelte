<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Send from "@lucide/svelte/icons/send";
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import { t, tf } from "$lib/i18n";
  import { registrarFechavel } from "$lib/voltar";
  import { FEEDBACK_URL } from "$lib/site";
  import { feedbackModal, fecharFeedbackModal } from "$lib/feedback-modal.svelte";
  import {
    TIPOS,
    LIMITE_MENSAGEM,
    montarPayload,
    erroDosCampos,
    enviarFeedback,
    type TipoFeedback,
  } from "$lib/feedback-envio";

  let tipo = $state<TipoFeedback>("sugestao");
  let mensagem = $state("");
  let nome = $state("");
  let enviando = $state(false);
  let resultado = $state<{ ok: boolean; texto: string } | null>(null);
  let areaMensagem: HTMLTextAreaElement | undefined = $state();

  const vazia = $derived(mensagem.trim() === "");

  // a cada abertura o formulário volta ao início e o foco vai para a mensagem
  $effect(() => {
    if (!feedbackModal.aberto) return;
    tipo = "sugestao";
    mensagem = "";
    nome = "";
    enviando = false;
    resultado = null;
    // no toque o foco abriria o teclado de cara e cobriria o formulario: a pessoa toca no campo
    if (!window.matchMedia("(pointer: coarse)").matches) queueMicrotask(() => areaMensagem?.focus());
  });

  // o botao Voltar do Android fecha o formulario em vez de sair do app
  $effect(() => {
    if (feedbackModal.aberto) return registrarFechavel(fecharFeedbackModal);
  });

  // com o teclado aberto, o campo tocado rola para a vista (a tela deitada tem 360 px de altura)
  function rolarParaVista(e: FocusEvent) {
    const campo = e.currentTarget as HTMLElement;
    setTimeout(() => campo.scrollIntoView({ block: "center", behavior: "smooth" }), 250);
  }

  function aoTeclar(e: KeyboardEvent) {
    if (feedbackModal.aberto && e.key === "Escape") fecharFeedbackModal();
  }

  async function enviar() {
    if (enviando || vazia) return;
    const campos = { tipo, mensagem, nome };
    const erro = erroDosCampos(campos);
    if (erro) {
      resultado = { ok: false, texto: t(erro) };
      return;
    }
    enviando = true;
    resultado = null;
    const r = await enviarFeedback(montarPayload(campos, feedbackModal.versao, feedbackModal.sistema));
    enviando = false;
    if (r === "erro") resultado = { ok: false, texto: t("feedback.erro-envio") };
    else resultado = { ok: true, texto: t(r === "enviado" ? "feedback.ok-enviado" : r === "guardado" ? "feedback.ok-guardado" : "feedback.ok-mailto") };
  }
</script>

<svelte:window onkeydown={aoTeclar} />

{#if feedbackModal.aberto}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fb-overlay" data-modal-fundo onclick={fecharFeedbackModal}>
    <div class="fb-caixa" data-modal-caixa role="dialog" aria-modal="true" aria-labelledby="fb-titulo" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="fb-cabecalho" data-modal-cabecalho>
        <h2 id="fb-titulo">{t("feedback.modal-titulo")}</h2>
        <button class="fb-fechar" data-modal-fechar onclick={fecharFeedbackModal} aria-label={t("feedback.fechar")}><X size={18} /></button>
      </div>

      <div class="fb-corpo" data-modal-corpo>
        <p class="fb-intro">{t("feedback.modal-intro")}</p>

        <div class="fb-campo">
          <span class="fb-rotulo" id="fb-tipo">{t("feedback.tipo")}</span>
          <div class="fb-chips" role="radiogroup" aria-labelledby="fb-tipo">
            {#each TIPOS as opcao}
              <button
                type="button"
                class="fb-chip"
                class:ativo={tipo === opcao}
                role="radio"
                aria-checked={tipo === opcao}
                onclick={() => (tipo = opcao)}
              >{t(`feedback.tipo-${opcao}` as "feedback.tipo-outro")}</button>
            {/each}
          </div>
        </div>

        <div class="fb-campo">
          <label class="fb-rotulo" for="fb-mensagem">{t("feedback.mensagem")}</label>
          <textarea
            id="fb-mensagem"
            bind:this={areaMensagem}
            bind:value={mensagem}
            onfocus={rolarParaVista}
            maxlength={LIMITE_MENSAGEM}
            rows="6"
            placeholder={t("feedback.mensagem-placeholder")}
          ></textarea>
          <span class="fb-contador">{tf("feedback.contador", { n: mensagem.length, max: LIMITE_MENSAGEM })}</span>
        </div>

        <div class="fb-campo">
          <label class="fb-rotulo" for="fb-nome">{t("feedback.nome")}</label>
          <input id="fb-nome" type="text" bind:value={nome} onfocus={rolarParaVista} placeholder={t("feedback.nome-placeholder")} autocomplete="off" />
        </div>

        {#if !FEEDBACK_URL}<p class="fb-rodape-texto">{t("feedback.aviso-email")}</p>{/if}

        {#if resultado}
          <div class="fb-aviso" class:ok={resultado.ok} role={resultado.ok ? "status" : "alert"}>
            {#if resultado.ok}<CircleCheck size={16} />{:else}<CircleAlert size={16} />{/if}
            <span>{resultado.texto}</span>
          </div>
        {/if}
      </div>

      <div class="fb-botoes">
        {#if resultado?.ok}
          <button class="fb-enviar" onclick={fecharFeedbackModal}>{t("feedback.fechar")}</button>
        {:else}
          <button class="fb-cancelar" onclick={fecharFeedbackModal}>{t("feedback.cancelar")}</button>
          <button class="fb-enviar" onclick={enviar} disabled={vazia || enviando}>
            <Send size={15} /> {enviando ? t("feedback.enviando") : t("feedback.enviar")}
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .fb-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    backdrop-filter: blur(4px);
  }
  .fb-caixa {
    background: var(--superficie-0, #1e1e2e);
    border: 1px solid var(--borda, #313244);
    border-radius: 12px;
    width: 100%;
    max-width: 520px;
    max-height: 92vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
    color: var(--texto, #cdd6f4);
  }
  .fb-cabecalho {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    border-bottom: 1px solid var(--borda, #313244);
  }
  .fb-cabecalho h2 { font-size: 1.125rem; font-weight: 600; margin: 0; }
  .fb-fechar {
    background: transparent;
    border: none;
    color: var(--texto-secundario, #a6adc8);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
  }
  .fb-fechar:hover { background: var(--hover, #313244); color: var(--texto, #cdd6f4); }
  .fb-corpo { padding: 16px 20px; overflow-y: auto; display: flex; flex-direction: column; gap: 14px; }
  .fb-intro { margin: 0; font-size: 0.875rem; line-height: 1.5; color: var(--texto-secundario, #a6adc8); }
  .fb-campo { display: flex; flex-direction: column; gap: 5px; flex: 1; min-width: 0; }
  .fb-rotulo { font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: var(--texto-secundario, #a6adc8); }
  .fb-chips { display: flex; flex-wrap: wrap; gap: 8px; }
  .fb-chip {
    background: var(--superficie-1, #181825);
    border: 1px solid var(--borda, #313244);
    color: var(--texto, #cdd6f4);
    border-radius: 999px;
    padding: 5px 14px;
    font-size: 0.8125rem;
    cursor: pointer;
  }
  .fb-chip:hover { background: var(--hover, #313244); }
  .fb-chip.ativo { background: #a6e3a1; border-color: #a6e3a1; color: #11111b; font-weight: 600; }
  textarea, input {
    background: var(--superficie-1, #181825);
    border: 1px solid var(--borda, #313244);
    border-radius: 8px;
    color: var(--texto, #cdd6f4);
    font: inherit;
    font-size: 0.875rem;
    padding: 8px 10px;
    width: 100%;
    box-sizing: border-box;
  }
  textarea { resize: vertical; min-height: 110px; }
  textarea:focus, input:focus { outline: 2px solid #89b4fa; outline-offset: -1px; }
  .fb-contador { align-self: flex-end; font-size: 0.75rem; color: var(--texto-secundario, #a6adc8); }
  .fb-rodape-texto { margin: 0; font-size: 0.75rem; line-height: 1.4; color: var(--texto-secundario, #a6adc8); }
  .fb-aviso {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 9px 12px;
    border-radius: 8px;
    font-size: 0.8125rem;
    background: rgba(243, 139, 168, 0.15);
    color: #f38ba8;
  }
  .fb-aviso.ok { background: rgba(166, 227, 161, 0.15); color: #a6e3a1; }
  .fb-botoes {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 12px 20px;
    border-top: 1px solid var(--borda, #313244);
  }
  .fb-cancelar, .fb-enviar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    padding: 7px 16px;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
  }
  .fb-cancelar { background: transparent; border: 1px solid var(--borda, #313244); color: var(--texto, #cdd6f4); }
  .fb-cancelar:hover { background: var(--hover, #313244); }
  .fb-enviar { background: #89b4fa; border: none; color: #11111b; }
  .fb-enviar:hover:not(:disabled) { background: #b4befe; }
  .fb-enviar:disabled { opacity: 0.45; cursor: not-allowed; }

  /* toque: alvos de 44 px nos chips, nos campos e nos botoes */
  @media (pointer: coarse) {
    .fb-chip { min-height: 44px; padding: 0 18px; }
    .fb-cancelar, .fb-enviar { min-height: 44px; }
    input { min-height: 44px; font-size: 1rem; }
    textarea { font-size: 1rem; }
  }

  /* a regra comum de tela cheia (+page.svelte) cuida do tamanho; aqui o rodape de botoes fica fixo */
  .fb-botoes { flex: none; }
</style>
