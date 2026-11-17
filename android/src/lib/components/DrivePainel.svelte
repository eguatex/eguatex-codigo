<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { entrarComGoogle } from "$lib/conta-google.svelte";
  import X from "@lucide/svelte/icons/x";
  import Girador from "./Girador.svelte";
  import CloudUpload from "@lucide/svelte/icons/cloud-upload";
  import FolderDown from "@lucide/svelte/icons/folder-down";
  import Users from "@lucide/svelte/icons/users";
  import Check from "@lucide/svelte/icons/check";
  import IconeDrive from "./IconeDrive.svelte";
  import { t } from "$lib/i18n";
  import { textoErro } from "$lib/drive-erro";

  interface InfoDrive { conectado: boolean; configurado: boolean; email: string; aviso?: string }
  interface ProjetoRemoto { id: string; nome: string; dono: string; modificado: string }
  interface ProjetoLocal { nome: string; caminho: string }

  interface Props {
    visivel: boolean;
    /** Projetos do computador (lista da tela inicial). */
    projetos: ProjetoLocal[];
    /** Pasta onde os projetos baixados ficam (Documentos\EguaTex). */
    pastaProjetos: string;
    onfechar?: () => void;
    /** Projeto baixado: caminho local e nome. */
    onaberto?: (caminho: string, nome: string) => void;
    /** Abre a lista de projetos que outras pessoas compartilharam. */
    oncompartilhados?: () => void;
  }

  let { visivel, projetos, pastaProjetos, onfechar, onaberto, oncompartilhados }: Props = $props();

  let conta = $state<InfoDrive | null>(null);
  /** id da pasta no Drive de cada projeto local que ja esta la */
  let noDrive = $state<Record<string, string>>({});
  let remotos = $state<ProjetoRemoto[] | null>(null);
  let ocupado = $state("");
  let erro = $state("");
  let aviso = $state("");

  async function carregar() {
    erro = "";
    aviso = "";
    remotos = null;
    try {
      conta = await invoke<InfoDrive>("cmd_drive_info");
      const ids: Record<string, string> = {};
      for (const p of projetos) {
        const id = await invoke<string | null>("cmd_drive_projeto", { caminho: p.caminho }).catch(() => null);
        if (id) ids[p.caminho] = id;
      }
      noDrive = ids;
      if (conta.conectado) remotos = await invoke<ProjetoRemoto[]>("cmd_drive_meus_projetos");
    } catch (e) {
      erro = textoErro(e);
    }
  }

  $effect(() => {
    if (visivel) void carregar();
  });

  async function entrar() {
    ocupado = "entrar";
    erro = "";
    try {
      await entrarComGoogle(); conta = await invoke<InfoDrive>("cmd_drive_info");
      await carregar();
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  async function sair() {
    await invoke("cmd_drive_sair").catch(() => {}); window.dispatchEvent(new Event("conta-google-mudou"));
    conta = await invoke<InfoDrive>("cmd_drive_info");
    remotos = null;
  }

  async function salvarNoDrive(p: ProjetoLocal) {
    ocupado = p.caminho;
    erro = "";
    aviso = "";
    try {
      const id = await invoke<string>("cmd_drive_enviar_projeto", { caminho: p.caminho });
      noDrive = { ...noDrive, [p.caminho]: id };
      aviso = `${p.nome} foi salvo no Google Drive. Enquanto ele estiver aberto, as mudanças sobem sozinhas.`;
      remotos = await invoke<ProjetoRemoto[]>("cmd_drive_meus_projetos");
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  async function baixar(r: ProjetoRemoto) {
    ocupado = r.id;
    erro = "";
    try {
      // se o Drive negar o id (404), o Rust abre o seletor do Google
      const caminho = await invoke<string | null>("cmd_drive_baixar_projeto", { idOuLink: r.id, pastaPai: pastaProjetos });
      if (caminho) onaberto?.(caminho, caminho.split(/[\\/]/).pop() || r.nome);
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  // projetos do Drive que ainda nao estao neste computador
  const soNoDrive = $derived(
    (remotos ?? []).filter((r) => !Object.values(noDrive).includes(r.id)),
  );

  function data(iso: string) {
    const d = new Date(iso);
    return isNaN(d.getTime()) ? "" : d.toLocaleDateString("pt-BR");
  }
</script>

{#if visivel}
  <div class="fundo" role="presentation" onclick={(e) => e.target === e.currentTarget && onfechar?.()}>
    <div class="janela" role="dialog" aria-modal="true" aria-labelledby="titulo-drive">
      <header>
        <IconeDrive size={20} />
        <h2 id="titulo-drive">Google Drive</h2>
        <button class="fechar" onclick={onfechar} title="Fechar" aria-label="Fechar"><X size={18} /></button>
      </header>

      {#if conta === null}
        <section class="centro"><Girador tamanho={18} /> Carregando...</section>
      {:else if !conta.configurado}
        <section><p>Esta versão do ÉguaTeX não tem a ligação com o Google configurada.</p></section>
      {:else if !conta.conectado}
        <section>
          <p>
            Entre com a sua conta Google para salvar projetos no Drive, abrir no outro computador e escrever junto
            com outras pessoas. Nada é enviado antes de você clicar em "Salvar no Drive".
          </p>
          {#if conta.aviso}<p class="erro">{conta.aviso}</p>{:else}<p class="vazio">{t("drive.permissao-aviso")}</p>{/if}
          <button class="botao primario" onclick={entrar} disabled={ocupado === "entrar"}>
            {#if ocupado === "entrar"}<Girador tamanho={16} />{:else}<IconeDrive size={16} />{/if}
            Entrar com Google
          </button>
        </section>
      {:else}
        <section class="conta">
          <span>Conectado como <strong>{conta.email}</strong></span>
          <button class="ligacao" onclick={sair}>Sair</button>
        </section>

        <section>
          <h3>Neste computador</h3>
          {#if projetos.length === 0}
            <p class="vazio">Nenhum projeto ainda.</p>
          {:else}
            <ul>
              {#each projetos as p (p.caminho)}
                <li>
                  <span class="nome" title={p.caminho}>{p.nome}</span>
                  {#if noDrive[p.caminho]}
                    <span class="estado"><Check size={14} /> No Drive</span>
                  {:else}
                    <button class="botao" onclick={() => salvarNoDrive(p)} disabled={!!ocupado}>
                      {#if ocupado === p.caminho}<Girador tamanho={14} />{:else}<CloudUpload size={14} />{/if}
                      Salvar no Drive
                    </button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        <section>
          <h3>Só no Drive</h3>
          {#if remotos === null}
            <p class="vazio"><Girador tamanho={14} /> Procurando...</p>
          {:else if soNoDrive.length === 0}
            <p class="vazio">Todos os seus projetos do Drive já estão neste computador.</p>
          {:else}
            <ul>
              {#each soNoDrive as r (r.id)}
                <li>
                  <span class="nome">{r.nome}</span>
                  <span class="data">{data(r.modificado)}</span>
                  <button class="botao" onclick={() => baixar(r)} disabled={!!ocupado}>
                    {#if ocupado === r.id}<Girador tamanho={14} />{:else}<FolderDown size={14} />{/if}
                    Abrir
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        <section>
          <button class="botao" onclick={oncompartilhados}><Users size={14} /> Projetos compartilhados comigo</button>
        </section>
      {/if}

      {#if aviso}<p class="aviso">{aviso}</p>{/if}
      {#if erro}<p class="erro">{erro}</p>{/if}
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
    width: min(600px, 100%);
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

  h3 {
    margin: 0 0 8px;
    font-size: 0.9rem;
  }

  p {
    margin: 0 0 12px;
    font-size: 0.875rem;
    line-height: 1.5;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.875rem;
  }

  .nome {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .data,
  .vazio {
    color: var(--texto-secundario);
    font-size: 0.8125rem;
  }

  .estado {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--sucesso, #2e7d32);
    font-size: 0.8125rem;
  }

  .conta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.875rem;
  }

  .centro {
    display: flex;
    align-items: center;
    gap: 8px;
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
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .botao:hover:not(:disabled) {
    background: var(--hover);
  }

  .botao:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .botao.primario {
    background: var(--primaria);
    border-color: var(--primaria);
    color: #fff;
  }

  .ligacao {
    border: none;
    background: none;
    color: var(--primaria);
    cursor: pointer;
    font: inherit;
  }

  .fechar {
    display: flex;
    padding: 4px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .aviso,
  .erro {
    margin: 0;
    padding: 10px 20px 14px;
    font-size: 0.8125rem;
  }

  .erro {
    color: var(--erro, #c62828);
    white-space: pre-wrap;
  }


</style>
