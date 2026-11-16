<!--
  Arvore de arquivos do projeto (recursiva) com menu de contexto: renomear, baixar e excluir.
  Comportamento baseado em features/file-tree do Overleaf (AGPL-3.0), reescrito em Svelte.
-->
<script lang="ts" module>
  export interface FileNode {
    nome: string;
    tipo: 'arquivo' | 'pasta';
    caminho: string;
    filhos?: FileNode[];
    aberta?: boolean;
    /** Bytes do arquivo, quando o backend informa. */
    tamanho?: number;
  }

  /** Chave do contexto que a arvore raiz divide com as arvores filhas (pastas abertas). */
  const CHAVE_ESTADO_ARVORE = Symbol('eguatex.arvore-de-arquivos');

  const CARACTERES_INVALIDOS = /[\u0000-\u001f\/\\:*?"<>|]/;
  const NOMES_RESERVADOS = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$/i;

  /** Troca \ por / e tira a barra do inicio, para comparar caminhos do projeto. */
  export function normalizarCaminho(caminho: string): string {
    return caminho.replace(/\\/g, '/').replace(/^\/+/, '');
  }

  /** Mensagem de erro para o nome de um arquivo ou pasta. Texto vazio quando o nome serve. */
  export function erroDeNome(nome: string): string {
    const limpo = nome.trim();
    if (limpo === '') return 'Informe um nome.';
    if (CARACTERES_INVALIDOS.test(limpo)) return 'O nome não pode ter / \\ : * ? " < > |';
    if (limpo.endsWith('.')) return 'O nome não pode terminar com ponto.';
    if (NOMES_RESERVADOS.test(limpo)) return 'Esse nome é reservado pelo Windows.';
    return '';
  }

  /** Mantem o foco do teclado dentro de uma janela: Tab e Shift+Tab dao a volta nos controles. */
  export function prenderFoco(e: KeyboardEvent, janela: HTMLElement): void {
    const controles = Array.from(
      janela.querySelectorAll<HTMLElement>('input:not(:disabled), button:not(:disabled)'),
    );
    if (controles.length === 0) return;
    const primeiro = controles[0];
    const ultimo = controles[controles.length - 1];
    const ativo = document.activeElement;
    const dentro = ativo instanceof Node && janela.contains(ativo);
    if (e.shiftKey && (!dentro || ativo === primeiro)) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && (!dentro || ativo === ultimo)) {
      e.preventDefault();
      primeiro.focus();
    }
  }
</script>

<script lang="ts">
  import { getContext, setContext, tick } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import FileTree from './FileTree.svelte';
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Folder from "@lucide/svelte/icons/folder";
  import FileText from "@lucide/svelte/icons/file-text";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import FileImage from "@lucide/svelte/icons/file-image";
  import IconeArquivo from "@lucide/svelte/icons/file";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Download from "@lucide/svelte/icons/download";
  import Trash from "@lucide/svelte/icons/trash";
  import { formatarTamanho } from '$lib/arquivos-util';
  import { t } from '$lib/i18n';

  interface Props {
    arquivos: FileNode[];
    selecionado?: string;
    /** Caminho do documento principal; a linha dele ganha o selo "principal". */
    principal?: string;
    onselecionar?: (caminho: string) => void;
    onrenomearitem?: (caminho: string, novoNome: string) => void;
    onexcluiritem?: (caminho: string) => void;
    onbaixaritem?: (caminho: string) => void;
  }

  let {
    arquivos,
    selecionado = '',
    principal = '',
    onselecionar,
    onrenomearitem,
    onexcluiritem,
    onbaixaritem,
  }: Props = $props();

  const uid = $props.id();

  // ---------- Pastas abertas ----------
  // A arvore raiz cria o estado e as arvores filhas (uma por pasta aberta) usam o mesmo.
  // Assim a escolha do usuario nao some quando uma pasta fecha e abre, e os dados recebidos
  // por props nunca sao alterados. O campo aberta de FileNode vale como estado inicial.
  interface EstadoArvore {
    abertas: SvelteMap<string, boolean>;
  }

  const estadoPai = getContext<EstadoArvore | undefined>(CHAVE_ESTADO_ARVORE);
  const ehRaiz = estadoPai === undefined;
  const estado: EstadoArvore = estadoPai ?? { abertas: new SvelteMap<string, boolean>() };
  if (ehRaiz) setContext(CHAVE_ESTADO_ARVORE, estado);

  function pastaAberta(node: FileNode): boolean {
    return estado.abertas.get(normalizarCaminho(node.caminho)) ?? node.aberta ?? false;
  }

  function alternarPasta(node: FileNode) {
    estado.abertas.set(normalizarCaminho(node.caminho), !pastaAberta(node));
  }

  // Quando o arquivo selecionado muda, abre as pastas que o contem (so a arvore raiz faz isso,
  // e so uma vez por arquivo, para o usuario poder fechar a pasta depois).
  let ultimoRevelado = '';
  $effect(() => {
    if (!ehRaiz) return;
    const alvo = normalizarCaminho(selecionado);
    if (alvo === '' || alvo === ultimoRevelado) return;
    ultimoRevelado = alvo;
    const partes = alvo.split('/');
    for (let i = 1; i < partes.length; i++) {
      estado.abertas.set(partes.slice(0, i).join('/'), true);
    }
  });

  // ---------- Aparencia ----------
  const selecionadoNorm = $derived(normalizarCaminho(selecionado));
  const principalNorm = $derived(normalizarCaminho(principal));

  function ehPrincipal(node: FileNode): boolean {
    return node.tipo === 'arquivo' && principalNorm !== '' && normalizarCaminho(node.caminho) === principalNorm;
  }

  function ehSelecionado(node: FileNode): boolean {
    return (
      node.tipo === 'arquivo' &&
      selecionadoNorm !== '' &&
      normalizarCaminho(node.caminho) === selecionadoNorm
    );
  }

  function iconeArquivo(nome: string) {
    const minusculo = nome.toLowerCase();
    if (minusculo.endsWith('.tex')) return FileText;
    if (minusculo.endsWith('.bib')) return BookOpen;
    if (/\.(png|jpe?g|gif|svg|webp|bmp)$/.test(minusculo)) return FileImage;
    return IconeArquivo;
  }

  let listaEl = $state<HTMLUListElement | null>(null);

  async function focarLinha(caminho: string) {
    await tick();
    const seletor = `[data-caminho="${CSS.escape(normalizarCaminho(caminho))}"]`;
    listaEl?.querySelector<HTMLElement>(seletor)?.focus({ preventScroll: true });
  }

  // ---------- Menu de contexto ----------
  interface MenuContexto {
    node: FileNode;
    x: number;
    y: number;
    gatilho: HTMLElement | null;
  }

  let menu = $state.raw<MenuContexto | null>(null);
  let menuEl = $state<HTMLDivElement | null>(null);

  function abrirMenu(evento: MouseEvent, node: FileNode) {
    evento.preventDefault();
    const gatilho = evento.currentTarget instanceof HTMLElement ? evento.currentTarget : null;
    let x = evento.clientX;
    let y = evento.clientY;
    if (x === 0 && y === 0 && gatilho) {
      // Menu aberto pelo teclado: fica ancorado no proprio item.
      const caixa = gatilho.getBoundingClientRect();
      x = caixa.left + 24;
      y = caixa.bottom;
    }
    menu = { node, x, y, gatilho };
  }

  function fecharMenu(restaurarFoco = false) {
    const gatilho = menu?.gatilho ?? null;
    menu = null;
    if (restaurarFoco && gatilho?.isConnected) gatilho.focus({ preventScroll: true });
  }

  // Enquanto o menu esta aberto: Esc, clique fora, perda de foco da janela, resize e rolagem fecham.
  $effect(() => {
    if (!menu) return;
    const aoPressionar = (e: PointerEvent) => {
      if (menuEl && e.target instanceof Node && menuEl.contains(e.target)) return;
      fecharMenu();
    };
    const aoTeclar = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      fecharMenu(true);
    };
    const fechar = () => fecharMenu();
    window.addEventListener('pointerdown', aoPressionar, true);
    window.addEventListener('keydown', aoTeclar, true);
    window.addEventListener('blur', fechar);
    window.addEventListener('resize', fechar);
    window.addEventListener('scroll', fechar, true);
    return () => {
      window.removeEventListener('pointerdown', aoPressionar, true);
      window.removeEventListener('keydown', aoTeclar, true);
      window.removeEventListener('blur', fechar);
      window.removeEventListener('resize', fechar);
      window.removeEventListener('scroll', fechar, true);
    };
  });

  // Depois de desenhado, mantem o menu dentro da janela (abre para cima perto da borda de baixo)
  // e leva o foco ao primeiro item.
  $effect(() => {
    if (!menu || !menuEl) return;
    const margem = 6;
    const largura = menuEl.offsetWidth;
    const altura = menuEl.offsetHeight;
    let x = menu.x;
    let y = menu.y;
    if (x + largura > window.innerWidth - margem) x = window.innerWidth - largura - margem;
    if (y + altura > window.innerHeight - margem) y = y - altura;
    menuEl.style.left = `${Math.max(margem, x)}px`;
    menuEl.style.top = `${Math.max(margem, y)}px`;
    menuEl.querySelector<HTMLElement>('button:not(:disabled)')?.focus({ preventScroll: true });
  });

  function teclaMenu(e: KeyboardEvent) {
    if (!menuEl) return;
    if (e.key === 'Tab') {
      e.preventDefault();
      fecharMenu(true);
      return;
    }
    const itens = Array.from(menuEl.querySelectorAll<HTMLElement>('button:not(:disabled)'));
    if (itens.length === 0) return;
    const atual = itens.indexOf(document.activeElement as HTMLElement);
    let destino: number;
    if (e.key === 'ArrowDown') destino = atual < 0 ? 0 : (atual + 1) % itens.length;
    else if (e.key === 'ArrowUp') destino = atual < 0 ? itens.length - 1 : (atual - 1 + itens.length) % itens.length;
    else if (e.key === 'Home') destino = 0;
    else if (e.key === 'End') destino = itens.length - 1;
    else return;
    e.preventDefault();
    itens[destino].focus();
  }

  // ---------- Renomear ----------
  let renomeando = $state<string | null>(null);
  let nomeEditado = $state('');

  function erroParaRenomear(nome: string, alvo: FileNode): string {
    const erro = erroDeNome(nome);
    if (erro !== '') return erro;
    const minusculo = nome.trim().toLowerCase();
    const alvoNorm = normalizarCaminho(alvo.caminho);
    const repetido = arquivos.some(
      (n) => normalizarCaminho(n.caminho) !== alvoNorm && n.nome.toLowerCase() === minusculo,
    );
    return repetido ? 'Já existe um item com esse nome.' : '';
  }

  const erroRenomear = $derived.by(() => {
    if (renomeando === null) return '';
    const alvo = arquivos.find((n) => normalizarCaminho(n.caminho) === renomeando);
    return alvo ? erroParaRenomear(nomeEditado, alvo) : '';
  });

  function iniciarRenomear() {
    const alvo = menu?.node;
    fecharMenu();
    if (!alvo || !onrenomearitem) return;
    nomeEditado = alvo.nome;
    renomeando = normalizarCaminho(alvo.caminho);
  }

  function confirmarRenomear(node: FileNode) {
    if (renomeando !== normalizarCaminho(node.caminho)) return;
    if (erroParaRenomear(nomeEditado, node) !== '') return;
    const novoNome = nomeEditado.trim();
    renomeando = null;
    if (novoNome === node.nome) focarLinha(node.caminho);
    else onrenomearitem?.(node.caminho, novoNome);
  }

  function cancelarRenomear(node: FileNode) {
    renomeando = null;
    focarLinha(node.caminho);
  }

  function aoSairDoCampo(node: FileNode) {
    // Enter e Esc ja encerraram a edicao: a saida do campo que vem depois nao faz nada.
    if (renomeando !== normalizarCaminho(node.caminho)) return;
    if (erroParaRenomear(nomeEditado, node) !== '') renomeando = null;
    else confirmarRenomear(node);
  }

  function teclaCampo(e: KeyboardEvent, node: FileNode) {
    if (e.key === 'Enter') {
      e.preventDefault();
      confirmarRenomear(node);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      cancelarRenomear(node);
    }
  }

  /** Foca o campo de nome e seleciona o nome sem a extensao (pastas: o nome inteiro). */
  function focarCampo(campo: HTMLInputElement, ehArquivo: boolean) {
    campo.focus();
    const ponto = campo.value.lastIndexOf('.');
    campo.setSelectionRange(0, ehArquivo && ponto > 0 ? ponto : campo.value.length);
  }

  // ---------- Baixar ----------
  function baixarItem() {
    const alvo = menu?.node;
    fecharMenu();
    if (alvo && alvo.tipo === 'arquivo') onbaixaritem?.(alvo.caminho);
  }

  // ---------- Excluir ----------
  let excluindo = $state.raw<FileNode | null>(null);
  let gatilhoExclusao: HTMLElement | null = null;
  let dialogoEl = $state<HTMLDivElement | null>(null);

  function pedirExclusao() {
    const alvo = menu?.node;
    const gatilho = menu?.gatilho ?? null;
    fecharMenu();
    if (!alvo || !onexcluiritem) return;
    gatilhoExclusao = gatilho;
    excluindo = alvo;
  }

  function fecharDialogoExclusao(restaurarFoco: boolean) {
    const gatilho = gatilhoExclusao;
    excluindo = null;
    gatilhoExclusao = null;
    if (restaurarFoco && gatilho?.isConnected) gatilho.focus({ preventScroll: true });
  }

  function cancelarExclusao() {
    fecharDialogoExclusao(true);
  }

  function confirmarExclusao() {
    const alvo = excluindo;
    fecharDialogoExclusao(false);
    if (alvo) onexcluiritem?.(alvo.caminho);
  }

  function focar(elemento: HTMLElement) {
    elemento.focus();
  }

  // Com a janela de confirmacao aberta: Esc cancela e o Tab nao sai da janela.
  $effect(() => {
    if (!excluindo) return;
    const aoTeclar = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        cancelarExclusao();
      } else if (e.key === 'Tab' && dialogoEl) {
        prenderFoco(e, dialogoEl);
      }
    };
    window.addEventListener('keydown', aoTeclar, true);
    return () => window.removeEventListener('keydown', aoTeclar, true);
  });
</script>

{#snippet icones(node: FileNode, aberta: boolean)}
  {#if node.tipo === 'pasta'}
    <span class="seta">
      {#if aberta}
        <ChevronDown size={14} />
      {:else}
        <ChevronRight size={14} />
      {/if}
    </span>
    <span class="icone icone-pasta">
      {#if aberta}
        <FolderOpen size={15} />
      {:else}
        <Folder size={15} />
      {/if}
    </span>
  {:else}
    {@const Icone = iconeArquivo(node.nome)}
    <span class="seta"></span>
    <span class="icone"><Icone size={15} /></span>
  {/if}
{/snippet}

<ul class="arvore" bind:this={listaEl}>
  {#each arquivos as node (node.caminho)}
    {@const caminho = normalizarCaminho(node.caminho)}
    {@const aberta = node.tipo === 'pasta' && pastaAberta(node)}
    <li class="item">
      {#if renomeando === caminho}
        <div class="linha editando">
          {@render icones(node, aberta)}
          <input
            class="campo-nome"
            type="text"
            bind:value={nomeEditado}
            use:focarCampo={node.tipo === 'arquivo'}
            onkeydown={(e) => teclaCampo(e, node)}
            onblur={() => aoSairDoCampo(node)}
            aria-label="Novo nome"
            aria-invalid={erroRenomear !== ''}
            autocomplete="off"
            spellcheck="false"
          />
        </div>
        {#if erroRenomear !== ''}
          <div class="erro-nome" role="alert">{erroRenomear}</div>
        {/if}
      {:else}
        <button
          type="button"
          class="linha"
          class:ativo={ehSelecionado(node)}
          class:alvo-menu={menu !== null && normalizarCaminho(menu.node.caminho) === caminho}
          aria-expanded={node.tipo === 'pasta' ? aberta : undefined}
          aria-current={ehSelecionado(node) ? 'true' : undefined}
          data-caminho={caminho}
          title={node.nome}
          onclick={() => (node.tipo === 'pasta' ? alternarPasta(node) : onselecionar?.(node.caminho))}
          oncontextmenu={(e) => abrirMenu(e, node)}
        >
          {@render icones(node, aberta)}
          <span class="nome">{node.nome}</span>
          {#if ehPrincipal(node)}
            <span class="selo">{t('arquivos.principal')}</span>
          {:else if node.tipo === 'arquivo' && node.tamanho !== undefined}
            <span class="tamanho">{formatarTamanho(node.tamanho)}</span>
          {/if}
        </button>
      {/if}
      {#if node.tipo === 'pasta' && aberta && node.filhos}
        <div class="filhos">
          <FileTree
            arquivos={node.filhos}
            {selecionado}
            {principal}
            {onselecionar}
            {onrenomearitem}
            {onexcluiritem}
            {onbaixaritem}
          />
        </div>
      {/if}
    </li>
  {/each}
</ul>

{#if menu}
  <div
    class="menu-contexto"
    role="menu"
    tabindex="-1"
    aria-label="Ações do item"
    bind:this={menuEl}
    style="left: {menu.x}px; top: {menu.y}px;"
    onkeydown={teclaMenu}
  >
    <button type="button" role="menuitem" class="menu-item" disabled={!onrenomearitem} onclick={iniciarRenomear}>
      <Pencil size={14} />
      <span>Renomear</span>
    </button>
    {#if menu.node.tipo === 'arquivo'}
      <button type="button" role="menuitem" class="menu-item" disabled={!onbaixaritem} onclick={baixarItem}>
        <Download size={14} />
        <span>Baixar</span>
      </button>
    {/if}
    <div class="menu-separador" role="separator"></div>
    <button type="button" role="menuitem" class="menu-item perigo" disabled={!onexcluiritem} onclick={pedirExclusao}>
      <Trash size={14} />
      <span>Excluir</span>
    </button>
  </div>
{/if}

{#if excluindo}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fundo-modal" onclick={(e) => { if (e.target === e.currentTarget) cancelarExclusao(); }}>
    <div
      class="dialogo"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="{uid}-mensagem"
      tabindex="-1"
      bind:this={dialogoEl}
    >
      <p class="dialogo-texto" id="{uid}-mensagem">Excluir <strong>{excluindo.nome}</strong>? Esta ação não pode ser desfeita.</p>
      <div class="dialogo-botoes">
        <button type="button" class="btn-secundario" use:focar onclick={cancelarExclusao}>Cancelar</button>
        <button type="button" class="btn-perigo" onclick={confirmarExclusao}>Excluir</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .arvore {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 0.8125rem;
    user-select: none;
  }

  .item {
    position: relative;
  }

  .linha {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 26px;
    padding: 0 6px;
    box-sizing: border-box;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    cursor: pointer;
  }

  .linha:hover,
  .linha.alvo-menu {
    background: var(--hover);
    color: var(--texto);
  }

  .linha:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .linha.ativo {
    background: var(--primaria-suave);
    color: var(--primaria);
    font-weight: 600;
  }

  .linha.editando,
  .linha.editando:hover {
    background: transparent;
    cursor: default;
  }

  .seta {
    flex: 0 0 14px;
    display: flex;
    align-items: center;
    width: 14px;
    color: var(--texto-terciario);
  }

  .icone {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    color: var(--texto-terciario);
  }

  .icone-pasta {
    color: var(--primaria);
  }

  .linha.ativo .icone {
    color: inherit;
  }

  .nome {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tamanho {
    flex: 0 0 auto;
    font-size: 0.6875rem;
    color: var(--texto-terciario);
  }

  .selo {
    flex: 0 0 auto;
    padding: 0 6px;
    border-radius: 8px;
    background: var(--acento);
    color: #11111b;
    font-size: 0.625rem;
    font-weight: 700;
    text-transform: lowercase;
  }

  .filhos {
    margin-left: 13px;
    padding-left: 3px;
    border-left: 1px solid var(--borda);
  }

  .campo-nome {
    flex: 1 1 auto;
    min-width: 0;
    height: 22px;
    padding: 0 4px;
    box-sizing: border-box;
    border: 1px solid var(--primaria);
    border-radius: 3px;
    background: var(--superficie-0);
    color: var(--texto);
    font-family: inherit;
    font-size: inherit;
    outline: none;
  }

  .campo-nome[aria-invalid="true"] {
    border-color: var(--perigo);
  }

  .erro-nome {
    position: absolute;
    z-index: 5;
    left: 20px;
    right: 4px;
    top: calc(100% + 2px);
    padding: 4px 8px;
    border: 1px solid var(--perigo);
    border-radius: 4px;
    background: var(--superficie-0);
    color: var(--perigo);
    font-size: 0.75rem;
    box-shadow: var(--sombra-2);
  }

  .menu-contexto {
    position: fixed;
    z-index: 3000;
    min-width: 170px;
    padding: 4px 0;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: var(--sombra-3);
    outline: none;
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: var(--texto);
    font-family: inherit;
    font-size: 0.8125rem;
    text-align: left;
    cursor: pointer;
  }

  .menu-item:hover:not(:disabled),
  .menu-item:focus-visible {
    background: var(--hover);
    outline: none;
  }

  .menu-item:disabled {
    color: var(--texto-terciario);
    cursor: not-allowed;
  }

  .menu-item.perigo:not(:disabled) {
    color: var(--perigo);
  }

  .menu-separador {
    height: 1px;
    margin: 4px 0;
    background: var(--borda);
  }

  .fundo-modal {
    position: fixed;
    inset: 0;
    z-index: 2100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--superficie-0) 60%, transparent);
  }

  .dialogo {
    width: min(380px, 92vw);
    padding: 18px 20px;
    box-sizing: border-box;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    box-shadow: var(--sombra-3);
    outline: none;
  }

  .dialogo-texto {
    margin: 0 0 16px;
    font-size: 0.875rem;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }

  .dialogo-botoes {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-secundario,
  .btn-perigo {
    padding: 7px 16px;
    border-radius: var(--raio-sm);
    font-family: inherit;
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .btn-secundario {
    border: 1px solid var(--borda);
    background: var(--superficie-0);
    color: var(--texto);
  }

  .btn-secundario:hover {
    background: var(--hover);
  }

  .btn-perigo {
    border: 1px solid var(--perigo);
    background: var(--perigo);
    color: var(--superficie-0);
    font-weight: 600;
  }

  .btn-perigo:hover {
    filter: brightness(0.92);
  }

  .btn-secundario:focus-visible,
  .btn-perigo:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 2px;
  }
</style>
