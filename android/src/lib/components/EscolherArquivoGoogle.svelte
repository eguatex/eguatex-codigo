<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { entrarComGoogle } from "$lib/conta-google.svelte";
  import X from "@lucide/svelte/icons/x";
  import Girador from "./Girador.svelte";
  import IconeDrive from "./IconeDrive.svelte";
  import { t } from "$lib/i18n";
  import { textoErro } from "$lib/drive-erro";

  interface ArquivoGoogle { id: string; nome: string; modificado: string }
  interface InfoDrive { conectado: boolean; configurado: boolean; email: string; aviso?: string; seletor_pronto?: boolean }

  interface Props {
    visivel: boolean;
    /** "planilha" (Planilhas Google) ou "documento" (Documentos Google) */
    tipo: "planilha" | "documento";
    onescolher?: (arquivo: ArquivoGoogle) => void;
    onfechar?: () => void;
  }

  let { visivel, tipo, onescolher, onfechar }: Props = $props();

  let conta = $state<InfoDrive | null>(null);
  let erro = $state("");
  let aguardando = $state(false);

  const titulo = $derived(tipo === "planilha" ? "Importar do Google Planilhas" : "Importar do Google Documentos");

  async function abrir() {
    erro = "";
    try {
      conta = await invoke<InfoDrive>("cmd_drive_info");
    } catch (e) {
      erro = textoErro(e);
    }
  }

  async function entrar() {
    erro = "";
    try {
      await entrarComGoogle(); conta = await invoke<InfoDrive>("cmd_drive_info");
    } catch (e) {
      erro = textoErro(e);
    }
  }

  /** Abre o seletor do Google no navegador; o app so passa a ler o arquivo escolhido. */
  async function escolher() {
    erro = "";
    aguardando = true;
    try {
      const arquivo = await invoke<ArquivoGoogle | null>("cmd_drive_escolher", { tipo });
      if (arquivo) onescolher?.(arquivo);
    } catch (e) {
      erro = textoErro(e);
    } finally {
      aguardando = false;
    }
  }

  $effect(() => {
    if (visivel) void abrir();
  });
</script>

{#if visivel}
  <div class="fundo" role="presentation" onclick={(e) => e.target === e.currentTarget && !aguardando && onfechar?.()}>
    <div class="janela" role="dialog" aria-modal="true" aria-label={titulo}>
      <header>
        <IconeDrive size={18} />
        <h2>{titulo}</h2>
        <button class="fechar" onclick={onfechar} title="Fechar" aria-label="Fechar"><X size={18} /></button>
      </header>
      {#if conta && !conta.conectado}
        <section>
          <p>Entre com a sua conta Google para escolher um arquivo. {t("drive.seletor-explica")}</p>
          {#if conta.aviso}<p class="erro-aviso">{conta.aviso}</p>{/if}
          <button class="botao" onclick={entrar}><IconeDrive size={14} /> Entrar com Google</button>
        </section>
      {:else if conta}
        <section>
          <p>{t("drive.seletor-explica")}</p>
          <button class="botao" onclick={escolher} disabled={aguardando || !conta.seletor_pronto}>
            {#if aguardando}<Girador tamanho={14} />{:else}<IconeDrive size={14} />{/if}
            {aguardando ? t("drive.seletor-aguardando") : t("drive.seletor-botao")}
          </button>
          {#if !conta.seletor_pronto}<p class="vazio">{t("drive.seletor-indisponivel")}</p>{/if}
        </section>
      {/if}
      {#if erro}<p class="erro">{erro}</p>{/if}
    </div>
  </div>
{/if}

<style>
  .fundo {
    position: fixed;
    inset: 0;
    z-index: 2100;
    display: grid;
    place-items: center;
    padding: 16px;
    background: color-mix(in srgb, #000 40%, transparent);
  }
  .janela {
    width: min(520px, 100%);
    max-height: calc(100vh - 32px);
    overflow: auto;
    border-radius: 10px;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--borda);
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 1rem;
  }
  section {
    padding: 12px 18px;
  }
  p {
    margin: 0 0 10px;
    font-size: 0.875rem;
  }
  .vazio {
    color: var(--texto-secundario);
    font-size: 0.8125rem;
  }
  .botao {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .botao:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .fechar {
    display: flex;
    padding: 4px;
    border: none;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }
  .erro,
  .erro-aviso {
    color: var(--erro, #c62828);
    font-size: 0.8125rem;
  }
  .erro {
    margin: 0;
    padding: 0 18px 12px;
  }
</style>
