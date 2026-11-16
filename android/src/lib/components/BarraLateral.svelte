<script lang="ts">
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import Search from "@lucide/svelte/icons/search";
  import Bot from "@lucide/svelte/icons/bot";
  import Settings from "@lucide/svelte/icons/settings";
  import HelpCircle from "@lucide/svelte/icons/help-circle";
  import Heart from "@lucide/svelte/icons/heart";
  import CartoesApoio from "./CartoesApoio.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { SITE, EMAIL_CONTATO, MAILTO_CONTATO } from "$lib/site";
  import { t } from "$lib/i18n";
  import { enviarFeedback, procurarAtualizacao } from "$lib/ajuda";

  interface Props {
    painelAtivo?: string;
    ontrocarpainel?: (painel: string) => void;
    onconfigurar?: () => void;
    onatalhos?: () => void;
    oncomousar?: () => void;
    onapoiar?: () => void;
  }

  let { painelAtivo = 'arquivos', ontrocarpainel, onconfigurar, onatalhos, oncomousar, onapoiar }: Props = $props();

  let ajudaAberta = $state(false);
  let apoioAberto = $state(false);

  function trocar(painel: string) {
    ontrocarpainel?.(painelAtivo === painel ? '' : painel);
  }

  function abrirLink(url: string) {
    ajudaAberta = false;
    openUrl(url).catch((e) => console.error('Erro ao abrir link:', e));
  }
</script>

<svelte:window onclick={(e) => { const alvo = e.target as HTMLElement; if (!alvo.closest('.ajuda')) ajudaAberta = false; if (!alvo.closest('.apoio')) apoioAberto = false; }} />

<aside class="barra-lateral">
  <div class="barra-topo">
    <button class="barra-btn" class:ativo={painelAtivo === 'arquivos'} onclick={() => trocar('arquivos')} title="Arquivos">
      <FolderTree size={20} />
    </button>
    <button class="barra-btn" class:ativo={painelAtivo === 'busca'} onclick={() => trocar('busca')} title="Buscar no projeto">
      <Search size={20} />
    </button>
    <button class="barra-btn ia" class:ativo={painelAtivo === 'chat'} onclick={() => trocar('chat')} title="Assistente de IA">
      <Bot size={20} />
    </button>
  </div>

  <div class="barra-rodape">
    <div class="apoio">
      <button class="barra-btn" class:ativo={apoioAberto} onclick={() => (apoioAberto = !apoioAberto)} title={t('sobre.apoiar-titulo')} aria-label={t('sobre.apoiar-titulo')}>
        <Heart size={20} />
      </button>
      {#if apoioAberto}
        <div class="apoio-menu" role="dialog" aria-label={t('sobre.apoiar-titulo')}>
          <CartoesApoio onfeedback={() => { apoioAberto = false; void enviarFeedback(); }} onapoiar={() => { apoioAberto = false; onapoiar?.(); }} />
        </div>
      {/if}
    </div>
    <div class="ajuda">
      <button class="barra-btn" class:ativo={ajudaAberta} onclick={() => (ajudaAberta = !ajudaAberta)} title="Ajuda">
        <HelpCircle size={20} />
      </button>
      {#if ajudaAberta}
        <div class="ajuda-menu" role="menu">
          <button class="ajuda-item" role="menuitem" onclick={() => { ajudaAberta = false; oncomousar?.(); }}>
            Como usar o ÉguaTex
          </button>
          <button class="ajuda-item" role="menuitem" onclick={() => { ajudaAberta = false; onatalhos?.(); }}>
            Atalhos de teclado
          </button>
          <button class="ajuda-item" role="menuitem" onclick={() => abrirLink('https://pt.wikibooks.org/wiki/LaTeX')}>
            Documentação do LaTeX
          </button>
          <button class="ajuda-item" role="menuitem" onclick={() => abrirLink(SITE)}>
            Site do ÉguaTex
          </button>
          <button class="ajuda-item" role="menuitem" onclick={() => abrirLink(MAILTO_CONTATO)}>
            Contato: {EMAIL_CONTATO}
          </button>
          <button class="ajuda-item" role="menuitem" onclick={() => { ajudaAberta = false; void enviarFeedback(); }}>
            {t('menu.feedback')}
          </button>
          <button class="ajuda-item" role="menuitem" onclick={() => { ajudaAberta = false; void procurarAtualizacao(); }}>
            {t('menu.atualizacao')}
          </button>
        </div>
      {/if}
    </div>
    <button class="barra-btn" onclick={onconfigurar} title="Configurações">
      <Settings size={20} />
    </button>
  </div>
</aside>

<style>
  .barra-lateral {
    width: 44px;
    background: var(--superficie-0);
    border-right: 1px solid var(--borda);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    flex-shrink: 0;
    padding: 4px 0;
  }

  .barra-topo,
  .barra-rodape {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .ajuda,
  .apoio {
    position: relative;
  }

  .apoio-menu {
    position: absolute;
    left: calc(100% + 6px);
    bottom: 0;
    width: 300px;
    max-width: calc(100vw - 60px);
    max-height: calc(100dvh - 56px);
    overflow-y: auto;
    padding: 8px;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 1000;
  }

  .ajuda-menu {
    position: absolute;
    left: calc(100% + 6px);
    bottom: 0;
    min-width: 200px;
    padding: 4px 0;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 1000;
  }

  .ajuda-item {
    display: block;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: var(--texto);
    font-size: 0.8125rem;
    text-align: left;
    cursor: pointer;
    font-family: inherit;
  }

  .ajuda-item:hover {
    background: var(--hover);
  }

  .barra-btn {
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--texto-terciario);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    position: relative;
  }

  .barra-btn:hover {
    background: var(--hover);
    color: var(--texto-secundario);
  }

  .barra-btn.ativo {
    background: color-mix(in srgb, var(--primaria) 12%, transparent);
    color: var(--primaria);
  }

  .barra-btn.ativo::before {
    content: '';
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: var(--primaria);
  }

  .barra-btn.ia.ativo {
    background: color-mix(in srgb, var(--acento) 12%, transparent);
    color: var(--acento);
  }

  .barra-btn.ia.ativo::before {
    background: var(--acento);
  }

  /* toque: botoes de 44 x 44 (a barra continua com 44 px de largura, que a gaveta usa como recuo) */
  @media (pointer: coarse) {
    .barra-btn {
      width: 44px;
      height: 44px;
    }

    .ajuda-item {
      min-height: 44px;
    }
  }
</style>
