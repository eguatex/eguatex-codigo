<script lang="ts">
  // Conteudo do menu de categorias da barra Matematica. Quem abre (EditorToolbar ou BarraMatematica)
  // cuida do popover (no toque, da folha inferior) e da posicao; aqui so ficam os itens.
  import Check from '@lucide/svelte/icons/check';
  import Plus from '@lucide/svelte/icons/plus';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import Zap from '@lucide/svelte/icons/zap';
  import Download from '@lucide/svelte/icons/download';
  import Upload from '@lucide/svelte/icons/upload';
  import Search from '@lucide/svelte/icons/search';
  import { t } from '$lib/i18n';
  import { atalhos, nomeDaCategoria } from "$lib/atalhos.svelte";
  import { preferencias } from '$lib/preferencias.svelte';
  import { categoriasVisiveis, GRUPOS_SIMBOLOS } from '$lib/barra-matematica';

  interface Props {
    /** Escolheu uma categoria (ou a ativa, para fechar a barra). */
    aoescolher: (id: string) => void;
    /** Escolheu um item de baixo: simbolo (busca), atalhos-nova-categoria, atalhos-gerenciar, atalhos-macros, atalhos-exportar ou atalhos-importar. */
    aoacao: (id: string) => void;
  }

  let { aoescolher, aoacao }: Props = $props();

  const categorias = $derived(categoriasVisiveis(atalhos.usuario));

  // o toque no item nao tira o foco do editor (o teclado nao fecha)
  function manterFoco(e: PointerEvent) {
    e.preventDefault();
  }
</script>

{#each categorias as c (c.id)}
  <button type="button" role="menuitem" class="item" class:ativo={preferencias.categoriaAtiva === c.id} onpointerdown={manterFoco} onclick={() => aoescolher(c.id)}>
    <span class="nome">{nomeDaCategoria(c)}</span>
    {#if preferencias.categoriaAtiva === c.id}
      <span class="marca"><Check size={14} /></span>
    {/if}
  </button>
{/each}
{#each GRUPOS_SIMBOLOS as g (g.id)}
  <button type="button" role="menuitem" class="item" class:ativo={preferencias.categoriaAtiva === g.id} onpointerdown={manterFoco} onclick={() => aoescolher(g.id)}>
    <span class="nome">{g.nome}</span>
    {#if preferencias.categoriaAtiva === g.id}
      <span class="marca"><Check size={14} /></span>
    {/if}
  </button>
{/each}
<button type="button" role="menuitem" class="item" onpointerdown={manterFoco} onclick={() => aoacao('simbolo')}>
  <Search size={16} />
  <span class="nome">{t('atalhos.buscar-simbolo')}</span>
</button>
<div class="sep"></div>
<button type="button" role="menuitem" class="item" onpointerdown={manterFoco} onclick={() => aoacao('atalhos-nova-categoria')}>
  <Plus size={16} />
  <span class="nome">{t('atalhos.nova-categoria')}</span>
</button>
<button type="button" role="menuitem" class="item" onpointerdown={manterFoco} onclick={() => aoacao('atalhos-gerenciar')}>
  <SlidersHorizontal size={16} />
  <span class="nome">{t('atalhos.gerenciar')}</span>
</button>
<button type="button" role="menuitem" class="item" onpointerdown={manterFoco} onclick={() => aoacao('atalhos-macros')}>
  <Zap size={16} />
  <span class="nome">{t('atalhos.macros')}</span>
</button>
<button type="button" role="menuitem" class="item" onpointerdown={manterFoco} onclick={() => aoacao('atalhos-exportar')}>
  <Download size={16} />
  <span class="nome">{t('atalhos.exportar')}</span>
</button>
<button type="button" role="menuitem" class="item" onpointerdown={manterFoco} onclick={() => aoacao('atalhos-importar')}>
  <Upload size={16} />
  <span class="nome">{t('atalhos.importar')}</span>
</button>

<style>
  .item {
    height: 32px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 0.8125rem;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  .item:hover {
    background: var(--hover);
  }

  .item.ativo {
    background: var(--primaria-suave);
  }

  .item:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .item :global(svg) {
    flex-shrink: 0;
    color: var(--texto-secundario);
  }

  .marca {
    margin-left: auto;
    padding-left: 16px;
    display: inline-flex;
  }

  .item .marca :global(svg) {
    color: var(--primaria);
  }

  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--borda);
  }

  /* toque: itens de 48 px, texto de 16 px, check de 20 px na categoria ativa */
  @media (pointer: coarse) {
    .item {
      height: 48px;
      padding: 0 14px;
      gap: 12px;
      font-size: 1rem;
    }

    .marca :global(svg) {
      width: 20px;
      height: 20px;
    }

    .sep {
      margin: 6px 8px;
    }
  }
</style>
