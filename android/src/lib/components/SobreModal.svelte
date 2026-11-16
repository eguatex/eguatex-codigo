<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { invoke } from "@tauri-apps/api/core";
  import { t, tf } from "$lib/i18n";
  import { registrarFechavel } from "$lib/voltar";
  import CartoesApoio from "./CartoesApoio.svelte";

  interface Props {
    visivel?: boolean;
    onfechar?: () => void;
    onfeedback?: () => void;
    onapoiar?: () => void;
  }

  let { visivel = false, onfechar, onfeedback, onapoiar }: Props = $props();

  let versao = $state("");
  let detalhes = $state("");

  $effect(() => {
    if (!visivel) return;
    invoke<string>("cmd_versao").then((v) => (versao = v)).catch(() => (versao = "ÉguaTex"));
    invoke<{ compilacao: string; tauri: string }>("cmd_detalhes_tecnicos")
      .then((d) => (detalhes = tf("sobre.detalhes", d)))
      .catch(() => (detalhes = ""));
  });

  // o botao Voltar do Android fecha o Sobre em vez de sair do app
  $effect(() => {
    if (visivel) return registrarFechavel(() => onfechar?.());
  });
</script>

<svelte:window onkeydown={(e) => { if (visivel && e.key === "Escape") onfechar?.(); }} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fundo-modal" data-modal-fundo onclick={onfechar}>
    <div class="modal" data-modal-caixa role="dialog" aria-modal="true" aria-labelledby="titulo-sobre" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header data-modal-cabecalho>
        <h2 id="titulo-sobre">{t("sobre.titulo")}</h2>
        <button class="fechar" data-modal-fechar onclick={onfechar} title={t("sobre.fechar")} aria-label={t("sobre.fechar")}><X size={18} /></button>
      </header>
      <div class="corpo" data-modal-corpo>
        <CartoesApoio
          onfeedback={() => { onfechar?.(); onfeedback?.(); }}
          onapoiar={() => { onfechar?.(); onapoiar?.(); }}
        />
        <p class="versao">{versao} · {t("sobre.autor")}</p>
        <p class="nota">{t("sobre.licenca")}</p>
        {#if detalhes}<p class="nota detalhes">{detalhes}</p>{/if}
        <p class="motores ok">{t("sobre.motores-android")}</p>
      </div>
    </div>
  </div>
{/if}

<style>
  .fundo-modal {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }
  .modal {
    width: min(560px, 92vw);
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 8px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid var(--borda);
  }
  h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .fechar {
    background: transparent;
    border: none;
    color: var(--texto-secundario);
    cursor: pointer;
    display: flex;
    padding: 4px;
    border-radius: 4px;
  }
  .fechar:hover {
    background: var(--hover);
    color: var(--texto);
  }
  .corpo {
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .versao,
  .nota,
  .motores {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
    text-align: center;
  }
  .motores {
    padding: 8px 12px;
    border: 1px solid var(--borda);
    border-radius: 6px;
  }
  .motores.ok {
    color: var(--sucesso, var(--texto));
  }
</style>
