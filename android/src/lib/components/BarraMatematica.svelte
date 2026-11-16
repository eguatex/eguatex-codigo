<script lang="ts">
  // Barra auxiliar abaixo da barra do editor (46 px no mouse, 48 px no toque): botao da categoria, botoes de LaTeX e fechar.
  // Nao insere nada sozinha: botao de ambiente chama oncomando(acao); os demais chamam oninserir(comando),
  // o mesmo caminho da paleta de simbolos (a selecao entra no primeiro {} do comando).
  import { onMount, tick } from 'svelte';
  import Calculator from '@lucide/svelte/icons/calculator';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import XIcon from '@lucide/svelte/icons/x';
  import MenuCategorias from './MenuCategorias.svelte';
  import { t, tf } from '$lib/i18n';
  import { atalhos, nomeDaCategoria, alternarCategoria, fecharBarraMatematica } from '$lib/atalhos.svelte';
  import { preferencias } from '$lib/preferencias.svelte';
  import { categoriaAtivaPorId, type Botao } from '$lib/barra-matematica';
  import { registrarFechavel } from '$lib/voltar';

  interface Props {
    oncomando?: (id: string) => void;
    oninserir?: (latex: string) => void;
  }

  let { oncomando, oninserir }: Props = $props();

  const categoria = $derived(categoriaAtivaPorId(atalhos.usuario, preferencias.categoriaAtiva));

  let itens: HTMLDivElement | null = $state(null);
  let gatilho: HTMLButtonElement | null = $state(null);
  let popover: HTMLDivElement | null = $state(null);
  let temEsquerda = $state(false);
  let temDireita = $state(false);
  let menuAberto = $state(false);
  let menuPos = $state({ x: 0, y: 0 });

  // o toque nao tira o foco do editor: sem isso o teclado do celular fecha a cada botao
  function manterFoco(e: PointerEvent) {
    e.preventDefault();
  }

  function medir() {
    if (!itens) return;
    temEsquerda = itens.scrollLeft > 1;
    temDireita = itens.scrollLeft + itens.clientWidth < itens.scrollWidth - 1;
  }

  function rolar(sentido: -1 | 1) {
    itens?.scrollBy({ left: sentido * Math.max(80, itens.clientWidth * 0.8), behavior: 'smooth' });
  }

  function usar(b: Botao) {
    if (b.acao) oncomando?.(b.acao);
    else oninserir?.(b.comando);
  }

  function alternarMenu() {
    if (menuAberto) {
      menuAberto = false;
      return;
    }
    const r = gatilho?.getBoundingClientRect();
    if (r) menuPos = { x: r.left, y: r.bottom + 2 };
    menuAberto = true;
  }

  function escolher(id: string) {
    menuAberto = false;
    alternarCategoria(id);
  }

  function acao(id: string) {
    menuAberto = false;
    oncomando?.(id);
  }

  // a folha de categorias fecha pelo botao Voltar do Android
  $effect(() => {
    if (menuAberto) return registrarFechavel(() => (menuAberto = false));
  });

  // refaz a medida quando a categoria muda (outra lista de botoes) e volta a rolagem ao inicio
  $effect(() => {
    void categoria?.botoes;
    void categoria?.id;
    void tick().then(() => {
      if (itens) itens.scrollLeft = 0;
      medir();
    });
  });

  onMount(() => {
    const observador = new ResizeObserver(medir);
    if (itens) observador.observe(itens);
    const fora = (e: PointerEvent) => {
      const alvo = e.target as Node | null;
      if (!menuAberto || !alvo) return;
      if (!popover?.contains(alvo) && !gatilho?.contains(alvo)) menuAberto = false;
    };
    const tecla = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && menuAberto) {
        menuAberto = false;
        e.stopPropagation();
      }
    };
    window.addEventListener('pointerdown', fora, true);
    window.addEventListener('keydown', tecla, true);
    return () => {
      observador.disconnect();
      window.removeEventListener('pointerdown', fora, true);
      window.removeEventListener('keydown', tecla, true);
    };
  });
</script>

{#if categoria}
  <div class="barra-matematica" role="toolbar" aria-label={t('atalhos.barra-rotulo')}>
    <div class="aba">
      <button
        type="button"
        class="categoria"
        bind:this={gatilho}
        title={tf('atalhos.categoria-titulo', { nome: nomeDaCategoria(categoria) })}
        aria-haspopup="menu"
        aria-expanded={menuAberto}
        onpointerdown={manterFoco}
        onclick={alternarMenu}
      >
        <Calculator size={16} />
        <span>{nomeDaCategoria(categoria)}</span>
        <ChevronDown size={14} />
      </button>
    </div>

    <div class="trilho">
      {#if temEsquerda}
        <button type="button" class="seta esquerda" aria-label={t('atalhos.rolar-esquerda')} title={t('atalhos.rolar-esquerda')} onpointerdown={manterFoco} onclick={() => rolar(-1)}>
          <ChevronLeft size={22} />
        </button>
      {/if}
      <div class="itens" bind:this={itens} onscroll={medir}>
        {#each categoria.botoes as b, i (`${i}:${b.comando}`)}
          <button type="button" class="botao" title={b.comando} onpointerdown={manterFoco} onclick={() => usar(b)}>
            {b.rotulo ?? b.comando}
          </button>
        {:else}
          <span class="vazio">{t('atalhos.sem-botoes')}</span>
        {/each}
      </div>
      {#if temDireita}
        <button type="button" class="seta direita" aria-label={t('atalhos.rolar-direita')} title={t('atalhos.rolar-direita')} onpointerdown={manterFoco} onclick={() => rolar(1)}>
          <ChevronRight size={22} />
        </button>
      {/if}
    </div>

    <button type="button" class="fechar" title={t('atalhos.fechar-barra')} aria-label={t('atalhos.fechar-barra')} onpointerdown={manterFoco} onclick={fecharBarraMatematica}>
      <XIcon size={16} />
    </button>
  </div>

  {#if menuAberto}
    <div class="popover" role="menu" aria-label={t('atalhos.menu-rotulo')} style:left="{menuPos.x}px" style:top="{menuPos.y}px" bind:this={popover}>
      <MenuCategorias aoescolher={escolher} aoacao={acao} />
    </div>
  {/if}
{/if}

<style>
  /* --sc-h do modelo: 46 px; fundo de superficie secundaria e borda embaixo */
  .barra-matematica {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    height: 46px;
    box-sizing: border-box;
    border-bottom: 1px solid var(--borda);
    background: var(--superficie-2, var(--hover));
    color: var(--texto);
  }

  .aba {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 8px;
    border-right: 1px solid var(--borda);
  }

  /* botao da categoria: 34 px de altura, padding 0 10, raio 9, 14 px peso 600, icone de 16 px */
  .categoria {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 10px;
    border: none;
    border-radius: 9px;
    background: var(--primaria-suave);
    color: var(--primaria);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
  }

  .categoria:hover {
    filter: brightness(0.97);
  }

  .trilho {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
  }

  /* rolagem horizontal sem barra visivel; gap 4, padding 0 8 */
  .itens {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }

  .itens::-webkit-scrollbar {
    display: none;
  }

  /* botoes: minimo 40x36, padding 0 10, raio 9, borda de 1 px, serifa de 15 px */
  .botao {
    flex: 0 0 auto;
    min-width: 40px;
    height: 36px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-radius: 9px;
    background: var(--superficie-0);
    color: var(--texto);
    font-family: 'Noto Serif', 'Times New Roman', serif;
    font-size: 15px;
    white-space: nowrap;
    cursor: pointer;
  }

  .botao:hover {
    background: var(--hover);
    border-color: var(--primaria);
  }

  .botao:focus-visible,
  .categoria:focus-visible,
  .seta:focus-visible,
  .fechar:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .vazio {
    padding: 0 8px;
    color: var(--texto-secundario);
    font-size: 0.8125rem;
    white-space: nowrap;
  }

  /* setas de 34 px nas pontas, so quando ha botao fora da area; flutuam sobre a faixa, sem mexer na largura */
  .seta {
    position: absolute;
    top: 6px;
    z-index: 1;
    width: 34px;
    height: 34px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--borda);
    border-radius: 9px;
    background: var(--superficie-0);
    color: var(--texto);
    cursor: pointer;
    box-shadow: var(--sombra-3);
  }

  .seta.esquerda {
    left: 2px;
  }

  .seta.direita {
    right: 2px;
  }

  .seta:hover {
    background: var(--hover);
  }

  .fechar {
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    margin: 0 6px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .fechar:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .popover {
    position: fixed;
    z-index: 1600;
    box-sizing: border-box;
    min-width: 240px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: var(--sombra-3);
  }

  /* Celular (toque): barra de 48 px, botoes e setas de 44 x 44, categorias em folha inferior */
  @media (pointer: coarse) {
    .barra-matematica {
      height: 48px;
    }

    .categoria {
      height: 44px;
      padding: 0 12px;
    }

    .botao {
      min-width: 44px;
      height: 44px;
    }

    .seta {
      top: 2px;
      width: 44px;
      height: 44px;
    }

    .fechar {
      width: 44px;
      height: 44px;
      margin: 0 2px;
    }

    .popover {
      left: 0 !important;
      right: 0;
      top: auto !important;
      bottom: 0;
      min-width: 0;
      max-height: calc(100dvh - 24px);
      overflow-y: auto;
      padding: 8px 8px calc(8px + env(safe-area-inset-bottom));
      border-radius: 14px 14px 0 0;
    }
  }
</style>
