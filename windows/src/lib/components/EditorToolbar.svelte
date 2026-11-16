<script lang="ts">
  // Barra do editor. Ordem dos grupos, menus e transbordo adaptados de
  // services/web/frontend/js/features/source-editor/components/toolbar/ (Overleaf, AGPL-3.0).
  // A barra nao executa nada: cada controle so chama oncomando(id).
  import { onMount, tick, type Component } from 'svelte';
  import Undo2 from '@lucide/svelte/icons/undo-2';
  import Redo2 from '@lucide/svelte/icons/redo-2';
  import Bold from '@lucide/svelte/icons/bold';
  import Italic from '@lucide/svelte/icons/italic';
  import Calculator from '@lucide/svelte/icons/calculator';
  import Link from '@lucide/svelte/icons/link';
  import Tag from '@lucide/svelte/icons/tag';
  import BookMarked from '@lucide/svelte/icons/book-marked';
  import ImageIcon from '@lucide/svelte/icons/image';
  import TableIcon from '@lucide/svelte/icons/table';
  import ListIcon from '@lucide/svelte/icons/list';
  import ListOrdered from '@lucide/svelte/icons/list-ordered';
  import IndentDecrease from '@lucide/svelte/icons/list-indent-decrease';
  import IndentIncrease from '@lucide/svelte/icons/list-indent-increase';
  import Search from '@lucide/svelte/icons/search';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Check from '@lucide/svelte/icons/check';
  import MenuCategorias from './MenuCategorias.svelte';
  import { alternarCategoria } from '$lib/atalhos.svelte';
  import Monitor from '@lucide/svelte/icons/monitor';
  import FileText from '@lucide/svelte/icons/file-text';
  import TypeIcon from '@lucide/svelte/icons/type';
  import IconeDrive from './IconeDrive.svelte';
  import { t, tf } from '$lib/i18n';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import Pencil from '@lucide/svelte/icons/pencil';
  import MessageSquareDiff from '@lucide/svelte/icons/message-square-diff';
  import CheckCheck from '@lucide/svelte/icons/check-check';
  import XIcon from '@lucide/svelte/icons/x';
  import Eye from '@lucide/svelte/icons/eye';
  import type { ModoEdicao } from '$lib/modo-edicao';
  import { calcularTransbordo } from '$lib/transbordo';

  interface Props {
    oncomando?: (id: string) => void;
    estilo?: string;
    /** Codigo/Visual, Editando/Revisando/Visualizando e numero de sugestoes pendentes. */
    modos?: { visual: boolean; modo: ModoEdicao; sugestoes: number; desfazer?: boolean; refazer?: boolean };
    /** Arquivo LaTeX: fora dele (.bib, .txt) so ficam desfazer, refazer e a zona direita (toolbar-items.tsx). */
    latex?: boolean;
    /** Listas abertas no cursor: o recuo so aparece dentro de uma e "Diminuir" so com duas ou mais. */
    profundidadeLista?: number;
    /** Mostra "Ir para o PDF" (so no layout Somente editor). */
    irParaPdf?: boolean;
  }

  let {
    oncomando,
    estilo = 'normal',
    modos = { visual: false, modo: 'editando', sugestoes: 0 },
    latex = true,
    profundidadeLista = 0,
    irParaPdf = false,
  }: Props = $props();

  // Em Visualizando a barra esconde desfazer, refazer e a formatacao (canEdit em toolbar-layout.tsx).
  const podeEditar = $derived(modos.modo !== 'visualizando');
  // Codigo e Visual usam a mesma barra; quem decide o que vai para o "mais" e so o transbordo por largura.
  const noMais = (id: IdUnidade) => transbordados.includes(id);
  const iconesModo = { editando: Pencil, revisando: MessageSquareDiff, visualizando: Eye } satisfies Record<ModoEdicao, TipoIcone>;
  const IconeModo = $derived(iconesModo[modos.modo]);

  let modoAberto = $state(false);
  let modoPos = $state({ direita: 0, topo: 0 });

  function alternarModo(e: MouseEvent) {
    if (modoAberto) {
      modoAberto = false;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = null;
    maisAberto = false;
    modoPos = { direita: Math.max(8, window.innerWidth - r.right), topo: r.bottom + 4 };
    modoAberto = true;
  }

  function escolherModo(id: string) {
    modoAberto = false;
    oncomando?.(id);
  }

  // qualquer icone do Lucide (cada um e importado pelo caminho individual)
  type TipoIcone = Component<any>;
  type Local = 'barra' | 'mais';
  type IdMenu = 'estilo' | 'matematica' | 'figura' | 'tabela' | 'listas';
  type IdUnidade = 'estilo' | 'formato' | 'matematica' | 'link' | 'referencia' | 'citacao' | 'figura' | 'tabela' | 'listas';

  interface MenuAberto {
    id: IdMenu;
    local: Local;
    x: number;
    y: number;
  }

  // Cada unidade pode sair da barra sozinha, de tras para frente, como em overflow.tsx. A unidade "formato"
  // (negrito e italico) nunca sai: no Overleaf esse grupo nao tem data-overflow (toolbar-items.tsx).
  // "inicio" marca o primeiro item de um grupo (leva o separador a esquerda).
  const UNIDADES: { id: IdUnidade; inicio: boolean }[] = [
    { id: 'estilo', inicio: true },
    { id: 'formato', inicio: true },
    { id: 'matematica', inicio: true },
    { id: 'link', inicio: true },
    { id: 'referencia', inicio: false },
    { id: 'citacao', inicio: false },
    { id: 'figura', inicio: false },
    { id: 'tabela', inicio: false },
    { id: 'listas', inicio: true },
  ];

  const ESTILOS = [
    { valor: 'normal', chave: 'barra-editor.estilo.normal' },
    { valor: 'secao', chave: 'barra-editor.estilo.secao' },
    { valor: 'subsecao', chave: 'barra-editor.estilo.subsecao' },
    { valor: 'subsubsecao', chave: 'barra-editor.estilo.subsubsecao' },
    { valor: 'paragrafo', chave: 'barra-editor.estilo.paragrafo' },
    { valor: 'subparagrafo', chave: 'barra-editor.estilo.subparagrafo' },
  ] as const;

  const ROTULO_MENU = {
    estilo: 'barra-editor.menu.estilo',
    matematica: 'barra-editor.menu.matematica',
    figura: 'barra-editor.menu.figura',
    tabela: 'barra-editor.menu.tabela',
    listas: 'barra-editor.menu.listas',
  } as const satisfies Record<IdMenu, string>;

  const TAMANHO_GRADE = 10;
  const INDICES = Array.from({ length: TAMANHO_GRADE }, (_, i) => i + 1);

  const rotuloEstilo = $derived(t(ESTILOS.find((e) => e.valor === estilo)?.chave ?? 'barra-editor.estilo.normal'));

  let menu = $state<MenuAberto | null>(null);
  let maisAberto = $state(false);
  let maisPos = $state({ direita: 0, topo: 0 });
  let transbordados = $state<IdUnidade[]>([]);
  let selTabela = $state({ l: 0, c: 0 });

  let area: HTMLDivElement | null = null;
  let gatilhoMais: HTMLButtonElement | null = null;
  let gatilhoMenu: HTMLElement | null = null;
  let popMenu = $state<HTMLElement | null>(null);
  let popMais = $state<HTMLElement | null>(null);

  const menuAberto = (id: IdMenu, local: Local) => menu !== null && menu.id === id && menu.local === local;

  function executar(id: string) {
    menu = null;
    maisAberto = false;
    oncomando?.(id);
  }

  // o clique nao tira o foco do editor
  function manterFoco(e: MouseEvent) {
    e.preventDefault();
  }

  async function focarPrimeiroItem() {
    await tick();
    if (menu?.id === 'tabela') popMenu?.focus();
    else (popMenu?.querySelector<HTMLElement>('[role="menuitem"].ativo') ?? popMenu?.querySelector<HTMLElement>('[role="menuitem"]'))?.focus();
  }

  function alternarMenu(id: IdMenu, local: Local, e: MouseEvent) {
    if (menuAberto(id, local)) {
      menu = null;
      return;
    }
    const gatilho = e.currentTarget as HTMLElement;
    const r = gatilho.getBoundingClientRect();
    gatilhoMenu = gatilho;
    selTabela = { l: 0, c: 0 };
    menu = { id, local, x: r.left, y: r.bottom + 2 };
    // detail 0 significa que o clique veio do teclado (Enter ou Espaco)
    if (e.detail === 0) focarPrimeiroItem();
  }

  async function alternarMais(e: MouseEvent) {
    if (maisAberto) {
      maisAberto = false;
      menu = null;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = null;
    maisPos = { direita: Math.max(8, window.innerWidth - r.right), topo: r.bottom + 2 };
    maisAberto = true;
    if (e.detail === 0) {
      await tick();
      popMais?.querySelector<HTMLElement>('button:not([disabled])')?.focus();
    }
  }

  // Mantem o popover dentro da janela.
  function ajustar(no: HTMLElement) {
    const folga = 8;
    const r = no.getBoundingClientRect();
    if (r.right > window.innerWidth - folga) {
      no.style.left = `${Math.max(folga, window.innerWidth - folga - r.width)}px`;
    }
  }

  function teclaMenu(e: KeyboardEvent) {
    if (menu?.id === 'tabela') {
      teclaTabela(e);
      return;
    }
    if (e.key === 'Tab') {
      menu = null;
      return;
    }
    const itens = Array.from(popMenu?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);
    if (itens.length === 0) return;
    const atual = itens.indexOf(document.activeElement as HTMLElement);
    let alvo = -1;
    if (e.key === 'ArrowDown') alvo = (atual + 1) % itens.length;
    else if (e.key === 'ArrowUp') alvo = (atual - 1 + itens.length) % itens.length;
    else if (e.key === 'Home') alvo = 0;
    else if (e.key === 'End') alvo = itens.length - 1;
    if (alvo >= 0) {
      e.preventDefault();
      itens[alvo].focus();
    }
  }

  const SETAS: Record<string, [number, number] | undefined> = {
    ArrowUp: [-1, 0],
    ArrowDown: [1, 0],
    ArrowLeft: [0, -1],
    ArrowRight: [0, 1],
  };

  function teclaTabela(e: KeyboardEvent) {
    const passo = SETAS[e.key];
    if (passo) {
      e.preventDefault();
      const limitar = (n: number) => Math.min(TAMANHO_GRADE, Math.max(1, n));
      selTabela = {
        l: selTabela.l === 0 ? 1 : limitar(selTabela.l + passo[0]),
        c: selTabela.c === 0 ? 1 : limitar(selTabela.c + passo[1]),
      };
    } else if ((e.key === 'Enter' || e.key === ' ') && selTabela.l > 0) {
      e.preventDefault();
      executar(`tabela:${selTabela.l}x${selTabela.c}`);
    }
  }

  // Largura do grupo do "mais" quando aparece: botao de 24 px com margem de 1 px dos dois lados e padding de 4 px do grupo.
  const LARGURA_MAIS = 34;

  // Mede os grupos com todos visiveis e esconde os de tras para frente ate caber (calcularTransbordo).
  function recalcular() {
    if (!area) return;
    const unidades = Array.from(area.querySelectorAll<HTMLElement>('[data-unidade]'));
    for (const u of unidades) u.removeAttribute('data-oculto');
    area.removeAttribute('data-estourou');
    const fixo = Array.from(area.querySelectorAll<HTMLElement>('.grupo-fixo, .unidade:not([data-unidade])')).reduce((soma, el) => soma + el.offsetWidth, 0);
    const saem = calcularTransbordo(unidades.map((u) => u.offsetWidth), area.clientWidth, { fixo, mais: LARGURA_MAIS });
    for (const i of saem) unidades[i].setAttribute('data-oculto', '');
    if (saem.length > 0) area.setAttribute('data-estourou', '');
    const ocultos = saem.map((i) => unidades[i].dataset.unidade ?? '');
    const novos = UNIDADES.map((u) => u.id).filter((id) => ocultos.includes(id));
    if (novos.length !== transbordados.length || novos.some((id, i) => id !== transbordados[i])) {
      transbordados = novos;
    }
    if (novos.length === 0) maisAberto = false;
  }

  // Quando entra ou sai um grupo (Visualizando, arquivo que nao e LaTeX) ou o recuo aparece ou some (profundidade da lista),
  // as larguras mudam: fecha os menus e mede de novo.
  $effect(() => {
    void podeEditar;
    void latex;
    void profundidadeLista;
    void modos.visual;
    menu = null;
    maisAberto = false;
    void tick().then(recalcular);
  });

  onMount(() => {
    const observador = new ResizeObserver(() => recalcular());
    if (area) observador.observe(area);
    // grupos que entram ou saem do DOM da barra (o Overleaf tambem observa mutacoes: codemirror-toolbar.tsx)
    const mutacoes = new MutationObserver(() => recalcular());
    if (area) mutacoes.observe(area, { childList: true });
    recalcular();
    document.fonts?.ready.then(recalcular);

    const fecharTudo = () => {
      menu = null;
      maisAberto = false;
    };

    // clique fora fecha; clicar dentro do menu aberto a partir do "..." nao fecha o "..."
    const aoPressionar = (e: PointerEvent) => {
      const alvo = e.target as Node | null;
      if (!alvo) return;
      const emMenu = !!popMenu?.contains(alvo) || !!gatilhoMenu?.contains(alvo);
      const emMais = !!popMais?.contains(alvo) || !!gatilhoMais?.contains(alvo);
      if (menu && !emMenu) menu = null;
      if (maisAberto && !emMais && !emMenu) maisAberto = false;
    };

    const aoTeclar = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || (!menu && !maisAberto)) return;
      e.preventDefault();
      e.stopPropagation();
      const gatilho = menu ? gatilhoMenu : gatilhoMais;
      const foco = document.activeElement;
      const focoNoPopover = !!foco && (!!popMenu?.contains(foco) || !!popMais?.contains(foco) || foco === gatilho);
      if (menu) menu = null;
      else maisAberto = false;
      if (focoNoPopover && gatilho?.isConnected) gatilho.focus();
    };

    window.addEventListener('pointerdown', aoPressionar, true);
    window.addEventListener('keydown', aoTeclar, true);
    window.addEventListener('resize', fecharTudo);
    return () => {
      observador.disconnect();
      mutacoes.disconnect();
      window.removeEventListener('pointerdown', aoPressionar, true);
      window.removeEventListener('keydown', aoTeclar, true);
      window.removeEventListener('resize', fecharTudo);
    };
  });
</script>

{#snippet botao(id: string, Icone: TipoIcone, rotulo: string, atalho?: string, ativo?: boolean, desabilitado = false)}
  <button
    type="button"
    class="tb"
    disabled={desabilitado}
    class:ativo={ativo === true}
    title={atalho ? `${rotulo} (${atalho})` : rotulo}
    aria-label={rotulo}
    aria-pressed={ativo}
    onmousedown={manterFoco}
    onclick={() => executar(id)}
  >
    <Icone size={20} />
  </button>
{/snippet}

{#snippet gatilho(id: IdMenu, local: Local, Icone: TipoIcone, rotulo: string)}
  {@const aberto = menuAberto(id, local)}
  <button
    type="button"
    class="tb"
    class:ativo={aberto}
    title={rotulo}
    aria-label={rotulo}
    aria-haspopup="menu"
    aria-expanded={aberto}
    onmousedown={manterFoco}
    onclick={(e) => alternarMenu(id, local, e)}
  >
    <Icone size={20} />
  </button>
{/snippet}

{#snippet item(id: string, Icone: TipoIcone, rotulo: string)}
  <button type="button" role="menuitem" class="item" onmousedown={manterFoco} onclick={() => executar(id)}>
    <Icone size={16} />
    <span>{rotulo}</span>
  </button>
{/snippet}

{#snippet conteudo(id: IdUnidade, local: Local)}
  {#if id === 'estilo'}
    {@const aberto = menuAberto('estilo', local)}
    <button
      type="button"
      class="tb estilo"
      class:ativo={aberto}
      title={tf('barra-editor.estilo-valor', { estilo: rotuloEstilo })}
      aria-label={tf('barra-editor.estilo-valor', { estilo: rotuloEstilo })}
      aria-haspopup="menu"
      aria-expanded={aberto}
      onmousedown={manterFoco}
      onclick={(e) => alternarMenu('estilo', local, e)}
    >
      <TypeIcon size={20} />
      <ChevronDown size={20} />
    </button>
  {:else if id === 'formato'}
    {@render botao('negrito', Bold, t('barra-editor.negrito'), 'Ctrl+B')}
    {@render botao('italico', Italic, t('barra-editor.italico'), 'Ctrl+I')}
  {:else if id === 'matematica'}
    {@render gatilho('matematica', local, Calculator, t('barra-editor.simbolos'))}
  {:else if id === 'link'}
    {@render botao('link', Link, t('barra-editor.link'))}
  {:else if id === 'referencia'}
    {@render botao('referencia', Tag, t('barra-editor.referencia'))}
  {:else if id === 'citacao'}
    {@render botao('citacao', BookMarked, t('barra-editor.citacao'))}
  {:else if id === 'figura'}
    {@render gatilho('figura', local, ImageIcon, t('barra-editor.figura'))}
  {:else if id === 'tabela'}
    {@render gatilho('tabela', local, TableIcon, t('barra-editor.tabela'))}
  {:else if id === 'listas'}
    {@render gatilho('listas', local, ListIcon, t('barra-editor.lista'))}
    {#if profundidadeLista >= 1}
      {@render botao('diminuir-recuo', IndentDecrease, t('barra-editor.diminuir-recuo'), undefined, undefined, profundidadeLista < 2)}
      {@render botao('aumentar-recuo', IndentIncrease, t('barra-editor.aumentar-recuo'))}
    {/if}
  {/if}
{/snippet}

<svelte:window onclick={(e) => { if (modoAberto && !(e.target as HTMLElement).closest('.modo-edicao, .modo-pop')) modoAberto = false; }} onkeydown={(e) => { if (e.key === 'Escape') modoAberto = false; }} />

<div class="editor-toolbar" role="toolbar" aria-label={t('barra-editor.rotulo')}>
  <div class="area" bind:this={area}>
    {#if podeEditar}
      <div class="grupo-fixo">
        {@render botao('desfazer', Undo2, t('barra-editor.desfazer'), 'Ctrl+Z', undefined, modos.desfazer === false)}
        {@render botao('refazer', Redo2, t('barra-editor.refazer'), 'Ctrl+Y', undefined, modos.refazer === false)}
      </div>

      {#if latex}
        {#each UNIDADES as u (u.id)}
          <div class="unidade" class:inicio={u.inicio} data-unidade={u.id === 'formato' ? undefined : u.id}>
            {@render conteudo(u.id, 'barra')}
          </div>
        {/each}
      {/if}
    {/if}

    <div class="mais">
      <button
        type="button"
        class="tb"
        class:ativo={maisAberto}
        title={t('barra-editor.mais')}
        aria-label={t('barra-editor.mais')}
        aria-haspopup="true"
        aria-expanded={maisAberto}
        bind:this={gatilhoMais}
        onmousedown={manterFoco}
        onclick={alternarMais}
      >
        <Ellipsis size={20} />
      </button>
    </div>
  </div>

  <!-- Ordem do lado direito como em toolbar-layout.tsx do Overleaf: Codigo/Visual, modo, busca. -->
  <div class="direita">
    <div class="alternador" role="radiogroup" aria-label={t('barra-editor.modo-grupo')}>
      <button type="button" role="radio" aria-checked={!modos.visual} class:marcado={!modos.visual} onmousedown={manterFoco} onclick={() => oncomando?.('modo-codigo')} title={t('barra-editor.codigo-titulo')}>{t('barra-editor.codigo')}</button>
      <button type="button" role="radio" aria-checked={modos.visual} class:marcado={modos.visual} onmousedown={manterFoco} onclick={() => oncomando?.('modo-visual')} title={t('barra-editor.visual-titulo')}>{t('barra-editor.visual')}</button>
    </div>
    <button
      type="button"
      class="modo-edicao"
      class:revisando={modos.modo === 'revisando'}
    class:visualizando={modos.modo === 'visualizando'}
      title={t(`barra-editor.modo-${modos.modo}-titulo`)}
      aria-haspopup="menu"
      aria-expanded={modoAberto}
      onmousedown={manterFoco}
      onclick={alternarModo}
    >
      <IconeModo size={16} />
      <span class="modo-rotulo">{t(`barra-editor.modo-${modos.modo}`)}</span>
      {#if modos.sugestoes > 0}<span class="contador">{modos.sugestoes}</span>{/if}
      <ChevronDown size={16} />
    </button>
    {@render botao('localizar', Search, t('barra-editor.localizar'), 'Ctrl+F')}
    {#if irParaPdf}
      <button type="button" class="ir-pdf" onmousedown={manterFoco} onclick={() => oncomando?.('layout-pdf')}>
        <FileText size={14} />
        <span>{t('barra-editor.ir-para-pdf')}</span>
      </button>
    {/if}
  </div>

  {#if modoAberto}
    <div class="popover modo-pop" role="menu" aria-label={t('barra-editor.modo-menu')} style:right="{modoPos.direita}px" style:top="{modoPos.topo}px">
      <button type="button" role="menuitem" class="modo-item" class:escolhido={modos.modo === 'editando'} onmousedown={manterFoco} onclick={() => escolherModo('modo-editando')}>
        <Pencil size={16} />
        <span><strong>{t('barra-editor.modo-editando')}</strong><small>{t('barra-editor.modo-editando-desc')}</small></span>
      </button>
      <button type="button" role="menuitem" class="modo-item" class:escolhido={modos.modo === 'revisando'} onmousedown={manterFoco} onclick={() => escolherModo('modo-revisando')}>
        <MessageSquareDiff size={16} />
        <span><strong>{t('barra-editor.modo-revisando')}</strong><small>{t('barra-editor.modo-revisando-desc')}</small></span>
      </button>
      <button type="button" role="menuitem" class="modo-item" class:escolhido={modos.modo === 'visualizando'} onmousedown={manterFoco} onclick={() => escolherModo('modo-visualizando')}>
        <Eye size={16} />
        <span><strong>{t('barra-editor.modo-visualizando')}</strong><small>{t('barra-editor.modo-visualizando-desc')}</small></span>
      </button>
      {#if modos.sugestoes > 0}
        <div class="modo-sep"></div>
        <button type="button" role="menuitem" class="modo-item" onmousedown={manterFoco} onclick={() => escolherModo('aceitar-todas')}>
          <CheckCheck size={16} />
          <span><strong>{t('barra-editor.aceitar-todas')}</strong><small>{tf(modos.sugestoes === 1 ? 'barra-editor.aceitar-desc-um' : 'barra-editor.aceitar-desc-varios', { n: modos.sugestoes })}</small></span>
        </button>
        <button type="button" role="menuitem" class="modo-item" onmousedown={manterFoco} onclick={() => escolherModo('rejeitar-todas')}>
          <XIcon size={16} />
          <span><strong>{t('barra-editor.rejeitar-todas')}</strong><small>{t('barra-editor.rejeitar-desc')}</small></span>
        </button>
      {/if}
    </div>
  {/if}

  {#if menu}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="popover"
      class:grade-pop={menu.id === 'tabela'}
      role={menu.id === 'tabela' ? 'dialog' : 'menu'}
      aria-label={t(ROTULO_MENU[menu.id])}
      tabindex="-1"
      style:left="{menu.x}px"
      style:top="{menu.y}px"
      style:max-height="calc(100dvh - {menu.y}px - 8px)"
      bind:this={popMenu}
      use:ajustar
      onkeydown={teclaMenu}
    >
      {#if menu.id === 'estilo'}
        {#each ESTILOS as e (e.valor)}
          <button
            type="button"
            role="menuitem"
            class="item item-estilo item-{e.valor}"
            class:ativo={e.valor === estilo}
            onmousedown={manterFoco}
            onclick={() => executar('estilo-' + e.valor)}
          >
            <span>{t(e.chave)}</span>
            {#if e.valor === estilo}
              <span class="marca"><Check size={14} /></span>
            {/if}
          </button>
        {/each}
      {:else if menu.id === 'matematica'}
        <!-- categorias da barra Matematica; os ambientes e a matematica em linha ficam na categoria Matematica -->
        <MenuCategorias
          aoescolher={(id) => {
            menu = null;
            alternarCategoria(id);
          }}
          aoacao={executar}
        />
      {:else if menu.id === 'figura'}
        {@render item('figura-projeto', FolderOpen, t('barra-editor.figura-projeto'))}
        {@render item('figura-computador', Monitor, t('barra-editor.figura-computador'))}
        {@render item('figura-drive', IconeDrive, t('figura.do-drive'))}
      {:else if menu.id === 'listas'}
        {@render item('lista-marcadores', ListIcon, t('barra-editor.lista-marcadores'))}
        {@render item('lista-numerada', ListOrdered, t('barra-editor.lista-numerada'))}
      {:else if menu.id === 'tabela'}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="grade" onmouseleave={() => (selTabela = { l: 0, c: 0 })}>
          {#each INDICES as l (l)}
            {#each INDICES as c (c)}
              <button
                type="button"
                tabindex="-1"
                class="celula"
                class:marcada={l <= selTabela.l && c <= selTabela.c}
                aria-label={tf('barra-editor.tabela-celula', { l, c })}
                onmousedown={manterFoco}
                onmouseenter={() => (selTabela = { l, c })}
                onclick={() => executar(`tabela:${l}x${c}`)}
              ></button>
            {/each}
          {/each}
        </div>
        <div class="grade-rotulo" aria-live="polite">
          {#if selTabela.l > 0}
            <strong>{selTabela.l} x {selTabela.c}</strong> {t('barra-editor.tabela-dimensoes')}
          {:else}
            {t('barra-editor.tabela-inserir')}
          {/if}
        </div>
        <button type="button" class="grade-assistente" onmousedown={manterFoco} onclick={() => executar('tabela-assistente')}>
          {t('barra-editor.tabela-assistente')}
        </button>
      {/if}
    </div>
  {/if}

  {#if maisAberto}
    <div
      class="popover mais-pop"
      role="toolbar"
      aria-label={t('barra-editor.mais')}
      style:right="{maisPos.direita}px"
      style:top="{maisPos.topo}px"
      bind:this={popMais}
    >
      {#each UNIDADES as u (u.id)}
        {#if noMais(u.id)}
          <div class="unidade" class:inicio={u.inicio}>
            {@render conteudo(u.id, 'mais')}
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Alternador Codigo/Visual (toolbar.scss do Overleaf): pista de 26 px com padding 2 px e raio total */
  .alternador {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    box-sizing: border-box;
    height: 26px;
    padding: 2px;
    border-radius: 999px;
    background: var(--superficie-2, var(--hover));
  }

  /* rotulo de 22 px, padding simetrico de 14 px (o texto fica centrado na pilula marcada), 14 px negrito */
  .alternador button {
    border: none;
    border-radius: 999px;
    height: 22px;
    line-height: 22px;
    padding: 0 14px;
    background: transparent;
    color: var(--texto-secundario);
    font: inherit;
    font-size: 0.875rem;
    font-weight: 700;
    cursor: pointer;
    transition:
      color 0.12s ease-out,
      background-color 0.12s ease-out,
      box-shadow 0.12s ease-out;
  }

  .alternador button.marcado {
    background: var(--primaria);
    color: #fff;
    box-shadow: 0 2px 4px rgb(30 37 48 / 16%);
  }

  /* botao de modo (review-panel.scss): raio 4 px, padding 2px 4px, gap 2 px, altura automatica, 14 px */
  .modo-edicao {
    --modo-revisando-fundo: #fcf1e3;
    --modo-revisando-texto: #7a4304;
    --modo-visualizando-fundo: #f1f4f9;
    --modo-visualizando-texto: #214475;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: auto;
    padding: 2px 4px;
    border: none;
    border-radius: 4px;
    background: var(--hover);
    color: var(--texto);
    font: inherit;
    font-size: 0.875rem;
    white-space: nowrap;
    cursor: pointer;
  }

  :global(:root[data-tema='escuro']) .modo-edicao {
    --modo-revisando-fundo: #633a0b;
    --modo-revisando-texto: #fcf1e3;
    --modo-visualizando-fundo: #214475;
    --modo-visualizando-texto: #f1f4f9;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-tema='claro'])) .modo-edicao {
      --modo-revisando-fundo: #633a0b;
      --modo-revisando-texto: #fcf1e3;
      --modo-visualizando-fundo: #214475;
      --modo-visualizando-texto: #f1f4f9;
    }
  }

  .modo-edicao.revisando {
    background: var(--modo-revisando-fundo);
    color: var(--modo-revisando-texto);
  }

  .modo-edicao.visualizando {
    background: var(--modo-visualizando-fundo);
    color: var(--modo-visualizando-texto);
  }

  /* "Ir para o PDF": botao secundario pequeno, padding 2px 12px, icone de 14 px */
  .ir-pdf {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 12px;
    border: 1px solid var(--borda);
    border-radius: 4px;
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 0.875rem;
    white-space: nowrap;
    cursor: pointer;
  }

  .ir-pdf:hover {
    background: var(--hover);
  }

  .contador {
    min-width: 16px;
    padding: 0 4px;
    border-radius: 999px;
    background: #2e7d32;
    color: #fff;
    font-size: 0.6875rem;
    font-weight: 700;
    text-align: center;
  }

  .modo-pop {
    left: auto;
    min-width: 240px;
    padding: 6px;
  }

  .modo-item {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px 12px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--texto);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .modo-item span {
    display: flex;
    flex-direction: column;
  }

  .modo-item small {
    font-size: 0.75rem;
    opacity: 0.75;
  }

  .modo-item:hover {
    background: var(--hover);
  }

  .modo-item.escolhido {
    background: #2e7d32;
    color: #fff;
  }

  .modo-sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--borda);
  }

  /* barra de 32 px com padding 0 5px (toolbar.scss e variables.scss do Overleaf); a borda de baixo so existe no tema escuro */
  .editor-toolbar {
    height: 32px;
    box-sizing: border-box;
    padding: 0 5px;
    display: flex;
    align-items: center;
    background: var(--superficie-0);
    flex-shrink: 0;
  }

  :global(:root[data-tema='escuro']) .editor-toolbar {
    border-bottom: 1px solid var(--borda);
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-tema='claro'])) .editor-toolbar {
      border-bottom: 1px solid var(--borda);
    }
  }

  .area {
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    overflow: hidden;
  }

  .grupo-fixo,
  .unidade,
  .direita {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    white-space: nowrap;
  }

  /* Medidas de editor-toolbar.scss do Overleaf: grupo com padding 0 4px e margem 4px 0. */
  .grupo-fixo,
  .unidade {
    padding: 0 4px;
    margin: 4px 0;
    line-height: 1;
  }

  .unidade:global([data-oculto]) {
    display: none;
  }

  /* grupo sem botao perde borda e padding (toolbar-panel.ts) */
  .grupo-fixo:empty,
  .unidade:empty {
    padding: 0;
    border-left: none;
  }

  /* separador entre grupos: borda esquerda do grupo */
  .unidade.inicio {
    border-left: 1px solid rgb(125 125 125 / 30%);
  }

  /* lado direito: espaco de 6 px e divisoria de 1x16 px */
  .direita {
    gap: 6px;
  }

  .direita::before {
    content: '';
    flex-shrink: 0;
    width: 1px;
    height: 16px;
    background: rgb(125 125 125 / 30%);
  }

  /* grupo elastico do "..." (flex: 1, entre a esquerda e a direita): so aparece quando alguma unidade saiu da barra */
  .mais {
    display: none;
    align-items: center;
    flex: 1 1 auto;
    padding: 0 4px;
    margin: 4px 0;
  }

  .area:global([data-estourou]) .mais {
    display: flex;
  }

  .tb {
    flex: 0 0 auto;
    width: 24px;
    height: 24px;
    margin: 0 1px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font: inherit;
    cursor: pointer;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .tb:hover:not(:disabled) {
    background: rgb(125 125 125 / 10%);
    color: var(--texto);
  }

  .tb:disabled {
    opacity: 0.2;
    cursor: not-allowed;
  }

  .tb:active:not(:disabled),
  .tb.ativo {
    background: rgb(125 125 125 / 20%);
    color: var(--primaria);
  }

  .tb:focus-visible,
  .item:focus-visible,
  .popover:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  /* estilo: icone e seta de 20 px nos dois modos, largura automatica; o estilo atual fica no title e no aria-label */
  .tb.estilo {
    width: auto;
    padding: 0;
  }

  .popover {
    position: fixed;
    z-index: 1500;
    box-sizing: border-box;
    min-width: 200px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: var(--sombra-3);
  }

  /* menus abertos a partir do popup "Mais" (ex.: Figura) ficam acima dele; os dois tem a mesma camada base */
  .popover:not(.mais-pop):not(.modo-pop) {
    z-index: 1600;
  }

  .popover:focus {
    outline: none;
  }

  .popover.mais-pop {
    min-width: 0;
    padding: 0;
    max-width: min(420px, calc(100vw - 16px));
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 0;
  }

  .popover.mais-pop .unidade.inicio {
    border-left: none;
  }

  .popover.mais-pop .unidade {
    margin-right: 8px;
  }

  .popover.grade-pop {
    min-width: 0;
    padding: 8px;
  }

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

  .item :global(svg) {
    flex-shrink: 0;
    color: var(--texto-secundario);
  }

  .item-estilo {
    height: 36px;
    font-weight: 700;
  }

  .item-estilo.item-normal {
    font-weight: 400;
  }

  .item-estilo.item-secao {
    font-size: 1.15rem;
  }

  .item-estilo.item-subsecao {
    font-size: 1rem;
  }

  .marca {
    margin-left: auto;
    padding-left: 16px;
    display: inline-flex;
    color: var(--primaria);
  }

  .item .marca :global(svg) {
    color: var(--primaria);
  }

  .grade {
    display: grid;
    grid-template-columns: repeat(10, 16px);
    gap: 2px;
  }

  .celula {
    width: 16px;
    height: 16px;
    padding: 0;
    border: 1px solid var(--borda);
    border-radius: 2px;
    background: var(--superficie-0);
    cursor: pointer;
  }

  .celula.marcada {
    border-color: var(--primaria);
    background: var(--primaria-suave);
  }

  .grade-rotulo {
    margin-top: 6px;
    text-align: center;
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .grade-assistente {
    display: block;
    width: 100%;
    margin-top: 6px;
    padding: 5px 8px;
    border: none;
    border-top: 1px solid var(--borda);
    background: transparent;
    color: var(--primaria);
    font: inherit;
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .grade-assistente:hover {
    background: var(--hover);
  }

  .grade-rotulo strong {
    color: var(--texto);
  }
</style>
