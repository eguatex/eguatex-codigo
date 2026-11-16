<!--
  Painel de arquivos: cabecalho com acoes, arvore do projeto e estrutura do documento.
  Estrutura de painel baseada no painel de arquivos e estrutura do Overleaf (AGPL-3.0),
  reescrita em Svelte.
-->
<script lang="ts">
  import FileTree, { erroDeNome, normalizarCaminho, prenderFoco, type FileNode } from './FileTree.svelte';
  import Estrutura from './Estrutura.svelte';
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import Upload from "@lucide/svelte/icons/upload";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import { contarAuxiliares, semAuxiliares } from '$lib/arquivos-util';
  import { t, tf } from '$lib/i18n';

  interface Props {
    arquivos: FileNode[];
    selecionado?: string;
    principal?: string;
    onatualizar?: () => void;
    conteudo?: string;
    linhaAtual?: number;
    onselecionar?: (caminho: string) => void;
    onirparalinha?: (linha: number) => void;
    oncriararquivo?: (caminhoRelativo: string) => void;
    oncriarpasta?: (caminhoRelativo: string) => void;
    onenviararquivo?: (pastaRelativa: string) => void;
    onrenomearitem?: (caminho: string, novoNome: string) => void;
    onexcluiritem?: (caminho: string) => void;
    onbaixaritem?: (caminho: string) => void;
  }

  let {
    arquivos,
    selecionado = '',
    principal = '',
    onatualizar,
    conteudo = '',
    linhaAtual = 0,
    onselecionar,
    onirparalinha,
    oncriararquivo,
    oncriarpasta,
    onenviararquivo,
    onrenomearitem,
    onexcluiritem,
    onbaixaritem,
  }: Props = $props();

  const uid = $props.id();

  // ---------- Arquivos auxiliares (.aux, .log...) ----------
  const CHAVE_AUXILIARES = 'eguatex.mostrar-auxiliares';

  function lerMostrarAuxiliares(): boolean {
    try {
      return localStorage.getItem(CHAVE_AUXILIARES) === '1';
    } catch {
      return false;
    }
  }

  let mostrarAuxiliares = $state(lerMostrarAuxiliares());

  function alternarAuxiliares() {
    mostrarAuxiliares = !mostrarAuxiliares;
    try {
      localStorage.setItem(CHAVE_AUXILIARES, mostrarAuxiliares ? '1' : '0');
    } catch {
      // sem armazenamento: vale so nesta sessao
    }
  }

  const arquivosVisiveis = $derived(mostrarAuxiliares ? arquivos : semAuxiliares(arquivos));
  const auxiliaresOcultos = $derived(mostrarAuxiliares ? 0 : contarAuxiliares(arquivos));

  // ---------- Pasta de destino ----------
  // Os caminhos da arvore comecam com barra (/main.tex, /capitulos/intro.tex). Os itens novos
  // seguem o mesmo formato: /capitulos/novo.tex, ou /novo.tex na raiz (pasta de destino vazia).
  const pastaDestino = $derived.by(() => {
    const limpo = normalizarCaminho(selecionado);
    const barra = limpo.lastIndexOf('/');
    return barra > 0 ? `/${limpo.slice(0, barra)}` : '';
  });

  const rotuloDestino = $derived(pastaDestino === '' ? 'raiz do projeto' : pastaDestino.slice(1));

  /** Itens que ja existem na pasta de destino. */
  function itensDoDestino(): FileNode[] {
    const alvo = normalizarCaminho(pastaDestino);
    let lista: FileNode[] = arquivos;
    if (alvo === '') return lista;
    let acumulado = '';
    for (const parte of alvo.split('/')) {
      acumulado = acumulado === '' ? parte : `${acumulado}/${parte}`;
      const pasta = lista.find((n) => n.tipo === 'pasta' && normalizarCaminho(n.caminho) === acumulado);
      if (!pasta) return [];
      lista = pasta.filhos ?? [];
    }
    return lista;
  }

  // ---------- Janela de novo arquivo e nova pasta ----------
  type TipoNovo = 'arquivo' | 'pasta';

  let criando = $state<TipoNovo | null>(null);
  let nomeNovo = $state('');
  let dialogoEl = $state<HTMLDivElement | null>(null);
  let gatilhoCriacao: HTMLElement | null = null;

  const erroCriacao = $derived.by(() => {
    if (criando === null) return '';
    const erro = erroDeNome(nomeNovo);
    if (erro !== '') return erro;
    const minusculo = nomeNovo.trim().toLowerCase();
    const existe = itensDoDestino().some((n) => n.nome.toLowerCase() === minusculo);
    return existe ? 'Já existe um item com esse nome nesta pasta.' : '';
  });

  // Nome vazio so desabilita o botao Criar; os outros erros aparecem escritos.
  const mostrarErro = $derived(erroCriacao !== '' && nomeNovo.trim() !== '');

  function abrirCriacao(tipo: TipoNovo, evento: MouseEvent) {
    gatilhoCriacao = evento.currentTarget instanceof HTMLElement ? evento.currentTarget : null;
    nomeNovo = tipo === 'arquivo' ? 'novo.tex' : 'nova-pasta';
    criando = tipo;
  }

  export function novoArquivo() {
    gatilhoCriacao = null;
    nomeNovo = 'novo.tex';
    criando = 'arquivo';
  }

  export function novaPasta() {
    gatilhoCriacao = null;
    nomeNovo = 'nova-pasta';
    criando = 'pasta';
  }

  function fecharCriacao() {
    const gatilho = gatilhoCriacao;
    criando = null;
    gatilhoCriacao = null;
    if (gatilho?.isConnected) gatilho.focus({ preventScroll: true });
  }

  function confirmarCriacao() {
    const tipo = criando;
    if (tipo === null || erroCriacao !== '') return;
    const caminho = `${pastaDestino}/${nomeNovo.trim()}`;
    fecharCriacao();
    if (tipo === 'arquivo') oncriararquivo?.(caminho);
    else oncriarpasta?.(caminho);
  }

  /** Foca o campo de nome e seleciona o nome sem a extensao (pasta: o nome inteiro). */
  function focarNome(campo: HTMLInputElement, ehArquivo: boolean) {
    campo.focus();
    const ponto = campo.value.lastIndexOf('.');
    campo.setSelectionRange(0, ehArquivo && ponto > 0 ? ponto : campo.value.length);
  }

  // Com a janela aberta: Esc cancela e o Tab nao sai da janela.
  $effect(() => {
    if (criando === null) return;
    const aoTeclar = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        fecharCriacao();
      } else if (e.key === 'Tab' && dialogoEl) {
        prenderFoco(e, dialogoEl);
      }
    };
    window.addEventListener('keydown', aoTeclar, true);
    return () => window.removeEventListener('keydown', aoTeclar, true);
  });

  // ---------- Divisoria entre a arvore e a estrutura ----------
  const CHAVE_DIVISAO = 'eguatex.divisao-estrutura';
  const DIVISAO_MINIMA = 0.2;
  const DIVISAO_MAXIMA = 0.8;
  const DIVISAO_PADRAO = 0.5;

  function limitarDivisao(valor: number): number {
    return Math.min(DIVISAO_MAXIMA, Math.max(DIVISAO_MINIMA, valor));
  }

  function lerDivisao(): number {
    try {
      const salvo = localStorage.getItem(CHAVE_DIVISAO);
      const numero = salvo === null ? Number.NaN : Number.parseFloat(salvo);
      if (Number.isFinite(numero)) return limitarDivisao(numero);
    } catch {
      // Sem acesso ao armazenamento: vale a divisao padrao.
    }
    return DIVISAO_PADRAO;
  }

  function gravarDivisao(valor: number): void {
    try {
      localStorage.setItem(CHAVE_DIVISAO, String(valor));
    } catch {
      // Sem acesso ao armazenamento: a divisao vale so enquanto o painel estiver aberto.
    }
  }

  /** Parte da altura que a arvore ocupa, de 0.2 a 0.8. */
  let proporcao = $state(lerDivisao());
  let estruturaAberta = $state(true);
  let areaEl = $state<HTMLDivElement | null>(null);
  let arrastando = $state(false);
  let deslocamentoNoDivisor = 0;

  function iniciarArraste(e: PointerEvent) {
    if (e.button !== 0 || !(e.currentTarget instanceof HTMLElement)) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    deslocamentoNoDivisor = e.clientY - e.currentTarget.getBoundingClientRect().top;
    arrastando = true;
  }

  function moverArraste(e: PointerEvent) {
    if (!arrastando || !areaEl) return;
    const caixa = areaEl.getBoundingClientRect();
    if (caixa.height <= 0) return;
    proporcao = limitarDivisao((e.clientY - deslocamentoNoDivisor - caixa.top) / caixa.height);
  }

  function terminarArraste(e: PointerEvent) {
    if (!arrastando) return;
    arrastando = false;
    if (e.currentTarget instanceof HTMLElement && e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    gravarDivisao(proporcao);
  }

  function teclaDivisor(e: KeyboardEvent) {
    let nova = proporcao;
    if (e.key === 'ArrowUp') nova -= 0.03;
    else if (e.key === 'ArrowDown') nova += 0.03;
    else if (e.key === 'Home') nova = DIVISAO_MINIMA;
    else if (e.key === 'End') nova = DIVISAO_MAXIMA;
    else return;
    e.preventDefault();
    proporcao = limitarDivisao(nova);
    gravarDivisao(proporcao);
  }
</script>

<div class="painel" class:arrastando>
  <header class="cabecalho">
    <h2 class="titulo">Arquivos</h2>
    <div class="acoes">
      <button
        type="button"
        class="btn-icone"
        title="Novo arquivo"
        aria-label="Novo arquivo"
        disabled={!oncriararquivo}
        onclick={(e) => abrirCriacao('arquivo', e)}
      >
        <FilePlus size={16} />
      </button>
      <button
        type="button"
        class="btn-icone"
        title="Nova pasta"
        aria-label="Nova pasta"
        disabled={!oncriarpasta}
        onclick={(e) => abrirCriacao('pasta', e)}
      >
        <FolderPlus size={16} />
      </button>
      <button
        type="button"
        class="btn-icone"
        title="Enviar arquivo"
        aria-label="Enviar arquivo"
        disabled={!onenviararquivo}
        onclick={() => onenviararquivo?.(pastaDestino)}
      >
        <Upload size={16} />
      </button>
      <button
        type="button"
        class="btn-icone"
        title={t('arquivos.atualizar')}
        aria-label={t('arquivos.atualizar')}
        disabled={!onatualizar}
        onclick={() => onatualizar?.()}
      >
        <RefreshCw size={16} />
      </button>
      <button
        type="button"
        class="btn-icone"
        title={mostrarAuxiliares ? t('arquivos.ocultar-auxiliares') : t('arquivos.mostrar-auxiliares')}
        aria-label={mostrarAuxiliares ? t('arquivos.ocultar-auxiliares') : t('arquivos.mostrar-auxiliares')}
        aria-pressed={mostrarAuxiliares}
        onclick={alternarAuxiliares}
      >
        {#if mostrarAuxiliares}
          <EyeOff size={16} />
        {:else}
          <Eye size={16} />
        {/if}
      </button>
    </div>
  </header>

  <div class="area" bind:this={areaEl} style="--proporcao: {proporcao * 100}%;">
    <div class="arvore" class:cheia={!estruturaAberta}>
      {#if arquivos.length === 0}
        <p class="vazio">Nenhum arquivo neste projeto.</p>
      {:else}
        <FileTree
          arquivos={arquivosVisiveis}
          {selecionado}
          {principal}
          {onselecionar}
          {onrenomearitem}
          {onexcluiritem}
          {onbaixaritem}
        />
        {#if auxiliaresOcultos > 0}
          <button type="button" class="ocultos" onclick={alternarAuxiliares}>
            {tf('arquivos.auxiliares-ocultos', { n: auxiliaresOcultos })}
          </button>
        {/if}
      {/if}
    </div>

    {#if estruturaAberta}
      <!-- Um separador que recebe foco e um widget (divisor de janela do WAI-ARIA), mas o Svelte nao reconhece. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="divisor"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Redimensionar a estrutura"
        aria-valuenow={Math.round(proporcao * 100)}
        aria-valuemin={20}
        aria-valuemax={80}
        tabindex="0"
        onpointerdown={iniciarArraste}
        onpointermove={moverArraste}
        onpointerup={terminarArraste}
        onpointercancel={terminarArraste}
        onlostpointercapture={terminarArraste}
        onkeydown={teclaDivisor}
      ></div>
    {/if}

    <section class="estrutura-secao" class:aberta={estruturaAberta}>
      <button
        type="button"
        class="estrutura-cabecalho"
        aria-expanded={estruturaAberta}
        aria-controls="{uid}-estrutura"
        onclick={() => (estruturaAberta = !estruturaAberta)}
      >
        {#if estruturaAberta}
          <ChevronDown size={14} />
        {:else}
          <ChevronRight size={14} />
        {/if}
        <span>Estrutura</span>
      </button>
      {#if estruturaAberta}
        <div class="estrutura-corpo" id="{uid}-estrutura">
          <Estrutura {conteudo} {linhaAtual} {onirparalinha} />
        </div>
      {/if}
    </section>
  </div>

  {#if criando !== null}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="fundo-modal" onclick={(e) => { if (e.target === e.currentTarget) fecharCriacao(); }}>
      <div
        class="dialogo"
        role="dialog"
        aria-modal="true"
        aria-labelledby="{uid}-titulo-criar"
        tabindex="-1"
        bind:this={dialogoEl}
      >
        <form onsubmit={(e) => { e.preventDefault(); confirmarCriacao(); }}>
          <h3 class="dialogo-titulo" id="{uid}-titulo-criar">
            {criando === 'arquivo' ? 'Novo arquivo' : 'Nova pasta'}
          </h3>
          <label class="rotulo" for="{uid}-nome">
            {criando === 'arquivo' ? 'Nome do arquivo' : 'Nome da pasta'}
          </label>
          <input
            id="{uid}-nome"
            class="campo"
            type="text"
            bind:value={nomeNovo}
            use:focarNome={criando === 'arquivo'}
            aria-invalid={mostrarErro}
            aria-describedby={mostrarErro ? `${uid}-erro` : undefined}
            autocomplete="off"
            spellcheck="false"
          />
          <p class="destino">Pasta de destino: {rotuloDestino}</p>
          {#if mostrarErro}
            <p class="erro" id="{uid}-erro" role="alert">{erroCriacao}</p>
          {/if}
          <div class="dialogo-botoes">
            <button type="button" class="btn-secundario" onclick={fecharCriacao}>Cancelar</button>
            <button type="submit" class="btn-primario" disabled={erroCriacao !== ''}>Criar</button>
          </div>
        </form>
      </div>
    </div>
  {/if}
</div>

<style>
  .painel {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    width: 100%;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--superficie-0);
    color: var(--texto);
    font-size: 0.8125rem;
  }

  .painel.arrastando {
    cursor: row-resize;
    user-select: none;
  }

  .cabecalho {
    flex: 0 0 36px;
    height: 36px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 6px 0 10px;
    border-bottom: 1px solid var(--borda);
  }

  .titulo {
    margin: 0;
    font-size: 0.8125rem;
    font-weight: 700;
    color: var(--texto);
  }

  .acoes {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .btn-icone {
    width: 26px;
    height: 26px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .btn-icone:hover:not(:disabled) {
    background: var(--hover);
    color: var(--texto);
  }

  .btn-icone:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .btn-icone:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .area {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .arvore {
    flex: 0 0 var(--proporcao);
    min-height: 0;
    overflow: auto;
    padding: 4px;
    box-sizing: border-box;
  }

  .arvore.cheia {
    flex: 1 1 0;
  }

  .vazio {
    margin: 0;
    padding: 10px 12px;
    color: var(--texto-terciario);
    font-style: italic;
    text-align: center;
  }

  .ocultos {
    display: block;
    width: 100%;
    margin-top: 4px;
    padding: 4px 8px;
    border: none;
    background: transparent;
    color: var(--texto-terciario);
    font: inherit;
    font-size: 0.75rem;
    text-align: left;
    cursor: pointer;
  }

  .ocultos:hover {
    color: var(--texto);
  }

  .divisor {
    flex: 0 0 5px;
    position: relative;
    cursor: row-resize;
    outline: none;
  }

  .divisor::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 2px;
    height: 1px;
    background: var(--borda);
    transition: background 0.15s;
  }

  .divisor:hover::before,
  .divisor:focus-visible::before,
  .painel.arrastando .divisor::before {
    top: 1px;
    height: 3px;
    background: var(--primaria);
  }

  .estrutura-secao {
    flex: 0 0 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--borda);
  }

  .estrutura-secao.aberta {
    flex: 1 1 0;
    border-top: none;
  }

  .estrutura-cabecalho {
    flex: 0 0 30px;
    height: 30px;
    width: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    border: none;
    background: transparent;
    color: var(--texto);
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 700;
    text-align: left;
    cursor: pointer;
  }

  .estrutura-cabecalho:hover {
    background: var(--hover);
  }

  .estrutura-cabecalho:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .estrutura-corpo {
    flex: 1 1 0;
    min-height: 0;
    overflow: auto;
    padding: 0 4px 4px;
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

  .dialogo-titulo {
    margin: 0 0 14px;
    font-size: 1rem;
    font-weight: 700;
  }

  .rotulo {
    display: block;
    margin-bottom: 4px;
    color: var(--texto-secundario);
    font-size: 0.75rem;
  }

  .campo {
    width: 100%;
    height: 32px;
    padding: 0 10px;
    box-sizing: border-box;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
    font-family: inherit;
    font-size: 0.875rem;
    outline: none;
  }

  .campo:focus {
    border-color: var(--primaria);
  }

  .campo[aria-invalid="true"] {
    border-color: var(--perigo);
  }

  .destino {
    margin: 6px 0 0;
    color: var(--texto-terciario);
    font-size: 0.75rem;
    overflow-wrap: anywhere;
  }

  .erro {
    margin: 6px 0 0;
    color: var(--perigo);
    font-size: 0.75rem;
  }

  .dialogo-botoes {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .btn-secundario,
  .btn-primario {
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

  .btn-primario {
    border: none;
    background: var(--primaria);
    color: #fff;
    font-weight: 600;
  }

  .btn-primario:hover:not(:disabled) {
    background: var(--primaria-hover);
  }

  .btn-primario:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .btn-secundario:focus-visible,
  .btn-primario:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 2px;
  }
</style>
