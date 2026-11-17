<script lang="ts">
  // Conta Google unica do ÉguaTeX: um login libera o Google Drive, o compartilhamento e a
  // abertura de arquivos do Google (o assistente de IA usa chave propria). O estado e as acoes ficam em
  // $lib/conta-google.svelte; aqui so ficam o botao (barra lateral ou icone) e o cartao da conta.
  import UserRound from "@lucide/svelte/icons/user-round";
  import LogOut from "@lucide/svelte/icons/log-out";
  import Check from "@lucide/svelte/icons/check";
  import X from "@lucide/svelte/icons/x";
  import CartoesApoio from "./CartoesApoio.svelte";
  import PixModal from "./PixModal.svelte";
  import { enviarFeedback } from "$lib/ajuda";
  import { conta, atualizarConta, entrarComGoogle, sairDaConta } from "$lib/conta-google.svelte";
  import { t } from "$lib/i18n";
  import { SITE, SITE_TERMOS } from "$lib/site";
  import { openUrl } from "@tauri-apps/plugin-opener";

  interface Props {
    /** lateral: linha com o nome (tela inicial). icone: avatar redondo (barra de cima). */
    variante?: "lateral" | "icone";
  }
  let { variante = "lateral" }: Props = $props();

  let aberto = $state(false);
  let pixVisivel = $state(false);

  $effect(() => {
    atualizarConta();
    // outros paineis (Drive, Assistente) avisam quando a conta muda
    const aoMudar = () => atualizarConta();
    window.addEventListener("conta-google-mudou", aoMudar);
    return () => window.removeEventListener("conta-google-mudou", aoMudar);
  });

  const info = $derived(conta.info);
  const inicial = $derived((info.nome || info.email || "").trim().charAt(0).toUpperCase());

  function alternar() {
    aberto = !aberto;
    if (aberto) atualizarConta();
  }

  // erros de login ja aparecem em conta.erro; aqui so evitamos o aviso de promessa sem tratamento
  const entrar = () => entrarComGoogle().catch(() => {});
</script>

{#snippet avatar(grande: boolean)}
  {#if info.conectado && info.foto}
    <img class="foto" class:grande src={info.foto} alt="" referrerpolicy="no-referrer" />
  {:else if info.conectado && inicial}
    <span class="foto inicial" class:grande aria-hidden="true">{inicial}</span>
  {:else}
    <span class="foto vazia" class:grande><UserRound size={grande ? 22 : 15} /></span>
  {/if}
{/snippet}

{#if variante === "lateral"}
  <button class="conta-botao" onclick={alternar} title={t("conta.sua-conta")}>
    {@render avatar(false)}
    <span class="rotulo">{info.conectado ? (info.nome || info.email) : t("conta.entrar")}</span>
  </button>
{:else}
  <button
    class="conta-icone"
    onclick={alternar}
    title={info.conectado ? (info.nome || info.email) : t("conta.entrar")}
    aria-label={t("conta.sua-conta")}
    aria-haspopup="dialog"
    aria-expanded={aberto}
  >
    {@render avatar(false)}
  </button>
{/if}

{#if aberto}
  <div class="fundo" role="presentation" onclick={() => (aberto = false)}></div>
  <div class="cartao {variante}" role="dialog" aria-label={t("conta.titulo")}>
    <button class="fechar" onclick={() => (aberto = false)} aria-label={t("conta.fechar")}><X size={16} /></button>
    {#if !info.configurado}
      <p class="texto">{t("conta.nao-configurado")}</p>
    {:else if !info.conectado}
      <h3>{t("conta.titulo")}</h3>
      {#if info.aviso}<p class="aviso">{info.aviso}</p>{/if}
      <p class="texto">{t("conta.explicacao")}</p>
      <button class="primario" onclick={entrar} disabled={conta.ocupado}>
        {conta.ocupado ? conta.progresso || t("conta.aguardando") : t("conta.entrar")}
      </button>
    {:else}
      <div class="perfil">
        {@render avatar(true)}
        <div>
          <div class="nome">{info.nome}</div>
          <div class="email">{info.email}</div>
        </div>
      </div>
      <!-- conectado so e verdadeiro quando o login trouxe o escopo drive.file (o Rust recusa a conta sem ele) -->
      <ul class="servicos">
        <li><Check size={14} /> {t("conta.drive")}</li>
        <li><Check size={14} /> {t("conta.arquivos-google")}</li>
      </ul>
      {#if info.aviso}<p class="aviso">{info.aviso}</p>{/if}
      <button class="secundario" onclick={sairDaConta}><LogOut size={14} /> {t("conta.sair")}</button>
    {/if}
    {#if conta.erro}<p class="erro">{conta.erro}</p>{/if}
    <button class="link" onclick={() => openUrl(`${SITE}/privacidade.html`)}>{t("privacidade.link")}</button>
    <button class="link" onclick={() => openUrl(SITE_TERMOS).catch(() => {})}>{t("termos.link")}</button>
    <div class="apoio">
      <CartoesApoio onfeedback={() => { aberto = false; void enviarFeedback(); }} onapoiar={() => { aberto = false; pixVisivel = true; }} />
    </div>
  </div>
{/if}

{#if pixVisivel}
  <PixModal aoFechar={() => (pixVisivel = false)} />
{/if}

<style>
  .conta-botao {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .conta-botao:hover {
    background: var(--hover);
  }
  .conta-icone {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    cursor: pointer;
  }
  .conta-icone:hover {
    background: var(--hover);
  }
  .rotulo {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .foto {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    flex: none;
    object-fit: cover;
  }
  .conta-icone .foto {
    width: 26px;
    height: 26px;
  }
  .foto.vazia,
  .foto.inicial {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .foto.vazia {
    background: var(--hover);
  }
  .foto.inicial {
    background: var(--primaria, #5f1a5c);
    color: #fff;
    font-size: 0.8125rem;
    font-weight: 600;
  }
  .foto.grande {
    width: 44px;
    height: 44px;
  }
  .foto.inicial.grande {
    font-size: 1.25rem;
  }
  .fundo {
    position: fixed;
    inset: 0;
    z-index: 900;
  }
  .cartao {
    position: fixed;
    z-index: 901;
    width: min(320px, calc(100vw - 32px));
    max-height: calc(100dvh - 16px);
    overflow-y: auto;
    padding: 16px;
    border-radius: 8px;
    background: var(--fundo, #fff);
    color: var(--texto);
    border: 1px solid var(--borda, #ccc);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .cartao.lateral {
    left: 16px;
    bottom: 16px;
  }
  .cartao.icone {
    top: 44px;
    right: 8px;
  }
  .fechar {
    position: absolute;
    top: 8px;
    right: 8px;
    border: none;
    background: transparent;
    color: var(--texto);
    cursor: pointer;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  .texto {
    margin: 0 0 12px;
    font-size: 13px;
    line-height: 1.4;
  }
  .aviso {
    margin: 0 0 12px;
    padding: 8px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, #e6a100 18%, transparent);
    font-size: 12px;
    line-height: 1.4;
  }
  .perfil {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .nome {
    font-weight: 600;
  }
  .email {
    font-size: 12px;
    opacity: 0.75;
    word-break: break-all;
  }
  .servicos {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    font-size: 13px;
  }
  .servicos li {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    padding: 3px 0;
  }
  .primario,
  .secundario {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    justify-content: center;
    padding: 8px 12px;
    border-radius: 6px;
    font: inherit;
    cursor: pointer;
  }
  .primario {
    border: none;
    background: #5f1a5c;
    color: #fff;
  }
  .secundario {
    border: 1px solid var(--borda, #ccc);
    background: transparent;
    color: var(--texto);
  }
  .link {
    border: none;
    background: none;
    color: #5f1a5c;
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
    padding: 0;
  }
  .apoio {
    margin-top: 12px;
  }
  .erro {
    margin: 10px 0 0;
    font-size: 12px;
    color: #c0392b;
    white-space: pre-wrap;
  }
</style>
