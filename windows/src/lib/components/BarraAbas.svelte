<script lang="ts" module>
  /** Id do elemento de cada aba; o painel do editor usa o da ativa em aria-labelledby. */
  export const idDaAba = (indice: number, base = 'aba'): string => `${base}-${indice}`;
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import FileText from '@lucide/svelte/icons/file-text';
  import FileImage from '@lucide/svelte/icons/file-image';
  import Star from '@lucide/svelte/icons/star';
  import X from '@lucide/svelte/icons/x';
  import { t, tf } from '$lib/i18n';
  import { destinoDaSoltura, nomeDaAba, type Aba } from '$lib/abas.svelte';

  interface Props {
    abas: Aba[];
    /** Caminho da aba ativa. */
    ativa: string;
    /** Caminhos das abas com texto ainda nao confirmado pelo disco. */
    sujas?: string[];
    /** Caminho do documento principal: a aba dele mostra um marcador. */
    principal?: string;
    /** Preferencia "Mostrar abas" desligada: so a ativa aparece (as outras continuam abertas). */
    ontrocar?: (caminho: string) => void;
    onfechar?: (caminho: string) => void;
    /** Menu do botao direito, "Fechar as outras". */
    onfecharoutras?: (caminho: string) => void;
    /** Arrastar: `para` e o indice na lista sem a aba que saiu (ver `mover` em abas.svelte.ts). */
    onmover?: (de: number, para: number) => void;
  }

  let { abas, ativa, sujas = [], principal = '', ontrocar, onfechar, onfecharoutras, onmover }: Props = $props();

  // a ultima aba aberta nao fecha (tabs-container.tsx: canCloseTab={tabs.length > 1})
  const podeFechar = $derived(abas.length > 1);

  let lista: HTMLDivElement | undefined = $state();
  // depois de Delete a aba some; o foco vai para a que ficou ativa
  let focarAtivaDepois = false;

  const visiveis = $derived(
    abas.map((aba, indice) => ({ aba, indice })),
  );

  function elementosDasAbas(): HTMLElement[] {
    return lista ? [...lista.querySelectorAll<HTMLElement>('[role="tab"]')] : [];
  }

  function focarPosicao(posicao: number) {
    const el = elementosDasAbas();
    if (el.length) el[(posicao + el.length) % el.length]?.focus();
  }

  function teclaNaAba(e: KeyboardEvent, caminho: string) {
    // tecla vinda do botao de fechar fica com o botao
    if (e.target !== e.currentTarget) return;
    const el = elementosDasAbas();
    const posicao = el.indexOf(e.currentTarget as HTMLElement);
    switch (e.key) {
      case 'ArrowRight':
        e.preventDefault();
        focarPosicao(posicao + 1);
        break;
      case 'ArrowLeft':
        e.preventDefault();
        focarPosicao(posicao - 1);
        break;
      case 'Home':
        e.preventDefault();
        focarPosicao(0);
        break;
      case 'End':
        e.preventDefault();
        focarPosicao(el.length - 1);
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        ontrocar?.(caminho);
        break;
      case 'Delete':
        e.preventDefault();
        if (!podeFechar) break;
        focarAtivaDepois = true;
        onfechar?.(caminho);
        break;
    }
  }

  // a aba ativa entra na tela quando a barra rola
  $effect(() => {
    void ativa;
    void abas.length;
    void tick().then(() => {
      const el = lista?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
      if (el && typeof el.scrollIntoView === 'function') el.scrollIntoView({ inline: 'nearest', block: 'nearest' });
    });
  });

  $effect(() => {
    void abas.length;
    if (!focarAtivaDepois) return;
    focarAtivaDepois = false;
    void tick().then(() => lista?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus());
  });

  // a roda do mouse (vertical) rola a barra na horizontal; so quando ha o que rolar
  $effect(() => {
    const el = lista;
    if (!el) return;
    const rolar = (e: WheelEvent) => {
      if (e.deltaX !== 0 || e.deltaY === 0 || el.scrollWidth <= el.clientWidth) return;
      e.preventDefault();
      el.scrollLeft += e.deltaY;
    };
    el.addEventListener('wheel', rolar, { passive: false });
    return () => el.removeEventListener('wheel', rolar);
  });

  function fechar(e: Event, caminho: string) {
    e.stopPropagation();
    if (podeFechar) onfechar?.(caminho);
  }

  // ---------- arrastar para reordenar ----------
  // Por ponteiro, nao por draggable: na janela do Tauri no Windows o arrastar nativo do HTML5 fica preso ao
  // recurso de soltar arquivos da janela. O indicador de 5 px e o lado da soltura seguem tab.tsx do Overleaf.
  const LIMIAR_ARRASTE = 5;
  let arraste: { x: number; caminho: string; ativo: boolean } | null = null;
  let arrastando = $state(false);
  let soltura = $state<{ caminho: string; lado: 'esquerda' | 'direita' } | null>(null);
  let ignorarClique = false;

  function alvoEm(x: number): { caminho: string; lado: 'esquerda' | 'direita' } | null {
    const el = elementosDasAbas();
    if (el.length === 0) return null;
    const primeiro = el[0].getBoundingClientRect();
    const ultimo = el[el.length - 1].getBoundingClientRect();
    if (x <= primeiro.left) return { caminho: el[0].dataset.caminho ?? '', lado: 'esquerda' };
    if (x >= ultimo.right) return { caminho: el[el.length - 1].dataset.caminho ?? '', lado: 'direita' };
    for (const e of el) {
      const r = e.getBoundingClientRect();
      if (x >= r.left && x <= r.right) return { caminho: e.dataset.caminho ?? '', lado: x < r.left + r.width / 2 ? 'esquerda' : 'direita' };
    }
    return null;
  }

  function aoPressionar(e: PointerEvent, caminho: string) {
    if (e.button !== 0 || abas.length < 2) return;
    if ((e.target as HTMLElement).closest('.fechar')) return;
    arraste = { x: e.clientX, caminho, ativo: false };
    window.addEventListener('pointermove', aoMoverPonteiro);
    window.addEventListener('pointerup', aoSoltar);
    window.addEventListener('pointercancel', encerrarArraste);
  }

  function aoMoverPonteiro(e: PointerEvent) {
    if (!arraste) return;
    if (!arraste.ativo && Math.abs(e.clientX - arraste.x) < LIMIAR_ARRASTE) return;
    arraste.ativo = true;
    arrastando = true;
    soltura = alvoEm(e.clientX);
  }

  function aoSoltar() {
    const atual = arraste;
    const alvo = soltura;
    if (atual?.ativo) {
      ignorarClique = true;
      // o clique, se vier, vem logo depois do pointerup; se nao vier, a marca some
      setTimeout(() => (ignorarClique = false), 0);
      if (alvo) {
        const de = abas.findIndex((a) => a.caminho === atual.caminho);
        const para = destinoDaSoltura(abas.length, de, abas.findIndex((a) => a.caminho === alvo.caminho), alvo.lado);
        if (para >= 0) onmover?.(de, para);
      }
    }
    encerrarArraste();
  }

  function encerrarArraste() {
    arraste = null;
    arrastando = false;
    soltura = null;
    window.removeEventListener('pointermove', aoMoverPonteiro);
    window.removeEventListener('pointerup', aoSoltar);
    window.removeEventListener('pointercancel', encerrarArraste);
  }

  function aoClicar(caminho: string) {
    if (ignorarClique) {
      ignorarClique = false;
      return;
    }
    ontrocar?.(caminho);
  }

  // ---------- menu do botao direito (tabs-context-menu.tsx) ----------
  let menuAba = $state<{ x: number; y: number; caminho: string } | null>(null);

  function abrirMenu(e: MouseEvent, caminho: string) {
    // Shift + botao direito deixa o menu do sistema aparecer, como no Overleaf
    if (e.shiftKey) {
      menuAba = null;
      return;
    }
    e.preventDefault();
    menuAba = { x: Math.min(e.clientX, window.innerWidth - 200), y: e.clientY, caminho };
  }

  function escolherDoMenu(acao: 'fechar' | 'outras') {
    const alvo = menuAba?.caminho;
    menuAba = null;
    if (!alvo || !podeFechar) return;
    if (acao === 'fechar') onfechar?.(alvo);
    else onfecharoutras?.(alvo);
  }
</script>

<svelte:window
  onpointerdown={(e) => { if (menuAba && !(e.target as HTMLElement).closest('.menu-aba')) menuAba = null; }}
  onkeydown={(e) => { if (e.key === 'Escape' && menuAba) menuAba = null; }}
  onblur={() => (menuAba = null)}
/>

<div class="faixa" class:arrastando>
  <div class="abas" role="tablist" aria-label={t('abas.lista')} bind:this={lista}>
    {#each visiveis as { aba, indice } (aba.caminho)}
      {@const ehAtiva = aba.caminho === ativa}
      {@const suja = sujas.includes(aba.caminho)}
      {@const nome = nomeDaAba(aba.caminho)}
      <div
        class="aba"
        class:ativa={ehAtiva}
        class:solta-esquerda={soltura?.caminho === aba.caminho && soltura.lado === 'esquerda'}
        class:solta-direita={soltura?.caminho === aba.caminho && soltura.lado === 'direita'}
        role="tab"
        id={idDaAba(indice)}
        data-caminho={aba.caminho}
        aria-selected={ehAtiva}
        aria-controls="painel-abas"
        tabindex={ehAtiva ? 0 : -1}
        title={aba.caminho.replace(/^\//, '')}
        onclick={() => aoClicar(aba.caminho)}
        onpointerdown={(e) => aoPressionar(e, aba.caminho)}
        oncontextmenu={(e) => abrirMenu(e, aba.caminho)}
        onmousedown={(e) => { if (e.button === 1) e.preventDefault(); }}
        onauxclick={(e) => { if (e.button === 1) { e.preventDefault(); if (podeFechar) onfechar?.(aba.caminho); } }}
        onkeydown={(e) => teclaNaAba(e, aba.caminho)}
      >
        <div class="conteudo">
          {#if aba.tipo === 'imagem'}<FileImage size={16} />{:else}<FileText size={16} />{/if}
          <span class="caminho">&lrm;{aba.caminho.replace(/^\//, '')}</span>
          {#if aba.caminho === principal}
            <span class="principal" role="img" title={t('abas.principal')} aria-label={t('abas.principal')}><Star size={11} /></span>
          {/if}
          <button
            type="button"
            class="fechar"
            class:suja
            tabindex="-1"
            disabled={!podeFechar}
            aria-label={suja ? `${tf('abas.fechar', { nome })}. ${t('abas.nao-salva')}` : tf('abas.fechar', { nome })}
            title={suja ? t('abas.nao-salva') : tf('abas.fechar', { nome })}
            onclick={(e) => fechar(e, aba.caminho)}
          >
            {#if suja}<span class="ponto" aria-hidden="true"></span>{/if}
            <span class="x" aria-hidden="true"><X size={16} /></span>
          </button>
        </div>
      </div>
    {/each}
  </div>
</div>

{#if menuAba}
  <div class="menu-aba" role="menu" aria-label={t('abas.menu')} style:left="{menuAba.x}px" style:top="{menuAba.y}px">
    <button type="button" role="menuitem" disabled={!podeFechar} onclick={() => escolherDoMenu('fechar')}>{t('abas.fechar-esta')}</button>
    <button type="button" role="menuitem" disabled={!podeFechar} onclick={() => escolherDoMenu('outras')}>{t('abas.fechar-outras')}</button>
  </div>
{/if}

<style>
  /* Medidas de tabs.scss do Overleaf (AGPL-3.0). Cores da paleta do EguaTex: faixa na superficie secundaria. */
  .faixa {
    display: flex;
    box-sizing: border-box;
    width: 100%;
    height: 36px;
    background: var(--superficie-1);
    border-bottom: 1px solid var(--borda);
    flex-shrink: 0;
  }

  /* a lista ocupa a largura toda e rola na horizontal sem barra visivel */
  .abas {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    flex-wrap: nowrap;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }

  .abas::-webkit-scrollbar {
    display: none;
  }

  /* casca da aba: padding 2px 4px 0 4px e divisor a direita; some perto da aba ativa */
  .aba {
    --fundo-aba: var(--superficie-1);
    --texto-aba: var(--texto-terciario);
    position: relative;
    flex: 0 0 auto;
    display: flex;
    box-sizing: border-box;
    height: 36px;
    padding: 2px 4px 0 4px;
    border-right: 1px solid var(--borda);
    cursor: pointer;
    user-select: none;
  }

  .aba:first-child {
    padding-left: 0;
  }

  .aba:last-child {
    padding-right: 0;
  }

  .aba.ativa,
  .aba:has(+ .aba.ativa) {
    border-right-color: transparent;
  }

  /* miolo: gap 4 px, padding 4px 6px, 14 px, borda de 1 px sem a de baixo */
  .conteudo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    box-sizing: border-box;
    padding: 4px 6px;
    border: 1px solid transparent;
    border-bottom-width: 0;
    background: var(--fundo-aba);
    color: var(--texto-aba);
    font-size: 0.875rem;
  }

  .aba:not(.ativa) .conteudo:hover {
    background: var(--superficie-2);
    border-radius: 8px;
    color: var(--texto);
  }

  /* aba ativa: fundo do editor, cantos superiores de 8 px, borda do divisor, sem risca de destaque */
  .aba.ativa {
    --fundo-aba: var(--superficie-0);
    --texto-aba: var(--texto);
  }

  .aba.ativa .conteudo {
    border-color: var(--borda);
    border-top-left-radius: 8px;
    border-top-right-radius: 8px;
  }

  /* indicador de soltura do arrastar: faixa de 5 px na cor de destaque */
  .aba.solta-esquerda::after,
  .aba.solta-direita::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    width: 5px;
    background: var(--primaria);
  }

  .aba.solta-esquerda::after {
    left: -2.5px;
  }

  .aba.solta-direita::after {
    right: -3.5px;
  }

  .arrastando .aba {
    cursor: grabbing;
  }

  .menu-aba {
    position: fixed;
    z-index: 1700;
    min-width: 180px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: var(--sombra-3);
  }

  .menu-aba button {
    height: 32px;
    padding: 0 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 0.8125rem;
    text-align: left;
    cursor: pointer;
  }

  .menu-aba button:hover:not(:disabled) {
    background: var(--hover);
  }

  .menu-aba button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .aba:focus-visible,
  .fechar:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  /* caminho relativo cortado a esquerda: direction rtl com reticencia, max 250 px */
  .caminho {
    max-width: 250px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    direction: rtl;
  }

  .principal {
    display: inline-flex;
    color: var(--acento);
  }

  .fechar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    padding: 0;
    border: none;
    border-radius: 100%;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .fechar:hover:not(:disabled) {
    background: var(--superficie-1);
    color: var(--texto);
  }

  /* ultima aba aberta: o botao fica visivel e desabilitado */
  .fechar:disabled {
    cursor: default;
    opacity: 0.4;
  }

  /* o x so aparece com o mouse na aba, na aba ativa ou com o foco no botao; o espaco fica reservado */
  .fechar:not(.suja) {
    opacity: 0;
  }

  .aba:hover .fechar:not(:disabled),
  .aba.ativa .fechar:not(:disabled),
  .fechar:focus-visible {
    opacity: 1;
  }

  .aba:hover .fechar:disabled,
  .aba.ativa .fechar:disabled {
    opacity: 0.4;
  }

  .ponto {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--primaria);
  }

  /* aba suja: o ponto fica no lugar do x; com o mouse em cima volta o x */
  .fechar.suja .x {
    display: none;
  }

  .aba:hover .fechar.suja .ponto,
  .fechar.suja:focus-visible .ponto {
    display: none;
  }

  .aba:hover .fechar.suja .x,
  .fechar.suja:focus-visible .x {
    display: inline-flex;
  }

  .x {
    display: inline-flex;
  }

  /* toque: alvo de 44 px */
  @media (pointer: coarse) {
    .faixa {
      height: 44px;
    }

    .aba {
      min-height: 44px;
      height: 44px;
    }

    .fechar {
      width: 44px;
      height: 44px;
    }
  }
</style>
