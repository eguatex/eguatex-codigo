<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { entrarComGoogle } from "$lib/conta-google.svelte";
  import X from "@lucide/svelte/icons/x";
  import Girador from "./Girador.svelte";
  import FolderDown from "@lucide/svelte/icons/folder-down";
  import { t } from "$lib/i18n";
  import { textoErro } from "$lib/drive-erro";

  interface InfoDrive { conectado: boolean; configurado: boolean; email: string; aviso?: string; seletor_pronto?: boolean }

  interface Props {
    visivel: boolean;
    /** Pasta onde os projetos baixados ficam (Documentos\EguaTex). */
    pastaProjetos: string;
    onfechar?: () => void;
    /** Projeto baixado: caminho local e nome. */
    onaberto?: (caminho: string, nome: string) => void;
  }

  let { visivel, pastaProjetos, onfechar, onaberto }: Props = $props();

  let conta = $state<InfoDrive | null>(null);
  let link = $state("");
  let ocupado = $state("");
  let erro = $state("");

  async function carregar() {
    erro = "";
    try {
      conta = await invoke<InfoDrive>("cmd_drive_info");
    } catch (e) {
      erro = textoErro(e);
    }
  }

  $effect(() => {
    if (visivel) void carregar();
  });

  async function entrar() {
    ocupado = "Aguardando o login no navegador...";
    erro = "";
    try {
      await entrarComGoogle(); conta = await invoke<InfoDrive>("cmd_drive_info");
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  /** Texto vazio abre o seletor do Google. Com um link, tenta o link e abre o seletor se o Drive negar (404). */
  async function baixar(idOuLink: string) {
    ocupado = idOuLink.trim() ? "Baixando o projeto..." : t("drive.seletor-aguardando");
    erro = "";
    try {
      const caminho = await invoke<string | null>("cmd_drive_baixar_projeto", { idOuLink, pastaPai: pastaProjetos });
      if (caminho) onaberto?.(caminho, caminho.split(/[\\/]/).pop() || "Projeto");
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  function tecla(e: KeyboardEvent) {
    if (visivel && e.key === "Escape" && !ocupado) onfechar?.();
  }
</script>

<svelte:window onkeydown={tecla} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fundo" onclick={() => !ocupado && onfechar?.()}>
    <div class="janela" role="dialog" aria-label="Compartilhados comigo" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>Compartilhados comigo</h2>
        <button class="icone" onclick={onfechar} disabled={!!ocupado} aria-label="Fechar"><X size={18} /></button>
      </header>

      {#if conta && !conta.conectado}
        <section class="centro">
          <p>Projetos que outras pessoas compartilharam com você pelo Google Drive.</p>
          {#if conta.aviso}<p class="aviso">{conta.aviso}</p>{/if}
          <button class="primario" onclick={entrar} disabled={!!ocupado || !conta.configurado}>Entrar com Google</button>
        </section>
      {:else if conta}
        <section>
          <p class="miudo">{t("drive.compartilhados-explica")}</p>
          <button class="primario" onclick={() => baixar("")} disabled={!!ocupado || !conta.seletor_pronto}>
            <FolderDown size={14} /> {t("drive.seletor-botao")}
          </button>
          {#if !conta.seletor_pronto}<p class="miudo">{t("drive.seletor-indisponivel")}</p>{/if}
        </section>
        <section>
          <label class="miudo" for="link-drive">Ou cole o link do arquivo que recebeu</label>
          <div class="linha">
            <input id="link-drive" bind:value={link} placeholder="https://drive.google.com/file/d/..." disabled={!!ocupado} />
            <button class="primario" onclick={() => baixar(link)} disabled={!!ocupado || !link.trim()}>Abrir</button>
          </div>
        </section>
      {/if}

      {#if ocupado}<p class="estado"><Girador tamanho={14} /> {ocupado}</p>{/if}
      {#if erro}<p class="estado erro">{erro}</p>{/if}
    </div>
  </div>
{/if}

<style>
  .fundo {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 16px;
    background: color-mix(in srgb, #000 40%, transparent);
  }

  .janela {
    width: min(560px, 100%);
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
    padding: 14px 20px;
    border-bottom: 1px solid var(--borda);
  }

  header h2 {
    flex: 1;
    margin: 0;
    font-size: 1.1rem;
  }

  section {
    padding: 14px 20px;
  }

  section + section {
    border-top: 1px solid var(--borda);
  }

  .centro {
    text-align: center;
  }

  .miudo {
    font-size: 0.8rem;
    opacity: 0.75;
  }

  .linha {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }

  input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.9rem;
    padding: 7px 10px;
    border-radius: 6px;
    border: 1px solid var(--borda);
    background: var(--superficie-1);
    color: var(--texto);
  }

  .estado {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 10px 20px;
    font-size: 0.9rem;
  }

  .erro {
    color: var(--perigo, #c0392b);
  }

  .aviso {
    color: var(--perigo, #c0392b);
    font-weight: 600;
  }

  button {
    font: inherit;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .primario {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 7px 14px;
    border: none;
    border-radius: 6px;
    background: var(--primaria);
    color: #fff;
    font-weight: 600;
  }

  .icone {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--texto);
  }
</style>
