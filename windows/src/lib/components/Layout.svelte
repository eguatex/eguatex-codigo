<script lang="ts">
  import Toolbar from './Toolbar.svelte';
  import BarraLateral from './BarraLateral.svelte';
  import EditorToolbar from './EditorToolbar.svelte';
  import PainelArquivos from './PainelArquivos.svelte';
  import PaletaSimbolos from './PaletaSimbolos.svelte';
  import Trilha from './Trilha.svelte';
  import PdfViewer, { type Destaque } from './PdfViewer.svelte';
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import { preferencias, definirPreferencia } from '$lib/preferencias.svelte';
  import { limitarLarguraPdf } from '$lib/divisor-pdf';
  import ChatIA from './ChatIA.svelte';
  import Search from "@lucide/svelte/icons/search";
  import AtalhosModal from './AtalhosModal.svelte';
  import ComoUsarModal from './ComoUsarModal.svelte';
  import PixModal from './PixModal.svelte';
  import BarraMatematica from './BarraMatematica.svelte';
  import GerenciarAtalhosModal from './GerenciarAtalhosModal.svelte';
  import MacrosModal from './MacrosModal.svelte';
  import { exportarAtalhos, importarAtalhosDeArquivo } from '$lib/atalhos-arquivo';
  import SobreModal from './SobreModal.svelte';
  import StatusBar from './StatusBar.svelte';
  import { acompanharCompilacao } from '$lib/tarefas.svelte';
  import { SITE, MAILTO_CONTATO } from '$lib/site';
  import { enviarFeedback, procurarAtualizacao } from '$lib/ajuda';
  import PaletaComandos from './PaletaComandos.svelte';
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { executar, type Acoes } from '$lib/comandos';
  import { buscarEmTexto } from '$lib/busca-projeto';
  import { t } from '$lib/i18n';
  import type { EstadoOrtografia } from '$lib/ortografia-idioma';
  import FileText from "@lucide/svelte/icons/file-text";
  import BarraAbas, { idDaAba } from './BarraAbas.svelte';
  import { atalhoDeAba, type Aba } from '$lib/abas.svelte';
  import type { FileNode } from './FileTree.svelte';
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';
  import { invoke } from "@tauri-apps/api/core";
  import type { ProjetoAgente } from '$lib/agente';
  import type { ModoEdicao } from '$lib/modo-edicao';

  interface ErroLog {
    linha?: number | null;
    arquivo?: string | null;
    mensagem: string;
    tipo: string;
  }

  interface ConfigIA {
    ativa: boolean;
    provedor: string;
    modelo: string;
    nivel?: string;
  }

  interface Props {
    /** Estado do corretor que o editor informa, para a sigla da barra de status. */
    ortografia?: EstadoOrtografia;
    arquivos?: FileNode[];
    arquivoAtual?: string;
    linha?: number;
    coluna?: number;
    statusCompilacao?: 'idle' | 'compilando' | 'sucesso' | 'parcial' | 'erro';
    /** Motor que rodou na ultima compilacao do projeto ("XeLaTeX"), para a barra de status. */
    motorDetectado?: string;
    compilando?: boolean;
    synctexDisponivel?: boolean;
    destaque?: Destaque | null;
    onpararcompilacao?: () => void;
    onrecompilardozero?: () => void;
    onirparapdf?: () => void;
    onirparacodigo?: (pagina: number, x: number, y: number) => void;
    logTexto?: string;
    /** Duracao da ultima compilacao em ms. */
    tempoMs?: number | null;
    erros?: ErroLog[];
    pdfDados?: Uint8Array | null;
    iaAtiva?: boolean;
    configIA?: ConfigIA;
    oncompilar?: () => void;
    onsalvar?: () => void;
    onselecionararquivo?: (caminho: string) => void | Promise<void>;
    onrenomear?: (nome: string) => void;
    caminhoProjeto?: string;
    agente?: ProjetoAgente;
    oncriararquivo?: (caminhoRelativo: string) => void;
    oncriarpasta?: (caminhoRelativo: string) => void;
    onenviararquivo?: (pastaRelativa: string) => void;
    onrenomearitem?: (caminho: string, novoNome: string) => void;
    onexcluiritem?: (caminho: string) => void;
    onbaixaritem?: (caminho: string) => void;
    /** "Definir como documento principal" de um .tex, como no Overleaf. */
    ondefinirprincipal?: (caminho: string) => void;
    /** Botao Atualizar do painel de arquivos: relista a pasta do projeto. */
    onatualizararquivos?: () => void;
    onirparalinha?: (linha: number) => void;
    onconfigurar?: () => void;
    /** Sigla do corretor na barra de status: abre a configuracao do corretor. */
    onconfigurarortografia?: () => void;
    /** O chat ligou ou mudou a IA: devolve a configuracao nova para a pagina. */
    onconfigIA?: (config: ConfigIA) => void;
    oninserirtexto?: (texto: string) => void;
    oninicio?: () => void;
    oncompartilhar?: () => void;
    estadoDrive?: string;
    /** Texto longo da sincronizacao (arquivos com falha), mostrado como title da barra. */
    detalheDrive?: string;
    acoes?: Acoes;
    estilo?: string;
    onbaixar?: (formato: string) => void;
    ontraduzirpdf?: (texto: string) => void;
    /** Estado dos botoes Codigo/Visual e Editando/Revisando da barra do editor. */
    modosEditor?: { visual: boolean; modo: ModoEdicao; sugestoes: number; desfazer?: boolean; refazer?: boolean };
    conteudo?: string;
    nomeProjeto?: string;
    /** Abas abertas, na ordem da barra, e o caminho da ativa (de texto ou de imagem). */
    abas?: Aba[];
    abaAtiva?: string;
    /** Abas com texto ainda nao confirmado pelo disco. */
    abasSujas?: string[];
    /** Caminho do documento principal (a aba dele mostra um marcador). */
    abaPrincipal?: string;
    onselecionaraba?: (caminho: string) => void;
    onfecharaba?: (caminho: string) => void;
    /** Menu da aba, "Fechar as outras": mantem so a aba do caminho. */
    onfecharoutras?: (caminho: string) => void;
    /** Arrastar uma aba: novo indice (na lista completa) depois de tirar a aba do lugar antigo. */
    onmoveraba?: (de: number, para: number) => void;
    /** Quantas listas estao abertas no cursor (a barra do editor so mostra o recuo dentro de uma). */
    profundidadeLista?: number;
    children?: Snippet;
    /** Visualizador da imagem da aba ativa; o editor fica montado e escondido enquanto ele aparece. */
    imagem?: Snippet;
  }

  let {
    arquivos = [],
    arquivoAtual = '',
    linha = 1,
    coluna = 1,
    statusCompilacao = 'idle',
    motorDetectado = '',
    compilando = false,
    synctexDisponivel = false,
    destaque = null,
    onpararcompilacao,
    onrecompilardozero,
    ortografia = { ligada: true },
    onirparapdf,
    onirparacodigo,
    logTexto = '',
    tempoMs = null,
    erros = [],
    pdfDados = null,
    iaAtiva = false,
    configIA = { ativa: false, provedor: '', modelo: '' },
    oncompilar,
    onsalvar,
    onselecionararquivo,
    onrenomear,
    caminhoProjeto = '',
    agente,
    oncriararquivo,
    oncriarpasta,
    onenviararquivo,
    onrenomearitem,
    onexcluiritem,
    onbaixaritem,
    ondefinirprincipal,
    onatualizararquivos,
    onirparalinha,
    onconfigurar,
    onconfigurarortografia,
    onconfigIA,
    oninserirtexto,
    oninicio,
    oncompartilhar,
    estadoDrive = '',
    detalheDrive = '',
    acoes = {},
    estilo = 'normal',
    onbaixar,
    ontraduzirpdf,
    modosEditor = { visual: false, modo: 'editando', sugestoes: 0 },
    conteudo = '',
    nomeProjeto = '',
    abas = [],
    abaAtiva = '',
    abasSujas = [],
    abaPrincipal = '',
    onselecionaraba,
    onfecharaba,
    onfecharoutras,
    onmoveraba,
    profundidadeLista = 0,
    children,
    imagem,
  }: Props = $props();

  // a compilacao entra no registro de tarefas: a barra do rodape e a faixa mostram o andamento
  // O efeito mora em acompanharCompilacao (le so o status, untrack na lista) e se limpa ao desmontar o Layout.
  acompanharCompilacao(() => statusCompilacao, t('tarefa.compilando'));

  let buscaResultados: Array<{ arquivo: string; linha: number; texto: string }> = $state([]);
  let buscaCarregando = $state(false);
  let buscaCortada = $state(false);

  let buscaFeita = $state(false);
  let buscaTimer: ReturnType<typeof setTimeout> | undefined;

  function arquivosDeTexto(nos: FileNode[], saida: FileNode[] = []): FileNode[] {
    for (const no of nos) {
      if (no.tipo === 'pasta') arquivosDeTexto(no.filhos ?? [], saida);
      else if (/\.(tex|bib|cls|sty)$/i.test(no.nome)) saida.push(no);
    }
    return saida;
  }

  async function realizarBusca() {
    clearTimeout(buscaTimer);
    const termo = buscaTexto.trim().toLowerCase();
    if (!termo) {
      buscaResultados = [];
      buscaCortada = false;
      buscaFeita = false;
      return;
    }
    buscaCarregando = true;
    const textos: Array<{ caminho: string; texto: string }> = [];
    try {
      for (const no of arquivosDeTexto(arquivos)) {
        let texto: string;
        if (no.caminho === arquivoAtual) {
          texto = conteudo;
        } else {
          try {
            texto = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: no.caminho });
          } catch {
            continue;
          }
        }
        textos.push({ caminho: no.caminho, texto });
      }
      const { achados, cortado } = buscarEmTexto(textos, termo);
      buscaResultados = achados;
      buscaCortada = cortado;
      buscaFeita = true;
    } finally {
      buscaCarregando = false;
    }
  }

  function agendarBusca() {
    clearTimeout(buscaTimer);
    buscaTimer = setTimeout(realizarBusca, 300);
  }

  async function abrirResultado(arquivo: string, linha: number) {
    if (arquivo !== arquivoAtual) {
      await onselecionararquivo?.(arquivo);
      await tick();
    }
    onirparalinha?.(linha);
  }

  type ModoLayout = 'ambos' | 'editor' | 'pdf';
  let modoLayout = $state<ModoLayout>('ambos');
  let atalhosVisivel = $state(false);
  let comoUsarVisivel = $state(false);
  let paletaVisivel = $state(false);
  let pixVisivel = $state(false);
  // barra Matematica: gerenciador (com ou sem o formulario de nova categoria), macros e o seletor de importacao
  let gerenciarAtalhos = $state<null | { nova: boolean }>(null);
  let macrosVisivel = $state(false);
  let entradaImportar: HTMLInputElement | null = null;
  let sobreVisivel = $state(false);

  function abrirLink(url: string) {
    openUrl(url).catch((e) => console.error('Erro ao abrir link:', e));
  }

  const acoesDoLayout: Acoes = {
    'compilar': () => oncompilar?.(),
    'layout-ambos': () => (modoLayout = 'ambos'),
    'layout-editor': () => (modoLayout = 'editor'),
    'layout-pdf': () => (modoLayout = 'pdf'),
    // a ultima aba aberta nao fecha, como no Overleaf (Ctrl+W e o menu passam por aqui; o x, Delete e o botao do meio, pela BarraAbas)
    'fechar-aba': () => { if (abas.length > 1) acoes['fechar-aba']?.(); },
    'sobre': () => (sobreVisivel = true),
    'paleta-comandos': () => (paletaVisivel = true),
    'simbolo': () => (paletaSimbolosAberta = true),
    'ver-trilha': () => definirPreferencia('mostrarTrilha', !preferencias.mostrarTrilha),
    'ver-abas': () => definirPreferencia('mostrarAbas', !preferencias.mostrarAbas),
    'ver-previa-equacao': () => definirPreferencia('previaEquacao', !preferencias.previaEquacao),
    'alternar-arquivos': () => (painelAtivo = painelAtivo === 'arquivos' ? '' : 'arquivos'),
    'novo-arquivo': () => abrirCriacaoNoPainel('arquivo'),
    'nova-pasta': () => abrirCriacaoNoPainel('pasta'),
    'pdf-mais-zoom': () => pdfRef?.maisZoom(),
    'pdf-menos-zoom': () => pdfRef?.menosZoom(),
    'pdf-ajustar-largura': () => pdfRef?.ajustarLargura(),
    'pdf-ajustar-altura': () => pdfRef?.ajustarAltura(),
    'pdf-apresentacao': () => pdfRef?.modoApresentacao(),
    'como-usar': () => (comoUsarVisivel = true),
    'atalhos': () => (atalhosVisivel = true),
    'apoiar': () => (pixVisivel = true),
    'atalhos-nova-categoria': () => (gerenciarAtalhos = { nova: true }),
    'atalhos-gerenciar': () => (gerenciarAtalhos = { nova: false }),
    'atalhos-macros': () => (macrosVisivel = true),
    'atalhos-exportar': () => void exportarAtalhos(),
    'atalhos-importar': () => entradaImportar?.click(),
    'documentacao': () => abrirLink('https://pt.wikibooks.org/wiki/LaTeX'),
    'site': () => abrirLink(SITE),
    'contato': () => abrirLink(MAILTO_CONTATO),
    'feedback': () => void enviarFeedback(),
    'procurar-atualizacao': () => void procurarAtualizacao(),
  };

  const todasAcoes = $derived<Acoes>({ ...acoes, ...acoesDoLayout });

  /** Erro na barra de status: mostra o painel do PDF (se estava escondido) com o log. */
  function abrirLog() {
    if (modoLayout === 'editor') modoLayout = 'ambos';
    pdfRef?.mostrarLogs();
  }

  /** Logs e auxiliares que o rodape do log oferece para abrir no editor. */
  function saidasDoLatex(nos: FileNode[]): string[] {
    return nos.flatMap((n) =>
      n.tipo === 'pasta' ? saidasDoLatex(n.filhos ?? []) : /\.(log|aux|bbl|blg)$/i.test(n.nome) ? [n.caminho] : [],
    );
  }
  const outrosArquivos = $derived(saidasDoLatex(arquivos));

  const marcados = $derived<Record<string, boolean>>({
    'layout-ambos': modoLayout === 'ambos',
    'layout-editor': modoLayout === 'editor',
    'layout-pdf': modoLayout === 'pdf',
    'ver-trilha': preferencias.mostrarTrilha,
    'ver-abas': preferencias.mostrarAbas,
    'ver-previa-equacao': preferencias.previaEquacao,
  });

  const ACAO_DE_ATALHO_DE_ABA = { proxima: 'proxima-aba', anterior: 'aba-anterior', fechar: 'fechar-aba' } as const;

  function teclaGlobal(e: KeyboardEvent) {
    // Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PageDown, Ctrl+PageUp e Ctrl+W
    const atalhoAba = atalhoDeAba(e);
    if (atalhoAba) {
      e.preventDefault();
      executar(todasAcoes, ACAO_DE_ATALHO_DE_ABA[atalhoAba]);
      return;
    }
    if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'p') {
      e.preventDefault();
      paletaVisivel = !paletaVisivel;
      return;
    }
  }

  let painelAtivo = $state('arquivos');
  let larguraPainel = $state(240);
  let larguraJanela = $state(typeof window !== 'undefined' ? window.innerWidth : 1280);
  let fracaoPdf = $state(preferencias.fracaoPdf);
  let redimensionando: 'painel' | 'pdf' | null = $state(null);
  let pdfRef: ReturnType<typeof PdfViewer> | undefined = $state();
  let buscaTexto = $state('');
  let chatRef: ReturnType<typeof ChatIA> | undefined = $state();

  /** Menu do editor, "Perguntar à IA": abre o chat e deixa o trecho na caixa de pergunta. */
  export async function perguntarIA(trecho: string) {
    painelAtivo = 'chat';
    await tick();
    chatRef?.perguntarSobre(trecho);
  }

  async function explicarErro(erro: string) {
    painelAtivo = 'chat';
    await tick();
    chatRef?.explicarErro(erro);
  }

  function acharNo(nos: FileNode[], caminho: string): boolean {
    return nos.some((n) => n.caminho === caminho || (n.tipo === 'pasta' && acharNo(n.filhos ?? [], caminho)));
  }

  // o log do LaTeX cita o arquivo como "./cap/intro.tex"; a arvore usa "/cap/intro.tex"
  async function irParaErro(linhaErro: number, arquivo?: string | null) {
    if (arquivo) {
      const rel = '/' + arquivo.replace(/\\/g, '/').replace(/^(\.\/|\/)+/, '');
      if (rel !== arquivoAtual && acharNo(arquivos, rel)) {
        await onselecionararquivo?.(rel);
        await tick();
      }
    }
    onirparalinha?.(linhaErro);
  }

  let painelArquivosRef: ReturnType<typeof PainelArquivos> | undefined = $state();
  let paletaSimbolosAberta = $state(false);

  // titulo da ultima secao antes do cursor, para a trilha "arquivo > secao"
  const secaoAtual = $derived.by(() => {
    const linhas = conteudo.split('\n');
    const re = /\\(?:part|chapter|section|subsection|subsubsection)\*?\{([^}]*)\}/;
    for (let i = Math.min(linha, linhas.length) - 1; i >= 0; i--) {
      const m = re.exec(linhas[i]);
      if (m) return m[1].trim();
    }
    return '';
  });

  async function abrirCriacaoNoPainel(tipo: 'arquivo' | 'pasta') {
    painelAtivo = 'arquivos';
    await tick();
    if (tipo === 'arquivo') painelArquivosRef?.novoArquivo();
    else painelArquivosRef?.novaPasta();
  }

  function irParaCodigoDoVisivel() {
    const pos = pdfRef?.posicaoVisivel();
    if (pos) onirparacodigo?.(pos.pagina, pos.x, pos.y);
  }

  function trocarPainel(painel: string) {
    painelAtivo = painel;
  }

  function iniciarRedimensionar(lado: 'painel' | 'pdf') {
    // Pointer Events: o mesmo codigo serve para mouse, caneta e toque.
    // setPointerCapture mantem os eventos no divisor mesmo com o dedo fora dele.
    return (e: PointerEvent) => {
      if (e.pointerType === 'mouse' && e.button !== 0) return;
      e.preventDefault();
      const alvo = e.currentTarget as HTMLElement;
      const id = e.pointerId;
      alvo.setPointerCapture(id);
      redimensionando = lado;

      const deslocamento = lado === 'painel' ? 44 : 0;

      const mover = (ev: PointerEvent) => {
        if (redimensionando === 'painel') {
          larguraPainel = Math.max(160, Math.min(400, ev.clientX - deslocamento));
        } else if (redimensionando === 'pdf') {
          fracaoPdf = limitarLarguraPdf(window.innerWidth - ev.clientX, window.innerWidth, outrosPaineis) / window.innerWidth;
        }
      };

      const soltar = () => {
        if (redimensionando === 'pdf' && Number.isFinite(fracaoPdf)) definirPreferencia('fracaoPdf', fracaoPdf);
        redimensionando = null;
        if (alvo.hasPointerCapture?.(id)) alvo.releasePointerCapture(id);
        alvo.removeEventListener('pointermove', mover);
        alvo.removeEventListener('pointerup', soltar);
        alvo.removeEventListener('pointercancel', soltar);
        alvo.removeEventListener('lostpointercapture', soltar);
      };

      alvo.addEventListener('pointermove', mover);
      alvo.addEventListener('pointerup', soltar);
      alvo.addEventListener('pointercancel', soltar);
      alvo.addEventListener('lostpointercapture', soltar);
    };
  }

  let nomeArquivo = $derived(
    arquivoAtual ? arquivoAtual.split('/').pop() || '' : ''
  );

  // fora de .tex, .sty e .cls a barra so mostra desfazer, refazer e a zona direita (toolbar-items.tsx, Overleaf)
  const arquivoLatex = $derived(arquivoAtual === '' || /\.(tex|sty|cls|ltx|dtx)$/i.test(arquivoAtual));

  let painelLateralVisivel = $derived(painelAtivo !== '');
  // barra de icones (44 px) e painel lateral aberto: o editor fica com o resto, no minimo 360 px
  let outrosPaineis = $derived(44 + (painelLateralVisivel ? (painelAtivo === 'chat' ? 340 : larguraPainel) : 0));
  let larguraPdf = $derived(larguraPdfLivre);
  let larguraPdfLivre = $derived(limitarLarguraPdf(fracaoPdf * larguraJanela, larguraJanela, outrosPaineis));

  // com uma imagem ativa o editor continua montado, so escondido
  const imagemAtiva = $derived(abas.find((a) => a.caminho === abaAtiva)?.tipo === 'imagem');
  const idAbaAtiva = $derived.by(() => {
    const i = abas.findIndex((a) => a.caminho === abaAtiva);
    return i >= 0 ? idDaAba(i) : undefined;
  });
</script>

<div class="layout">
  <Toolbar
    {nomeProjeto}
    {modoLayout}
    onlayout={(m) => (modoLayout = m)}
    {onrenomear}
    {oninicio}
    {oncompartilhar}
    onhistorico={() => todasAcoes['versoes-anteriores']?.()}
    {estadoDrive}
    {detalheDrive}
    acoes={todasAcoes}
    {marcados}
  />

  <div class="corpo">
    <BarraLateral
      {painelAtivo}
      ontrocarpainel={trocarPainel}
      {onconfigurar}
      onatalhos={() => (atalhosVisivel = true)}
      oncomousar={() => (comoUsarVisivel = true)}
      onapoiar={() => (pixVisivel = true)}
    />

    {#if painelLateralVisivel}
      <aside class="painel-lateral" style="width: {painelAtivo === 'chat' ? 340 : larguraPainel}px">
        {#if painelAtivo === 'arquivos'}
          <PainelArquivos
            bind:this={painelArquivosRef}
            {arquivos}
            selecionado={abaAtiva || arquivoAtual}
            principal={abaPrincipal}
            onatualizar={onatualizararquivos}
            {conteudo}
            linhaAtual={linha}
            onselecionar={onselecionararquivo}
            {onirparalinha}
            {oncriararquivo}
            {oncriarpasta}
            {onenviararquivo}
            {onrenomearitem}
            {onexcluiritem}
            {onbaixaritem}
            {ondefinirprincipal}
          />
        {:else if painelAtivo === 'busca'}
          <div class="painel-cabecalho">
            <Search size={13} />
            <span>Buscar no projeto</span>
          </div>
          <div class="painel-conteudo">
            <div class="busca-campo">
              <!-- svelte-ignore a11y_autofocus -->
              <input
                type="text"
                bind:value={buscaTexto}
                oninput={agendarBusca}
                onkeydown={(e) => { if (e.key === 'Enter') realizarBusca(); }}
                placeholder="Buscar no projeto..."
                class="busca-input"
                autofocus
              />
            </div>
            {#if buscaCarregando}
              <p class="busca-info">Buscando...</p>
            {:else if buscaFeita}
              <p class="busca-info">{buscaResultados.length} {buscaResultados.length === 1 ? 'resultado' : 'resultados'}</p>
              {#if buscaCortada}
                <p class="busca-info">{t('busca.cortado')}</p>
              {/if}
              {#each buscaResultados as r, i}
                {#if i === 0 || buscaResultados[i - 1].arquivo !== r.arquivo}
                  <div class="busca-arquivo"><FileText size={12} /> {r.arquivo.split('/').pop()}</div>
                {/if}
                <button class="busca-item" onclick={() => abrirResultado(r.arquivo, r.linha)} title={r.texto}>
                  <span class="busca-linha">{r.linha}</span>
                  <span class="busca-texto">{r.texto}</span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}
        <!-- fica montado o tempo todo: trocar de painel so esconde, a conversa continua -->
        <ChatIA
          bind:this={chatRef}
          visivel={painelAtivo === 'chat'}
          config={configIA}
          {agente}
          onfechar={() => painelAtivo = 'arquivos'}
          onconfigIA={onconfigIA}
        />
      </aside>

      {#if painelAtivo !== 'chat'}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="divisor" onpointerdown={iniciarRedimensionar('painel')}></div>
      {/if}
    {/if}

    <div class="area-central" class:oculto={modoLayout === 'pdf'}>
      <main class="painel-editor" class:redimensionando={redimensionando !== null}>
        <!-- com "Mostrar abas" desligado a barra inteira some, como no Overleaf -->
        {#if preferencias.mostrarAbas}
          <BarraAbas
            {abas}
            ativa={abaAtiva}
            sujas={abasSujas}
            principal={abaPrincipal}
            ontrocar={onselecionaraba}
            onfechar={onfecharaba}
            onfecharoutras={onfecharoutras}
            onmover={onmoveraba}
          />
        {/if}
        {#if !imagemAtiva}
          <EditorToolbar
            oncomando={(id) => executar(todasAcoes, id)}
            {estilo}
            modos={modosEditor}
            latex={arquivoLatex}
            {profundidadeLista}
            irParaPdf={modoLayout === 'editor'}
          />
          {#if arquivoLatex && modosEditor.modo !== 'visualizando'}
            <BarraMatematica oncomando={(id) => executar(todasAcoes, id)} oninserir={(latex) => oninserirtexto?.(latex)} />
          {/if}
          {#if preferencias.mostrarTrilha}
            <Trilha arquivo={nomeArquivo} secao={secaoAtual} />
          {/if}
        {/if}
        <div class="editor-conteudo" role="tabpanel" id="painel-abas" aria-labelledby={idAbaAtiva}>
          <div class="slot-editor" class:oculto={imagemAtiva}>
            {#if children}{@render children()}{/if}
          </div>
          <div class="slot-imagem" class:oculto={!imagemAtiva}>
            {#if imagem}{@render imagem()}{/if}
          </div>
        </div>
        <PaletaSimbolos
          visivel={paletaSimbolosAberta}
          oninserir={(latex) => oninserirtexto?.(latex)}
          onfechar={() => (paletaSimbolosAberta = false)}
        />
      </main>

    </div>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="divisor" class:oculto={modoLayout !== 'ambos'} onpointerdown={iniciarRedimensionar('pdf')}>
      {#if synctexDisponivel && pdfDados}
        <div class="sync">
          <button
            class="sync-btn"
            title="Ir para a posição no PDF"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={onirparapdf}
          ><ArrowRight size={13} /></button>
          <button
            class="sync-btn"
            title="Ir para a posição no código"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={irParaCodigoDoVisivel}
          ><ArrowLeft size={13} /></button>
        </div>
      {/if}
    </div>

    <aside
      class="painel-pdf"
      class:oculto={modoLayout === 'editor'}
      class:cheio={modoLayout === 'pdf'}
      style="width: {larguraPdf}px"
    >
      <PdfViewer
        bind:this={pdfRef}
        dados={pdfDados}
        {compilando}
        {erros}
        log={logTexto}
        {tempoMs}
        {iaAtiva}
        autoCompilar={preferencias.autoCompilar}
        pararNoPrimeiroErro={preferencias.pararNoPrimeiroErro}
        {synctexDisponivel}
        {destaque}
        {oncompilar}
        {onpararcompilacao}
        {onrecompilardozero}
        onautocompilar={(ligado) => definirPreferencia('autoCompilar', ligado)}
        onpararnoprimeiroerro={(ligado) => definirPreferencia('pararNoPrimeiroErro', ligado)}
        {onbaixar}
        ontraduzir={ontraduzirpdf}
        onirparalinha={irParaErro}
        onexplicarerro={explicarErro}
        {onirparacodigo}
        {outrosArquivos}
        onlimparcache={() => todasAcoes['limpar-auxiliares']?.()}
        onabrirarquivo={(c) => onselecionararquivo?.(c)}
      />
    </aside>
  </div>

  <AtalhosModal visivel={atalhosVisivel} onfechar={() => (atalhosVisivel = false)} />
  <ComoUsarModal visivel={comoUsarVisivel} onfechar={() => (comoUsarVisivel = false)} />
    <StatusBar
      {linha}
      {coluna}
      {statusCompilacao}
      {erros}
      salvo={abasSujas.length === 0}
      motor={motorDetectado}
      {ortografia}
      onabrirlog={abrirLog}
      onortografia={() => onconfigurarortografia?.()}
    />

  {#if pixVisivel}
    <PixModal aoFechar={() => (pixVisivel = false)} />
  {/if}
  {#if gerenciarAtalhos}
    <GerenciarAtalhosModal iniciarNova={gerenciarAtalhos.nova} aoFechar={() => (gerenciarAtalhos = null)} />
  {/if}
  {#if macrosVisivel}
    <MacrosModal aoFechar={() => (macrosVisivel = false)} />
  {/if}
  <input
    type="file"
    accept=".json,application/json"
    hidden
    bind:this={entradaImportar}
    onchange={(e) => {
      const entrada = e.currentTarget;
      const arquivo = entrada.files?.[0];
      entrada.value = '';
      if (arquivo) void importarAtalhosDeArquivo(arquivo);
    }}
  />
  <SobreModal
    visivel={sobreVisivel}
    onfechar={() => (sobreVisivel = false)}
    onfeedback={() => void enviarFeedback()}
    onapoiar={() => (pixVisivel = true)}
  />
  <PaletaComandos visivel={paletaVisivel} acoes={todasAcoes} onfechar={() => (paletaVisivel = false)} />
</div>

<svelte:window onkeydown={teclaGlobal} bind:innerWidth={larguraJanela} />

<style>
  .layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--fundo);
    color: var(--texto);
    font-family: "Segoe UI", Inter, system-ui, -apple-system, sans-serif;
  }

  .corpo {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  /* Sidebar escuro estilo Overleaf */
  .painel-lateral {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    background: var(--superficie-0);
    border-right: 1px solid var(--borda);
    overflow: hidden;
    width: 260px;
  }

  .painel-cabecalho {
    height: 36px;
    box-sizing: border-box;
    padding: 0 10px;
    display: flex;
    align-items: center;
    background: var(--superficie-0);
    border-bottom: 1px solid var(--borda);
    flex-shrink: 0;
  }

  .painel-conteudo {
    flex: 1;
    overflow: auto;
    padding: 4px;
    background: var(--superficie-0);
  }

  .painel-placeholder {
    padding: 16px;
    color: var(--texto-terciario);
    font-size: 0.8125rem;
    text-align: center;
  }

  .busca-campo {
    padding: 8px;
  }

  .busca-input {
    width: 100%;
    padding: 7px 12px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    font-size: 0.8125rem;
    font-family: inherit;
    outline: none;
    box-sizing: border-box;
  }

  .busca-input:focus {
    border-color: var(--primaria);
  }

  .area-central {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    overflow: hidden;
  }

  .painel-editor {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--fundo);
  }

  .painel-editor.redimensionando {
    pointer-events: none;
  }

  .editor-conteudo {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: #FFFFFF;  /* fundo branco do editor, igual Overleaf */
    color: #212121;
  }

  .slot-editor,
  .slot-imagem {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .oculto {
    display: none !important;
  }

  .painel-pdf.cheio {
    flex: 1;
    width: auto !important;
    border-left: none;
  }

  .busca-info {
    margin: 6px 8px;
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .busca-arquivo {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 8px 6px 2px;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--texto);
  }

  .busca-item {
    display: flex;
    gap: 8px;
    width: 100%;
    padding: 3px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font-size: 0.75rem;
    text-align: left;
    cursor: pointer;
    font-family: inherit;
  }

  .busca-item:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .busca-linha {
    flex-shrink: 0;
    min-width: 28px;
    color: var(--texto-terciario);
  }

  .busca-texto {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .painel-pdf {
    /* encolhe quando a janela nao cabe editor (min 200 px), painel lateral e PDF */
    flex-shrink: 1;
    min-width: 0;
    background: var(--superficie-1);
    border-left: 1px solid var(--borda);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .divisor {
    width: 4px;
    cursor: col-resize;
    touch-action: none;
    z-index: 2;
    background: transparent;
    flex-shrink: 0;
    transition: background 0.15s;
    position: relative;
  }

  .divisor::after {
    content: '';
    position: absolute;
    top: 0;
    left: -3px;
    right: -3px;
    bottom: 0;
  }

  /* no toque a area de pegar tem 16 px, mesmo com a linha visivel de 4 px */
  @media (pointer: coarse) {
    .divisor::after {
      left: -6px;
      right: -6px;
    }
  }

  .divisor:hover {
    background: var(--primaria);
  }

  .sync {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    gap: 4px;
    z-index: 5;
  }

  .sync-btn {
    width: 22px;
    height: 22px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--borda);
    border-radius: 4px;
    background: var(--superficie-0);
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .sync-btn:hover {
    background: var(--hover);
    color: var(--texto);
  }
</style>
