<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Heart from "@lucide/svelte/icons/heart";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import { invoke } from "@tauri-apps/api/core";
  import { t, tf } from "$lib/i18n";

  interface Props {
    visivel?: boolean;
    onfechar?: () => void;
    onfeedback?: () => void;
    onapoiar?: () => void;
  }

  let { visivel = false, onfechar, onfeedback, onapoiar }: Props = $props();

  let versao = $state("");
  let detalhes = $state("");
  let motores = $state<string[]>([]);
  let distribuicao = $state("");

  const NOMES: Record<string, string> = {
    pdflatex: "pdfLaTeX",
    xelatex: "XeLaTeX",
    lualatex: "LuaLaTeX",
    latexmk: "latexmk",
    bibtex: "BibTeX",
    biber: "Biber",
  };

  $effect(() => {
    if (!visivel) return;
    invoke<string>("cmd_versao").then((v) => (versao = v)).catch(() => (versao = "ÉguaTex"));
    invoke<{ compilacao: string; tauri: string }>("cmd_detalhes_tecnicos")
      .then((d) => (detalhes = tf("sobre.detalhes", d)))
      .catch(() => (detalhes = ""));
    invoke<{ distribuicao?: string | null; motores: Array<{ nome: string }> }>("cmd_detectar_motores")
      .then((info) => {
        distribuicao = info.distribuicao ?? "";
        motores = info.motores.map((m) => NOMES[m.nome.toLowerCase()] ?? m.nome);
      })
      .catch(() => (motores = []));
  });

  const linhaMotores = $derived.by(() => {
    if (motores.length === 0) return t("sobre.sem-motores");
    const lista = distribuicao ? `${distribuicao} (${motores.join(", ")})` : motores.join(", ");
    return tf("sobre.motores", { lista });
  });
</script>

<svelte:window onkeydown={(e) => { if (visivel && e.key === "Escape") onfechar?.(); }} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="fundo-modal" onclick={onfechar}>
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="titulo-sobre" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2 id="titulo-sobre">{t("sobre.titulo")}</h2>
        <button class="fechar" onclick={onfechar} title={t("sobre.fechar")} aria-label={t("sobre.fechar")}><X size={18} /></button>
      </header>
      <div class="corpo">
        <div class="cartoes">
          <button class="cartao" onclick={() => { onfechar?.(); onfeedback?.(); }}>
            <MessageSquare size={20} />
            <div>
              <strong>{t("sobre.feedback-titulo")}</strong>
              <span>{t("sobre.feedback-descricao")}</span>
            </div>
          </button>
          <button class="cartao" onclick={() => { onfechar?.(); onapoiar?.(); }}>
            <Heart size={20} />
            <div>
              <strong>{t("sobre.apoiar-titulo")}</strong>
              <span>{t("sobre.apoiar-descricao")}</span>
            </div>
          </button>
        </div>
        <p class="versao">{versao} · {t("sobre.autor")}</p>
        <p class="nota">{t("sobre.licenca")}</p>
        <p class="motores" class:ok={motores.length > 0}>{linhaMotores}</p>
        {#if detalhes}<p class="nota detalhes">{detalhes}</p>{/if}
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
  .cartoes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .cartao {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    text-align: left;
    background: var(--superficie-1);
    border: 1px solid var(--borda);
    border-radius: 8px;
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .cartao:hover {
    background: var(--hover);
  }
  .cartao div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .cartao span {
    font-size: 0.8125rem;
    color: var(--texto-secundario);
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
