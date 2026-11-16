<script lang="ts">
  import MenuBarra from './MenuBarra.svelte';
  import BotaoTema from './BotaoTema.svelte';
  import ContaUsuario from './ContaUsuario.svelte';
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Columns2 from "@lucide/svelte/icons/columns-2";
  import Check from "@lucide/svelte/icons/check";
  import UserPlus from "@lucide/svelte/icons/user-plus";
  import House from "@lucide/svelte/icons/house";
  import { tick } from 'svelte';
  import type { Acoes } from '$lib/comandos';
  import { t } from '$lib/i18n';
  import { registrarFechavel } from '$lib/voltar';

  type ModoLayout = 'ambos' | 'editor' | 'pdf';

  interface Props {
    nomeProjeto?: string;
    modoLayout?: ModoLayout;
    onlayout?: (modo: ModoLayout) => void;
    onrenomear?: (nome: string) => void;
    oninicio?: () => void;
    acoes?: Acoes;
    marcados?: Record<string, boolean>;
    oncompartilhar?: () => void;
    /** Texto curto da sincronizacao com o Google Drive ("Sincronizado 21:30"). */
    estadoDrive?: string;
    /** Texto longo da sincronizacao (arquivos com falha), mostrado como title. */
    detalheDrive?: string;
    /** Verdadeiro enquanto o nome do projeto esta sendo editado (a barra fica visivel com o teclado). */
    editandoTitulo?: boolean;
  }

  let {
    nomeProjeto = '',
    modoLayout = 'ambos',
    onlayout,
    onrenomear,
    oninicio,
    acoes = {},
    marcados = {},
    oncompartilhar,
    estadoDrive = '',
    detalheDrive = '',
    editandoTitulo = $bindable(false),
  }: Props = $props();

  const opcoesLayout: Array<{ modo: ModoLayout; nome: string }> = [
    { modo: 'ambos', nome: 'Editor e PDF' },
    { modo: 'editor', nome: 'Somente editor' },
    { modo: 'pdf', nome: 'Somente PDF' },
  ];

  let layoutAberto = $state(false);

  // o botao Voltar do Android fecha o menu Layout em vez de sair do app
  $effect(() => {
    if (layoutAberto) return registrarFechavel(() => (layoutAberto = false));
  });
  let tituloEditado = $state('');
  let campoTitulo: HTMLInputElement | undefined = $state();

  function escolherLayout(modo: ModoLayout) {
    onlayout?.(modo);
    layoutAberto = false;
  }

  async function comecarEdicao() {
    tituloEditado = nomeProjeto;
    editandoTitulo = true;
    await tick();
    campoTitulo?.select();
  }

  function teclaTitulo(e: KeyboardEvent) {
    if (e.key === 'Enter') confirmarTitulo();
    else if (e.key === 'Escape') editandoTitulo = false;
  }

  function confirmarTitulo() {
    if (!editandoTitulo) return;
    editandoTitulo = false;
    const nome = tituloEditado.trim();
    if (nome && nome !== nomeProjeto) onrenomear?.(nome);
  }
</script>

<svelte:window onclick={(e) => {
  const alvo = e.target as HTMLElement;
  if (!alvo.closest('.toolbar-direita')) layoutAberto = false;
}} />

<nav class="toolbar">
  <div class="toolbar-esquerda">
    <div class="toolbar-logo">
      <!-- como no Overleaf: o logo vira a casinha quando o mouse passa por cima -->
      <button onclick={oninicio} class="toolbar-logo-btn btn-inicio" aria-label="Voltar aos seus projetos">
        <img src="/curupira-icone-barra.png" alt="" width="40" height="40" class="toolbar-icone" />
        <span class="casa"><House size={18} /></span>
        <span class="dica" role="tooltip">Voltar aos seus projetos</span>
      </button>
    </div>
    <MenuBarra {acoes} {marcados} />
  </div>

  <div class="toolbar-centro">
    {#if nomeProjeto}
      {#if editandoTitulo}
        <input
          bind:this={campoTitulo}
          bind:value={tituloEditado}
          onkeydown={teclaTitulo}
          onblur={confirmarTitulo}
          class="toolbar-projeto-input"
          aria-label="Nome do projeto"
        />
      {:else}
        <button class="toolbar-projeto" onclick={comecarEdicao} title="{nomeProjeto} (clique para renomear)">{nomeProjeto}</button>
      {/if}
    {/if}
  </div>

  <div class="toolbar-direita">
    {#if estadoDrive}<span class="estado-drive" title={detalheDrive || t('drive.titulo-sincronizacao')}>{estadoDrive}</span>{/if}
    {#if oncompartilhar}
      <button class="btn-compartilhar" onclick={oncompartilhar} title="Convidar pessoas para editar este projeto">
        <UserPlus size={15} />
        <span>Compartilhar</span>
      </button>
    {/if}
    <ContaUsuario variante="icone" />
    <BotaoTema />
    <button class="btn-layout" onclick={() => (layoutAberto = !layoutAberto)} title="Layout">
      <Columns2 size={15} />
      <span>Layout</span>
      <ChevronDown size={12} />
    </button>
    {#if layoutAberto}
      <div class="layout-menu" role="menu">
        {#each opcoesLayout as op}
          <button class="layout-opcao" role="menuitem" onclick={() => escolherLayout(op.modo)}>
            <span class="layout-check">{#if modoLayout === op.modo}<Check size={14} />{/if}</span>
            {op.nome}
          </button>
        {/each}
        <button class="layout-opcao" role="menuitem" disabled title="em breve">
          <span class="layout-check"></span>
          PDF em janela separada
        </button>
      </div>
    {/if}
  </div>
</nav>

<style>
  .btn-compartilhar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border: none;
    border-radius: 999px;
    background: var(--primaria);
    color: #fff;
    font: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
  }

  .btn-compartilhar:hover {
    filter: brightness(1.1);
  }

  .estado-drive {
    font-size: 0.75rem;
    opacity: 0.7;
    white-space: nowrap;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    background: var(--superficie-0);
    border-bottom: 1px solid var(--borda);
    /* 44 px: a logo do Curupira com 40 px mostra o cabelo, a orelha e as folhas */
    min-height: 44px;
    box-sizing: border-box;
    gap: 12px;
    flex-shrink: 0;
  }

  .toolbar-esquerda {
    display: flex;
    align-items: center;
    gap: 8px;
    /* cresce para centralizar o nome, mas nao encolhe abaixo dos menus: quem encolhe e o nome */
    flex: 1 0 auto;
  }

  .toolbar-logo {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .toolbar-logo-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    border: none;
    background: transparent;
    padding: 0;
    cursor: pointer;
    border-radius: 6px;
  }

  .btn-inicio {
    position: relative;
    width: 30px;
    height: 30px;
    justify-content: center;
    color: var(--texto);
  }

  .btn-inicio .casa {
    position: absolute;
    inset: 0;
    display: none;
    place-items: center;
    border-radius: 50%;
    background: var(--hover);
  }

  .btn-inicio:hover .casa,
  .btn-inicio:focus-visible .casa {
    display: grid;
  }

  .btn-inicio:hover .toolbar-icone,
  .btn-inicio:focus-visible .toolbar-icone {
    visibility: hidden;
  }

  .btn-inicio .dica {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    padding: 6px 10px;
    border-radius: 4px;
    background: #111;
    color: #fff;
    font-size: 0.8125rem;
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.12s;
    z-index: 1000;
  }

  .btn-inicio:hover .dica,
  .btn-inicio:focus-visible .dica {
    opacity: 1;
  }

  .toolbar-icone {
    border-radius: 6px;
  }

  .toolbar-centro {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    justify-content: center;
    /* sem isto o nome do projeto nao encolhe e cobre os menus em janela estreita (tablet em pe) */
    min-width: 0;
    overflow: hidden;
  }

  .toolbar-projeto {
    min-width: 0;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--texto);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 320px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    padding: 3px 8px;
    cursor: text;
    font-family: inherit;
  }

  .toolbar-projeto:hover {
    border-color: var(--borda);
  }

  .toolbar-projeto-input {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--texto);
    background: var(--fundo);
    border: 1px solid var(--primaria);
    border-radius: 4px;
    padding: 3px 8px;
    width: 280px;
    font-family: inherit;
    outline: none;
  }

  .toolbar-direita {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    flex: 1;
  }

  .btn-layout {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font-size: 0.8125rem;
    cursor: pointer;
    font-family: inherit;
  }

  .btn-layout:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .layout-menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    min-width: 210px;
    padding: 4px 0;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 1000;
  }

  .layout-opcao {
    display: flex;
    align-items: center;
    gap: 6px;
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

  .layout-opcao:hover:not(:disabled) {
    background: var(--hover);
  }

  .layout-opcao:disabled {
    color: var(--texto-terciario);
    cursor: not-allowed;
  }

  .layout-check {
    display: inline-flex;
    width: 16px;
  }

  /* janela abaixo de 1000 px: Compartilhar e Layout ficam so com o icone (o title continua) */
  @media (max-width: 999px) {
    .btn-compartilhar span,
    .btn-layout span {
      display: none;
    }

    .btn-compartilhar {
      padding: 6px 8px;
    }
  }

  /* celular deitado: mesma barra, so com medidas menores */
  :global(.layout.compacto) .toolbar {
    min-height: 38px;
    height: 38px;
    padding: 0 6px;
    gap: 6px;
  }

  :global(.layout.compacto) .toolbar-esquerda,
  :global(.layout.compacto) .toolbar-direita {
    gap: 4px;
    flex: 0 0 auto;
  }

  :global(.layout.compacto) .toolbar-centro {
    flex: 1 1 0;
    min-width: 0;
    justify-content: flex-start;
  }

  :global(.layout.compacto) .toolbar-icone {
    width: 24px;
    height: 24px;
  }

  :global(.layout.compacto) .toolbar-projeto {
    max-width: 160px;
  }

  :global(.layout.compacto) .toolbar-projeto-input {
    width: 160px;
  }

  :global(.layout.compacto) .estado-drive {
    max-width: 70px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>