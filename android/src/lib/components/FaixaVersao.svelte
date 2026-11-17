<script lang="ts">
  // Faixa discreta no alto da tela: oferta do aviso de versao nova e resultado da consulta.
  // Nao bloqueia nada: fica por cima, sem fundo escurecido.
  import X from "@lucide/svelte/icons/x";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n";
  import { faixaVersao, ligarAviso, agoraNao, fecharFaixa } from "$lib/aviso-versao.svelte";

  const faixa = $derived(faixaVersao.atual);

  function abrir(link: string) {
    openUrl(link).catch((e) => console.error("Erro ao abrir o link:", e));
  }
</script>

{#if faixa}
  <div class="faixa-versao" role="status">
    {#if faixa.tipo === "oferta"}
      <span>{t("aviso-versao.pergunta")}</span>
      <button class="primario" onclick={ligarAviso}>{t("aviso-versao.ligar")}</button>
      <button onclick={agoraNao}>{t("aviso-versao.agora-nao")}</button>
    {:else if faixa.tipo === "pronto"}
      <span>{t("aviso-versao.pronto")}</span>
    {:else}
      {@const link = faixa.link}
      <span class="textos">
        <span>{faixa.texto}</span>
        {#if faixa.oferecerAviso}<span class="extra">{t("aviso-versao.oferta-manual")}</span>{/if}
      </span>
      {#if link}<button class="primario" onclick={() => abrir(link)}>{t("aviso-versao.baixar")}</button>{/if}
      {#if faixa.oferecerAviso}<button onclick={ligarAviso}>{t("aviso-versao.ligar")}</button>{/if}
      <button class="fechar" onclick={fecharFaixa} title={t("aviso-versao.fechar")} aria-label={t("aviso-versao.fechar")}><X size={14} /></button>
    {/if}
  </div>
{/if}

<style>
  .faixa-versao {
    position: fixed;
    top: 8px;
    left: 0;
    right: 0;
    margin: 0 auto;
    width: fit-content;
    box-sizing: border-box;
    z-index: 1500;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 10px;
    max-width: min(720px, calc(100vw - 24px));
    padding: 8px 12px;
    border-radius: 8px;
    background: var(--superficie-1, #2a2a2a);
    color: var(--texto, #e8e8e8);
    border: 1px solid var(--borda, #444);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25);
    font-size: 0.85rem;
  }
  .faixa-versao > span:first-child,
  .textos {
    flex: 1 1 220px;
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .extra {
    opacity: 0.8;
  }
  button {
    flex-shrink: 0;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--borda, #555);
    background: transparent;
    color: inherit;
    cursor: pointer;
    font: inherit;
  }
  button.primario {
    background: var(--acento, #8e24aa);
    border-color: var(--acento, #8e24aa);
    color: #fff;
  }
  button.fechar {
    padding: 4px;
    border: none;
    display: inline-flex;
  }
</style>
