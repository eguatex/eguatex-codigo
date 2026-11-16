<script lang="ts" module>
  import type * as PdfJs from "pdfjs-dist";

  export type Destaque = {
    pagina: number;
    x: number;
    y: number;
    largura: number;
    altura: number;
  };

  // Biblioteca carregada uma unica vez e compartilhada entre as instancias.
  let pdfjsLib: typeof PdfJs | null = null;
</script>

<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { planejarBarraPdf, ORDEM_CORTE_DIREITO, type GrupoDireito } from "$lib/corte-da-barra";
  import type { Action } from "svelte/action";
  import Play from "@lucide/svelte/icons/play";
  import Girador from "./Girador.svelte";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Download from "@lucide/svelte/icons/download";
  import Contrast from "@lucide/svelte/icons/contrast";
  import ZoomIn from "@lucide/svelte/icons/zoom-in";
  import ZoomOut from "@lucide/svelte/icons/zoom-out";
  import Presentation from "@lucide/svelte/icons/presentation";
  import FileText from "@lucide/svelte/icons/file-text";
  import Check from "@lucide/svelte/icons/check";
  import ItensCompilacao from "./ItensCompilacao.svelte";
  import ListaLogs, { type ErroLog } from "./ListaLogs.svelte";
  import { t, tf } from "$lib/i18n";
  import { contagens } from "$lib/filtro-log";
  import { preferencias, definirPreferencia } from "$lib/preferencias.svelte";
  import { alternarModo, pdfInvertido } from "$lib/pdf-escuro";
  import { formatarTempo } from "$lib/formatar-tempo";

  interface Props {
    dados?: Uint8Array | null;
    compilando?: boolean;
    erros?: ErroLog[];
    log?: string;
    /** Duracao da ultima compilacao em ms, mostrada ao lado do Compilar. */
    tempoMs?: number | null;
    iaAtiva?: boolean;
    autoCompilar?: boolean;
    pararNoPrimeiroErro?: boolean;
    synctexDisponivel?: boolean;
    destaque?: Destaque | null;
    oncompilar?: () => void;
    onpararcompilacao?: () => void;
    onrecompilardozero?: () => void;
    onautocompilar?: (ligado: boolean) => void;
    onpararnoprimeiroerro?: (ligado: boolean) => void;
    /** Salvar como: "pdf", "docx", "md", "html" ou "zip". */
    onbaixar?: (formato: string) => void;
    /** Clique direito, "Traduzir": recebe o texto selecionado no PDF. */
    ontraduzir?: (texto: string) => void;
    onirparalinha?: (linha: number, arquivo?: string | null) => void;
    onexplicarerro?: (erro: string) => void;
    onirparacodigo?: (pagina: number, x: number, y: number) => void;
    /** Caminhos dos .log, .aux, .bbl e .blg do projeto, para o rodape do log. */
    outrosArquivos?: string[];
    onlimparcache?: () => void;
    onabrirarquivo?: (caminho: string) => void;
  }

  let {
    dados = null,
    compilando = false,
    erros = [],
    log = "",
    tempoMs = null,
    iaAtiva = false,
    autoCompilar = false,
    pararNoPrimeiroErro = false,
    synctexDisponivel = false,
    destaque = null,
    oncompilar,
    onpararcompilacao,
    onrecompilardozero,
    onautocompilar,
    onpararnoprimeiroerro,
    onbaixar,
    ontraduzir,
    onirparalinha,
    onexplicarerro,
    onirparacodigo,
    outrosArquivos = [],
    onlimparcache,
    onabrirarquivo,
  }: Props = $props();

  type ModoZoom = "largura" | "altura" | "manual";

  interface PaginaInfo {
    numero: number;
    largura: number; // em pontos, na escala 1
    altura: number;
  }

  interface EstadoPagina {
    numero: number;
    elemento: HTMLDivElement;
    canvas: HTMLCanvasElement | null;
    /** Camada invisivel com o texto da pagina, para selecionar e copiar. */
    camadaTexto: HTMLDivElement | null;
    /** geracaoDoc do PDF de onde a camada de texto saiu */
    geracaoTexto?: number;
    escalaTexto: number;
    chaveDesenhada: string;
    chaveEmAndamento: string;
    tarefa: PdfJs.RenderTask | null;
    visivel: boolean;
    ticket: number;
  }

  interface Ancora {
    numero: number;
    fracao: number; // posicao dentro da pagina, de 0 a 1
    folgaPx: number; // quando a rolagem esta na margem acima da pagina (valor negativo, em px)
    fracaoX: number;
  }

  const MARGEM_PAGINA = 12;
  const ZOOM_MIN = 0.1;
  const ZOOM_MAX = 8;
  const PASSOS_ZOOM = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4, 5];
  // Um passo so conta se mudar o zoom em pelo menos 4%, para nao gastar um clique em 74% para 75%.
  const FOLGA_PASSO = 1.04;
  const ZOOMS_MENU = [0.5, 0.75, 1, 1.25, 1.5, 2, 4];
  const MAX_PIXELS_CANVAS = 2 ** 25;
  const DURACAO_DESTAQUE_MS = 2000;
  const SELETOR_ITEM_MENU = '[role^="menuitem"]:not(:disabled)';

  // Estado visual (reativo)
  let paginas = $state.raw<PaginaInfo[]>([]);
  let totalPaginas = $state(0);
  let paginaAtual = $state(1);
  let escala = $state(1);
  let modoZoom = $state<ModoZoom>("largura");
  let vistaLogs = $state(false);
  const totaisLog = $derived(contagens(erros, log));

  let apresentando = $state(false);
  let carregando = $state(false);
  let erroCarregamento = $state("");
  let aviso = $state("");
  let pdfjsPronto = $state(false);
  let menuCompilarAberto = $state(false);
  let menuSalvarAberto = $state(false);
  // Barra do PDF: quem sai quando falta espaco vem de planejarBarraPdf, com larguras medidas no DOM.
  let barraEl = $state<HTMLDivElement | null>(null);
  let apresentacaoNoMenu = $state(false);
  let quadroBarra = 0;

  // PDF escuro: inverte as cores das paginas so na tela (o arquivo nao muda).
  // Modo em preferencias.pdfEscuro: desligado, com o tema da interface ou sempre.
  let temaEscuro = $state(false);
  $effect(() => {
    const raiz = document.documentElement;
    const midia = window.matchMedia("(prefers-color-scheme: dark)");
    const medir = () => {
      const fixo = raiz.dataset.tema;
      temaEscuro = fixo ? fixo === "escuro" : midia.matches;
    };
    medir();
    const obs = new MutationObserver(medir);
    obs.observe(raiz, { attributes: true, attributeFilter: ["data-tema"] });
    midia.addEventListener("change", medir);
    return () => {
      obs.disconnect();
      midia.removeEventListener("change", medir);
    };
  });
  const pdfEscuro = $derived(pdfInvertido(preferencias.pdfEscuro, temaEscuro));

  function alternarPdfEscuro() {
    definirPreferencia("pdfEscuro", alternarModo(preferencias.pdfEscuro, pdfEscuro));
  }

  // ---------- menu do clique direito (o mesmo do editor) ----------
  let menuTexto = $state<{ x: number; y: number; comSelecao: boolean } | null>(null);

  /** Texto selecionado dentro das paginas do PDF. */
  function selecaoPdf(): string {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !areaRolagem) return "";
    const no = sel.anchorNode;
    if (!no || !areaRolagem.contains(no)) return "";
    return sel.toString();
  }

  function abrirMenuTexto(ev: MouseEvent) {
    ev.preventDefault();
    fecharMenus();
    const x = Math.max(8, Math.min(ev.clientX, window.innerWidth - 280 - 8));
    const y = Math.max(8, Math.min(ev.clientY, window.innerHeight - 250));
    menuTexto = { x, y, comSelecao: !!selecaoPdf().trim() };
  }

  async function copiarPdf() {
    const texto = selecaoPdf();
    menuTexto = null;
    if (texto) await navigator.clipboard.writeText(texto).catch(() => {});
  }

  function traduzirPdf() {
    const texto = selecaoPdf();
    menuTexto = null;
    if (texto.trim()) ontraduzir?.(texto);
  }

  function selecionarTudoPdf() {
    menuTexto = null;
    const alvo = areaRolagem?.querySelector(".paginas");
    const sel = window.getSelection();
    if (!alvo || !sel) return;
    const faixa = document.createRange();
    faixa.selectNodeContents(alvo);
    sel.removeAllRanges();
    sel.addRange(faixa);
  }

  function teclaPdf(ev: KeyboardEvent) {
    const tecla = ev.key.toLowerCase();
    if (ev.ctrlKey && ev.shiftKey && tecla === "t") {
      ev.preventDefault();
      traduzirPdf();
    } else if (ev.ctrlKey && !ev.shiftKey && tecla === "a") {
      ev.preventDefault();
      selecionarTudoPdf();
    }
  }

  // Os 5 formatos do botao Baixar, na ordem do plano de 09/11/2026.
  const FORMATOS_SALVAR = [
    { id: "pdf", chave: "pdf.formato.pdf" },
    { id: "zip", chave: "pdf.formato.zip" },
    { id: "docx", chave: "pdf.formato.docx" },
    { id: "md", chave: "pdf.formato.md" },
    { id: "html", chave: "pdf.formato.html" },
  ] as const;

  function salvarComo(formato: string) {
    menuSalvarAberto = false;
    onbaixar?.(formato);
  }
  let menuZoomAberto = $state(false);
  let campoPagina = $state("1");
  let editandoPagina = $state(false);
  let destaqueAtivo = $state<Destaque | null>(null);

  // Estado interno (nao reativo)
  let areaRolagem: HTMLDivElement | null = null;
  let menuCompilar = $state<HTMLDivElement | null>(null);
  let menuZoom = $state<HTMLDivElement | null>(null);
  let botaoSeta = $state<HTMLButtonElement | null>(null);
  let botaoZoom = $state<HTMLButtonElement | null>(null);
  let pdfDoc: PdfJs.PDFDocumentProxy | null = null;
  let tokenCarga = 0;
  let geracaoDoc = 0;
  const paginasEstado = new Map<number, EstadoPagina>();
  let observador: IntersectionObserver | null = null;
  let observadorTamanho: ResizeObserver | null = null;
  let temporizadorRedesenho: ReturnType<typeof setTimeout> | undefined;
  let temporizadorAviso: ReturnType<typeof setTimeout> | undefined;
  let temporizadorDestaque: ReturnType<typeof setTimeout> | undefined;
  let bloqueioRenderAte = 0;
  let quadroRolagem = 0;
  let quadroTamanho = 0;
  let ultimoDestaque: Destaque | null = null;
  let compilandoAntes = false;
  let logsPorErro = false;
  let antesDaApresentacao: { modo: ModoZoom; escala: number } | null = null;

  // Contador do botao de logs: erros mais avisos, como no Overleaf (pdf-hybrid-logs-button.tsx).
  const totalLog = $derived(totaisLog.erros + totaisLog.avisos);
  const seloTexto = $derived(totalLog > 99 ? "99+" : String(totalLog));
  const percentual = $derived(Math.round(escala * 100));
  const semPaginas = $derived(paginas.length === 0);
  const podeMenosZoom = $derived(PASSOS_ZOOM.some((p) => p < escala / FOLGA_PASSO));
  const podeMaisZoom = $derived(PASSOS_ZOOM.some((p) => p > escala * FOLGA_PASSO));

  // ---------- funcoes exportadas ----------

  export function maisZoom() {
    passoZoom(1);
  }

  export function menosZoom() {
    passoZoom(-1);
  }

  export function ajustarLargura() {
    void aplicarModoZoom("largura", 0);
  }

  export function ajustarAltura() {
    void aplicarModoZoom("altura", 0);
  }

  export function modoApresentacao() {
    void entrarApresentacao();
  }

  // ---------- utilidades ----------

  function limitar(valor: number, minimo: number, maximo: number): number {
    return Math.min(maximo, Math.max(minimo, valor));
  }

  function nomeDoErro(erro: unknown): string {
    if (typeof erro === "object" && erro !== null && "name" in erro) {
      return String((erro as { name: unknown }).name);
    }
    return "";
  }

  function detalheDoErro(erro: unknown): string {
    return erro instanceof Error && erro.message ? `: ${erro.message}` : "";
  }

  function mostrarAviso(texto: string) {
    aviso = texto;
    clearTimeout(temporizadorAviso);
    temporizadorAviso = setTimeout(() => {
      aviso = "";
    }, 4000);
  }

  // ---------- carregamento do PDF.js ----------

  onMount(() => {
    let ativo = true;

    void (async () => {
      try {
        const mod = await import("pdfjs-dist");
        mod.GlobalWorkerOptions.workerSrc = new URL(
          "pdfjs-dist/build/pdf.worker.mjs",
          import.meta.url
        ).toString();
        pdfjsLib = mod;
        if (ativo) pdfjsPronto = true;
      } catch (erro) {
        if (ativo) erroCarregamento = tf("pdf.falha-visualizador", { detalhe: detalheDoErro(erro) });
      }
    })();

    if (areaRolagem) {
      observadorTamanho = new ResizeObserver(() => {
        cancelAnimationFrame(quadroTamanho);
        quadroTamanho = requestAnimationFrame(aoRedimensionar);
      });
      observadorTamanho.observe(areaRolagem, { box: "border-box" });
    }

    // a barra se mede de novo quando muda de largura (painel, janela, escala do Windows) e quando muda o que ela mostra
    let observadorBarra: ResizeObserver | null = null;
    let mutacoesBarra: MutationObserver | null = null;
    if (barraEl) {
      observadorBarra = new ResizeObserver(agendarBarra);
      observadorBarra.observe(barraEl);
      mutacoesBarra = new MutationObserver(agendarBarra);
      mutacoesBarra.observe(barraEl, { childList: true, subtree: true, characterData: true });
      agendarBarra();
      void document.fonts?.ready.then(agendarBarra);
    }

    return () => {
      ativo = false;
      tokenCarga += 1;
      observador?.disconnect();
      observador = null;
      observadorTamanho?.disconnect();
      observadorTamanho = null;
      observadorBarra?.disconnect();
      mutacoesBarra?.disconnect();
      cancelAnimationFrame(quadroBarra);
      clearTimeout(temporizadorRedesenho);
      clearTimeout(temporizadorAviso);
      clearTimeout(temporizadorDestaque);
      cancelAnimationFrame(quadroRolagem);
      cancelAnimationFrame(quadroTamanho);
      cancelarRenderizacoes();
      const doc = pdfDoc;
      pdfDoc = null;
      void doc?.destroy();
      if (areaRolagem && document.fullscreenElement === areaRolagem) {
        void document.exitFullscreen();
      }
    };
  });

  // Carrega, troca ou limpa o documento quando os bytes mudam.
  $effect(() => {
    const bytes = dados;
    if (!pdfjsPronto) return;
    untrack(() => {
      if (bytes) void carregarPdf(bytes);
      else limparDocumento();
    });
  });

  async function carregarPdf(bytes: Uint8Array) {
    const lib = pdfjsLib;
    if (!lib) return;
    const meuToken = ++tokenCarga;
    carregando = true;
    erroCarregamento = "";
    let novo: PdfJs.PDFDocumentProxy | null = null;
    try {
      // O PDF.js transfere o buffer para o worker; a copia protege o original (usado em Baixar PDF).
      novo = await lib.getDocument({ data: bytes.slice() }).promise;
      const lista = await lerTamanhos(novo, meuToken);
      if (lista === null || meuToken !== tokenCarga) {
        void novo.destroy();
        return;
      }
      const pronto = novo;
      novo = null;
      await aplicarDocumento(pronto, lista);
    } catch (erro) {
      if (novo) void novo.destroy();
      if (meuToken === tokenCarga) {
        erroCarregamento = tf("pdf.falha-abrir", { detalhe: detalheDoErro(erro) });
      }
    } finally {
      if (meuToken === tokenCarga) carregando = false;
    }
  }

  async function lerTamanhos(doc: PdfJs.PDFDocumentProxy, token: number): Promise<PaginaInfo[] | null> {
    const total = doc.numPages;
    const lista: PaginaInfo[] = [];
    const lote = 50;
    for (let inicio = 1; inicio <= total; inicio += lote) {
      const fim = Math.min(total, inicio + lote - 1);
      const pedidos: Promise<PaginaInfo>[] = [];
      for (let n = inicio; n <= fim; n++) {
        pedidos.push(
          doc.getPage(n).then((pagina) => {
            const vista = pagina.getViewport({ scale: 1 });
            return { numero: n, largura: vista.width, altura: vista.height };
          })
        );
      }
      lista.push(...(await Promise.all(pedidos)));
      if (token !== tokenCarga) return null;
    }
    return lista;
  }

  function mesmosTamanhos(a: PaginaInfo[], b: PaginaInfo[]): boolean {
    if (a.length !== b.length) return false;
    return a.every((p, i) => p.largura === b[i].largura && p.altura === b[i].altura);
  }

  // Troca o documento mantendo zoom e posicao de rolagem.
  async function aplicarDocumento(doc: PdfJs.PDFDocumentProxy, lista: PaginaInfo[]) {
    const antigo = pdfDoc;
    const mudouTamanho = !mesmosTamanhos(paginas, lista);
    const ancora = capturarAncora();

    cancelarRenderizacoes();
    geracaoDoc += 1;
    pdfDoc = doc;
    paginas = lista;
    totalPaginas = lista.length;
    paginaAtual = limitar(paginaAtual, 1, lista.length);

    let escalaMudou = false;
    if (modoZoom !== "manual") {
      const fit = calcularFit(modoZoom, lista);
      if (fit !== null) {
        const nova = limitar(fit, ZOOM_MIN, ZOOM_MAX);
        escalaMudou = Math.abs(nova - escala) > 0.0005;
        escala = nova;
      }
    }

    await tick();
    if (mudouTamanho || escalaMudou) restaurarAncora(ancora);
    redesenharVisiveis();
    agendarAtualizacaoPagina();
    void antigo?.destroy();
  }

  function limparDocumento() {
    tokenCarga += 1;
    cancelarRenderizacoes();
    geracaoDoc += 1;
    const antigo = pdfDoc;
    pdfDoc = null;
    paginas = [];
    totalPaginas = 0;
    paginaAtual = 1;
    carregando = false;
    erroCarregamento = "";
    destaqueAtivo = null;
    void antigo?.destroy();
  }

  // ---------- desenho das paginas ----------

  function obterObservador(no: HTMLElement): IntersectionObserver | null {
    if (observador) return observador;
    const raiz = areaRolagem ?? no.closest<HTMLElement>(".rolagem");
    if (!raiz) return null;
    observador = new IntersectionObserver(aoInterceptar, { root: raiz, rootMargin: "100% 0px" });
    return observador;
  }

  function aoInterceptar(entradas: IntersectionObserverEntry[]) {
    for (const entrada of entradas) {
      const numero = Number((entrada.target as HTMLElement).dataset.pagina);
      const est = paginasEstado.get(numero);
      if (!est) continue;
      est.visivel = entrada.isIntersecting;
      if (est.visivel) void desenharPagina(est);
      else liberarPagina(est);
    }
    agendarAtualizacaoPagina();
  }

  const registrarPagina: Action<HTMLDivElement, number> = (no, numero) => {
    const est: EstadoPagina = {
      numero,
      elemento: no,
      canvas: null,
      camadaTexto: null,
      escalaTexto: 0,
      chaveDesenhada: "",
      chaveEmAndamento: "",
      tarefa: null,
      visivel: false,
      ticket: 0,
    };
    paginasEstado.set(numero, est);
    no.addEventListener("dblclick", aoDuploClique);
    obterObservador(no)?.observe(no);
    return {
      destroy() {
        observador?.unobserve(no);
        no.removeEventListener("dblclick", aoDuploClique);
        cancelarPagina(est);
        if (paginasEstado.get(numero) === est) paginasEstado.delete(numero);
      },
    };
  };

  // Escala efetiva do canvas: escala da tela vezes a densidade de pixels, limitada pela memoria.
  function escalaTotal(info: PaginaInfo, escalaTela: number): number {
    const densidade = window.devicePixelRatio || 1;
    const area = info.largura * escalaTela * info.altura * escalaTela;
    let saida = densidade;
    if (area * saida * saida > MAX_PIXELS_CANVAS) saida = Math.sqrt(MAX_PIXELS_CANVAS / area);
    return escalaTela * saida;
  }

  async function desenharPagina(est: EstadoPagina): Promise<void> {
    const doc = pdfDoc;
    if (!doc || !est.visivel || performance.now() < bloqueioRenderAte) return;
    const info = paginas[est.numero - 1];
    if (!info) return;

    const alvo = escalaTotal(info, escala);
    const chave = `${geracaoDoc}:${alvo.toFixed(4)}`;
    if (est.canvas && est.chaveDesenhada === chave) return;
    if (est.chaveEmAndamento === chave) return;

    est.ticket += 1;
    const ticket = est.ticket;
    est.tarefa?.cancel();
    est.tarefa = null;
    est.chaveEmAndamento = chave;

    try {
      const pagina = await doc.getPage(est.numero);
      if (ticket !== est.ticket || doc !== pdfDoc) return;

      const vista = pagina.getViewport({ scale: alvo });
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.floor(vista.width));
      canvas.height = Math.max(1, Math.floor(vista.height));
      const contexto = canvas.getContext("2d");
      if (!contexto) {
        est.chaveEmAndamento = "";
        return;
      }

      const tarefa = pagina.render({ canvasContext: contexto, viewport: vista });
      est.tarefa = tarefa;
      await tarefa.promise;
      if (ticket !== est.ticket || doc !== pdfDoc) return;

      est.tarefa = null;
      est.chaveEmAndamento = "";
      descartarCanvas(est.canvas);
      est.elemento.prepend(canvas);
      est.canvas = canvas;
      est.chaveDesenhada = chave;
      void desenharTexto(est, pagina, ticket, doc);
    } catch (erro) {
      if (ticket !== est.ticket) return;
      est.tarefa = null;
      est.chaveEmAndamento = "";
      if (nomeDoErro(erro) !== "RenderingCancelledException") {
        mostrarAviso(tf("pdf.falha-desenhar", { pagina: est.numero }));
      }
    }
  }

  // Texto selecionavel por cima do desenho (o mesmo recurso do Ctrl+C no Overleaf e no
  // Acrobat). O pdflatex grava o texto no PDF, entao nao precisa de OCR.
  async function desenharTexto(
    est: EstadoPagina,
    pagina: PdfJs.PDFPageProxy,
    ticket: number,
    doc: PdfJs.PDFDocumentProxy,
  ) {
    const lib = pdfjsLib;
    // a camada so serve se for do mesmo PDF (geracaoDoc) e do mesmo zoom; depois de
    // recompilar ela e refeita, senao Copiar e Traduzir pegariam o texto antigo
    if (!lib || (est.camadaTexto && est.escalaTexto === escala && est.geracaoTexto === geracaoDoc)) return;
    const geracaoDaVez = geracaoDoc;
    const escalaDaVez = escala;
    const camada = document.createElement("div");
    camada.className = "textLayer";
    camada.style.setProperty("--scale-factor", String(escalaDaVez));
    try {
      const tarefa = new lib.TextLayer({
        textContentSource: pagina.streamTextContent(),
        container: camada,
        viewport: pagina.getViewport({ scale: escalaDaVez }),
      });
      await tarefa.render();
      const fim = document.createElement("div");
      fim.className = "endOfContent";
      camada.append(fim);
    } catch {
      return;
    }
    if (ticket !== est.ticket || doc !== pdfDoc || escalaDaVez !== escala) return;
    est.camadaTexto?.remove();
    est.elemento.append(camada);
    est.camadaTexto = camada;
    est.escalaTexto = escalaDaVez;
    est.geracaoTexto = geracaoDaVez;
  }

  function descartarCanvas(canvas: HTMLCanvasElement | null) {
    if (!canvas) return;
    canvas.remove();
    canvas.width = 0;
    canvas.height = 0;
  }

  function cancelarPagina(est: EstadoPagina) {
    est.ticket += 1;
    est.tarefa?.cancel();
    est.tarefa = null;
    est.chaveEmAndamento = "";
  }

  function cancelarRenderizacoes() {
    for (const est of paginasEstado.values()) cancelarPagina(est);
  }

  // Pagina fora da zona de visao: solta o bitmap para economizar memoria.
  function liberarPagina(est: EstadoPagina) {
    cancelarPagina(est);
    descartarCanvas(est.canvas);
    est.canvas = null;
    est.camadaTexto?.remove();
    est.camadaTexto = null;
    est.escalaTexto = 0;
    est.chaveDesenhada = "";
    const doc = pdfDoc;
    if (doc) void limparRecursos(doc, est.numero);
  }

  async function limparRecursos(doc: PdfJs.PDFDocumentProxy, numero: number) {
    try {
      const pagina = await doc.getPage(numero);
      pagina.cleanup();
    } catch (erro) {
      // Documento ja destruido: nao ha o que liberar.
      return;
    }
  }

  function redesenharVisiveis() {
    const a = areaRolagem;
    if (!pdfDoc || !a) return;
    const centro = a.scrollTop + a.clientHeight / 2;
    const candidatas = [...paginasEstado.values()].filter((e) => e.visivel);
    const distancia = (e: EstadoPagina) =>
      Math.abs(e.elemento.offsetTop + e.elemento.offsetHeight / 2 - centro);
    candidatas.sort((x, y) => distancia(x) - distancia(y));
    for (const est of candidatas) void desenharPagina(est);
  }

  // Adia o desenho enquanto o zoom ainda esta mudando (ex.: arrastar o divisor).
  function agendarRedesenho(atraso: number) {
    clearTimeout(temporizadorRedesenho);
    if (atraso <= 0) {
      bloqueioRenderAte = 0;
      redesenharVisiveis();
      return;
    }
    bloqueioRenderAte = performance.now() + atraso;
    temporizadorRedesenho = setTimeout(() => {
      bloqueioRenderAte = 0;
      redesenharVisiveis();
    }, atraso);
  }

  // ---------- pagina atual e navegacao ----------

  function agendarAtualizacaoPagina() {
    if (quadroRolagem) return;
    quadroRolagem = requestAnimationFrame(() => {
      quadroRolagem = 0;
      atualizarPaginaAtual();
    });
  }

  function atualizarPaginaAtual() {
    const a = areaRolagem;
    if (!a || paginas.length === 0) return;
    const caixa = a.getBoundingClientRect();
    let melhor = 0;
    let melhorVisivel = 0;
    let atualInteira = false;
    for (const est of paginasEstado.values()) {
      if (!est.visivel) continue;
      const pagina = est.elemento.getBoundingClientRect();
      const visivel = Math.min(pagina.bottom, caixa.bottom) - Math.max(pagina.top, caixa.top);
      if (visivel <= 0) continue;
      if (est.numero === paginaAtual && visivel >= pagina.height - 1) atualInteira = true;
      if (visivel > melhorVisivel || (visivel === melhorVisivel && est.numero < melhor)) {
        melhor = est.numero;
        melhorVisivel = visivel;
      }
    }
    if (atualInteira || melhor === 0) return;
    if (melhor !== paginaAtual) paginaAtual = melhor;
  }

  function irParaPagina(numero: number) {
    const a = areaRolagem;
    if (!a || totalPaginas === 0) return;
    const alvo = limitar(Math.round(numero), 1, totalPaginas);
    const est = paginasEstado.get(alvo);
    if (!est) return;
    const margem = apresentando ? 0 : MARGEM_PAGINA;
    a.scrollTop = Math.max(0, est.elemento.offsetTop - margem);
    paginaAtual = alvo;
  }

  // Mantem o campo de pagina igual a pagina atual enquanto o usuario nao esta digitando.
  $effect(() => {
    const atual = paginaAtual;
    if (!editandoPagina) campoPagina = String(atual);
  });

  function aoFocarPagina(ev: FocusEvent & { currentTarget: HTMLInputElement }) {
    editandoPagina = true;
    ev.currentTarget.select();
  }

  function aoDesfocarPagina() {
    editandoPagina = false;
    campoPagina = String(paginaAtual);
  }

  function aoDigitarPagina(ev: Event & { currentTarget: HTMLInputElement }) {
    const limpo = ev.currentTarget.value.replace(/\D/g, "");
    ev.currentTarget.value = limpo;
    campoPagina = limpo;
  }

  function aoTeclarPagina(ev: KeyboardEvent & { currentTarget: HTMLInputElement }) {
    if (ev.key === "Enter") {
      ev.preventDefault();
      const numero = parseInt(campoPagina, 10);
      if (!Number.isNaN(numero)) irParaPagina(numero);
      campoPagina = String(paginaAtual);
      ev.currentTarget.select();
    } else if (ev.key === "Escape") {
      ev.preventDefault();
      campoPagina = String(paginaAtual);
      ev.currentTarget.blur();
    }
  }

  // ---------- zoom ----------

  function calcularFit(modo: "largura" | "altura", lista: PaginaInfo[]): number | null {
    const a = areaRolagem;
    if (!a || lista.length === 0) return null;
    const referencia = lista[limitar(paginaAtual, 1, lista.length) - 1];
    const margem = apresentando ? 0 : MARGEM_PAGINA * 2;
    const larguraLivre = a.clientWidth - margem;
    const alturaLivre = a.offsetHeight - margem;
    if (larguraLivre <= 0 || alturaLivre <= 0) return null;
    const porLargura = larguraLivre / referencia.largura;
    const porAltura = alturaLivre / referencia.altura;
    if (apresentando) return Math.min(porLargura, porAltura);
    return modo === "largura" ? porLargura : porAltura;
  }

  // ponto (em pontos PDF) no centro da area visivel, para o SyncTeX ir do PDF ao codigo
  export function posicaoVisivel(): { pagina: number; x: number; y: number } | null {
    const a = areaRolagem;
    if (!a) return null;
    const centro = a.scrollTop + a.clientHeight / 2;
    for (const p of paginas) {
      const el = paginasEstado.get(p.numero)?.elemento;
      if (!el) continue;
      if (el.offsetTop + el.offsetHeight >= centro) {
        const dentro = Math.max(0, centro - el.offsetTop);
        return { pagina: p.numero, x: el.offsetWidth / 2 / escala, y: dentro / escala };
      }
    }
    return null;
  }

  function capturarAncora(): Ancora | null {
    const a = areaRolagem;
    if (!a) return null;
    for (const p of paginas) {
      const el = paginasEstado.get(p.numero)?.elemento;
      if (!el) continue;
      if (el.offsetTop + el.offsetHeight > a.scrollTop) {
        const dentro = a.scrollTop - el.offsetTop;
        return {
          numero: p.numero,
          fracao: dentro > 0 ? dentro / Math.max(1, el.offsetHeight) : 0,
          folgaPx: Math.min(0, dentro),
          fracaoX: (a.scrollLeft + a.clientWidth / 2) / Math.max(1, a.scrollWidth),
        };
      }
    }
    return null;
  }

  function restaurarAncora(ancora: Ancora | null) {
    const a = areaRolagem;
    if (!a || !ancora) return;
    const el = paginasEstado.get(ancora.numero)?.elemento;
    if (!el) return;
    a.scrollTop = el.offsetTop + ancora.fracao * el.offsetHeight + ancora.folgaPx;
    a.scrollLeft = ancora.fracaoX * a.scrollWidth - a.clientWidth / 2;
  }

  async function definirEscala(nova: number, modo: ModoZoom, atraso: number) {
    modoZoom = modo;
    const alvo = limitar(nova, ZOOM_MIN, ZOOM_MAX);
    if (Math.abs(alvo - escala) < 0.0005) return;
    const ancora = capturarAncora();
    cancelarRenderizacoes();
    escala = alvo;
    await tick();
    restaurarAncora(ancora);
    agendarRedesenho(atraso);
    agendarAtualizacaoPagina();
  }

  async function aplicarModoZoom(modo: "largura" | "altura", atraso: number) {
    modoZoom = modo;
    const fit = calcularFit(modo, paginas);
    if (fit !== null) await definirEscala(fit, modo, atraso);
  }

  function passoZoom(direcao: 1 | -1) {
    if (paginas.length === 0) return;
    const alvo =
      direcao > 0
        ? PASSOS_ZOOM.find((p) => p > escala * FOLGA_PASSO)
        : [...PASSOS_ZOOM].reverse().find((p) => p < escala / FOLGA_PASSO);
    if (alvo !== undefined) void definirEscala(alvo, "manual", 0);
  }

  const numeroCss = (valor: string) => Number.parseFloat(valor) || 0;

  /** Soma as larguras reais dos filhos visiveis (com margens e espacos entre eles). */
  function larguraNatural(pai: Element | null): number {
    if (!pai) return 0;
    const filhos = Array.from(pai.children).filter((f) => getComputedStyle(f).display !== "none");
    if (filhos.length === 0) return 0;
    let total = numeroCss(getComputedStyle(pai).columnGap) * (filhos.length - 1);
    for (const f of filhos) {
      const css = getComputedStyle(f);
      total += f.getBoundingClientRect().width + numeroCss(css.marginLeft) + numeroCss(css.marginRight);
    }
    return total;
  }

  /** Mede a barra com tudo visivel, decide o corte e aplica por atributos (sem passar pelo estado, para nao medir em ciclo). */
  function recalcularBarra() {
    const b = barraEl;
    if (!b || b.getBoundingClientRect().width === 0) return;
    const esq = b.querySelector(".lado-esquerdo");
    const dir = b.querySelector(".lado-direito");
    b.removeAttribute("data-sem-rotulo");
    b.removeAttribute("data-sem-tempo");
    const grupos = Array.from(b.querySelectorAll<HTMLElement>("[data-grupo-pdf]"));
    for (const g of grupos) g.removeAttribute("data-oculto");

    const estilo = getComputedStyle(b);
    const gapBarra = numeroCss(estilo.columnGap);
    const gapDireita = dir ? numeroCss(getComputedStyle(dir).columnGap) : 0;
    const disponivel = b.getBoundingClientRect().width - numeroCss(estilo.paddingLeft) - numeroCss(estilo.paddingRight);

    const esquerdaCheia = larguraNatural(esq);
    b.setAttribute("data-sem-rotulo", "");
    const esquerdaCompacta = larguraNatural(esq);
    const tempoEl = b.querySelector<HTMLElement>(".tempo");
    const tempo = tempoEl ? tempoEl.getBoundingClientRect().width + numeroCss(getComputedStyle(tempoEl).marginRight) + numeroCss(estilo.columnGap) : 0;
    b.removeAttribute("data-sem-rotulo");

    const larguras: Record<GrupoDireito, number> = { pagina: 0, zoom: 0, apresentacao: 0 };
    for (const g of grupos) {
      const id = g.dataset.grupoPdf as GrupoDireito;
      larguras[id] = g.getBoundingClientRect().width + gapDireita;
    }
    const direitaFixa = Math.max(0, larguraNatural(dir) - ORDEM_CORTE_DIREITO.reduce((s, g) => s + larguras[g], 0));

    const plano = planejarBarraPdf({
      disponivel,
      gap: gapBarra,
      esquerdaCheia,
      esquerdaCompacta,
      tempo,
      direitaFixa,
      grupos: larguras,
    });
    if (!plano.rotulos) b.setAttribute("data-sem-rotulo", "");
    if (plano.semTempo) b.setAttribute("data-sem-tempo", "");
    for (const g of grupos) if (plano.ocultos.includes(g.dataset.grupoPdf as GrupoDireito)) g.setAttribute("data-oculto", "");
    apresentacaoNoMenu = plano.ocultos.includes("apresentacao");
  }

  function agendarBarra() {
    cancelAnimationFrame(quadroBarra);
    quadroBarra = requestAnimationFrame(recalcularBarra);
  }

  function aoRedimensionar() {
    const a = areaRolagem;
    if (!a || a.clientWidth === 0 || a.offsetHeight === 0) return;
    if (modoZoom !== "manual") void aplicarModoZoom(modoZoom, 150);
  }

  function escolherModo(modo: "largura" | "altura") {
    fecharMenus(true);
    void aplicarModoZoom(modo, 0);
  }

  function escolherZoom(valor: number) {
    fecharMenus(true);
    void definirEscala(valor, "manual", 0);
  }

  // ---------- SyncTeX: duplo clique e destaque ----------

  function aoDuploClique(ev: MouseEvent) {
    if (!synctexDisponivel || !onirparacodigo) return;
    const no = ev.currentTarget as HTMLElement;
    const numero = Number(no.dataset.pagina);
    const caixa = no.getBoundingClientRect();
    const x = Math.round(((ev.clientX - caixa.left) / escala) * 100) / 100;
    const y = Math.round(((ev.clientY - caixa.top) / escala) * 100) / 100;
    onirparacodigo(numero, x, y);
  }

  $effect(() => {
    const d = destaque;
    const quantas = paginas.length;
    if (!d) {
      ultimoDestaque = null;
      return;
    }
    if (quantas === 0 || d === ultimoDestaque) return;
    ultimoDestaque = d;
    untrack(() => void mostrarDestaque(d));
  });

  async function mostrarDestaque(d: Destaque) {
    clearTimeout(temporizadorDestaque);
    logsPorErro = false;
    vistaLogs = false;
    await tick();
    const a = areaRolagem;
    const est = paginasEstado.get(d.pagina);
    if (!a || !est) return;

    paginaAtual = limitar(d.pagina, 1, totalPaginas);
    const topo = est.elemento.offsetTop + (d.y - d.altura) * escala;
    const esquerda = est.elemento.offsetLeft + d.x * escala;
    const alturaRetangulo = d.altura * escala;
    const larguraRetangulo = d.largura * escala;
    a.scrollTop = Math.max(0, topo - Math.max(0, (a.clientHeight - alturaRetangulo) / 2));
    if (esquerda < a.scrollLeft || esquerda + larguraRetangulo > a.scrollLeft + a.clientWidth) {
      a.scrollLeft = Math.max(0, esquerda - (a.clientWidth - larguraRetangulo) / 2);
    }

    destaqueAtivo = d;
    temporizadorDestaque = setTimeout(() => {
      destaqueAtivo = null;
    }, DURACAO_DESTAQUE_MS);
  }

  // ---------- modo apresentacao ----------

  async function entrarApresentacao() {
    const a = areaRolagem;
    if (!a || paginas.length === 0) return;
    if (document.fullscreenElement === a) {
      await document.exitFullscreen();
      return;
    }
    if (!document.fullscreenEnabled) {
      mostrarAviso(t("pdf.tela-cheia-indisponivel"));
      return;
    }
    logsPorErro = false;
    vistaLogs = false;
    fecharMenus();
    await tick();
    try {
      await a.requestFullscreen();
    } catch (erro) {
      mostrarAviso(t("pdf.tela-cheia-falha"));
    }
  }

  function aoMudarTelaCheia() {
    const nossa = areaRolagem !== null && document.fullscreenElement === areaRolagem;
    if (nossa && !apresentando) {
      antesDaApresentacao = { modo: modoZoom, escala };
      apresentando = true;
      void ajustarParaApresentacao();
    } else if (!nossa && apresentando) {
      apresentando = false;
      void restaurarDepoisDaApresentacao();
    }
  }

  async function ajustarParaApresentacao() {
    const pagina = paginaAtual;
    await tick();
    await aplicarModoZoom("altura", 0);
    await tick();
    irParaPagina(pagina);
  }

  async function restaurarDepoisDaApresentacao() {
    const antes = antesDaApresentacao;
    antesDaApresentacao = null;
    const pagina = paginaAtual;
    await tick();
    if (antes && antes.modo === "manual") await definirEscala(antes.escala, "manual", 0);
    else await aplicarModoZoom(antes?.modo === "altura" ? "altura" : "largura", 0);
    await tick();
    irParaPagina(pagina);
  }

  // ---------- menus ----------

  function fecharMenus(devolverFoco = false) {
    const tinhaCompilar = menuCompilarAberto;
    const tinhaZoom = menuZoomAberto;
    menuCompilarAberto = false;
    menuZoomAberto = false;
    menuSalvarAberto = false;
    if (devolverFoco) {
      if (tinhaCompilar) botaoSeta?.focus();
      else if (tinhaZoom) botaoZoom?.focus();
    }
  }

  async function alternarMenuCompilar() {
    menuZoomAberto = false;
    menuCompilarAberto = !menuCompilarAberto;
    if (menuCompilarAberto) {
      await tick();
      menuCompilar?.querySelector<HTMLElement>(SELETOR_ITEM_MENU)?.focus();
    }
  }

  async function alternarMenuZoom() {
    menuCompilarAberto = false;
    menuZoomAberto = !menuZoomAberto;
    if (menuZoomAberto) {
      await tick();
      menuZoom?.querySelector<HTMLElement>(SELETOR_ITEM_MENU)?.focus();
    }
  }

  function navegarMenu(ev: KeyboardEvent & { currentTarget: HTMLElement }) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(ev.key)) return;
    const itens = Array.from(ev.currentTarget.querySelectorAll<HTMLElement>(SELETOR_ITEM_MENU));
    if (itens.length === 0) return;
    ev.preventDefault();
    const atual = itens.indexOf(document.activeElement as HTMLElement);
    let proximo = 0;
    if (ev.key === "ArrowDown") proximo = atual < 0 ? 0 : (atual + 1) % itens.length;
    else if (ev.key === "ArrowUp") proximo = atual <= 0 ? itens.length - 1 : atual - 1;
    else if (ev.key === "End") proximo = itens.length - 1;
    itens[proximo].focus();
  }

  function aoClicarJanela(ev: MouseEvent) {
    if (menuTexto && !(ev.target as HTMLElement | null)?.closest(".menu-texto")) menuTexto = null;
    if (!menuCompilarAberto && !menuZoomAberto && !menuSalvarAberto) return;
    const alvo = ev.target as HTMLElement | null;
    if (alvo?.closest(".compilar") || alvo?.closest(".zoom") || alvo?.closest(".salvar-como")) return;
    fecharMenus();
  }

  function aoTeclarJanela(ev: KeyboardEvent) {
    // Esc sai do modo apresentacao: no WebView2 a tela cheia do elemento nao sai sozinha pelo Esc
    if (ev.key === "Escape" && apresentando && document.fullscreenElement) {
      ev.preventDefault();
      void document.exitFullscreen();
      return;
    }
    if (ev.key === "Escape" && menuTexto) menuTexto = null;
    if (ev.key === "Escape" && (menuCompilarAberto || menuZoomAberto || menuSalvarAberto)) {
      ev.preventDefault();
      fecharMenus(true);
      return;
    }
    if (!apresentando) return;
    switch (ev.key) {
      case "PageDown":
      case "ArrowDown":
      case "ArrowRight":
        ev.preventDefault();
        irParaPagina(paginaAtual + 1);
        break;
      case "PageUp":
      case "ArrowUp":
      case "ArrowLeft":
        ev.preventDefault();
        irParaPagina(paginaAtual - 1);
        break;
      case " ":
        ev.preventDefault();
        irParaPagina(paginaAtual + (ev.shiftKey ? -1 : 1));
        break;
      case "Home":
        ev.preventDefault();
        irParaPagina(1);
        break;
      case "End":
        ev.preventDefault();
        irParaPagina(totalPaginas);
        break;
    }
  }

  // ---------- logs ----------

  function alternarLogs() {
    logsPorErro = false;
    vistaLogs = !vistaLogs;
    fecharMenus();
  }

  /** Abre a lista de erros e o log (clique no erro da barra de status). */
  export function mostrarLogs() {
    logsPorErro = false;
    vistaLogs = true;
    fecharMenus();
  }

  // O PDF tem prioridade: os logs so abrem sozinhos quando nao ha PDF nenhum para mostrar.
  // Com PDF, os erros ficam no botao de logs (com a contagem), como no Overleaf.
  $effect(() => {
    const agora = compilando;
    if (compilandoAntes && !agora) untrack(() => void aoTerminarCompilacao());
    compilandoAntes = agora;
  });

  async function aoTerminarCompilacao() {
    await tick();
    const temErro = erros.some((e) => e.tipo !== "aviso");
    if (temErro && !dados) {
      vistaLogs = true;
      logsPorErro = true;
    } else if (logsPorErro) {
      vistaLogs = false;
      logsPorErro = false;
    }
  }
</script>

<svelte:window onclick={aoClicarJanela} onkeydown={aoTeclarJanela} />
<svelte:document onfullscreenchange={aoMudarTelaCheia} />

<div class="painel">
  <div
    class="barra barra-principal"
    class:com-menu={menuCompilarAberto || menuZoomAberto || menuSalvarAberto}
    role="toolbar"
    aria-label={t("pdf.barra")}
    bind:this={barraEl}
  >
    <div class="lado-esquerdo">
      <div class="compilar">
        <button
          type="button"
          class="verde btn-compilar"
          title={t("compilar.recompilar-titulo")}
          aria-label={compilando ? t("compilar.compilando") : t("compilar.recompilar")}
          disabled={compilando}
          onclick={() => oncompilar?.()}
        >
          {#if compilando}
            <Girador tamanho={14} />
            <span class="rotulo">{t("compilar.compilando")}</span>
          {:else}
            <Play size={14} />
            <span class="rotulo">{t("compilar.recompilar")}</span>
          {/if}
        </button>
        <button
          type="button"
          class="verde btn-seta"
          bind:this={botaoSeta}
          title={t("compilar.opcoes")}
          aria-label={t("compilar.opcoes")}
          aria-haspopup="menu"
          aria-expanded={menuCompilarAberto}
          onclick={alternarMenuCompilar}
        >
          <ChevronDown size={14} />
        </button>

        {#if menuCompilarAberto}
          <div
            class="menu menu-compilar"
            role="menu"
            tabindex="-1"
            aria-label={t("compilar.opcoes")}
            bind:this={menuCompilar}
            onkeydown={navegarMenu}
          >
            <ItensCompilacao
              {autoCompilar}
              {pararNoPrimeiroErro}
              {compilando}
              oncompilar={() => { fecharMenus(true); oncompilar?.(); }}
              onautocompilar={(ligado) => { fecharMenus(true); onautocompilar?.(ligado); }}
              onpararnoprimeiroerro={(ligado) => { fecharMenus(true); onpararnoprimeiroerro?.(ligado); }}
              onpararcompilacao={() => { fecharMenus(true); onpararcompilacao?.(); }}
              onrecompilardozero={() => { fecharMenus(true); onrecompilardozero?.(); }}
            />
          </div>
        {/if}
      </div>
      {#if !compilando && tempoMs !== null}
        <span class="tempo" title={tf("compilar.tempo-titulo", { tempo: formatarTempo(tempoMs) })}>
          {formatarTempo(tempoMs)}
        </span>
      {/if}


      <button
        type="button"
        class="icone-btn btn-logs"
        class:ativo={vistaLogs}
        title={vistaLogs ? t("log.voltar-pdf") : t("log.abrir")}
        aria-label={vistaLogs ? t("log.voltar-pdf") : t("log.abrir")}
        onclick={alternarLogs}
      >
        {#if vistaLogs}
          <ArrowLeft size={16} />
          <span>{t("log.voltar-pdf")}</span>
        {:else}
          <ScrollText size={16} />
          {#if totalLog > 0}
            <span class="selo" class:aviso={totaisLog.erros === 0}>{seloTexto}</span>
          {/if}
        {/if}
      </button>

      <div class="salvar-como">
        <button
          type="button"
          class="icone-btn btn-baixar"
          title={t("pdf.baixar-titulo")}
          aria-label={t("pdf.baixar")}
          aria-haspopup="menu"
          aria-expanded={menuSalvarAberto}
          onclick={() => { const abrir = !menuSalvarAberto; fecharMenus(); menuSalvarAberto = abrir; }}
        >
          <Download size={16} />
          <span class="rotulo">{t("pdf.baixar")}</span>
        </button>
        {#if menuSalvarAberto}
          <div class="menu menu-salvar" role="menu" aria-label={t("pdf.baixar-menu")}>
            <div class="menu-titulo-salvar">{t("pdf.baixar-menu")}</div>
            {#each FORMATOS_SALVAR as f (f.id)}
              <button
                type="button"
                class="menu-item"
                role="menuitem"
                disabled={f.id === "pdf" && !dados}
                title={f.id === "pdf" && !dados ? t("pdf.baixar-pdf-bloqueado") : ""}
                onclick={() => salvarComo(f.id)}
              >
                <span>{t(f.chave)}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    {#if dados && !vistaLogs}
      <div class="lado-direito" role="group" aria-label={t("pdf.lado-direito")}>
        <button
          type="button"
          class="icone-btn"
          class:ativo-escuro={pdfEscuro}
          title={pdfEscuro ? t("pdf.escuro-desligar") : t("pdf.escuro-titulo")}
          aria-label={t("pdf.escuro")}
          aria-pressed={pdfEscuro}
          onclick={alternarPdfEscuro}
        >
          <Contrast size={16} />
        </button>

        <div class="grupo-pdf" data-grupo-pdf="pagina">
          <input
            class="campo-pagina"
            type="text"
            inputmode="numeric"
            autocomplete="off"
            aria-label={t("pdf.pagina")}
            title={t("pdf.pagina-titulo")}
            disabled={semPaginas}
            value={campoPagina}
            onfocus={aoFocarPagina}
            onblur={aoDesfocarPagina}
            oninput={aoDigitarPagina}
            onkeydown={aoTeclarPagina}
          />
          <span class="total">/ {totalPaginas || "..."}</span>

          <span class="divisoria" aria-hidden="true"></span>
        </div>

        <div class="grupo-pdf" data-grupo-pdf="zoom">
          <button
            type="button"
            class="icone-btn"
            title={t("pdf.zoom-menos")}
            aria-label={t("pdf.zoom-menos")}
            disabled={semPaginas || !podeMenosZoom}
            onclick={() => menosZoom()}
          >
            <ZoomOut size={16} />
          </button>
          <button
            type="button"
            class="icone-btn"
            title={t("pdf.zoom-mais")}
            aria-label={t("pdf.zoom-mais")}
            disabled={semPaginas || !podeMaisZoom}
            onclick={() => maisZoom()}
          >
            <ZoomIn size={16} />
          </button>
        </div>

        <div class="zoom">
          <button
            type="button"
            class="icone-btn btn-zoom"
            bind:this={botaoZoom}
            title={t("pdf.zoom-nivel")}
            aria-label={tf("pdf.zoom-nivel-valor", { valor: percentual })}
            aria-haspopup="menu"
            aria-expanded={menuZoomAberto}
            disabled={semPaginas}
            onclick={alternarMenuZoom}
          >
            <span>{percentual}%</span>
            <ChevronDown size={12} />
          </button>

          {#if menuZoomAberto}
            <div
              class="menu menu-zoom"
              role="menu"
              tabindex="-1"
              aria-label={t("pdf.zoom-nivel")}
              bind:this={menuZoom}
              onkeydown={navegarMenu}
            >
              <button
                type="button"
                class="menu-item"
                role="menuitemradio"
                aria-checked={modoZoom === "largura"}
                onclick={() => escolherModo("largura")}
              >
                <span class="marca">{#if modoZoom === "largura"}<Check size={14} />{/if}</span>
                <span>{t("pdf.zoom-largura")}</span>
              </button>
              <button
                type="button"
                class="menu-item"
                role="menuitemradio"
                aria-checked={modoZoom === "altura"}
                onclick={() => escolherModo("altura")}
              >
                <span class="marca">{#if modoZoom === "altura"}<Check size={14} />{/if}</span>
                <span>{t("pdf.zoom-altura")}</span>
              </button>
              <div class="separador" role="separator"></div>
              {#each ZOOMS_MENU as valor}
                {@const marcado = modoZoom === "manual" && Math.abs(escala - valor) < 0.005}
                <button
                  type="button"
                  class="menu-item"
                  role="menuitemradio"
                  aria-checked={marcado}
                  onclick={() => escolherZoom(valor)}
                >
                  <span class="marca">{#if marcado}<Check size={14} />{/if}</span>
                  <span>{Math.round(valor * 100)}%</span>
                </button>
              {/each}
              {#if apresentacaoNoMenu}
                <div class="separador" role="separator"></div>
                <button
                  type="button"
                  class="menu-item"
                  role="menuitem"
                  disabled={semPaginas}
                  onclick={() => { fecharMenus(); modoApresentacao(); }}
                >
                  <span class="marca"><Presentation size={14} /></span>
                  <span>{t("pdf.apresentacao")}</span>
                </button>
              {/if}
            </div>
          {/if}
        </div>

        <div class="grupo-pdf" data-grupo-pdf="apresentacao">
          <button
            type="button"
            class="icone-btn"
            title={t("pdf.apresentacao")}
            aria-label={t("pdf.apresentacao")}
            disabled={semPaginas}
            onclick={() => modoApresentacao()}
          >
            <Presentation size={16} />
          </button>
        </div>
      </div>
    {/if}
  </div>

  <div class="area">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="rolagem"
      class:apresentando
      bind:this={areaRolagem}
      onscroll={() => { menuTexto = null; agendarAtualizacaoPagina(); }}
      role="document"
      oncontextmenu={abrirMenuTexto}
      onkeydown={teclaPdf}
      tabindex="-1"
      inert={vistaLogs}
    >
      <div class="paginas" class:escuro={pdfEscuro}>
        {#each paginas as p (p.numero)}
          <div
            class="pagina"
            role="img"
            aria-label={tf("pdf.pagina-rotulo", { n: p.numero, total: totalPaginas })}
            data-pagina={p.numero}
            style:width="{Math.round(p.largura * escala)}px"
            style:height="{Math.round(p.altura * escala)}px"
            use:registrarPagina={p.numero}
          >
            {#if destaqueAtivo && destaqueAtivo.pagina === p.numero}
              <div
                class="destaque"
                style:left="{destaqueAtivo.x * escala}px"
                style:top="{(destaqueAtivo.y - destaqueAtivo.altura) * escala}px"
                style:width="{destaqueAtivo.largura * escala}px"
                style:height="{destaqueAtivo.altura * escala}px"
              ></div>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    {#if semPaginas && !vistaLogs}
      <div class="vazio">
        {#if erroCarregamento}
          <p class="erro-texto" role="alert">{erroCarregamento}</p>
        {:else if !dados}
          <FileText size={40} />
          <p>{t("pdf.vazio")}</p>
        {:else}
          <Girador tamanho={24} />
          <p>{t("pdf.carregando")}</p>
        {/if}
      </div>
    {/if}

    {#if !semPaginas && erroCarregamento}
      <div class="faixa faixa-erro" role="alert">{erroCarregamento}</div>
    {:else if aviso}
      <div class="faixa" role="status">{aviso}</div>
    {/if}

    {#if vistaLogs}
      <div class="logs">
        <ListaLogs
          {erros}
          {log}
          {iaAtiva}
          {outrosArquivos}
          {onlimparcache}
          {onabrirarquivo}
          {onirparalinha}
          {onexplicarerro}
        />
      </div>
    {/if}
  </div>
</div>

{#if menuTexto}
  <div class="menu menu-texto" role="menu" tabindex="-1" style:left="{menuTexto.x}px" style:top="{menuTexto.y}px">
    <button type="button" class="menu-item" role="menuitem" disabled title={t("pdf.menu.so-leitura")}><span>{t("pdf.menu.recortar")}</span><kbd>Ctrl+X</kbd></button>
    <button type="button" class="menu-item" role="menuitem" disabled={!menuTexto.comSelecao} onclick={copiarPdf}><span>{t("pdf.menu.copiar")}</span><kbd>Ctrl+C</kbd></button>
    <button type="button" class="menu-item" role="menuitem" disabled title={t("pdf.menu.so-leitura")}><span>{t("pdf.menu.colar")}</span><kbd>Ctrl+V</kbd></button>
    <div class="separador" role="separator"></div>
    <button type="button" class="menu-item" role="menuitem" disabled={!menuTexto.comSelecao} onclick={traduzirPdf}><span>{t("pdf.menu.traduzir")}</span><kbd>Ctrl+Shift+T</kbd></button>
    <div class="separador" role="separator"></div>
    <button type="button" class="menu-item" role="menuitem" onclick={selecionarTudoPdf}><span>{t("pdf.menu.selecionar-tudo")}</span><kbd>Ctrl+A</kbd></button>
  </div>
{/if}

<style>
  .painel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--superficie-2);
    color: var(--texto);
    font-size: 0.8125rem;
  }

  /* ---------- barras ---------- */

  .barra {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    box-sizing: border-box;
    padding: 0 8px;
    border-bottom: 1px solid var(--borda);
  }

  .barra-principal {
    height: 36px;
    background: var(--superficie-0);
    z-index: 6;
  }

  /* Uma barra so, com dois lados, como pdf-preview-hybrid-toolbar.tsx do Overleaf. */
  .lado-esquerdo,
  .lado-direito {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    /* nenhum lado passa por cima do outro: o que nao cabe e cortado (a medida de verdade fica em recalcularBarra) */
    overflow: hidden;
    /* o respiro vertical deixa o selo do botao de logs, que sai 3 px para cima, fora do corte */
    padding-block: 4px;
    margin-block: -4px;
  }

  /* com um menu aberto o corte sai, senao o menu (posicionado abaixo da barra) some */
  .barra-principal.com-menu .lado-esquerdo,
  .barra-principal.com-menu .lado-direito {
    overflow: visible;
  }

  .lado-esquerdo > *,
  .lado-direito > * {
    flex-shrink: 0;
  }

  .lado-direito {
    margin-left: auto;
  }

  .grupo-pdf {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  /* quando falta espaco: sem rotulo em Recompilar e Baixar (fica o icone), sem o tempo, sem os grupos cortados */
  .barra-principal:global([data-sem-rotulo]) .rotulo,
  .barra-principal:global([data-sem-tempo]) .tempo,
  .grupo-pdf:global([data-oculto]) {
    display: none;
  }

  .barra-principal:global([data-sem-rotulo]) .btn-compilar {
    gap: 0;
    padding: 0 8px;
  }

  .icone-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-width: 26px;
    height: 26px;
    padding: 0 5px;
    box-sizing: border-box;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }

  .icone-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--texto);
  }

  .icone-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .icone-btn.ativo {
    background: var(--primaria-suave);
    color: var(--primaria);
    font-weight: 600;
  }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 1px;
  }

  .selo {
    position: absolute;
    top: -3px;
    right: -4px;
    min-width: 15px;
    height: 15px;
    padding: 0 4px;
    box-sizing: border-box;
    border-radius: 8px;
    background: var(--perigo);
    color: var(--superficie-0);
    font-size: 0.625rem;
    font-weight: 700;
    line-height: 15px;
    text-align: center;
  }

  /* Sem erro, so avisos: o contador usa a cor de aviso, como o badge warning do Overleaf. */
  .selo.aviso {
    background: var(--acento);
  }

  .divisoria {
    width: 1px;
    height: 16px;
    margin: 0 4px;
    background: var(--borda);
  }

  /* ---------- botao compilar ---------- */

  .compilar {
    position: relative;
    display: flex;
    align-items: stretch;
    height: 26px;
    margin-right: 4px;
  }

  .tempo {
    margin-right: 4px;
    color: var(--texto-secundario);
    font-size: 12px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .verde {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 26px;
    border: none;
    background: var(--primaria);
    color: #fff;
    font: inherit;
    cursor: pointer;
  }

  .verde:hover:not(:disabled) {
    background: var(--primaria-hover);
  }

  .btn-compilar {
    gap: 6px;
    padding: 0 12px;
    border-radius: 4px 0 0 4px;
    font-weight: 600;
    white-space: nowrap;
  }

  .btn-compilar:disabled {
    opacity: 0.8;
    cursor: progress;
  }

  .btn-seta {
    width: 24px;
    padding: 0;
    border-left: 1px solid var(--primaria-hover);
    border-radius: 0 4px 4px 0;
  }



  /* ---------- menus ---------- */

  .menu {
    position: absolute;
    top: calc(100% + 4px);
    min-width: 220px;
    padding: 4px 0;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 6px;
    box-shadow: 0 8px 24px color-mix(in srgb, var(--texto) 25%, transparent);
    z-index: 30;
  }

  .menu:focus {
    outline: none;
  }

  .menu-compilar {
    left: 0;
  }

  .salvar-como {
    position: relative;
  }

  .menu-texto {
    position: fixed;
    top: auto;
    max-height: calc(100dvh - 16px);
    overflow-y: auto;
    min-width: 210px;
    padding: 6px;
    border-radius: 10px;
    z-index: 70;
  }

  .menu-texto .menu-item {
    justify-content: space-between;
    gap: 16px;
    border-radius: 6px;
  }

  .menu-texto kbd {
    font: inherit;
    font-size: 0.75rem;
    opacity: 0.6;
  }

  .rolagem:focus {
    outline: none;
  }

  /* abre para a direita, por cima do PDF: a esquerda seria cortada pela borda do painel */
  .menu-salvar {
    left: 0;
  }

  .menu-titulo-salvar {
    padding: 6px 12px 4px;
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--texto-secundario, var(--texto));
  }

  .menu-zoom {
    right: 0;
    min-width: 170px;
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  .menu-item:hover:not(:disabled),
  .menu-item:focus-visible {
    background: var(--hover);
    outline: none;
  }

  .menu-item:disabled {
    color: var(--texto-terciario);
    cursor: default;
  }

  .marca {
    display: inline-flex;
    width: 16px;
    flex-shrink: 0;
  }

  .separador {
    height: 1px;
    margin: 4px 0;
    background: var(--borda);
  }

  /* ---------- faixa de pagina e zoom ---------- */

  .campo-pagina {
    width: 36px;
    height: 24px;
    box-sizing: border-box;
    padding: 0 4px;
    border: 1px solid var(--borda);
    border-radius: 4px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    text-align: center;
  }

  .campo-pagina:disabled {
    opacity: 0.45;
  }

  .total {
    color: var(--texto-secundario);
    white-space: nowrap;
  }

  .zoom {
    position: relative;
  }

  .btn-zoom {
    min-width: 62px;
  }

  /* ---------- area do PDF ---------- */

  .area {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .rolagem {
    position: absolute;
    inset: 0;
    overflow-x: auto;
    overflow-y: scroll;
    background: var(--superficie-2);
  }

  .rolagem.apresentando {
    background: var(--fundo);
    scroll-snap-type: y mandatory;
    scrollbar-width: none;
  }

  .paginas {
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-sizing: border-box;
    width: max-content;
    min-width: 100%;
    padding: 12px;
  }

  .paginas.escuro .pagina {
    filter: invert(0.9) hue-rotate(180deg);
  }

  .ativo-escuro {
    color: var(--primaria);
    background: var(--primaria-suave, var(--hover));
  }

  .apresentando .paginas {
    gap: 0;
    padding: 0;
  }

  .pagina {
    position: relative;
    flex: none;
    margin: 0 auto;
    overflow: hidden;
    background: var(--superficie-0);
    box-shadow: 0 1px 4px color-mix(in srgb, var(--texto) 30%, transparent);
    user-select: none;
  }

  .apresentando .pagina {
    box-shadow: none;
    scroll-snap-align: center;
  }

  /* Camada de texto do PDF.js (copiada de pdfjs-dist/web/pdf_viewer.css, versao 4.10) */
  .pagina :global(.textLayer) {
    position: absolute;
    text-align: initial;
    inset: 0;
    overflow: clip;
    opacity: 1;
    line-height: 1;
    text-size-adjust: none;
    forced-color-adjust: none;
    transform-origin: 0 0;
    caret-color: CanvasText;
    z-index: 0;
    user-select: text;
  }
  .pagina :global(.textLayer :is(span, br)) {
    color: transparent;
    position: absolute;
    white-space: pre;
    cursor: text;
    transform-origin: 0% 0%;
  }
  .pagina :global(.textLayer > :not(.markedContent)),
  .pagina :global(.textLayer .markedContent span:not(.markedContent)) {
    z-index: 1;
  }
  .pagina :global(.textLayer span.markedContent) {
    top: 0;
    height: 0;
  }
  .pagina :global(.textLayer ::selection) {
    background: color-mix(in srgb, AccentColor, transparent 70%);
  }
  .pagina :global(.textLayer br::selection) {
    background: transparent;
  }
  .pagina :global(.textLayer .endOfContent) {
    display: block;
    position: absolute;
    inset: 100% 0 0;
    z-index: 0;
    cursor: default;
    user-select: none;
  }
  .pagina :global(.textLayer.selecting .endOfContent) {
    top: 0;
  }

  .pagina :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .destaque {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid var(--primaria);
    border-radius: 2px;
    background: color-mix(in srgb, var(--primaria) 35%, transparent);
    pointer-events: none;
    animation: pdf-destaque 2s ease-in forwards;
  }

  @keyframes pdf-destaque {
    0%,
    70% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }

  .vazio {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    color: var(--texto-secundario);
    text-align: center;
    pointer-events: none;
  }

  .vazio p {
    margin: 0;
  }

  .erro-texto {
    color: var(--perigo);
  }

  .faixa {
    position: absolute;
    left: 50%;
    bottom: 16px;
    z-index: 3;
    max-width: calc(100% - 32px);
    padding: 6px 12px;
    transform: translateX(-50%);
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    box-shadow: 0 4px 12px color-mix(in srgb, var(--texto) 20%, transparent);
  }

  .faixa-erro {
    border-color: var(--perigo);
    color: var(--perigo);
  }

  .logs {
    position: absolute;
    inset: 0;
    z-index: 4;
    background: var(--fundo);
  }
</style>
