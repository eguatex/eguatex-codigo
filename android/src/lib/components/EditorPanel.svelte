<script lang="ts" module>
  import { Compartment, type EditorState as EstadoDoEditor } from "@codemirror/state";

  export type Estilo = 'normal' | 'secao' | 'subsecao' | 'subsubsecao' | 'paragrafo' | 'subparagrafo';

  /** O que uma aba guarda: o EditorState inteiro (texto, historico, selecao, sugestoes) e a rolagem. */
  export interface EstadoGuardado {
    estado: EstadoDoEditor;
    rolagem: number;
  }

  // No escopo do modulo: todos os EditorState (um por aba) usam os mesmos Compartments, entao um
  // estado guardado continua valido se o painel for montado de novo.
  const compHistorico = new Compartment();
  const compAparencia = new Compartment();
  const compAutocompletar = new Compartment();
  const compColchetes = new Compartment();
  const compVisual = new Compartment();
  const compRevisando = new Compartment();
  const compSomenteLeitura = new Compartment();
  const compVerificacao = new Compartment();
  const compLinhas = new Compartment();
</script>

<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { blocoEmLinhaPropria } from "$lib/figura";
  import { ambienteMatematica, ehIdAmbiente } from "$lib/ambientes-matematica";
  import { regioesMatematica } from "$lib/editor/analise-latex";
  import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection, rectangularSelection } from "@codemirror/view";
  import { EditorState, Transaction } from "@codemirror/state";
  import { autocompletarLatex } from "$lib/editor/autocompletar";
  import { frasesDoEditor } from "$lib/editor/frases";
  import { listaDeLinhas, type AmbienteLista } from "$lib/listas";
  import { preferencias } from "$lib/preferencias.svelte";
  import { extensaoMacros } from "$lib/editor/macros";
  import { previaFiguras } from "$lib/editor/referencias";
  import { atalhos } from "$lib/atalhos.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { codigoSpellcheck, decidirOrtografia, fracaoDesconhecida, inglesPeloBabel, palavrasAmostra, pareceOutroIdioma, type EstadoOrtografia } from "$lib/ortografia-idioma";
  import { defaultKeymap, history, historyKeymap, indentWithTab, undo, redo, undoDepth, redoDepth, selectAll, indentMore, indentLess, toggleComment } from "@codemirror/commands";
  import { syntaxHighlighting, defaultHighlightStyle, indentOnInput, bracketMatching, foldGutter, foldKeymap } from "@codemirror/language";
  import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
  import { searchKeymap, highlightSelectionMatches, openSearchPanel } from "@codemirror/search";
  import { latexMode } from "$lib/editor/latex-language";
  import { temaClaro } from "$lib/editor/tema-eguatex";
  import { modoVisual } from "$lib/editor/visual";
  import { revisoes, revisando, campoRevisoes, definirRevisoes, aceitar, rejeitar, type Revisao } from "$lib/editor/revisao";
  import { bloqueioDeEdicao } from "$lib/editor/somente-leitura";
  import { profundidadeDaLista } from "$lib/editor/profundidade-lista";
  import { ortografia, ortografiaLigada, idiomaOrtografia, palavraErradaEm, sugestoesPara, adicionarAoDicionario } from "$lib/editor/ortografia";

  interface Props {
    conteudo?: string;
    onchange?: (texto: string) => void;
    oncursor?: (linha: number, coluna: number) => void;
    onsalvar?: () => void;
    oncompilar?: () => void;
    onestilo?: (estilo: Estilo) => void;
    /** Se ha algo para desfazer e para refazer (trava os botoes da barra). */
    onhistorico?: (h: { desfazer: boolean; refazer: boolean }) => void;
    /** Quantas listas (itemize, enumerate, description) estao abertas no cursor; a barra so mostra o recuo dentro de uma. */
    onlista?: (profundidade: number) => void;
    /** Clique direito, "Traduzir": recebe o trecho selecionado. */
    ontraduzir?: (texto: string) => void;
    /** Clique direito, "Perguntar à IA": recebe o trecho selecionado. */
    onperguntarIA?: (texto: string) => void;
    /** O item do menu so aparece com a IA ligada. */
    iaAtiva?: boolean;
    /** Modo Visual: esconde os comandos e mostra o texto formatado. */
    visual?: boolean;
    /** Modo Revisando: as edicoes viram sugestoes. */
    revisandoAtivo?: boolean;
    /** Modo Visualizando: o texto so pode ser lido. */
    somenteLeitura?: boolean;
    /** Arquivo aberto: o corretor so roda em .tex, .md e .txt (um .bib ficaria todo sublinhado). */
    arquivo?: string;
    /** Numero de sugestoes pendentes no arquivo aberto. */
    onrevisoes?: (quantidade: number) => void;
    /** Se o corretor esta sublinhando este arquivo e, se nao, por que. */
    onortografia?: (estado: EstadoOrtografia) => void;
  }

  const COMANDO_DO_ESTILO: Record<Exclude<Estilo, 'normal'>, string> = {
    secao: 'section',
    subsecao: 'subsection',
    subsubsecao: 'subsubsection',
    paragrafo: 'paragraph',
    subparagrafo: 'subparagraph',
  };
  const ESTILO_DO_COMANDO: Record<string, Estilo> = Object.fromEntries(
    Object.entries(COMANDO_DO_ESTILO).map(([estilo, cmd]) => [cmd, estilo as Estilo])
  );
  const RE_TITULO = /^(\s*)\\(section|subsection|subsubsection|paragraph|subparagraph)(\*?)\{(.*)\}\s*$/;

  let { conteudo = "", onchange, oncursor, onsalvar, oncompilar, onestilo, onhistorico, onlista, ontraduzir, onperguntarIA, iaAtiva = false, visual = false, revisandoAtivo = false, somenteLeitura = false, onrevisoes, onortografia, arquivo = '' }: Props = $props();

  // ---------- idioma do texto (corretor so em portugues) ----------
  let codigoMagico = $state<string | null>(null);
  let fracaoEstrangeira = $state<number | null>(null);
  let fracaoIngles = $state<number | null>(null);
  let babelIngles = $state(false);
  const estadoOrtografia = $derived(decidirOrtografia(preferencias.ortografia, codigoMagico, fracaoEstrangeira, fracaoIngles, babelIngles));
  const idiomaDoCorretor = $derived(estadoOrtografia.ligada && estadoOrtografia.idioma === 'en' ? 'en' : 'pt');
  $effect(() => {
    onortografia?.(estadoOrtografia);
  });

  // trocar a escolha nas Configuracoes refaz a deteccao com o texto de agora
  $effect(() => {
    void preferencias.ortografia;
    untrack(() => {
      if (view) agendarDeteccaoIdioma(view.state.doc.toString());
    });
  });

  let temporizadorIdioma: ReturnType<typeof setTimeout> | undefined;
  let pedidoIdioma = 0;
  function agendarDeteccaoIdioma(texto: string) {
    codigoMagico = codigoSpellcheck(texto);
    babelIngles = inglesPeloBabel(texto);
    clearTimeout(temporizadorIdioma);
    if (codigoMagico !== null || preferencias.ortografia !== 'automatico') {
      fracaoEstrangeira = null;
      fracaoIngles = null;
      return;
    }
    temporizadorIdioma = setTimeout(async () => {
      const meu = ++pedidoIdioma;
      const palavras = palavrasAmostra(texto);
      try {
        const ruins = await invoke<string[]>('cmd_ortografia_verificar', { palavras, idioma: 'pt' });
        const fracao = fracaoDesconhecida(palavras, ruins);
        // texto que nao parece portugues: confere no dicionario ingles
        let ingles: number | null = null;
        if (pareceOutroIdioma(fracao) || babelIngles) {
          const ruinsEn = await invoke<string[]>('cmd_ortografia_verificar', { palavras, idioma: 'en' });
          ingles = fracaoDesconhecida(palavras, ruinsEn);
        }
        if (meu === pedidoIdioma) {
          fracaoEstrangeira = fracao;
          fracaoIngles = ingles;
        }
      } catch {
        if (meu === pedidoIdioma) {
          fracaoEstrangeira = null;
          fracaoIngles = null;
        }
      }
    }, 800);
  }

  // ---------- menu do clique direito ----------
  interface MenuOrtografia {
    palavra: string;
    de: number;
    ate: number;
    sugestoes: string[] | null;
  }
  let menu = $state<{ x: number; y: number; comSelecao: boolean; ortografia: MenuOrtografia | null } | null>(null);

  function abrirMenu(e: MouseEvent) {
    if (!view) return;
    e.preventDefault();
    const { from, to } = view.state.selection.main;
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
    const errada = pos === null ? null : palavraErradaEm(view.state, pos);
    // o menu nao pode sair da janela
    const x = Math.max(8, Math.min(e.clientX, window.innerWidth - 280 - 8));
    const y = Math.max(8, Math.min(e.clientY, window.innerHeight - (errada ? 470 : 250)));
    const orto: MenuOrtografia | null = errada ? { ...errada, sugestoes: null } : null;
    menu = { x: Math.max(4, x), y: Math.max(4, y), comSelecao: from !== to, ortografia: orto };
    if (errada) {
      void sugestoesPara(errada.palavra, idiomaDoCorretor).then((lista) => {
        if (menu?.ortografia?.de === errada.de && menu.ortografia.palavra === errada.palavra) {
          menu.ortografia.sugestoes = lista;
        }
      });
    }
  }

  function trocarPalavra(o: MenuOrtografia, nova: string) {
    menu = null;
    if (!view || view.state.sliceDoc(o.de, o.ate) !== o.palavra) return;
    view.dispatch({ changes: { from: o.de, to: o.ate, insert: nova }, selection: { anchor: o.de + nova.length }, userEvent: "input.ortografia" });
    view.focus();
  }

  function adicionarPalavra(o: MenuOrtografia) {
    menu = null;
    if (!view) return;
    adicionarAoDicionario(view, o.palavra);
    view.focus();
  }

  /** O indice do projeto mudou (arquivo salvo, projeto aberto). O Android ainda nao tem os avisos de referencia. */
  export function indiceAtualizado() {}

  function textoSelecionado(): string {
    if (!view) return "";
    const { from, to } = view.state.selection.main;
    return view.state.sliceDoc(from, to);
  }

  async function copiarSelecao(recortar: boolean) {
    menu = null;
    if (!view) return;
    const texto = textoSelecionado();
    if (!texto) return;
    try {
      await navigator.clipboard.writeText(texto);
    } catch {
      return;
    }
    if (recortar) {
      const { from, to } = view.state.selection.main;
      view.dispatch({ changes: { from, to, insert: "" }, userEvent: "delete.cut" });
    }
    view.focus();
  }

  async function colar() {
    menu = null;
    if (!view) return;
    try {
      const texto = await navigator.clipboard.readText();
      const { from, to } = view.state.selection.main;
      view.dispatch({ changes: { from, to, insert: texto }, selection: { anchor: from + texto.length }, userEvent: "input.paste" });
    } catch {
      // sem permissao para ler a area de transferencia: o Ctrl+V continua funcionando
    }
    view.focus();
  }

  function traduzirSelecao() {
    menu = null;
    const texto = textoSelecionado();
    if (texto.trim()) ontraduzir?.(texto);
  }

  function perguntarSelecao() {
    menu = null;
    const texto = textoSelecionado();
    if (texto.trim()) onperguntarIA?.(texto);
  }

  /** Atalho do menu Editar: abre o tradutor com a selecao atual. */
  export function traduzir(): boolean {
    const texto = textoSelecionado();
    if (!texto.trim()) return false;
    ontraduzir?.(texto);
    return true;
  }

  let container: HTMLDivElement;
  let view: EditorView | undefined;
  let ultimoEstilo: Estilo | null = null;
  let ultimoHistorico = "";

  const ALTURA_LINHA = { compacta: 1.3, normal: 1.5, larga: 1.8 } as const;

  function verificacao() {
    return [
      ortografiaLigada.of(estadoOrtografia.ligada && (!arquivo || /\.(tex|md|txt)$/i.test(arquivo))),
      idiomaOrtografia.of(idiomaDoCorretor),
    ];
  }

  function aparencia() {
    const estilos: Record<string, Record<string, string>> = {
      "&": { fontSize: `${preferencias.tamanhoFonte}px` },
      ".cm-content": { lineHeight: String(ALTURA_LINHA[preferencias.alturaLinha]) },
    };
    if (preferencias.familiaFonte !== "padrao") {
      estilos[".cm-content, .cm-gutters"] = { fontFamily: `"${preferencias.familiaFonte}", monospace` };
    }
    return EditorView.theme(estilos);
  }

  let ultimaLista = -1;

  function avisarLista(estado: EditorState) {
    const antes = estado.doc.sliceString(0, estado.selection.main.head);
    const profundidade = profundidadeDaLista(antes, antes.length);
    if (profundidade === ultimaLista) return;
    ultimaLista = profundidade;
    onlista?.(profundidade);
  }

  /** Numeros de linha e quebra de linhas longas, conforme as preferencias. */
  function linhas() {
    return [
      preferencias.numerosLinha ? lineNumbers() : [],
      preferencias.quebrarLinhas ? EditorView.lineWrapping : [],
    ];
  }

  /** Visualizando: readOnly trava os comandos do CodeMirror, editable tira o cursor e o filtro barra as acoes do EguaTex. */
  function extensaoSomenteLeitura() {
    return [EditorState.readOnly.of(somenteLeitura), EditorView.editable.of(!somenteLeitura), bloqueioDeEdicao];
  }

  /** Tema, visual, revisando e as outras opcoes de agora (um estado guardado pode ter as de antes). */
  function efeitosDeConfiguracao() {
    return [
      compLinhas.reconfigure(linhas()),
      compAparencia.reconfigure(aparencia()),
      compAutocompletar.reconfigure(preferencias.autocompletar ? autocompletarLatex : []),
      compColchetes.reconfigure(preferencias.fecharColchetes ? closeBrackets() : []),
      compVisual.reconfigure(modoVisual(visual)),
      compRevisando.reconfigure(revisando.of(revisandoAtivo)),
      compSomenteLeitura.reconfigure(extensaoSomenteLeitura()),
      compVerificacao.reconfigure(verificacao()),
    ];
  }

  $effect(() => {
    const efeitos = efeitosDeConfiguracao();
    view?.dispatch({ effects: efeitos });
  });

  function estiloDaLinha(texto: string): Estilo {
    const m = RE_TITULO.exec(texto);
    return m ? ESTILO_DO_COMANDO[m[2]] : 'normal';
  }

  const updateListener = EditorView.updateListener.of((update) => {
    // a troca de arquivo (definirConteudo) nao e edicao do usuario: nao agenda salvar
    if (update.docChanged && !update.transactions.some((t) => t.annotation(Transaction.userEvent) === 'eguatex.trocar')) {
      onchange?.(update.state.doc.toString());
    }
    if (update.docChanged) agendarDeteccaoIdioma(update.state.doc.toString());
    if (update.startState.field(campoRevisoes) !== update.state.field(campoRevisoes)) {
      onrevisoes?.(update.state.field(campoRevisoes).length);
    }
    if (update.selectionSet) {
      const pos = update.state.selection.main.head;
      const line = update.state.doc.lineAt(pos);
      oncursor?.(line.number, pos - line.from + 1);
    }
    if (update.selectionSet || update.docChanged) {
      const estilo = estiloDaLinha(update.state.doc.lineAt(update.state.selection.main.head).text);
      if (estilo !== ultimoEstilo) {
        ultimoEstilo = estilo;
        onestilo?.(estilo);
      }
      avisarLista(update.state);
    }
    const historico = { desfazer: undoDepth(update.state) > 0, refazer: redoDepth(update.state) > 0 };
    const chave = `${historico.desfazer}${historico.refazer}`;
    if (chave !== ultimoHistorico) {
      ultimoHistorico = chave;
      onhistorico?.(historico);
    }
  });

  /** As extensoes de todo EditorState do painel (uma por aba). */
  function extensoes() {
    return [
      highlightActiveLineGutter(),
      highlightActiveLine(),
      compHistorico.of(history()),
      foldGutter(),
      drawSelection(),
      rectangularSelection(),
      indentOnInput(),
      bracketMatching(),
      compColchetes.of(preferencias.fecharColchetes ? closeBrackets() : []),
      compAutocompletar.of(preferencias.autocompletar ? autocompletarLatex : []),
      highlightSelectionMatches(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      latexMode,
      frasesDoEditor(),
      temaClaro,
      compAparencia.of(aparencia()),
      revisoes,
      compVisual.of(modoVisual(visual)),
      compRevisando.of(revisando.of(revisandoAtivo)),
      compSomenteLeitura.of(extensaoSomenteLeitura()),
      compVerificacao.of(verificacao()),
      ortografia,
      previaFiguras,
      keymap.of([
        { key: "Mod-s", preventDefault: true, run: () => { onsalvar?.(); return true; } },
        { key: "Mod-Enter", preventDefault: true, run: () => { oncompilar?.(); return true; } },
        { key: "Mod-b", preventDefault: true, run: () => { envolver("\\textbf{", "}"); return true; } },
        { key: "Mod-i", preventDefault: true, run: () => { envolver("\\textit{", "}"); return true; } },
        { key: "Mod-Shift-t", preventDefault: true, run: () => traduzir() },
        { key: "Mod-h", preventDefault: true, run: () => { buscar(); return true; } },
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
        ...foldKeymap,
        ...closeBracketsKeymap,
        indentWithTab,
      ]),
      extensaoMacros(() => atalhos.usuario.macros),
      compLinhas.of(linhas()),
      updateListener,
    ];
  }

  /** Um EditorState novo, para um arquivo que vai ganhar aba: historico zerado e as opcoes de agora. */
  export function criarEstado(texto: string): EditorState {
    return EditorState.create({ doc: texto, extensions: extensoes() });
  }

  onMount(() => {
    view = new EditorView({
      state: criarEstado(conteudo),
      parent: container,
    });
    agendarDeteccaoIdioma(conteudo);
  });

  onDestroy(() => {
    view?.destroy();
  });

  // Celular deitado com teclado aberto: a janela encolhe (adjustResize) e o cursor pode ficar
  // atras do teclado. No evento resize o editor rola ate a linha do cursor.
  let timerResize: ReturnType<typeof setTimeout> | undefined;
  function aoRedimensionarJanela() {
    clearTimeout(timerResize);
    timerResize = setTimeout(() => {
      if (!view || window.innerHeight >= 500 || !view.hasFocus) return;
      view.dispatch({ effects: EditorView.scrollIntoView(view.state.selection.main.head, { y: "center" }) });
    }, 120);
  }
  // O teclado encolhe a janela com o menu aberto: sobe o menu para ele continuar visivel.
  function reposicionarMenu() {
    if (!menu) return;
    const y = Math.max(8, Math.min(menu.y, window.innerHeight - 250));
    if (y !== menu.y) menu = { ...menu, y };
  }
  onMount(() => {
    window.addEventListener("resize", reposicionarMenu);
    window.visualViewport?.addEventListener("resize", reposicionarMenu);
    window.addEventListener("resize", aoRedimensionarJanela);
    // reserva: alguns WebViews so avisam pelo visualViewport
    window.visualViewport?.addEventListener("resize", aoRedimensionarJanela);
    return () => {
      clearTimeout(timerResize);
      window.removeEventListener("resize", reposicionarMenu);
      window.visualViewport?.removeEventListener("resize", reposicionarMenu);
      window.removeEventListener("resize", aoRedimensionarJanela);
      window.visualViewport?.removeEventListener("resize", aoRedimensionarJanela);
    };
  });

  /** Guarda o que a aba tem agora: o estado inteiro do editor e a posicao da rolagem. */
  export function exportarEstado(): EstadoGuardado | null {
    return view ? { estado: view.state, rolagem: view.scrollDOM.scrollTop } : null;
  }

  /**
   * Mostra o estado de outra aba. Texto, Ctrl+Z, selecao e sugestoes voltam como estavam; as opcoes
   * (tema, visual, revisando) sao as de agora. O setState nao chama o updateListener, entao o painel
   * avisa a pagina do cursor, do estilo, do historico e das sugestoes aqui.
   */
  export function importarEstado(guardado: EstadoGuardado) {
    if (!view) return;
    const estado = guardado.estado.update({ effects: efeitosDeConfiguracao(), annotations: Transaction.addToHistory.of(false) }).state;
    view.setState(estado);
    view.scrollDOM.scrollTop = guardado.rolagem;
    // a rolagem so fica no lugar depois que o CodeMirror mede o texto novo
    requestAnimationFrame(() => {
      if (view && view.state === estado) view.scrollDOM.scrollTop = guardado.rolagem;
    });
    avisarEstado(estado);
    indiceAtualizado();
    agendarDeteccaoIdioma(estado.doc.toString());
  }

  function avisarEstado(estado: EditorState) {
    const pos = estado.selection.main.head;
    const linhaDoCursor = estado.doc.lineAt(pos);
    oncursor?.(linhaDoCursor.number, pos - linhaDoCursor.from + 1);
    ultimoEstilo = estiloDaLinha(linhaDoCursor.text);
    onestilo?.(ultimoEstilo);
    avisarLista(estado);
    const historico = { desfazer: undoDepth(estado) > 0, refazer: redoDepth(estado) > 0 };
    ultimoHistorico = `${historico.desfazer}${historico.refazer}`;
    onhistorico?.(historico);
    onrevisoes?.(estado.field(campoRevisoes).length);
  }

  /** Troca o texto do editor (outro arquivo). O Ctrl+Z nao volta ao arquivo anterior:
   *  o historico e zerado tirando e recolocando a extensao history(). */
  export function definirConteudo(texto: string) {
    if (view) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: texto },
        effects: [definirRevisoes.of([]), compHistorico.reconfigure([])],
        annotations: [Transaction.addToHistory.of(false), Transaction.userEvent.of('eguatex.trocar')],
      });
      view.dispatch({ effects: compHistorico.reconfigure(history()) });
    }
  }

  /** Sugestoes do arquivo aberto (para guardar ao trocar de arquivo). */
  export function obterRevisoes(): Revisao[] {
    return view ? view.state.field(campoRevisoes) : [];
  }

  /** Mostra as sugestoes guardadas; as que nao cabem mais no texto ficam de fora. */
  export function carregarRevisoes(lista: Revisao[]) {
    if (!view) return;
    const n = view.state.doc.length;
    const validas = lista.filter((r) => r.de >= 0 && r.ate <= n && r.de <= r.ate);
    // fora do historico: o invertedEffects de revisao.ts faria desta carga um passo de "Desfazer" ao abrir o arquivo
    view.dispatch({ effects: definirRevisoes.of(validas), annotations: Transaction.addToHistory.of(false) });
    onrevisoes?.(validas.length);
  }

  export function aceitarTodas() {
    if (view) aceitar(view);
  }

  export function rejeitarTodas() {
    if (view) rejeitar(view);
  }

  export function obterConteudo(): string {
    return view?.state.doc.toString() ?? "";
  }

  // o texto selecionado entra no primeiro {} do modelo, e o cursor fica ali dentro
  export function inserir(modelo: string) {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const selecionado = view.state.sliceDoc(from, to);
    const marca = modelo.indexOf("{}");
    let texto = modelo;
    let cursor = modelo.length;
    if (marca >= 0) {
      texto = modelo.slice(0, marca + 1) + selecionado + modelo.slice(marca + 1);
      cursor = marca + 1 + selecionado.length;
    }
    view.dispatch({ changes: { from, to, insert: texto }, selection: { anchor: from + cursor }, userEvent: "input.ferramenta" });
    view.focus();
  }

  /** Insere o texto no cursor, tal como esta (sem o {} do modelo), e deixa o cursor no fim. */
  export function inserirTexto(texto: string) {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    view.dispatch({ changes: { from, to, insert: texto }, selection: { anchor: from + texto.length }, scrollIntoView: true, userEvent: "input.ferramenta" });
    view.focus();
  }

  /** Insere um bloco (figura) em linha propria: quebra antes e depois se houver texto ao redor do cursor. */
  export function inserirBloco(bloco: string) {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const antes = view.state.sliceDoc(view.state.doc.lineAt(from).from, from);
    const depois = view.state.sliceDoc(to, view.state.doc.lineAt(to).to);
    inserirTexto(blocoEmLinhaPropria(bloco, antes, depois));
  }

  /** Inserir > Matematica: equation, align, gather, cases, matrizes, integral e fracao. A selecao entra no primeiro campo. */
  export function inserirAmbiente(id: string) {
    if (!view || !ehIdAmbiente(id)) return;
    const { from, to } = view.state.selection.main;
    const estado = view.state;
    const emMatematica = regioesMatematica(estado.doc.toString()).some((r) => r.de < from && from < r.ate);
    const { texto, cursor } = ambienteMatematica(id, {
      selecao: estado.sliceDoc(from, to),
      emMatematica,
      antesNaLinha: estado.sliceDoc(estado.doc.lineAt(from).from, from),
      depoisNaLinha: estado.sliceDoc(to, estado.doc.lineAt(to).to),
    });
    view.dispatch({ changes: { from, to, insert: texto }, selection: { anchor: from + cursor }, scrollIntoView: true, userEvent: "input.ferramenta" });
    view.focus();
  }

  /** Agente de IA: devolve o trecho selecionado e as linhas onde ele esta. */
  export function obterSelecao(): { texto: string; linhaInicio: number; linhaFim: number } | null {
    if (!view) return null;
    const { from, to } = view.state.selection.main;
    if (from === to) return null;
    return {
      texto: view.state.sliceDoc(from, to),
      linhaInicio: view.state.doc.lineAt(from).number,
      linhaFim: view.state.doc.lineAt(to).number,
    };
  }

  /** Traducao: posicao e texto da selecao, para trocar depois sem depender de busca. */
  export function selecaoIntervalo(): { from: number; to: number; texto: string } | null {
    if (!view) return null;
    const { from, to } = view.state.selection.main;
    if (from === to) return null;
    return { from, to, texto: view.state.sliceDoc(from, to) };
  }

  /** Troca o intervalo se ele ainda tiver o texto esperado (a pessoa pode ter editado enquanto esperava). */
  export function trocarIntervalo(from: number, to: number, esperado: string, novo: string): boolean {
    if (!view || to > view.state.doc.length || view.state.sliceDoc(from, to) !== esperado) return false;
    view.dispatch({
      changes: { from, to, insert: novo },
      selection: { anchor: from, head: from + novo.length },
      scrollIntoView: true,
      userEvent: "input.traducao",
    });
    view.focus();
    return true;
  }

  /** Agente de IA: troca um trecho que aparece uma unica vez, numa transacao que o Ctrl+Z desfaz. */
  export function substituirTrecho(original: string, novo: string): boolean {
    if (!view) return false;
    const texto = view.state.doc.toString();
    const pos = texto.indexOf(original);
    if (pos < 0 || texto.indexOf(original, pos + 1) >= 0) return false;
    view.dispatch({
      changes: { from: pos, to: pos + original.length, insert: novo },
      selection: { anchor: pos + novo.length },
      scrollIntoView: true,
      userEvent: "input.ia",
    });
    return true;
  }

  /** Agente de IA: insere texto antes da linha indicada (ou no fim, se a linha passar do total). */
  export function inserirNaLinha(numero: number, conteudo: string): boolean {
    if (!view) return false;
    const doc = view.state.doc;
    const texto = conteudo.endsWith("\n") ? conteudo : conteudo + "\n";
    const from = numero > doc.lines ? doc.length : doc.line(Math.max(1, numero)).from;
    const insert = numero > doc.lines && !doc.toString().endsWith("\n") ? "\n" + texto : texto;
    view.dispatch({ changes: { from, insert }, scrollIntoView: true, userEvent: "input.ia" });
    return true;
  }

  export function irParaLinha(numero: number) {
    if (!view) return;
    const linha = view.state.doc.line(Math.max(1, Math.min(numero, view.state.doc.lines)));
    view.dispatch({ selection: { anchor: linha.from }, scrollIntoView: true });
    view.focus();
  }

  export function desfazer() {
    if (view) undo(view);
  }

  export function refazer() {
    if (view) redo(view);
  }

  export function buscar() {
    if (view) openSearchPanel(view);
  }

  // envolve a selecao; sem selecao, o cursor fica entre "antes" e "depois"
  export function envolver(antes: string, depois: string) {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const selecionado = view.state.sliceDoc(from, to);
    const inicio = from + antes.length;
    view.dispatch({
      changes: { from, to, insert: antes + selecionado + depois },
      selection: { anchor: inicio, head: inicio + selecionado.length },
      userEvent: "input.ferramenta",
    });
    view.focus();
  }

  /** Lista com marcadores ou numerada: um \item por linha selecionada. Sem seleção, insere o modelo. */
  export function inserirLista(ambiente: AmbienteLista) {
    if (!view) return;
    let { from } = view.state.selection.main;
    const { to } = view.state.selection.main;
    // seleção que começa depois do recuo da linha: leva o recuo junto, para o bloco ficar alinhado
    const linha = view.state.doc.lineAt(from);
    if (from < to && /^[ \t]*$/.test(view.state.sliceDoc(linha.from, from))) from = linha.from;
    const { texto, cursor } = listaDeLinhas(view.state.sliceDoc(from, to), ambiente);
    view.dispatch({ changes: { from, to, insert: texto }, selection: { anchor: from + cursor }, scrollIntoView: true, userEvent: "input.ferramenta" });
    view.focus();
  }

  /** Comenta ou descomenta as linhas da seleção com "%" (o mesmo que Ctrl+/). */
  export function comentar() {
    if (!view) return;
    toggleComment(view);
    view.focus();
  }

  export function selecionarTudo() {
    if (!view) return;
    selectAll(view);
    view.focus();
  }

  export function aumentarRecuo() {
    if (!view) return;
    indentMore(view);
    view.focus();
  }

  export function diminuirRecuo() {
    if (!view) return;
    indentLess(view);
    view.focus();
  }

  export function definirEstilo(estilo: Estilo) {
    if (!view) return;
    const linha = view.state.doc.lineAt(view.state.selection.main.head);
    const m = RE_TITULO.exec(linha.text);
    const recuo = m ? m[1] : (/^\s*/.exec(linha.text)?.[0] ?? '');
    const titulo = m ? m[4] : linha.text.trim();
    const estrela = m ? m[3] : '';
    const novo = estilo === 'normal'
      ? recuo + titulo
      : `${recuo}\\${COMANDO_DO_ESTILO[estilo]}${estrela}{${titulo}}`;
    if (novo === linha.text) {
      view.focus();
      return;
    }
    const cursor = estilo === 'normal' ? linha.from + novo.length : linha.from + novo.length - 1;
    view.dispatch({ changes: { from: linha.from, to: linha.to, insert: novo }, selection: { anchor: cursor }, userEvent: "input.ferramenta" });
    view.focus();
  }
</script>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => { if (e.key === "Escape") menu = null; }} onblur={() => (menu = null)} />

<div class="editor-container" bind:this={container} oncontextmenu={abrirMenu} role="presentation"></div>

{#if menu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="menu-contexto" role="menu" tabindex="-1" style:left="{menu.x}px" style:top="{menu.y}px" onclick={(e) => e.stopPropagation()}>
    {#if menu.ortografia}
      {@const o = menu.ortografia}
      {#if o.sugestoes === null}
        <div class="nota">Buscando sugestões...</div>
      {:else if o.sugestoes.length === 0}
        <div class="nota">Sem sugestões para "{o.palavra}"</div>
      {:else}
        {#each o.sugestoes as s (s)}
          <button role="menuitem" class="sugestao" onclick={() => trocarPalavra(o, s)}><span>{s}</span></button>
        {/each}
      {/if}
      <button role="menuitem" onclick={() => adicionarPalavra(o)}><span>Adicionar "{o.palavra}" ao dicionário</span></button>
      <div class="sep"></div>
    {/if}
    <button role="menuitem" disabled={!menu.comSelecao} onclick={() => copiarSelecao(true)}><span>Recortar</span><kbd>Ctrl+X</kbd></button>
    <button role="menuitem" disabled={!menu.comSelecao} onclick={() => copiarSelecao(false)}><span>Copiar</span><kbd>Ctrl+C</kbd></button>
    <button role="menuitem" onclick={colar}><span>Colar</span><kbd>Ctrl+V</kbd></button>
    <div class="sep"></div>
    <button role="menuitem" disabled={!menu.comSelecao} onclick={traduzirSelecao}><span>Traduzir</span><kbd>Ctrl+Shift+T</kbd></button>
    {#if iaAtiva}
      <button role="menuitem" disabled={!menu.comSelecao} onclick={perguntarSelecao}><span>Perguntar à IA</span></button>
    {/if}
    <div class="sep"></div>
    <button role="menuitem" onclick={() => { menu = null; selecionarTudo(); }}><span>Selecionar tudo</span><kbd>Ctrl+A</kbd></button>
  </div>
{/if}

<style>
  .editor-container {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .editor-container :global(.cm-editor) {
    flex: 1;
    outline: none;
  }

  .editor-container :global(.cm-scroller) {
    overflow: auto;
  }

  .menu-contexto {
    position: fixed;
    z-index: 70;
    max-height: calc(100dvh - 16px);
    overflow-y: auto;
    min-width: 210px;
    padding: 6px;
    border-radius: 10px;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.25);
  }

  .menu-contexto button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 0.875rem;
    text-align: left;
    cursor: pointer;
  }

  .menu-contexto button:hover:not(:disabled) {
    background: var(--superficie-2);
  }

  .menu-contexto button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .menu-contexto kbd {
    font: inherit;
    font-size: 0.75rem;
    opacity: 0.6;
  }

  .menu-contexto .sugestao {
    font-weight: 600;
  }

  .menu-contexto .nota {
    padding: 7px 10px;
    font-size: 0.8125rem;
    opacity: 0.7;
  }

  .menu-contexto .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--borda);
  }
</style>
