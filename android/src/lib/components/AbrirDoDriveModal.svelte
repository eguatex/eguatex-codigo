<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { entrarComGoogle } from "$lib/conta-google.svelte";
  import X from "@lucide/svelte/icons/x";
  import Girador from "./Girador.svelte";
  import FolderDown from "@lucide/svelte/icons/folder-down";
  import { t } from "$lib/i18n";
  import { textoErro } from "$lib/drive-erro";

  interface InfoDrive { conectado: boolean; configurado: boolean; email: string; aviso?: string; seletor_pronto?: boolean }

  /** O que o Rust trouxe do Drive (cmd_drive_abrir_arquivos). */
  interface ResultadoAbrir {
    caminho: string;
    novo: boolean;
    adicionados: string[];
    vinculados: number;
    avisos: string[];
    projeto_do_drive: boolean;
    /** .tex com \documentclass que vira o principal do projeto (caminho relativo). */
    principal_sugerido?: string | null;
  }

  interface Props {
    visivel: boolean;
    /** "novo" cria um projeto em `pastaProjetos`; senao, o caminho do projeto aberto. */
    destino: string;
    /** Pasta onde os projetos ficam (Documentos\EguaTex). */
    pastaProjetos: string;
    /**
     * Com a conta conectada e o seletor pronto, abre o seletor do Google na hora, sem a tela de
     * apresentacao. Sem login, sem seletor ou com erro, a tela de sempre aparece.
     */
    direto?: boolean;
    onfechar?: () => void;
    /** Os arquivos chegaram e a pessoa clicou em continuar. */
    onconcluir?: (r: ResultadoAbrir) => void;
  }

  let { visivel, destino, pastaProjetos, direto = false, onfechar, onconcluir }: Props = $props();

  let conta = $state<InfoDrive | null>(null);
  let ocupado = $state("");
  let erro = $state("");
  let resultado = $state<ResultadoAbrir | null>(null);
  // modo direto: o seletor ja abriu no navegador e so um spinner aparece
  let esperandoSeletor = $state(false);

  async function carregar() {
    erro = "";
    resultado = null;
    try {
      conta = await invoke<InfoDrive>("cmd_drive_info");
    } catch (e) {
      erro = textoErro(e);
      return;
    }
    if (direto && conta.conectado && conta.seletor_pronto) {
      esperandoSeletor = true;
      const r = await escolher();
      esperandoSeletor = false;
      // null: a pessoa cancelou no seletor, e nao ha o que mostrar
      if (r === null) onfechar?.();
    }
  }

  $effect(() => {
    if (visivel) void carregar();
  });

  async function entrar() {
    ocupado = t("drive.abrir-aguardando");
    erro = "";
    try {
      await entrarComGoogle();
      conta = await invoke<InfoDrive>("cmd_drive_info");
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  /** Abre o seletor. Devolve o resultado, `null` se a pessoa cancelou ou `undefined` se deu erro. */
  async function escolher(): Promise<ResultadoAbrir | null | undefined> {
    ocupado = t("drive.abrir-aguardando");
    erro = "";
    try {
      const r = await invoke<ResultadoAbrir | null>("cmd_drive_abrir_arquivos", { destino, pastaPai: pastaProjetos });
      if (r) resultado = r;
      return r;
    } catch (e) {
      erro = textoErro(e);
      return undefined;
    } finally {
      ocupado = "";
    }
  }

  function concluir() {
    const r = resultado;
    resultado = null;
    if (r) onconcluir?.(r);
  }

  function tecla(e: KeyboardEvent) {
    if (visivel && e.key === "Escape" && !ocupado) onfechar?.();
  }
</script>

<svelte:window onkeydown={tecla} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fundo" onclick={() => !ocupado && onfechar?.()}>
    <div class="janela" role="dialog" aria-label={t("drive.abrir-titulo")} tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>{t("drive.abrir-titulo")}</h2>
        <button class="icone" onclick={onfechar} disabled={!!ocupado} aria-label="Fechar"><X size={18} /></button>
      </header>

      {#if resultado}
        <section>
          <p><strong>{t("drive.abrir-pronto")}</strong> {resultado.adicionados.length}</p>
          {#if resultado.adicionados.length}
            <ul class="lista">
              {#each resultado.adicionados.slice(0, 12) as nome}<li>{nome}</li>{/each}
              {#if resultado.adicionados.length > 12}<li>...</li>{/if}
            </ul>
          {/if}
          {#if resultado.vinculados > 0}<p class="miudo">{t("drive.abrir-volta")}</p>{/if}
          {#each resultado.avisos as aviso}<p class="miudo aviso-item">{aviso}</p>{/each}
          <button class="primario" onclick={concluir}>{resultado.novo ? t("drive.abrir-continuar") : t("drive.abrir-fechar")}</button>
        </section>
      {:else if esperandoSeletor}
        <section class="centro">
          <p class="espera"><Girador tamanho={14} /> {t("drive.abrir-seletor-aberto")}</p>
        </section>
      {:else if conta && !conta.conectado}
        <section class="centro">
          <p>{t("drive.abrir-aviso")}</p>
          <p class="miudo">{t("drive.entrar-para-abrir")}</p>
          {#if conta.aviso}<p class="aviso">{conta.aviso}</p>{/if}
          <button class="primario" onclick={entrar} disabled={!!ocupado || !conta.configurado}>Entrar com Google</button>
        </section>
      {:else if conta}
        <section>
          <p>{t("drive.abrir-aviso")}</p>
          <button class="primario" onclick={escolher} disabled={!!ocupado || !conta.seletor_pronto}>
            <FolderDown size={14} /> {t("drive.abrir-escolher")}
          </button>
          {#if !conta.seletor_pronto}<p class="miudo">{t("drive.seletor-indisponivel")}</p>{/if}
        </section>
      {/if}

      {#if ocupado && !esperandoSeletor}<p class="estado"><Girador tamanho={14} /> {ocupado}</p>{/if}
      {#if erro}
        <p class="estado erro">{erro}</p>
        <p class="estado miudo">{t("drive.abrir-detalhe")}</p>
      {/if}
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

  .centro {
    text-align: center;
  }

  .espera {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin: 0;
    padding: 10px 0;
    font-size: 0.9rem;
  }

  .miudo {
    font-size: 0.8rem;
    opacity: 0.75;
  }

  .lista {
    margin: 4px 0 8px;
    padding-left: 20px;
    font-size: 0.85rem;
    max-height: 160px;
    overflow: auto;
  }

  .aviso-item {
    opacity: 1;
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
