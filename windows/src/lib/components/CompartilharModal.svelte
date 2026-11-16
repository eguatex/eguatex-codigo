<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { entrarComGoogle } from "$lib/conta-google.svelte";
  import X from "@lucide/svelte/icons/x";
  import Users from "@lucide/svelte/icons/users";
  import Link from "@lucide/svelte/icons/link";
  import Lock from "@lucide/svelte/icons/lock";
  import Girador from "./Girador.svelte";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import { t } from "$lib/i18n";
  import { textoErro } from "$lib/drive-erro";

  interface InfoDrive { conectado: boolean; configurado: boolean; email: string; nome: string; aviso?: string }
  interface Pessoa { id: string; email: string; nome: string; papel: string }
  interface Compartilhamento { publicado: boolean; link: string; link_aberto: boolean; pessoas: Pessoa[]; arquivo: string }

  interface Props {
    visivel: boolean;
    caminhoProjeto: string;
    nomeProjeto: string;
    onfechar?: () => void;
    /** Avisa a pagina que o projeto passou a sincronizar com o Drive. */
    onpublicado?: () => void;
    onsincronizar?: () => void;
  }

  let { visivel, caminhoProjeto, nomeProjeto, onfechar, onpublicado, onsincronizar }: Props = $props();

  let conta = $state<InfoDrive | null>(null);
  let dados = $state<Compartilhamento | null>(null);
  let emails = $state("");
  let papel = $state("writer");
  let mensagem = $state("");
  let ocupado = $state("");
  let erro = $state("");
  let copiado = $state(false);

  async function carregar() {
    erro = "";
    try {
      conta = await invoke<InfoDrive>("cmd_drive_info");
      if (conta.conectado) dados = await invoke<Compartilhamento>("cmd_drive_compartilhamento", { caminho: caminhoProjeto });
    } catch (e) {
      erro = textoErro(e);
    }
  }

  $effect(() => {
    if (visivel) void carregar();
  });

  async function fazer(rotulo: string, acao: () => Promise<void>) {
    ocupado = rotulo;
    erro = "";
    try {
      await acao();
    } catch (e) {
      erro = textoErro(e);
    } finally {
      ocupado = "";
    }
  }

  const entrar = () =>
    fazer("Aguardando o login no navegador...", async () => {
      await entrarComGoogle(); conta = await invoke<InfoDrive>("cmd_drive_info");
      dados = await invoke<Compartilhamento>("cmd_drive_compartilhamento", { caminho: caminhoProjeto });
    });

  const sair = () =>
    fazer("Saindo...", async () => {
      await invoke("cmd_drive_sair"); window.dispatchEvent(new Event("conta-google-mudou"));
      conta = await invoke<InfoDrive>("cmd_drive_info");
      dados = null;
    });

  function listaEmails(): string[] {
    return emails.split(/[,;\s]+/).map((e) => e.trim()).filter(Boolean);
  }

  const convidar = () =>
    fazer(dados?.publicado ? "Enviando convites..." : "Enviando o projeto para o Google Drive...", async () => {
      const antes = dados?.publicado;
      dados = await invoke<Compartilhamento>("cmd_drive_convidar", { caminho: caminhoProjeto, emails: listaEmails(), papel, mensagem });
      emails = "";
      mensagem = "";
      if (!antes) onpublicado?.();
    });

  const alternarLink = () =>
    fazer("Atualizando o link...", async () => {
      const antes = dados?.publicado;
      dados = await invoke<Compartilhamento>("cmd_drive_link", { caminho: caminhoProjeto, ligado: !dados?.link_aberto });
      if (!antes) onpublicado?.();
    });

  const remover = (p: Pessoa) =>
    fazer(`Removendo ${p.email}...`, async () => {
      dados = await invoke<Compartilhamento>("cmd_drive_remover_pessoa", { caminho: caminhoProjeto, permissao: p.id });
    });

  async function copiarLink() {
    if (!dados?.link) return;
    await navigator.clipboard.writeText(dados.link).catch(() => {});
    copiado = true;
    setTimeout(() => (copiado = false), 1500);
  }

  const NOME_PAPEL: Record<string, string> = { owner: "Dono", writer: "Pode editar", reader: "Só ver", commenter: "Comentar" };

  function tecla(e: KeyboardEvent) {
    if (visivel && e.key === "Escape" && !ocupado) onfechar?.();
  }
</script>

<svelte:window onkeydown={tecla} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fundo" onclick={() => !ocupado && onfechar?.()}>
    <div class="janela" role="dialog" aria-label="Compartilhar" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>Compartilhar {nomeProjeto}</h2>
        <button class="icone" onclick={onfechar} disabled={!!ocupado} aria-label="Fechar"><X size={18} /></button>
      </header>

      {#if conta === null}
        <p class="estado"><Girador tamanho={14} /> {t("tarefa.carregando")}</p>
      {:else if !conta.configurado}
        <p class="estado erro">O login com Google não está configurado nesta versão do ÉguaTex.</p>
      {:else if !conta.conectado}
        <section class="entrar">
          <Users size={32} />
          <p>O ÉguaTex compartilha pelo <strong>Google Drive</strong>. {t("drive.compartilhar-explica")} Não há limite de colaboradores.</p>
          {#if conta.aviso}<p class="estado erro">{conta.aviso}</p>{:else}<p class="miudo">{t("drive.permissao-aviso")}</p>{/if}
          <button class="primario" onclick={entrar} disabled={!!ocupado}>Entrar com Google</button>
        </section>
      {:else}
        <section>
          <label class="rotulo" for="emails">E-mails separados por vírgula</label>
          <div class="linha">
            <input id="emails" bind:value={emails} placeholder="nome@exemplo.com, outro@exemplo.com" disabled={!!ocupado}
              onkeydown={(e) => { if (e.key === "Enter" && listaEmails().length) convidar(); }} />
            <select bind:value={papel} disabled={!!ocupado} aria-label="Permissão">
              <option value="writer">Pode editar</option>
              <option value="reader">Só ver</option>
            </select>
            <button class="primario" onclick={convidar} disabled={!!ocupado || !listaEmails().length}>Convidar</button>
          </div>
          {#if listaEmails().length}
            <textarea bind:value={mensagem} rows="2" placeholder="Mensagem para o e-mail do convite (opcional)" disabled={!!ocupado}></textarea>
          {/if}
        </section>

        <section class="link">
          {#if dados?.link_aberto}
            <Link size={16} /> <span>Qualquer pessoa com o link pode editar.</span>
            <button class="texto" onclick={alternarLink} disabled={!!ocupado}>Desligar o link</button>
          {:else}
            <Lock size={16} /> <span>Só as pessoas convidadas têm acesso.</span>
            <button class="texto" onclick={alternarLink} disabled={!!ocupado}>Ligar o link</button>
          {/if}
        </section>

        <section>
          <h3>Acesso ao projeto</h3>
          {#if !dados?.publicado}
            <p class="miudo">Ninguém convidado ainda. Ao convidar a primeira pessoa, o projeto vai para o seu Google Drive como um arquivo .eguatex.zip.</p>
          {:else}
            {#if dados.arquivo}<p class="miudo">Arquivo no Drive: <strong>{dados.arquivo}</strong>. {t("drive.compartilhar-explica")}</p>{/if}
            <ul class="pessoas">
              {#each dados.pessoas as p (p.id)}
                <li>
                  <div class="quem">
                    <span class="nome">{p.nome || p.email}</span>
                    {#if p.nome}<span class="email">{p.email}</span>{/if}
                  </div>
                  <span class="papel">{NOME_PAPEL[p.papel] ?? p.papel}</span>
                  {#if p.papel !== "owner"}
                    <button class="icone" title="Remover acesso" aria-label="Remover acesso de {p.email}" onclick={() => remover(p)} disabled={!!ocupado}><Trash2 size={14} /></button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        {#if ocupado}
          <p class="estado"><Girador tamanho={14} /> {ocupado}</p>
        {/if}
      {/if}

      {#if erro}<p class="estado erro">{erro}</p>{/if}

      <footer>
        {#if conta?.conectado}
          <button class="texto" onclick={copiarLink} disabled={!dados?.link}><Link size={14} /> {copiado ? "Link copiado" : "Copiar link"}</button>
          {#if dados?.publicado}
            <button class="texto" onclick={() => onsincronizar?.()} disabled={!!ocupado}><RefreshCw size={14} /> Sincronizar agora</button>
          {/if}
          <span class="conta" title="Conta do Google Drive">{conta.email} <button class="texto miudo" onclick={sair} disabled={!!ocupado}>Sair</button></span>
        {/if}
        <button class="fechar" onclick={onfechar} disabled={!!ocupado}>Fechar</button>
      </footer>
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
    width: min(620px, 100%);
    max-height: calc(100vh - 32px);
    overflow: auto;
    border-radius: 10px;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }

  header,
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 20px;
  }

  header {
    border-bottom: 1px solid var(--borda);
  }

  header h2 {
    flex: 1;
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
  }

  footer {
    border-top: 1px solid var(--borda);
    flex-wrap: wrap;
  }

  section {
    padding: 14px 20px;
  }

  section + section {
    border-top: 1px solid var(--borda);
  }

  h3 {
    margin: 0 0 8px;
    font-size: 0.95rem;
  }

  .rotulo {
    display: block;
    font-size: 0.85rem;
    margin-bottom: 6px;
  }

  .linha {
    display: flex;
    gap: 8px;
  }

  input,
  select,
  textarea {
    font: inherit;
    font-size: 0.9rem;
    padding: 7px 10px;
    border-radius: 6px;
    border: 1px solid var(--borda);
    background: var(--superficie-1);
    color: var(--texto);
  }

  input {
    flex: 1;
    min-width: 0;
  }

  textarea {
    width: 100%;
    box-sizing: border-box;
    margin-top: 8px;
    resize: vertical;
  }

  .link {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.9rem;
  }

  .link span {
    flex: 1;
  }

  .pessoas {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .pessoas li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }

  .quem {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .nome {
    font-size: 0.9rem;
  }

  .email,
  .papel,
  .miudo,
  .conta {
    font-size: 0.8rem;
    opacity: 0.75;
  }

  .conta {
    margin-left: auto;
  }

  .entrar {
    text-align: center;
  }

  .entrar p {
    line-height: 1.5;
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
    white-space: pre-wrap;
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
    padding: 7px 16px;
    border: none;
    border-radius: 6px;
    background: var(--primaria);
    color: #fff;
    font-weight: 600;
  }

  .texto {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: none;
    background: transparent;
    color: var(--primaria);
    padding: 4px 6px;
    font-size: 0.875rem;
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

  .icone:hover:not(:disabled) {
    background: var(--superficie-2);
  }

  .fechar {
    padding: 7px 16px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: transparent;
    color: var(--texto);
  }
</style>
