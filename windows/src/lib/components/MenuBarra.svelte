<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { MENUS, executar, type Acoes, type ItemMenu } from '$lib/comandos';

  interface Props {
    acoes?: Acoes;
    marcados?: Record<string, boolean>;
  }

  let { acoes = {}, marcados = {} }: Props = $props();

  let aberto = $state<number | null>(null);
  let subAberto = $state<string | null>(null);

  function clicarMenu(i: number) {
    aberto = aberto === i ? null : i;
    subAberto = null;
  }

  function passarMenu(i: number) {
    if (aberto !== null && aberto !== i) {
      aberto = i;
      subAberto = null;
    }
  }

  function fechar() {
    clearTimeout(timerSub);
    aberto = null;
    subAberto = null;
  }

  // Os ganchos de passagem usam so o mouse: no toque o submenu abre e fecha por toque, nunca sozinho.
  // Ao sair de um submenu na diagonal, o mouse passa por outros itens do menu.
  // A folga evita que o submenu feche antes de o mouse chegar nele.
  let timerSub: ReturnType<typeof setTimeout> | undefined;

  // O menu rola quando e mais alto que a tela, e a rolagem cortaria o submenu.
  // Por isso o submenu usa posicao fixa, calculada a partir do item que o abriu.
  let subPos = $state({ x: 0, y: 0 });

  function abrirSub(nome: string, el: Element | null) {
    clearTimeout(timerSub);
    const caixa = el?.closest('.sub')?.getBoundingClientRect();
    if (caixa) {
      const x = caixa.right + 230 > window.innerWidth ? Math.max(0, caixa.left - 230) : caixa.right - 2;
      subPos = { x, y: Math.max(8, caixa.top - 5) };
    }
    subAberto = nome;
  }

  // Depois de desenhado, sobe o submenu o quanto precisar para caber inteiro na tela.
  // Se ainda for mais alto que a tela, o max-height liga a rolagem.
  function caberNaTela(el: HTMLElement, y: number) {
    const ajustar = (alvo: number) => {
      el.style.top = `${Math.max(8, Math.min(alvo, window.innerHeight - el.offsetHeight - 8))}px`;
    };
    ajustar(y);
    return { update: ajustar };
  }

  function agendarFecharSub() {
    clearTimeout(timerSub);
    if (subAberto !== null) timerSub = setTimeout(() => (subAberto = null), 400);
  }

  function rodar(id: string) {
    if (!acoes[id]) return;
    fechar();
    executar(acoes, id);
  }

  function temMarcavel(itens: ItemMenu[]): boolean {
    return itens.some((i) => i.tipo === 'comando' && i.marcavel);
  }
</script>

<svelte:window
  onclick={(e) => { if (aberto !== null && !(e.target as HTMLElement).closest('.menu-bar')) fechar(); }}
  onkeydown={(e) => { if (aberto !== null && e.key === 'Escape') fechar(); }}
/>

{#snippet lista(itens: ItemMenu[], dentroDoSub = false)}
  {@const recuo = temMarcavel(itens)}
  {#each itens as item}
    {#if item.tipo === 'sep'}
      <div class="separador" role="separator"></div>
    {:else if item.tipo === 'titulo'}
      <div class="titulo">{item.nome}</div>
    {:else if item.tipo === 'sub'}
      <div class="sub" role="none" onpointerenter={(e) => { if (e.pointerType === 'mouse') abrirSub(item.nome, e.currentTarget); }}>
        <button class="item" class:ativo={subAberto === item.nome} role="menuitem" aria-haspopup="true" onclick={(e) => abrirSub(item.nome, e.currentTarget)}>
          <span class="nome">{item.nome}</span>
          <ChevronRight size={14} />
        </button>
        {#if subAberto === item.nome}
          <div class="menu submenu" role="menu" tabindex="-1" use:caberNaTela={subPos.y} style:left="{subPos.x}px" style:max-height="calc(100dvh - 16px)" onpointerenter={(e) => { if (e.pointerType === 'mouse') clearTimeout(timerSub); }}>
            {@render lista(item.itens, true)}
          </div>
        {/if}
      </div>
    {:else}
      <button
        class="item"
        role="menuitem"
        disabled={!acoes[item.id]}
        onpointerenter={(e) => { if (e.pointerType === 'mouse' && !dentroDoSub) agendarFecharSub(); }}
        onclick={() => rodar(item.id)}
      >
        {#if recuo}
          <span class="marca">{#if marcados[item.id]}<Check size={14} />{/if}</span>
        {/if}
        <span class="nome">{item.nome}</span>
        {#if item.atalho}<span class="atalho">{item.atalho}</span>{/if}
      </button>
    {/if}
  {/each}
{/snippet}

<div class="menu-bar" role="menubar">
  {#each MENUS as menu, i}
    <div class="menu-raiz">
      <button
        class="menu-titulo"
        class:aberto={aberto === i}
        role="menuitem"
        aria-haspopup="true"
        aria-expanded={aberto === i}
        onclick={() => clicarMenu(i)}
        onpointerenter={(e) => { if (e.pointerType === 'mouse') passarMenu(i); }}
      >
        {menu.nome}
      </button>
      {#if aberto === i}
        <div class="menu" role="menu">
          {@render lista(menu.itens)}
        </div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .menu-bar {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 100%;
  }

  .menu-raiz {
    position: relative;
  }

  .menu-titulo {
    height: 28px;
    padding: 0 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto);
    font-size: 0.8125rem;
    cursor: pointer;
    font-family: inherit;
  }

  .menu-titulo:hover,
  .menu-titulo.aberto {
    background: var(--hover);
  }

  .menu {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    min-width: 230px;
    max-height: calc(100dvh - 48px);
    overflow-y: auto;
    padding: 4px 0;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 1000;
  }

  .sub {
    position: relative;
  }

  .submenu {
    position: fixed;
  }

  .item {
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

  .item:hover:not(:disabled),
  .item.ativo {
    background: var(--hover);
  }

  .item:disabled {
    color: var(--texto-terciario);
    cursor: default;
  }

  .nome {
    flex: 1;
    white-space: nowrap;
  }

  .marca {
    display: inline-flex;
    width: 16px;
    flex-shrink: 0;
  }

  .atalho {
    margin-left: 24px;
    font-size: 0.75rem;
    color: var(--texto-terciario);
    white-space: nowrap;
  }

  .titulo {
    padding: 6px 12px 2px;
    font-size: 0.6875rem;
    font-weight: 600;
    color: var(--texto-secundario);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .separador {
    height: 1px;
    margin: 4px 0;
    background: var(--borda);
  }
</style>
