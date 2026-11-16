<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { acessoPerdido, textoErro } from "$lib/drive-erro";
  import { t, tf } from "$lib/i18n";
  import { IDS_AMBIENTES } from "$lib/ambientes-matematica";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { tick, untrack, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { definirProjeto, definirArquivoAberto, atualizarArquivo, manterSomente, arquivoIndexado, esquecerArquivo } from "$lib/editor/projeto";
  import Layout from "$lib/components/Layout.svelte";
  import Abertura from "$lib/components/Abertura.svelte";
  import EditorPanel, { type EstadoGuardado } from "$lib/components/EditorPanel.svelte";
  import TraducaoPainel from "$lib/components/TraducaoPainel.svelte";
  import CompartilharModal from "$lib/components/CompartilharModal.svelte";
  import AbrirDoDriveModal from "$lib/components/AbrirDoDriveModal.svelte";
  import { blocoFigura, caminhoDeFigura, EXTENSOES_FIGURA } from "$lib/figura";
  import { naFila, esperar } from "$lib/fila-gravacao";
  import { entrarComGoogle } from "$lib/conta-google.svelte";
  import IconeDrive from "$lib/components/IconeDrive.svelte";
  import type { Acoes } from "$lib/comandos";
  import type { Revisao } from "$lib/editor/revisao";
  import { preferencias } from "$lib/preferencias.svelte";
  import { comTarefa, escutarProgressoRust, iniciarTarefa } from "$lib/tarefas.svelte";
  import type { EstadoOrtografia } from "$lib/ortografia-idioma";
  import { faltandoNosErros, nomeDoFaltando } from "$lib/faltando";
  import { chavePrincipalDoProjeto, temBeginDocument } from "$lib/principal";
  import {
    abasEstado,
    aplicarEstadoAbas,
    abasParaSalvar,
    fotoDasAbas,
    abrir as abrirNaLista,
    fechar as fecharNaLista,
    mover as moverNaLista,
    proxima as proximaDaLista,
    anterior as anteriorDaLista,
    renomearPrefixo,
    removerPrefixo,
    estadoVazio,
    lerAbasSalvas,
    gravarAbasSalvas,
    sujas,
    dadosDasAbas,
    marcarSuja,
    estaSuja,
    renomearDados,
    removerDados,
    limparDadosDasAbas,
    dentroDe,
    type Aba,
    type EstadoAbas,
  } from "$lib/abas.svelte";
  import { ehImagem } from "$lib/imagem";
  import VisualizadorImagem from "$lib/components/VisualizadorImagem.svelte";
  import { campoRevisoes } from "$lib/editor/revisao";
  import ConfiguracoesModal from "$lib/components/ConfiguracoesModal.svelte";
  import ContagemPalavrasModal from "$lib/components/ContagemPalavrasModal.svelte";
  import VersoesModal from "$lib/components/VersoesModal.svelte";
  import AssistenteTabela from "$lib/components/AssistenteTabela.svelte";
  import type { FileNode } from "$lib/components/FileTree.svelte";
  import { modelos, nomeDePasta, type Modelo } from "$lib/modelos";
  import Plus from "@lucide/svelte/icons/plus";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import FileArchive from "@lucide/svelte/icons/archive";
  import Folder from "@lucide/svelte/icons/folder";
  import Search from "@lucide/svelte/icons/search";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Info from "@lucide/svelte/icons/info";
  import X from "@lucide/svelte/icons/x";
  import Mail from "@lucide/svelte/icons/mail";
  import BotaoTema from "$lib/components/BotaoTema.svelte";
  import ContaUsuario from "$lib/components/ContaUsuario.svelte";
  import CartoesApoio from "$lib/components/CartoesApoio.svelte";
  import PixModal from "$lib/components/PixModal.svelte";
  import FeedbackModal from "$lib/components/FeedbackModal.svelte";
  import { reenviarPendentes } from "$lib/feedback-envio";
  import { enviarFeedback } from "$lib/ajuda";
  import type { ModoEdicao } from "$lib/modo-edicao";
  import { SITE, EMAIL_CONTATO, MAILTO_CONTATO, SITE_INSTALAR_TEX } from "$lib/site";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import { localCurto, ordenarPorData, pastaDoProjeto, tempoRelativo, textoMostrando, type Ordem } from "$lib/lista-projetos";
  import Archive from "@lucide/svelte/icons/archive";
  import ArchiveRestore from "@lucide/svelte/icons/archive-restore";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Copy from "@lucide/svelte/icons/copy";
  import type { ProjetoAgente, NoArquivo } from "$lib/agente";
  import Download from "@lucide/svelte/icons/download";
  import CloudUpload from "@lucide/svelte/icons/cloud-upload";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";

  type Modo = 'inicio' | 'editor';

  interface ConfigIAData {
    ativa: boolean;
    provedor: string;
    modelo: string;
    nivel?: string;
  }

  type EstadoProjeto = 'ativo' | 'arquivado' | 'lixeira';
  type Filtro = 'todos' | 'arquivados' | 'lixeira';

  interface Recente {
    nome: string;
    caminho: string;
    abertoEm: string;
    estado?: EstadoProjeto;
  }

  const FILTROS: Array<{ id: Filtro; nome: string; titulo: string; estado: EstadoProjeto; icone: typeof Folder }> = [
    { id: 'todos', nome: 'Todos os projetos', titulo: 'Todos os projetos', estado: 'ativo', icone: Folder },
    { id: 'arquivados', nome: 'Arquivados', titulo: 'Projetos arquivados', estado: 'arquivado', icone: Archive },
    { id: 'lixeira', nome: 'Lixeira', titulo: 'Projetos na lixeira', estado: 'lixeira', icone: Trash2 },
  ];

  const CHAVE_RECENTES = 'eguatex.recentes';
  const CHAVE_AVISO = 'eguatex.aviso-fechado';

  let modo: Modo = $state('inicio');
  let pixInicioVisivel = $state(false);
  let versaoRust = $state("");
  let semTex = $state(false);
  let compilando = $state(false);
  let statusCompilacao: 'idle' | 'compilando' | 'sucesso' | 'parcial' | 'erro' = $state('idle');
  // arquivoAtual vem da aba ativa e so existe se ela for de texto: com uma imagem ativa fica vazio,
  // e entao nada grava texto (salvar, salvamento automatico, agente) sobre a imagem.
  const abaAtiva = $derived(abasEstado.abas.find((a) => a.caminho === abasEstado.ativa));
  const arquivoAtual = $derived(abaAtiva?.tipo === 'texto' ? abaAtiva.caminho : '');
  let linha = $state(1);
  let coluna = $state(1);
  let logTexto = $state('');
  // duracao da ultima compilacao em ms (null: ainda nao compilou neste projeto)
  let tempoMs = $state<number | null>(null);
  let erros: Array<{linha?: number | null; arquivo?: string | null; mensagem: string; tipo: string}> = $state([]);
  let pdfDados: Uint8Array | null = $state(null);
  let caminhoProjeto = $state('');
  let conteudoEditor = $state('');
  let nomeProjeto = $state('');
  let arquivosConteudo: Record<string, string> = $state({});

  // ---------- modos do editor: Codigo/Visual e Editando/Revisando ----------
  function lerModoVisual(): boolean {
    try {
      return localStorage.getItem('eguatex.modo-visual') === '1';
    } catch {
      return false;
    }
  }
  let visualAtivo = $state(lerModoVisual());
  let modoEdicao = $state<ModoEdicao>('editando');
  let qtdSugestoes = $state(0);
  // sugestoes de cada arquivo, guardadas em .eguatex/revisoes.json
  let revisoesPorArquivo: Record<string, Revisao[]> = {};
  let revisoesGravadas = '{}';
  const ARQUIVO_REVISOES = '.eguatex/revisoes.json';

  function mudarVisual(ligado: boolean) {
    visualAtivo = ligado;
    try {
      localStorage.setItem('eguatex.modo-visual', ligado ? '1' : '0');
    } catch {}
  }

  async function carregarRevisoesDoProjeto(caminho: string) {
    revisoesPorArquivo = {};
    revisoesGravadas = '{}';
    try {
      const texto = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto: caminho, caminhoRelativo: ARQUIVO_REVISOES });
      revisoesPorArquivo = JSON.parse(texto) ?? {};
      revisoesGravadas = JSON.stringify(revisoesPorArquivo);
    } catch {
      // projeto sem sugestoes
    }
  }

  async function gravarRevisoes() {
    if (!caminhoProjeto) return;
    // as sugestoes vivem no EditorState de cada aba; o objeto abaixo so serve para gravar em disco
    for (const [caminho, dados] of dadosDasAbas) revisoesPorArquivo[caminho] = dados.estado.field(campoRevisoes);
    if (arquivoAtual && editorRef) revisoesPorArquivo[arquivoAtual] = editorRef.obterRevisoes();
    for (const k of Object.keys(revisoesPorArquivo)) if (!revisoesPorArquivo[k]?.length) delete revisoesPorArquivo[k];
    const json = JSON.stringify(revisoesPorArquivo);
    if (json === revisoesGravadas) return;
    try {
      await invoke('cmd_salvar_arquivo', { caminhoProjeto, caminhoRelativo: ARQUIVO_REVISOES, conteudo: json });
      revisoesGravadas = json;
    } catch (e) {
      console.error('Erro ao gravar as sugestões:', e);
    }
  }
  let arquivosProjeto: FileNode[] = $state([]);
  let principalDetectado = $state('');
  /** Motor que rodou na ultima compilacao do projeto ("XeLaTeX"), para a barra de status; vazio antes dela. */
  let motorDetectado = $state('');

  let editorRef: EditorPanel | undefined = $state();
  /** Listas abertas no cursor (para o botao de recuo da barra). */
  let profundidadeLista = $state(0);
  // As acoes de edicao usam este, que fica vazio sem aba de texto ativa.
  const editorAtivo = $derived(arquivoAtual ? editorRef : undefined);

  let configIA: ConfigIAData = $state({
    ativa: false,
    provedor: '',
    modelo: '',
  });
  let layoutRef: ReturnType<typeof Layout> | undefined = $state();

  let pastaProjetos = $state('');
  let recentes: Recente[] = $state(lerRecentes());
  let busca = $state('');
  let menuNovoAberto = $state(false);
  let avisoVisivel = $state(lerAvisoVisivel());
  let erroInicio = $state('');

  let modeloEscolhido: Modelo | null = $state(null);
  let nomeNovo = $state('');
  let criando = $state(false);

  let filtro = $state<Filtro>('todos');
  let selecionados = $state<string[]>([]);
  const filtroAtual = $derived(FILTROS.find((f) => f.id === filtro) ?? FILTROS[0]);

  // Lista como a do Overleaf: ordenada pela ultima modificacao (cabecalho clicavel), com o tempo relativo.
  let ordemData = $state<Ordem>('desc');
  /** Ultima modificacao de cada projeto (ms), lida do disco pelo Rust. */
  let modificacoes = $state<Record<string, number>>({});
  /** Relogio da coluna "Última modificação"; anda a cada 30 s enquanto a tela inicial aparece. */
  let agoraLista = $state(Date.now());

  function dataDoProjeto(r: Recente): number {
    return modificacoes[r.caminho] ?? Date.parse(r.abertoEm);
  }

  let recentesFiltrados = $derived(
    ordenarPorData(
      recentes.filter(
        (r) => (r.estado ?? 'ativo') === filtroAtual.estado && r.nome.toLowerCase().includes(busca.trim().toLowerCase())
      ),
      dataDoProjeto,
      ordemData,
    )
  );

  $effect(() => {
    if (modo !== 'inicio') return;
    const caminhos = recentes.map((r) => r.caminho);
    agoraLista = Date.now();
    invoke<Array<number | null>>('cmd_modificacao_projetos', { caminhos })
      .then((datas) => {
        const novas: Record<string, number> = {};
        caminhos.forEach((c, i) => {
          const d = datas[i];
          if (typeof d === 'number') novas[c] = d;
        });
        modificacoes = novas;
      })
      .catch(() => {});
    const relogio = setInterval(() => (agoraLista = Date.now()), 30_000);
    return () => clearInterval(relogio);
  });
  const todosSelecionados = $derived(
    recentesFiltrados.length > 0 && recentesFiltrados.every((r) => selecionados.includes(r.caminho))
  );

  function alternarSelecao(caminho: string) {
    selecionados = selecionados.includes(caminho)
      ? selecionados.filter((c) => c !== caminho)
      : [...selecionados, caminho];
  }

  function alternarTodos() {
    selecionados = todosSelecionados ? [] : recentesFiltrados.map((r) => r.caminho);
  }

  function mudarEstado(caminhos: string[], estado: EstadoProjeto) {
    recentes = recentes.map((r) => (caminhos.includes(r.caminho) ? { ...r, estado } : r));
    selecionados = selecionados.filter((c) => !caminhos.includes(c));
    gravarRecentes();
  }

  function tirarDaLista(caminhos: string[]) {
    recentes = recentes.filter((r) => !caminhos.includes(r.caminho));
    selecionados = selecionados.filter((c) => !caminhos.includes(c));
    gravarRecentes();
  }

  async function copiarProjeto(projeto: Recente) {
    try {
      const novo: string = await invoke("cmd_copiar_projeto", { caminho: projeto.caminho });
      const nome = novo.split(/[\\/]/).pop() || novo;
      recentes = [{ nome, caminho: novo, abertoEm: new Date().toISOString(), estado: 'ativo' }, ...recentes];
      gravarRecentes();
    } catch (e) {
      erroInicio = `Erro ao copiar o projeto: ${e}`;
    }
  }

  async function baixarZip(caminhos: string[]) {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      for (const caminho of caminhos) {
        const projeto = recentes.find((r) => r.caminho === caminho);
        if (!projeto) {
          erroInicio = t('exportar.zip-fora-recentes');
          continue;
        }
        const destino = await save({
          title: t('exportar.zip-titulo'),
          defaultPath: `${projeto.nome}.zip`,
          filters: [{ name: t('exportar.zip-filtro'), extensions: ["zip"] }],
        });
        if (destino) await comTarefa(t('tarefa.exportando-zip'), () => invoke("cmd_exportar_zip", { caminhoProjeto: projeto.caminho, destino }), { canal: 'zip-exportar' });
      }
    } catch (e) {
      erroInicio = t('exportar.zip-erro').replace('{erro}', String(e));
    }
  }

  function lerRecentes(): Recente[] {
    try {
      const bruto = localStorage.getItem(CHAVE_RECENTES);
      return bruto ? JSON.parse(bruto) : [];
    } catch {
      return [];
    }
  }

  function gravarRecentes() {
    try {
      localStorage.setItem(CHAVE_RECENTES, JSON.stringify(recentes));
    } catch {}
  }

  function lerAvisoVisivel(): boolean {
    try {
      return localStorage.getItem(CHAVE_AVISO) !== '1';
    } catch {
      return true;
    }
  }

  function fecharAviso() {
    avisoVisivel = false;
    try {
      localStorage.setItem(CHAVE_AVISO, '1');
    } catch {}
  }

  function registrarRecente(nome: string, caminho: string) {
    recentes = [
      { nome, caminho, abertoEm: new Date().toISOString(), estado: 'ativo' } satisfies Recente,
      ...recentes.filter((r) => r.caminho !== caminho),
    ].slice(0, 30);
    gravarRecentes();
  }

  function removerRecente(caminho: string) {
    recentes = recentes.filter((r) => r.caminho !== caminho);
    gravarRecentes();
  }

  async function carregarInicio() {
    try {
      versaoRust = await invoke("cmd_versao");
      pastaProjetos = await invoke("cmd_pasta_projetos");
      synctexDisponivel = await invoke("cmd_synctex_disponivel");
    } catch (e) {
      console.error(e);
    }
    // try proprio: uma falha acima nao pode esconder o aviso de TeX ausente
    try {
      const info: { motores: Array<{ nome: string }> } = await invoke("cmd_detectar_motores");
      semTex = !info.motores.some((m) => ["pdflatex", "xelatex", "lualatex"].includes(m.nome));
    } catch (e) {
      // sem como detectar o TeX, o usuario precisa ver o aviso
      console.error(e);
      semTex = true;
    }
  }

  function escolherModelo(modelo: Modelo) {
    menuNovoAberto = false;
    modeloEscolhido = modelo;
    nomeNovo = modelo.nomePadrao;
    erroInicio = '';
  }

  async function criarProjeto() {
    if (!modeloEscolhido || !nomeNovo.trim()) return;
    criando = true;
    erroInicio = '';
    try {
      const projeto: any = await invoke("cmd_criar_projeto", {
        diretorio: pastaProjetos,
        nome: nomeDePasta(nomeNovo),
      });
      await invoke("cmd_salvar_arquivo", {
        caminhoProjeto: projeto.caminho,
        caminhoRelativo: "/main.tex",
        conteudo: modeloEscolhido.conteudo,
      });
      const nome = nomeNovo.trim();
      modeloEscolhido = null;
      await entrarComBarra(projeto.caminho, nome);
    } catch (e) {
      erroInicio = String(e);
    } finally {
      criando = false;
    }
  }

  async function abrirPasta() {
    menuNovoAberto = false;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const pasta = await open({ directory: true, title: "Abrir pasta do projeto" });
      if (pasta) {
        const caminho = pasta as string;
        await entrarComBarra(caminho, caminho.split(/[\\/]/).pop() || caminho);
      }
    } catch (e) {
      erroInicio = `Erro ao abrir pasta: ${e}`;
    }
  }

  async function abrirZip() {
    menuNovoAberto = false;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const zip = await open({
        title: "Abrir projeto em .zip",
        filters: [{ name: "Arquivo zip", extensions: ["zip"] }],
      });
      if (!zip) return;
      const pasta: string = await comTarefa(t('tarefa.importando-zip'), () => invoke("cmd_importar_zip", { arquivoZip: zip as string, pastaPai: pastaProjetos }), { canal: 'zip-importar' });
      const arvoreZip: FileNode[] = await invoke("cmd_listar_arvore", { diretorio: pasta });
      const achado = await detectarPrincipal(pasta, arvoreZip);
      if (achado) guardarPrincipal(pasta, achado);
      await entrarComBarra(pasta, pasta.split(/[\\/]/).pop() || pasta);
    } catch (e) {
      erroInicio = `Erro ao abrir o zip: ${e}`;
    }
  }

  /** Abrir projeto com a barra do rodape: a tarefa abre e fecha aqui, fora do corpo de entrarNoProjeto. */
  async function entrarComBarra(caminho: string, nome: string) {
    const tarefa = iniciarTarefa(t('tarefa.abrindo-projeto'));
    try {
      await entrarNoProjeto(caminho, nome);
    } finally {
      tarefa.terminar();
    }
  }

  async function entrarNoProjeto(caminho: string, nome: string) {
    erroInicio = '';
    let arvore: FileNode[];
    try {
      arvore = await invoke("cmd_listar_arvore", { diretorio: caminho });
    } catch (e) {
      erroInicio = `A pasta do projeto não foi encontrada (${caminho}). Ela pode ter sido movida ou apagada. ${e}`;
      return;
    }
    principalDetectado = await detectarPrincipal(caminho, arvore);
    const principal = encontrarMainTex(arvore, caminho);
    if (!principal) {
      erroInicio = `Nenhum arquivo .tex encontrado em ${caminho}`;
      return;
    }

    // o projeto que sai: salva as abas sujas antes de soltar o texto delas. Se nao salvar, a pessoa
    // escolhe: descartar e abrir o outro, ou voltar ao projeto atual para resolver o erro
    if (caminhoProjeto && !(await salvarTodas())) {
      const arquivos = arquivosSujosTexto();
      if (!confirm(tf('projeto.descartar-ao-trocar', { arquivos }))) {
        erroInicio = tf('projeto.sair-sem-salvar', { arquivos });
        modo = 'editor';
        return;
      }
      cancelarTimerSalvar();
      for (const c of [...sujas]) marcarSuja(c, false);
    }

    caminhoProjeto = caminho;
    nomeProjeto = nome;
    arquivosProjeto = arvore;
    arquivosConteudo = {};
    // nada das abas do projeto anterior sobra; as deste so voltam depois de lidas (abaixo)
    cancelarTimerSalvar();
    limparDadosDasAbas();
    aplicarEstadoAbas(estadoVazio());
    conteudoEditor = '';
    pdfDados = null;
    caminhoPdf = '';
    destaquePdf = null;
    logTexto = '';
    tempoMs = null;
    erros = [];
    statusCompilacao = 'idle';
    motorDetectado = '';
    registrarRecente(nome, caminho);
    await carregarRevisoesDoProjeto(caminho);

    // abas da ultima vez neste projeto: so as que ainda existem na arvore
    const salvas = lerAbasSalvas(caminho, (c) => existeNaArvore(arvore, c));

    modo = 'editor';
    // o editor so existe depois que a tela do projeto monta
    await tick();
    mostrarEditorVazio();
    aplicarEstadoAbas({ abas: salvas.abas, ativa: '', recentes: [] });
    await selecionarArquivo(salvas.ativa || principal);
    void conferirDrive(caminho);
  }

  function voltarAoInicio() {
    clearTimeout(timerSync);
    clearTimeout(timerCompilar);
    precisaRecompilar = false;
    // as abas sujas salvam antes de tudo: a sincronizacao do Drive pode estar em andamento e sair sem salvar
    void salvarTodas();
    if (driveAtivo) void sincronizarDrive().finally(() => (driveAtivo = false));
    modo = 'inicio';
  }

  // Documento principal de cada projeto: guardado no localStorage com o caminho do projeto na chave.
  // Preenchido na importacao de um zip ou de arquivos do Drive.
  function principalGuardado(caminho: string): string {
    if (!caminho) return '';
    try {
      return localStorage.getItem(chavePrincipalDoProjeto(caminho)) ?? '';
    } catch {
      return '';
    }
  }

  function guardarPrincipal(caminho: string, relativo: string) {
    try {
      localStorage.setItem(chavePrincipalDoProjeto(caminho), relativo.replace(/^\/+/, ''));
    } catch {
      // sem localStorage: o principal volta a ser detectado
    }
  }

  // Ordem: principal do projeto, o detectado (\begin{document}, como no Overleaf), main.tex da
  // raiz, qualquer main.tex e o primeiro .tex.
  function encontrarMainTex(nos: FileNode[], caminho: string = caminhoProjeto): string | null {
    const todos: FileNode[] = [];
    const visitar = (lista: FileNode[]) => {
      for (const no of lista) {
        todos.push(no);
        if (no.filhos) visitar(no.filhos);
      }
    };
    visitar(nos);
    const guardado = principalGuardado(caminho);
    const doProjeto = guardado ? todos.find((n) => n.tipo === 'arquivo' && n.caminho === comBarra(guardado)) : undefined;
    const mainDaRaiz = nos.find((n) => n.tipo === 'arquivo' && n.nome === 'main.tex');
    const detectado = principalDetectado ? todos.find((n) => n.tipo === 'arquivo' && n.caminho === principalDetectado) : undefined;
    const main = todos.find((n) => n.tipo === 'arquivo' && n.nome === 'main.tex');
    const tex = todos.find((n) => n.tipo === 'arquivo' && n.nome.endsWith('.tex'));
    return (doProjeto ?? detectado ?? mainDaRaiz ?? main ?? tex)?.caminho ?? null;
  }

  /**
   * Como no Overleaf, o principal e o .tex com \begin{document} fora de comentario, em qualquer
   * pasta; um preambulo so com \documentclass nunca e. Empate: \documentclass no proprio arquivo,
   * main.tex, o nome da pasta, o maior (regra do Rust, arquivos.rs: principal_entre). Um principal
   * guardado que nao tem \begin{document} (gravado por uma versao antiga) e esquecido.
   */
  async function detectarPrincipal(caminho: string, arvore: FileNode[]): Promise<string> {
    const guardado = principalGuardado(caminho);
    if (guardado && existeNaArvore(arvore, comBarra(guardado))) {
      const texto = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto: caminho, caminhoRelativo: comBarra(guardado) }).catch(() => null);
      if (texto !== null && !temBeginDocument(texto)) esquecerPrincipal(caminho);
    }
    const achado = await invoke<string | null>('cmd_detectar_principal', { diretorio: caminho }).catch(() => null);
    return achado ? comBarra(achado) : '';
  }

  function esquecerPrincipal(caminho: string) {
    try {
      localStorage.removeItem(chavePrincipalDoProjeto(caminho));
    } catch {
      // sem localStorage: nada guardado
    }
  }

  async function carregarArvore() {
    try {
      arquivosProjeto = await comTarefa(t('tarefa.abrindo-projeto'), () => invoke<FileNode[]>("cmd_listar_arvore", { diretorio: caminhoProjeto }));
    } catch (e) {
      console.error("Erro ao listar arvore:", e);
    }
  }

  // salvamento automatico: um timer por aba (por caminho). Digitar numa aba nunca cancela o salvamento
  // pendente de outra, e o timer grava o arquivo certo mesmo que a aba ativa mude.
  let timerSalvar: Map<string, ReturnType<typeof setTimeout>> = new Map();

  /** Sem caminho cancela o timer de todas as abas; com um caminho, so o dessa aba. */
  function cancelarTimerSalvar(caminho?: string) {
    for (const c of caminho === undefined ? [...timerSalvar.keys()] : [caminho]) {
      const timer = timerSalvar.get(c);
      if (timer !== undefined) clearTimeout(timer);
      timerSalvar.delete(c);
    }
  }

  function agendarSalvar(caminho: string) {
    cancelarTimerSalvar(caminho);
    timerSalvar.set(caminho, setTimeout(() => {
      timerSalvar.delete(caminho);
      void salvar(caminho);
    }, 1000));
  }
  let timerCompilar: ReturnType<typeof setTimeout> | undefined;

  // ---------- Google Drive: sincronizacao do projeto compartilhado ----------
  let compartilharVisivel = $state(false);
  // Abrir do Google Drive: arquivos soltos escolhidos no seletor do Google (tela inicial: projeto
  // novo; menu Arquivo: adiciona ao projeto aberto)
  let abrirDriveVisivel = $state(false);
  let adicionarDriveVisivel = $state(false);
  // evita repetir, a cada sincronizacao de 30 s, o mesmo aviso do Drive
  let ultimoAvisoDrive = '';

  let driveAtivo = $state(false);
  let estadoDrive = $state('');
  /** Texto longo do estado da sincronizacao (lista de arquivos com falha), para o title da barra. */
  let detalheDrive = $state('');
  let timerSync: ReturnType<typeof setTimeout> | undefined;
  let sincronizando = false;
  // o Drive negou o arquivo (404): a pessoa escolhe o arquivo de novo no seletor do Google
  let driveAcessoPerdido = $state(false);
  let reabrindoSeletor = $state(false);

  async function sincronizarDrive() {
    if (!driveAtivo || !caminhoProjeto || sincronizando || driveAcessoPerdido) return;
    sincronizando = true;
    const projeto = caminhoProjeto;
    estadoDrive = t('drive.sincronizando');
    detalheDrive = '';
    try {
      await salvarTodas();
      const textoAntes = editorAtivo?.obterConteudo() ?? conteudoEditor;
      const r = await comTarefa(t('tarefa.sincronizando-drive'), () => invoke<{ enviados: number; baixados: number; apagados: number; conflitos: string[]; falhas?: string[]; copias_do_drive?: string[]; alterados: string[]; convertido: boolean; avisos: string[] }>(
        'cmd_drive_sincronizar', { caminho: projeto },
      ), { canal: 'drive-baixar' });
      if (projeto !== caminhoProjeto) return;
      if (r.alterados.length) {
        // apaga o cache e o indice antes de recarregar a arvore: a reindexacao le o texto novo
        for (const rel of r.alterados) {
          delete arquivosConteudo[comBarra(rel)];
          esquecerArquivo(comBarra(rel));
        }
        await recarregarArvore();
        // aba aberta que nao e a ativa e nao tem edicao pendente: o estado guardado esta velho,
        // entao some e o arquivo volta a ser lido do disco quando a aba for aberta
        for (const rel of r.alterados) {
          const c = comBarra(rel);
          if (c !== arquivoAtual && !estaSuja(c)) dadosDasAbas.delete(c);
        }
        const atual = arquivoAtual;
        const mudouAberto = r.alterados.some((rel) => comBarra(rel) === atual);
        // so recarrega o editor se a pessoa nao digitou nada enquanto sincronizava
        if (mudouAberto && (editorAtivo?.obterConteudo() ?? conteudoEditor) === textoAntes) {
          try {
            const texto = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: atual });
            editorAtivo?.definirConteudo(texto);
            conteudoEditor = texto;
            arquivosConteudo[atual] = texto;
          } catch {
            // arquivo apagado por outra pessoa: o editor fica com o texto atual
          }
        } else if (mudouAberto) {
          // a pessoa digitou durante a sincronizacao: o proximo salvamento gravaria o texto
          // dela por cima da versao que acabou de chegar. A versao do Drive vira uma copia.
          try {
            const chegou = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: atual });
            const copia = atual.replace(/(\.[^./]+)?$/, (ext) => ` (versão do Drive)${ext}`);
            await invoke('cmd_salvar_arquivo', { caminhoProjeto, caminhoRelativo: copia, conteudo: chegou });
            await recarregarArvore();
            alert(
              `Outra pessoa mudou ${atual.split('/').pop()} enquanto você digitava. O seu texto continua no editor, ` +
                `e a versão que chegou do Google Drive foi salva como ${copia.split('/').pop()}.`,
            );
          } catch {
            // arquivo apagado por outra pessoa: o editor fica com o texto atual
          }
        }
      }
      const textoAvisos = (r.avisos ?? []).join('\n\n');
      if (textoAvisos && textoAvisos !== ultimoAvisoDrive) alert(textoAvisos);
      ultimoAvisoDrive = textoAvisos;
      if (r.copias_do_drive?.length) {
        alert(`${t('drive.vinculo-conflito')}\n\n${r.copias_do_drive.join('\n')}`);
      }
      if (r.convertido) {
        alert(t('drive.convertido'));
      }
      if (r.conflitos.length) {
        alert(
          'Você e outra pessoa mudaram o mesmo arquivo ao mesmo tempo. A versão do Google Drive ficou no lugar e a sua foi salva como:\n\n' +
            r.conflitos.join('\n'),
        );
      }
      const hora = new Date().toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
      // `falhas` pode faltar (Rust mais antigo): vale como lista vazia
      const falhas = r.falhas ?? [];
      if (falhas.length) {
        estadoDrive = t('drive.sincronizado-falhas').replace('{n}', String(falhas.length));
        detalheDrive = falhas.join('\n');
      } else {
        estadoDrive = t('drive.sincronizado').replace('{hora}', hora);
        detalheDrive = '';
      }
    } catch (e) {
      if (acessoPerdido(e)) {
        driveAcessoPerdido = true;
        estadoDrive = t('drive.sem-acesso');
      } else {
        estadoDrive = t('drive.sem-sincronizar');
      }
      console.error('Drive:', textoErro(e));
    } finally {
      sincronizando = false;
    }
  }

  // a cada 30 s busca o que os colaboradores mudaram
  $effect(() => {
    if (modo !== 'editor' || !driveAtivo) return;
    const id = setInterval(() => void sincronizarDrive(), 30000);
    return () => clearInterval(id);
  });

  async function reabrirPeloSeletor() {
    if (!caminhoProjeto || reabrindoSeletor) return;
    reabrindoSeletor = true;
    try {
      const escolheu = await invoke<boolean>('cmd_drive_reabrir_seletor', { caminho: caminhoProjeto });
      if (escolheu) {
        driveAcessoPerdido = false;
        await sincronizarDrive();
      }
    } catch (e) {
      alert(textoErro(e));
    } finally {
      reabrindoSeletor = false;
    }
  }

  async function conferirDrive(caminho: string) {
    try {
      driveAtivo = !!(await invoke<string | null>('cmd_drive_projeto', { caminho }));
    } catch {
      driveAtivo = false;
    }
    estadoDrive = '';
    driveAcessoPerdido = false;
    if (driveAtivo) void sincronizarDrive();
  }

  function aoMudarTexto(texto: string) {
    // so a aba de texto ativa recebe edicao
    if (!arquivoAtual) return;
    conteudoEditor = texto;
    marcarSuja(arquivoAtual, true);
    if (driveAtivo) {
      clearTimeout(timerSync);
      timerSync = setTimeout(() => void sincronizarDrive(), 8000);
    }
    if (preferencias.autoSalvar) agendarSalvar(arquivoAtual);
    clearTimeout(timerCompilar);
    if (preferencias.autoCompilar) timerCompilar = setTimeout(() => compilar(true), 2500);
  }

  $effect(() => {
    const raiz = document.documentElement;
    if (preferencias.tema === 'sistema') delete raiz.dataset.tema;
    else raiz.dataset.tema = preferencias.tema;
  });

  let estiloLinha = $state('normal');
  let historicoEditor = $state({ desfazer: false, refazer: false });
  let configuracoesVisivel = $state(false);
  let configuracoesAba = $state<'compilador' | 'editor' | 'aparencia'>('compilador');

  function abrirConfiguracoes(aba: 'compilador' | 'editor' | 'aparencia' = 'compilador') {
    configuracoesAba = aba;
    configuracoesVisivel = true;
  }
  let contagemVisivel = $state(false);
  let versoesVisivel = $state(false);
  // sugestoes de revisao do arquivo no momento em que a janela abriu (restaurar as descarta)
  let versoesSugestoes = $state(0);
  let assistenteTabela = $state<{ visivel: boolean; tamanho: string }>({ visivel: false, tamanho: '3x3' });
  let textoContagem = $state('');

  function caminhosTex(nos: FileNode[], saida: string[] = []): string[] {
    for (const n of nos) {
      if (n.tipo === 'pasta') caminhosTex(n.filhos ?? [], saida);
      else if (n.nome.toLowerCase().endsWith('.tex')) saida.push(n.caminho);
    }
    return saida;
  }

  async function abrirContagem() {
    const partes: string[] = [];
    for (const caminho of caminhosTex(arquivosProjeto)) {
      const emMemoria = textoDaAba(caminho);
      if (emMemoria !== undefined) {
        // aba aberta: vale o texto dela, que pode ser mais novo que o do disco
        partes.push(emMemoria);
      } else {
        try {
          partes.push(await invoke<string>("cmd_ler_arquivo", { caminhoProjeto, caminhoRelativo: caminho }));
        } catch {
          continue;
        }
      }
    }
    textoContagem = partes.join('\n');
    contagemVisivel = true;
  }

  async function renomearProjeto(novoNome: string) {
    if (!caminhoProjeto) return;
    await salvarTodas();
    try {
      const novoCaminho: string = await invoke("cmd_renomear_projeto", { caminho: caminhoProjeto, novoNome });
      removerRecente(caminhoProjeto);
      caminhoProjeto = novoCaminho;
      // as abas abertas acompanham o projeto renomeado
      gravarAbasSalvas(novoCaminho, fotoDasAbas());
      nomeProjeto = novoNome;
      registrarRecente(novoNome, novoCaminho);
    } catch (e) {
      alert(`Não foi possível renomear o projeto: ${e}`);
    }
  }

  // ---------- indice do projeto: \label dos .tex e entradas dos .bib ----------
  function caminhosIndexaveis(nos: FileNode[], saida: string[] = []): string[] {
    for (const n of nos) {
      if (n.tipo === 'pasta') caminhosIndexaveis(n.filhos ?? [], saida);
      else if (/\.(tex|bib)$/i.test(n.nome)) saida.push(n.caminho);
    }
    return saida;
  }

  const LIMITE_ARQUIVOS_INDICE = 400;
  const LIMITE_TAMANHO_INDICE = 3_000_000;
  const cederVez = () => new Promise<void>((resolver) => setTimeout(resolver, 0));

  let rodadaIndice = 0;
  // So le do disco o que ainda nao esta no indice. O indice se mantem em dia ao salvar, restaurar
  // versao, gravar pelo agente e sincronizar com o Drive (esses pontos chamam atualizarArquivo
  // ou esquecerArquivo), entao trocar a arvore nao rele todos os arquivos.
  async function indexarProjeto(projeto: string, arvore: FileNode[]) {
    const rodada = ++rodadaIndice;
    definirProjeto(projeto);
    // .bib primeiro: se o corte de arquivos acontecer, a bibliografia nao fica de fora
    const todos = caminhosIndexaveis(arvore);
    const ehBib = (c: string) => /\.bib$/i.test(c);
    const ordenados = [...todos.filter(ehBib), ...todos.filter((c) => !ehBib(c))];
    if (ordenados.length > LIMITE_ARQUIVOS_INDICE) {
      console.warn(`Indice do projeto: ${ordenados.length} arquivos .tex e .bib, so os ${LIMITE_ARQUIVOS_INDICE} primeiros entram nas sugestoes de referencias e citacoes.`);
    }
    const caminhos = ordenados.slice(0, LIMITE_ARQUIVOS_INDICE);
    manterSomente(caminhos);
    for (const c of caminhos) {
      if (rodada !== rodadaIndice) return;
      if (arquivoIndexado(c)) continue;
      let texto = arquivosConteudo[c];
      if (texto === undefined) {
        try {
          texto = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto: projeto, caminhoRelativo: c });
        } catch {
          continue;
        }
        // o projeto ou a arvore mudou durante a leitura: esta rodada ja nao vale
        if (rodada !== rodadaIndice) return;
      }
      if (texto.length > LIMITE_TAMANHO_INDICE) {
        console.warn(`Indice do projeto: ${c} passa de ${LIMITE_TAMANHO_INDICE} caracteres e ficou de fora.`);
        continue;
      }
      atualizarArquivo(c, texto);
      // cede a vez entre arquivos para a tela nao travar com .bib grande
      await cederVez();
    }
    if (rodada === rodadaIndice) editorAtivo?.indiceAtualizado();
  }

  // refaz o indice ao abrir o projeto e sempre que a arvore de arquivos muda
  $effect(() => {
    const projeto = caminhoProjeto;
    const arvore = arquivosProjeto;
    if (projeto) untrack(() => void indexarProjeto(projeto, arvore));
  });

  // ---------- abas: um unico editor, um EditorState por aba ----------
  // Uma operacao de aba por vez. Trocar, fechar, renomear e apagar mexem no editor e na lista de
  // abas juntos; duas ao mesmo tempo (Ctrl+Tab apertado duas vezes) gravariam o texto de uma aba na outra.
  let filaDeAbas: Promise<unknown> = Promise.resolve();
  // arquivos ou pastas cujo comando de apagar esta rodando: nada grava neles ate a lista de abas fechar
  let apagando: string[] = [];
  function comTrava<T>(tarefa: () => Promise<T>): Promise<T> {
    const resultado = filaDeAbas.then(tarefa);
    filaDeAbas = resultado.catch(() => {});
    return resultado;
  }

  /** Texto de uma aba de texto: o editor, se for a ativa; senao o estado guardado. `undefined` se a aba nunca foi aberta nesta sessao (nada a salvar). */
  function textoDaAba(caminho: string): string | undefined {
    if (caminho === arquivoAtual) return editorRef?.obterConteudo() ?? conteudoEditor;
    return dadosDasAbas.get(caminho)?.estado.doc.toString();
  }

  /** Aplica a lista de abas, guarda no localStorage e solta o que sobrou das abas que sairam (limite de 12). */
  function mudarAbas(novo: EstadoAbas) {
    const antes = abasEstado.abas.map((a) => a.caminho);
    const depois = new Set(novo.abas.map((a) => a.caminho));
    aplicarEstadoAbas(novo);
    for (const c of antes) if (!depois.has(c)) descartarAba(c);
    if (caminhoProjeto) gravarAbasSalvas(caminhoProjeto, novo);
  }

  function descartarAba(caminho: string) {
    const dados = dadosDasAbas.get(caminho);
    if (dados) revisoesPorArquivo[caminho] = dados.estado.field(campoRevisoes);
    dadosDasAbas.delete(caminho);
    sujas.delete(caminho);
  }

  /** Editor vazio, sem caminho: o que sobra quando a ultima aba fecha. */
  function mostrarEditorVazio() {
    if (!editorRef) return;
    editorRef.importarEstado({ estado: editorRef.criarEstado(''), rolagem: 0 });
    definirArquivoAberto('');
    conteudoEditor = '';
  }

  /** Le o texto de um arquivo para abrir numa aba. `null` se nao abre (PDF, binario, texto fora do UTF-8). */
  async function lerTexto(caminho: string): Promise<string | null> {
    const emCache = arquivosConteudo[caminho];
    if (emCache !== undefined) return emCache;
    try {
      const texto = await invoke<string>("cmd_ler_arquivo", { caminhoProjeto, caminhoRelativo: caminho });
      arquivosConteudo[caminho] = texto;
      // primeira abertura do arquivo na sessao: guarda o estado antes de editar
      void invoke("cmd_versoes_gravar", { caminhoProjeto, caminhoRelativo: caminho, intervaloS: 0 }).catch(() => {});
      return texto;
    } catch (e) {
      console.error("Erro ao ler arquivo:", e);
      alert(
        `O arquivo ${caminho.split('/').pop()} não abre no editor. ` +
          'O editor abre somente arquivos de texto em UTF-8 (PDFs e outros arquivos binários ficam de fora).'
      );
      return null;
    }
  }

  /** A aba de texto que sai de cena: salva se estiver suja e guarda o estado do editor (historico, selecao, rolagem). */
  async function guardarAtiva() {
    const atual = arquivoAtual;
    if (!atual) return;
    if (estaSuja(atual)) await salvar(atual);
    // as sugestoes gravam ANTES de exportar: o editor ainda mostra esta aba durante a espera, e uma tecla
    // digitada nela tem de entrar no estado exportado (nada pode esperar entre exportar e trocar)
    await gravarRevisoes();
    const guardado = editorRef?.exportarEstado();
    if (guardado) {
      dadosDasAbas.set(atual, { ...guardado, sujo: estaSuja(atual) });
      const texto = guardado.estado.doc.toString();
      arquivosConteudo[atual] = texto;
      atualizarArquivo(atual, texto);
    }
  }

  /** `nao-abre`: o arquivo da aba nao le; `abortado`: o gancho `antesDeTrocar` mandou ficar onde esta. */
  type ResultadoMostrar = 'ok' | 'nao-abre' | 'abortado';

  /**
   * Mostra a aba `novo.ativa` (no editor) e aplica a lista `novo`. O que pode
   * falhar (ler o arquivo) acontece antes de mexer no editor: se falhar, a aba anterior continua ativa e
   * o salvamento automatico nunca grava o texto de um arquivo por cima de outro. A troca em si e sincrona,
   * entao `arquivoAtual` e o texto do editor nunca ficam de abas diferentes.
   */
  async function mostrar(novo: EstadoAbas, guardarAtual = true, antesDeTrocar?: () => Promise<boolean>): Promise<ResultadoMostrar> {
    const alvo = novo.abas.find((a) => a.caminho === novo.ativa);
    let preparado: (EstadoGuardado & { novo: boolean }) | null = null;
    if (alvo?.tipo === 'texto') {
      const guardado = dadosDasAbas.get(alvo.caminho);
      if (guardado) {
        preparado = { estado: guardado.estado, rolagem: guardado.rolagem, novo: false };
      } else {
        const texto = await lerTexto(alvo.caminho);
        const estado = texto === null ? undefined : editorRef?.criarEstado(texto);
        if (!estado) {
          // aba reaberta de uma sessao anterior que nao abre mais: sai da lista
          if (texto === null && abasEstado.abas.some((a) => a.caminho === alvo.caminho)) mudarAbas(fecharNaLista(fotoDasAbas(), alvo.caminho));
          return 'nao-abre';
        }
        preparado = { estado, rolagem: 0, novo: true };
      }
    }
    if (guardarAtual) await guardarAtiva();
    // o gancho roda depois da leitura e logo antes da troca: o que foi digitado enquanto o arquivo carregava
    // ainda entra no que ele salva, e nada espera entre ele e a troca
    if (antesDeTrocar && !(await antesDeTrocar())) return 'abortado';

    if (alvo?.tipo === 'texto' && preparado && editorRef) {
      dadosDasAbas.delete(alvo.caminho);
      editorRef.importarEstado({ estado: preparado.estado, rolagem: preparado.rolagem });
      // as sugestoes de uma aba que acabou de abrir vem do disco; as de uma aba guardada ja estao no estado
      if (preparado.novo) editorRef.carregarRevisoes(revisoesPorArquivo[alvo.caminho] ?? []);
      definirArquivoAberto(alvo.caminho);
      conteudoEditor = preparado.estado.doc.toString();
    } else if (alvo?.tipo === 'imagem') {
      // o editor fica montado e escondido; arquivoAtual vira vazio e nada grava texto sobre a imagem
      conteudoEditor = '';
    } else {
      mostrarEditorVazio();
    }
    mudarAbas(novo);
    return 'ok';
  }

  /**
   * Depois de fechar ou apagar a aba ativa: mostra a vizinha que assume (`novo` ja esta sem a aba removida).
   * Vizinha que nao abre (binario, apagada fora do app) sai da lista e a proxima vizinha e tentada. Se
   * nenhuma abrir, abre `principal` (se houver) ou deixa o editor vazio, sempre com a lista sem a aba removida.
   */
  async function mostrarVizinha(novo: EstadoAbas, principal = '', antesDeTrocar?: () => Promise<boolean>): Promise<void> {
    let lista = novo;
    while (lista.ativa) {
      if ((await mostrar(lista, false, antesDeTrocar)) !== 'nao-abre') return;
      // mostrar() tira da lista a vizinha que nao abriu; se ela continua la, nao da para seguir
      if (abasEstado.abas.some((a) => a.caminho === lista.ativa)) break;
      lista = fecharNaLista(lista, lista.ativa);
    }
    const semAtiva: EstadoAbas = { ...lista, ativa: '' };
    if (principal && (await mostrar(abrirNaLista(semAtiva, abaDe(principal), sujas), false, antesDeTrocar)) !== 'nao-abre') return;
    await mostrar(semAtiva, false, antesDeTrocar);
  }

  /** Imagem abre no visualizador; o resto, no editor. */
  const abaDe = (caminho: string): Aba => (ehImagem(caminho) ? { tipo: 'imagem', caminho } : { tipo: 'texto', caminho });

  async function abrirNaAba(caminho: string): Promise<boolean> {
    if (caminho === abasEstado.ativa && abaAtiva) return true;
    return (await mostrar(abrirNaLista(fotoDasAbas(), abaDe(caminho), sujas))) === 'ok';
  }

  /** Abrir pela arvore, pela busca ou pelo SyncTeX: texto no editor, imagem no visualizador, na mesma barra de abas. */
  function selecionarArquivo(caminho: string): Promise<void> {
    return comTrava(async () => {
      await abrirNaAba(caminho);
    });
  }

  /** Menu da aba, "Fechar as outras": ativa a aba escolhida e fecha cada uma das demais. Aba suja que nao salvar continua aberta. */
  async function fecharOutrasAbas(caminho: string): Promise<void> {
    if (abasEstado.ativa !== caminho) await selecionarArquivo(caminho);
    if (abasEstado.ativa !== caminho) return;
    for (const aba of [...abasEstado.abas]) {
      if (aba.caminho !== caminho) await fecharAba(aba.caminho);
    }
  }

  /** Arrastar abas: so troca a ordem (e grava no localStorage); nenhum texto passa por aqui. */
  function moverAba(de: number, para: number): Promise<void> {
    // na fila das outras operacoes: a lista so e lida quando chega a vez, depois de salvar ou fechar abas
    return comTrava(async () => {
      mudarAbas(moverNaLista(fotoDasAbas(), de, para));
    });
  }

  function fecharAba(caminho: string): Promise<void> {
    return comTrava(async () => {
      const aba = abasEstado.abas.find((a) => a.caminho === caminho);
      if (!aba) return;
      // aba suja salva primeiro; se falhar (ou a pessoa digitar durante o salvamento), ela fica aberta
      const preparar = async (): Promise<boolean> => {
        if (aba.tipo !== 'texto') return true;
        if (estaSuja(caminho) && !(await salvar(caminho, true))) return false;
        if (estaSuja(caminho)) return false;
        // as sugestoes ficam guardadas para quando o arquivo abrir de novo
        const estado = dadosDasAbas.get(caminho)?.estado;
        revisoesPorArquivo[caminho] = caminho === arquivoAtual
          ? (editorRef?.obterRevisoes() ?? [])
          : estado ? estado.field(campoRevisoes) : (revisoesPorArquivo[caminho] ?? []);
        return true;
      };
      const novo = fecharNaLista(fotoDasAbas(), caminho);
      if (caminho !== abasEstado.ativa) {
        if (!(await preparar())) return;
        mudarAbas(novo);
      } else {
        // aba ativa: o salvamento roda depois de a vizinha carregar e logo antes da troca,
        // para a digitacao feita durante a leitura nao se perder
        await mostrarVizinha(novo, '', preparar);
      }
      await gravarRevisoes();
    });
  }

  function andarAba(passo: 1 | -1): Promise<void> {
    return comTrava(async () => {
      const novo = passo === 1 ? proximaDaLista(fotoDasAbas()) : anteriorDaLista(fotoDasAbas());
      if (novo.ativa !== abasEstado.ativa) await mostrar(novo);
    });
  }

  /**
   * Grava o texto de uma aba de texto (a ativa, se nao disser qual). Devolve se deu certo. A aba fica suja
   * ate o disco confirmar; se a pessoa digitar durante o salvamento, continua suja e o timer grava de novo.
   */
  async function salvar(caminho: string = arquivoAtual, avisarSempre = false): Promise<boolean> {
    if (!caminhoProjeto || !caminho) return true;
    // o arquivo esta sendo apagado: gravar agora o recriaria
    if (apagando.some((pasta) => dentroDe(caminho, pasta))) return true;
    // segunda trava: texto so vai para aba de texto. Imagem nunca recebe texto, venha o caminho de onde vier.
    const aba = abasEstado.abas.find((a) => a.caminho === caminho);
    if (!aba || aba.tipo !== 'texto') return true;
    // uma gravacao por vez por arquivo (a janela fechando, o Drive sincronizando e a IA gravam juntos).
    // Quem espera so grava se a de antes nao deixou tudo salvo.
    return naFila(caminho, async (esperou) => {
      if (esperou && (!estaSuja(caminho) || apagando.some((pasta) => dentroDe(caminho, pasta)))) return true;
      const texto = textoDaAba(caminho);
      if (texto === undefined) return true;
      const projeto = caminhoProjeto;
      try {
        // historico local: guarda o que esta em disco antes de sobrescrever (no maximo a cada 5 min)
        await invoke("cmd_versoes_gravar", { caminhoProjeto: projeto, caminhoRelativo: caminho, intervaloS: 300 }).catch(() => {});
        await invoke("cmd_salvar_arquivo", {
          caminhoProjeto: projeto,
          caminhoRelativo: caminho,
          conteudo: texto,
        });
        // a pessoa trocou de projeto durante o salvamento: o que vale agora e do outro projeto
        if (projeto !== caminhoProjeto) return true;
        arquivosConteudo[caminho] = texto;
        atualizarArquivo(caminho, texto);
        const agora = textoDaAba(caminho);
        if (agora === undefined || agora === texto) {
          marcarSuja(caminho, false);
          cancelarTimerSalvar(caminho);
        }
        erroAoSalvar = '';
        await gravarRevisoes();
        return true;
      } catch (e) {
        console.error("Erro ao salvar:", e);
        // avisa uma vez por erro, sem repetir a cada salvamento automatico
        const mensagem = `Não foi possível salvar ${caminho}: ${e}`;
        if (avisarSempre || mensagem !== erroAoSalvar) {
          erroAoSalvar = mensagem;
          alert(mensagem);
        }
        return false;
      }
    });
  }

  /** Salva todas as abas sujas (compilar, sincronizar, fechar a janela). Devolve se todas deram certo. */
  async function salvarTodas(): Promise<boolean> {
    cancelarTimerSalvar();
    const caminhos = abasParaSalvar(abasEstado.abas, sujas);
    const resultados = await Promise.all(caminhos.map((c) => salvar(c)));
    return resultados.every(Boolean);
  }

  /** Nomes das abas ainda sujas, para as mensagens de erro ("main.tex, cap1.tex"). */
  function arquivosSujosTexto(): string {
    return abasParaSalvar(abasEstado.abas, sujas).map((c) => c.split('/').pop() || c).join(', ');
  }

  /**
   * Aba em segundo plano cujo arquivo foi gravado por fora (agente de IA): o estado guardado passa a ter o
   * texto gravado, so no trecho que mudou (o Ctrl+Z e as sugestoes seguem), e a aba fica limpa.
   */
  function acompanharDiscoNaAba(caminho: string, texto: string) {
    const dados = dadosDasAbas.get(caminho);
    if (!dados) return;
    const antes = dados.estado.doc.toString();
    if (antes !== texto) {
      let ini = 0;
      const max = Math.min(antes.length, texto.length);
      while (ini < max && antes[ini] === texto[ini]) ini++;
      let fimAntes = antes.length;
      let fimTexto = texto.length;
      while (fimAntes > ini && fimTexto > ini && antes[fimAntes - 1] === texto[fimTexto - 1]) {
        fimAntes--;
        fimTexto--;
      }
      dados.estado = dados.estado.update({ changes: { from: ini, to: fimAntes, insert: texto.slice(ini, fimTexto) }, userEvent: 'input.ia' }).state;
    }
    arquivosConteudo[caminho] = texto;
    marcarSuja(caminho, false);
  }

  let erroAoSalvar = '';

  // Versoes anteriores: depois de restaurar, o editor passa a mostrar o texto devolvido.
  function aoRestaurarVersao(arquivo: string, texto: string) {
    arquivosConteudo[arquivo] = texto;
    atualizarArquivo(arquivo, texto);
    if (arquivo === arquivoAtual) {
      conteudoEditor = texto;
      editorAtivo?.definirConteudo(texto);
      editorAtivo?.indiceAtualizado();
      marcarSuja(arquivo, false);
    } else if (dadosDasAbas.has(arquivo)) {
      // aba aberta em segundo plano: o estado guardado e da versao antiga e o arquivo volta a ser lido do disco
      dadosDasAbas.delete(arquivo);
      marcarSuja(arquivo, false);
    }
  }

  // feedback guardado sem rede sai na próxima abertura do app
  // andamento das tarefas longas do Rust (Drive e zip) para a barra do rodape
  onMount(() => {
    let desligar: (() => void) | undefined;
    void escutarProgressoRust().then((f) => (desligar = f)).catch(() => {});
    return () => desligar?.();
  });

  onMount(() => {
    void reenviarPendentes();
  });

  // "Baixar pacote" e "Baixar fonte": instala pelo tlmgr o que falta e compila de novo, uma vez
  let instalandoPacote = false;
  async function instalarFaltando(itens: string[]) {
    if (instalandoPacote || itens.length === 0) return;
    instalandoPacote = true;
    const anterior = logTexto;
    const instalados: string[] = [];
    try {
      for (const arquivo of itens) {
        const nome = nomeDoFaltando(arquivo).replace(/\.(sty|cls|def|tex)$/i, '');
        logTexto = tf('faltando.baixando', { nome });
        try {
          instalados.push(await comTarefa(tf('tarefa.baixando-pacote', { pacote: nome }), () => invoke<string>('cmd_instalar_pacote_tex', { arquivo })));
        } catch (erro) {
          alert(tf('faltando.falhou', { nome, erro: String(erro) }));
        }
      }
      if (instalados.length === 0) {
        logTexto = anterior;
        return;
      }
      logTexto = tf('faltando.instalado', { pacotes: instalados.join(', ') });
    } finally {
      instalandoPacote = false;
    }
    await compilar();
  }
  /** O que ja foi oferecido neste projeto: a pergunta nao volta a cada compilacao. */
  const faltandoOferecido = new Set<string>();
  /** Depois da compilacao: pacote ou fonte que falta vira uma pergunta, sem esperar a pessoa abrir a lista. */
  function oferecerFaltando(lista: ReadonlyArray<{ mensagem: string }>) {
    const novos = faltandoNosErros(lista).filter((f) => !faltandoOferecido.has(caminhoProjeto + '|' + f));
    if (novos.length === 0 || instalandoPacote) return;
    for (const f of novos) faltandoOferecido.add(caminhoProjeto + '|' + f);
    const nomes = novos.map((f) => '• ' + (f.startsWith('fonte:') ? tf('faltando.fonte', { nome: nomeDoFaltando(f) }) : f)).join('\n');
    // depois do PDF aparecer na tela
    setTimeout(() => {
      if (confirm(tf('faltando.pergunta', { lista: nomes }))) void instalarFaltando(novos);
    }, 300);
  }
  onMount(() => {
    const aoPedir = (e: Event) => {
      const arquivo = (e as CustomEvent<string>).detail;
      if (arquivo) void instalarFaltando([arquivo]);
    };
    window.addEventListener('eguatex:instalar-pacote', aoPedir);
    return () => window.removeEventListener('eguatex:instalar-pacote', aoPedir);
  });

  // Fechar a janela (ou o app ir para segundo plano no celular) salva antes o texto que
  // ainda esta no timer do salvamento automatico.
  onMount(() => {
    const salvarAgora = () => salvarTodas();
    const aoEsconder = () => {
      if (document.visibilityState === 'hidden') void salvarAgora();
    };
    document.addEventListener('visibilitychange', aoEsconder);
    let desligar: (() => void) | undefined;
    getCurrentWindow()
      .onCloseRequested(async (evento) => {
        if (await salvarAgora()) return;
        // nao salvou: a janela fica aberta, com o erro na tela, ate a pessoa escolher sair mesmo assim
        if (!confirm(tf('janela.sair-sem-salvar', { arquivos: arquivosSujosTexto() }))) evento.preventDefault();
      })
      .then((f) => (desligar = f))
      .catch(() => {});
    return () => {
      document.removeEventListener('visibilitychange', aoEsconder);
      desligar?.();
    };
  });

  // como no Overleaf, compila o documento principal, nao o arquivo aberto
  function documentoPrincipal(): string {
    const guardado = principalGuardado(caminhoProjeto);
    const doProjeto = guardado && existeNaArvore(arquivosProjeto, comBarra(guardado)) ? comBarra(guardado) : '';
    const escolhido = doProjeto || (preferencias.documentoPrincipal ? comBarra(preferencias.documentoPrincipal) : '');
    const principal = escolhido && existeNaArvore(arquivosProjeto, escolhido)
      ? escolhido
      : encontrarMainTex(arquivosProjeto) ?? arquivoAtual;
    return principal.replace(/^\/+/, '') || 'main.tex';
  }

  function nomeDoMotor(motor: string): string {
    return motor === 'xelatex' ? 'XeLaTeX' : motor === 'lualatex' ? 'LuaLaTeX' : 'pdfLaTeX';
  }

  // Motor invisivel, como no Overleaf: o Rust escolhe pelo documento e, se esse motor nao gerar o PDF,
  // tenta o XeLaTeX e o LuaLaTeX. O que funcionou fica guardado por projeto e documento principal
  // (localStorage) como "motor@impressao" e volta para o Rust na proxima compilacao, que o roda primeiro
  // enquanto o preambulo e os pacotes nao mudarem (a impressao e do Rust; aqui o valor e opaco).
  function chaveMotorDoProjeto(caminho: string, principal: string): string {
    const projeto = caminho.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
    return `eguatex.motor:${projeto}|${principal.replace(/\\/g, '/').replace(/^\/+/, '').toLowerCase()}`;
  }

  function motorLembrado(caminho: string, principal: string): string | null {
    try {
      return localStorage.getItem(chaveMotorDoProjeto(caminho, principal));
    } catch {
      return null;
    }
  }

  function lembrarMotor(caminho: string, principal: string, motor: string | null | undefined) {
    try {
      if (motor) localStorage.setItem(chaveMotorDoProjeto(caminho, principal), motor);
      else localStorage.removeItem(chaveMotorDoProjeto(caminho, principal));
    } catch {
      // sem localStorage: a proxima compilacao comeca de novo pelo motor detectado
    }
  }

  function existeNaArvore(nos: FileNode[], caminho: string): boolean {
    return nos.some((n) => n.caminho === caminho || (n.tipo === 'pasta' && existeNaArvore(n.filhos ?? [], caminho)));
  }

  async function pararCompilacao() {
    try {
      await invoke("cmd_parar_compilacao");
    } catch (e) {
      logTexto = `Não foi possível parar a compilação: ${e}`;
    }
  }

  // marcada quando o compilar automatico chega com outra compilacao em andamento
  let precisaRecompilar = false;

  async function compilar(automatica = false) {
    if (!caminhoProjeto) return;
    if (compilando) {
      if (automatica) precisaRecompilar = true;
      return;
    }
    clearTimeout(timerCompilar);
    // marca antes do primeiro await: dois cliques seguidos nao rodam duas compilacoes
    compilando = true;
    const projetoDaCompilacao = caminhoProjeto;

    // compila o principal, com o texto de todas as abas abertas (o .tex secundario editado tambem entra)
    if (!(await salvarTodas())) {
      // compilar agora geraria o PDF da versao velha que esta no disco
      const mensagem = tf('compilar.sem-salvar', { arquivos: arquivosSujosTexto() });
      compilando = false;
      precisaRecompilar = false;
      statusCompilacao = 'erro';
      logTexto = mensagem;
      erros = [{ mensagem, tipo: 'fatal' }];
      return;
    }

    statusCompilacao = 'compilando';
    logTexto = 'Compilando...';
    erros = [];

    try {
      // o motor lembrado e do principal desta compilacao
      const principalDaCompilacao = documentoPrincipal();
      const resultado: any = await invoke("cmd_compilar_completo", {
        arquivo: documentoPrincipal(),
        diretorio: caminhoProjeto,
        motor: 'Automatico',
        pararNoErro: preferencias.pararNoPrimeiroErro,
        maxPassadas: preferencias.maxPassadas,
        motorLembrado: motorLembrado(projetoDaCompilacao, principalDaCompilacao),
      });

      // a pessoa trocou de projeto durante a compilacao: o resultado nao e deste
      if (projetoDaCompilacao !== caminhoProjeto) return;
      motorDetectado = resultado.motor_usado ? nomeDoMotor(resultado.motor_usado) : '';
      // com PDF, guarda o motor que funcionou (ou esquece, se foi o detectado); sem PDF, nada muda
      if (resultado.pdf) lembrarMotor(projetoDaCompilacao, principalDaCompilacao, resultado.motor_lembrar);
      erros = resultado.erros || [];
      oferecerFaltando(erros);
      tempoMs = typeof resultado.tempo_ms === 'number' ? resultado.tempo_ms : null;
      const pdfNovo = /Output written on/.test(resultado.log || '');
      statusCompilacao = resultado.sucesso ? 'sucesso' : pdfNovo ? 'parcial' : 'erro';
      if (pdfNovo && preferencias.synctexAposCompilar && !automatica) setTimeout(() => void irParaPdf(), 400);
      logTexto = resultado.sucesso
        ? `Compilação concluída (${resultado.tempo_ms} ms, ${resultado.motor_usado})\n\n${resultado.log || ''}`
        : pdfNovo
          ? `O PDF foi gerado, mas o LaTeX apontou ${erros.filter((e: any) => e.tipo !== 'aviso').length || 'alguns'} erro(s) no documento. O Overleaf mostra os mesmos erros e também gera o PDF. Veja a lista para corrigir.

${resultado.log || ''}`
          : resultado.log || '';

      if (resultado.pdf) {
        caminhoPdf = resultado.pdf;
        try {
          const bytes = await invoke<ArrayBuffer>("cmd_ler_pdf_bytes", { caminho: resultado.pdf });
          pdfDados = new Uint8Array(bytes);
        } catch (e) {
          logTexto += `\n\nErro ao carregar o PDF gerado: ${e}`;
        }
      }
    } catch (e) {
      if (projetoDaCompilacao !== caminhoProjeto) return;
      // nenhum motor rodou
      motorDetectado = '';
      statusCompilacao = 'erro';
      logTexto = `Erro na compilação: ${e}`;
      tempoMs = null;
      erros = [{ mensagem: String(e), tipo: 'fatal' }];
    } finally {
      compilando = false;
      // o texto mudou durante a compilacao automatica: compila de novo
      if (precisaRecompilar && projetoDaCompilacao === caminhoProjeto) {
        precisaRecompilar = false;
        void compilar(true);
      }
      precisaRecompilar = false;
    }
  }

  let caminhoPdf = $state('');
  let synctexDisponivel = $state(false);
  let estadoOrtografia = $state<EstadoOrtografia>({ ligada: true });
  let destaquePdf = $state<{ pagina: number; x: number; y: number; largura: number; altura: number } | null>(null);

  function comBarra(caminho: string): string {
    return caminho.startsWith('/') ? caminho : '/' + caminho;
  }

  function relativoDoProjeto(absoluto: string): string | null {
    const base = caminhoProjeto.replace(/\\/g, '/').replace(/\/+$/, '');
    const alvo = absoluto.replace(/\\/g, '/');
    return alvo.toLowerCase().startsWith(base.toLowerCase() + '/') ? alvo.slice(base.length) : null;
  }

  function absolutoDoProjeto(relativo: string): string {
    return `${caminhoProjeto}\\${relativo.replace(/^\/+/, '').replace(/\//g, '\\')}`;
  }

  async function recarregarArvore() {
    arquivosProjeto = await comTarefa(t('tarefa.abrindo-projeto'), () => invoke<FileNode[]>("cmd_listar_arvore", { diretorio: caminhoProjeto }));
  }

  async function criarArquivo(caminhoRelativo: string) {
    try {
      await invoke("cmd_criar_arquivo", { caminhoProjeto, caminhoRelativo });
      await recarregarArvore();
      if (/\.(tex|bib|cls|sty|txt|md)$/i.test(caminhoRelativo)) await selecionarArquivo(comBarra(caminhoRelativo));
    } catch (e) {
      alert(`Não foi possível criar o arquivo: ${e}`);
    }
  }

  async function criarPasta(caminhoRelativo: string) {
    try {
      await invoke("cmd_criar_pasta", { caminhoProjeto, caminhoRelativo });
      await recarregarArvore();
    } catch (e) {
      alert(`Não foi possível criar a pasta: ${e}`);
    }
  }

  async function enviarArquivos(pastaRelativa = '') {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const escolhidos = await open({ multiple: true, title: "Enviar arquivos para o projeto" });
      if (!escolhidos) return;
      const origens = Array.isArray(escolhidos) ? escolhidos : [escolhidos];
      await comTarefa(t('tarefa.enviando-arquivos'), async (atualizar) => {
        for (const [i, origem] of origens.entries()) {
          atualizar(i, origens.length);
          await invoke("cmd_importar_arquivo", { caminhoProjeto, origem, pastaRelativa });
        }
        atualizar(origens.length, origens.length);
      });
      await recarregarArvore();
    } catch (e) {
      alert(`Não foi possível enviar os arquivos: ${e}`);
    }
  }

  function trocarPrefixo(caminho: string, antigo: string, novo: string): string {
    if (caminho === antigo) return novo;
    return caminho.startsWith(antigo + '/') ? novo + caminho.slice(antigo.length) : caminho;
  }

  /**
   * Renomear (e mover): as abas abertas acompanham. Antes salva as abas sujas do que vai mudar, e se
   * alguma nao salvar nao renomeia; o caminho das abas so troca depois que o comando confirma.
   */
  function renomearItem(caminho: string, novoNome: string): Promise<void> {
    return comTrava(async () => {
      for (const c of [...sujas].filter((c) => dentroDe(c, caminho))) {
        if (!(await salvar(c, true))) return;
      }
      try {
        const novo: string = await invoke("cmd_renomear_item", { caminhoProjeto, caminhoRelativo: caminho, novoNome });
        // as chaves do Map e das abas sujas trocam junto com a lista de abas
        renomearDados(caminho, novo);
        mudarAbas(renomearPrefixo(fotoDasAbas(), caminho, novo));
        // o indice do projeto precisa do nome novo do arquivo aberto; o nome antigo sai dele
        if (arquivoAtual) definirArquivoAberto(arquivoAtual);
        esquecerArquivo(caminho);
        arquivosConteudo = Object.fromEntries(
          Object.entries(arquivosConteudo).map(([k, v]) => [trocarPrefixo(k, caminho, novo), v])
        );
        revisoesPorArquivo = Object.fromEntries(
          Object.entries(revisoesPorArquivo).map(([k, v]) => [trocarPrefixo(k, caminho, novo), v])
        );
        await recarregarArvore();
      } catch (e) {
        alert(`Não foi possível renomear: ${e}`);
      }
    });
  }

  /** Apagar: as abas do que sumiu fecham sem salvar (o arquivo nao existe mais) e a vizinha assume. */
  function excluirItem(caminho: string): Promise<void> {
    return comTrava(async () => {
      const afetada = (c: string) => dentroDe(c, caminho);
      // ate a lista de abas fechar, nenhum salvamento (timer, compilar, trocar de aba) pode gravar no que esta sendo apagado
      apagando.push(caminho);
      try {
        for (const c of [...timerSalvar.keys()]) if (afetada(c)) cancelarTimerSalvar(c);
        const sujasAfetadas = [...sujas].filter(afetada);
        for (const c of sujasAfetadas) marcarSuja(c, false);
        // as outras abas sujas salvam antes
        await salvarTodas();
        // gravacao em voo do que vai ser apagado (IA, Drive, salvamento automatico): termina antes, senao recriaria o arquivo
        await esperar(afetada);
        try {
          await invoke("cmd_excluir_item",{ caminhoProjeto, caminhoRelativo: caminho });
        } catch (e) {
          // nao apagou: as abas continuam abertas e com a edicao ainda por salvar
          for (const c of sujasAfetadas) marcarSuja(c, true);
          alert(`Não foi possível excluir: ${e}`);
          return;
        }
        const ativaFoiExcluida = afetada(abasEstado.ativa);
        removerDados(caminho);
        arquivosConteudo = Object.fromEntries(
          Object.entries(arquivosConteudo).filter(([k]) => !afetada(k))
        );
        for (const k of Object.keys(revisoesPorArquivo)) if (afetada(k)) delete revisoesPorArquivo[k];
        const restantes = removerPrefixo(fotoDasAbas(), caminho);
        try {
          await recarregarArvore();
        } catch (e) {
          console.error('Erro ao listar arvore:', e);
        }
        if (!ativaFoiExcluida) {
          mudarAbas(restantes);
          return;
        }
        // a vizinha assume; sem vizinha que abra, abre o principal do projeto, se houver
        await mostrarVizinha(restantes, encontrarMainTex(arquivosProjeto) ?? '');
      } finally {
        apagando = apagando.filter((p) => p !== caminho);
        // texto digitado durante a espera nao tem para onde ir: o arquivo nao existe mais
        for (const c of [...timerSalvar.keys()]) if (afetada(c)) cancelarTimerSalvar(c);
      }
    });
  }

  async function baixarItem(caminho: string) {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const destino = await save({ title: "Baixar arquivo", defaultPath: caminho.split('/').pop() ?? 'arquivo' });
      if (destino) await comTarefa(t('tarefa.baixando'), () => invoke("cmd_copiar_para", { caminhoProjeto, caminhoRelativo: caminho, destino }));
    } catch (e) {
      alert(`Não foi possível baixar: ${e}`);
    }
  }

  async function copiarEsteProjeto() {
    await salvarTodas();
    try {
      const novo: string = await invoke("cmd_copiar_projeto", { caminho: caminhoProjeto });
      await entrarComBarra(novo, novo.split(/[\\/]/).pop() || novo);
    } catch (e) {
      alert(`Não foi possível copiar o projeto: ${e}`);
    }
  }

  async function limparAuxiliares() {
    if (!caminhoProjeto || compilando) return;
    try {
      const apagados: number = await invoke("cmd_limpar_auxiliares", { caminhoProjeto });
      logTexto = tf("projeto.auxiliares-apagados", { quantidade: apagados });
    } catch (e) {
      logTexto = tf("projeto.auxiliares-erro", { erro: String(e) });
    }
    await recarregarArvore();
  }

  async function recompilarDoZero() {
    if (!caminhoProjeto || compilando) return;
    try {
      await invoke("cmd_limpar_auxiliares", { caminhoProjeto });
    } catch (e) {
      logTexto = `Erro ao limpar os arquivos auxiliares: ${e}`;
    }
    await compilar();
  }

  async function irParaPdf() {
    if (!caminhoPdf || !arquivoAtual) return;
    try {
      destaquePdf = await invoke("cmd_synctex_ir_pdf", {
        pdf: caminhoPdf,
        arquivoTex: absolutoDoProjeto(arquivoAtual),
        linha,
        coluna,
      });
    } catch (e) {
      logTexto = String(e);
    }
  }

  async function irParaCodigo(pagina: number, x: number, y: number) {
    if (!caminhoPdf) return;
    try {
      const pos: { arquivo: string; linha: number } | null = await invoke("cmd_synctex_ir_codigo", { pdf: caminhoPdf, pagina, x, y });
      const rel = pos ? relativoDoProjeto(pos.arquivo) : null;
      if (!pos || !rel) return;
      if (rel !== arquivoAtual) await selecionarArquivo(rel);
      await tick();
      editorAtivo?.irParaLinha(pos.linha);
    } catch (e) {
      logTexto = String(e);
    }
  }

  const FIGURA = blocoFigura();

  function tabelaLatex(tamanho = '3x3'): string {
    const [l, c] = tamanho.split('x').map((n) => Math.max(1, Math.min(20, parseInt(n, 10) || 3)));
    const linha = '    ' + Array(c).fill(' ').join('&') + ' \\\\';
    return [
      '\\begin{table}[h]',
      '  \\centering',
      `  \\begin{tabular}{${'c'.repeat(c)}}`,
      ...Array(l).fill(linha),
      '  \\end{tabular}',
      '  \\caption{}',
      '  \\label{tab:}',
      '\\end{table}',
    ].join('\n');
  }

  // Inserir > Figura > Do computador (Windows) / Do aparelho (Android): copia a figura para
  // figuras/ do projeto (nome repetido vira "nome (2).ext") e insere o bloco no cursor
  async function figuraDoComputador() {
    if (!caminhoProjeto) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const escolhida = await open({
        multiple: false,
        title: t('figura.escolher'),
        filters: [{ name: t('figura.filtro'), extensions: EXTENSOES_FIGURA }],
      });
      if (!escolhida) return;
      const origem = Array.isArray(escolhida) ? escolhida[0] : escolhida;
      const relativo = await invoke<string>('cmd_importar_figura', { caminhoProjeto, origem });
      editorAtivo?.inserirBloco(blocoFigura(caminhoDeFigura(relativo)));
      await recarregarArvore();
    } catch (e) {
      alert(t('figura.erro-inserir').replace('{erro}', String(e)));
    }
  }

  // Arquivo > Salvar no Google Drive (e o icone da nuvem na lista de projetos): entra com o Google se
  // precisar, salva as abas, cria o projeto no Drive e liga a sincronizacao. Projeto que ja esta no
  // Drive so sincroniza agora.
  let salvandoNoDrive = false;
  async function salvarProjetoNoDrive(caminho: string) {
    if (!caminho || salvandoNoDrive) return;
    salvandoNoDrive = true;
    try {
      const info = await invoke<{ conectado: boolean }>('cmd_drive_info');
      if (!info.conectado) await entrarComGoogle();
      const noEditor = caminho === caminhoProjeto && modo === 'editor';
      if (noEditor && !(await salvarTodas())) return;
      if (noEditor && driveAtivo) {
        await sincronizarDrive();
      } else {
        await comTarefa(t('drive.salvando-no-drive'), () => invoke<string>('cmd_drive_enviar_projeto', { caminho }));
        if (noEditor) {
          driveAtivo = true;
          estadoDrive = 'Sincronizado';
        }
      }
      alert(t('drive.salvo-no-drive'));
    } catch (e) {
      alert(t('drive.erro-salvar-no-drive').replace('{erro}', textoErro(e)));
    } finally {
      salvandoNoDrive = false;
    }
  }

  // Inserir > Figura > Do Google Drive: seletor do Google com uma figura, baixada para figuras/
  async function figuraDoDrive() {
    if (!caminhoProjeto) return;
    try {
      const info = await invoke<{ conectado: boolean }>('cmd_drive_info');
      if (!info.conectado) await entrarComGoogle();
      const relativo = await invoke<string | null>('cmd_drive_abrir_figura', { caminho: caminhoProjeto });
      if (!relativo) return;
      editorAtivo?.inserirBloco(blocoFigura(caminhoDeFigura(relativo)));
      await recarregarArvore();
      await conferirDrive(caminhoProjeto);
    } catch (e) {
      alert(t('figura.erro-inserir').replace('{erro}', textoErro(e)));
    }
  }

  const acoes = $derived<Acoes>({
    'desfazer': () => editorAtivo?.desfazer(),
    'refazer': () => editorAtivo?.refazer(),
    'localizar': () => editorAtivo?.buscar(),
    'buscar-substituir': () => editorAtivo?.buscar(),
    'limpar-auxiliares': limparAuxiliares,
    'modo-codigo': () => mudarVisual(false),
    'modo-visual': () => mudarVisual(true),
    'modo-editando': () => (modoEdicao = 'editando'),
    'modo-revisando': () => (modoEdicao = 'revisando'),
    'modo-visualizando': () => (modoEdicao = 'visualizando'),
    'aceitar-todas': () => editorAtivo?.aceitarTodas(),
    'rejeitar-todas': () => editorAtivo?.rejeitarTodas(),
    'selecionar-tudo': () => editorAtivo?.selecionarTudo(),
    'comentario': () => editorAtivo?.comentar(),
    'negrito': () => editorAtivo?.envolver('\\textbf{', '}'),
    'italico': () => editorAtivo?.envolver('\\textit{', '}'),
    'matematica-linha': () => editorAtivo?.envolver('\\(', '\\)'),
    'matematica-destaque': () => editorAtivo?.envolver('\\[', '\\]'),
    // Inserir > Matematica: equation, align, gather, cases, matrizes, integral e fracao
    ...Object.fromEntries(IDS_AMBIENTES.map((id) => [`ambiente-${id}`, () => editorAtivo?.inserirAmbiente(id)])),
    'link': () => editorAtivo?.inserir('\\href{https://}{}'),
    'referencia': () => editorAtivo?.inserir('\\ref{}'),
    'citacao': () => editorAtivo?.inserir('\\cite{}'),
    'figura-projeto': () => editorAtivo?.inserirBloco(FIGURA),
    'figura-computador': figuraDoComputador,
    'figura-drive': figuraDoDrive,
    'tabela': (tamanho) => editorAtivo?.inserir(tabelaLatex(tamanho)),
    'tabela-assistente': (tamanho) => (assistenteTabela = { visivel: true, tamanho: tamanho ?? '3x3' }),
    'tabela-planilha': () => (assistenteTabela = { visivel: true, tamanho: '3x3' }),
    'lista-marcadores': () => editorAtivo?.inserirLista('itemize'),
    'lista-numerada': () => editorAtivo?.inserirLista('enumerate'),
    'aumentar-recuo': () => editorAtivo?.aumentarRecuo(),
    'diminuir-recuo': () => editorAtivo?.diminuirRecuo(),
    'estilo-normal': () => editorAtivo?.definirEstilo('normal'),
    'estilo-secao': () => editorAtivo?.definirEstilo('secao'),
    'estilo-subsecao': () => editorAtivo?.definirEstilo('subsecao'),
    'estilo-subsubsecao': () => editorAtivo?.definirEstilo('subsubsecao'),
    'estilo-paragrafo': () => editorAtivo?.definirEstilo('paragrafo'),
    'estilo-subparagrafo': () => editorAtivo?.definirEstilo('subparagrafo'),
    'configuracoes': () => abrirConfiguracoes(),
    'contar-palavras': abrirContagem,
    'versoes-anteriores': async () => {
      if (!arquivoAtual) return;
      // a lista guarda o que esta no disco: se a gravacao falhou, abrir mostraria uma versao velha como se fosse a atual
      if (!(await salvar(arquivoAtual, true))) return;
      versoesSugestoes = editorAtivo?.obterRevisoes().length ?? 0;
      versoesVisivel = true;
    },
    'proxima-aba': () => void andarAba(1),
    'aba-anterior': () => void andarAba(-1),
    'fechar-aba': () => { if (abasEstado.ativa) void fecharAba(abasEstado.ativa); },
    'enviar-arquivo': () => enviarArquivos(''),
    'drive-adicionar-arquivos': () => (adicionarDriveVisivel = true),
    'salvar-no-drive': () => void salvarProjetoNoDrive(caminhoProjeto),
    'copiar-projeto': copiarEsteProjeto,
    'baixar-zip': () => baixarZip([caminhoProjeto]),
    'traduzir': abrirTraducao,
    'exportar-docx': () => exportarDocumento('docx'),
    'exportar-md': () => exportarDocumento('md'),
    'exportar-html': () => exportarDocumento('html'),
    ...(pdfDados ? { 'baixar-pdf': baixarPdf } : {}),
  });

  // ---------- traducao (Google Tradutor, painel TraducaoPainel) ----------
  let textoTraducao = $state<string | null>(null);
  let intervaloTraducao = $state<{ from: number; to: number; texto: string } | null>(null);

  function abrirTraducao() {
    const sel = editorAtivo?.selecaoIntervalo();
    if (!sel || !sel.texto.trim()) {
      alert('Selecione no editor o texto que quer traduzir.');
      return;
    }
    intervaloTraducao = sel;
    textoTraducao = sel.texto;
  }

  /** Texto selecionado no PDF: traduz, mas nao ha onde substituir. */
  function traduzirDoPdf(texto: string) {
    intervaloTraducao = null;
    textoTraducao = texto;
  }

  function substituirPelaTraducao(traducao: string) {
    const sel = intervaloTraducao;
    textoTraducao = null;
    if (!sel) return;
    if (!editorAtivo?.trocarIntervalo(sel.from, sel.to, sel.texto, traducao)) {
      void navigator.clipboard.writeText(traducao).catch(() => {});
      alert('O texto mudou enquanto o tradutor estava aberto. A tradução foi copiada; cole com Ctrl+V.');
    }
  }

  /** Icone de download do PDF: o mesmo "Salvar como" do menu Arquivo. */
  function salvarComoFormato(formato: string) {
    if (formato === 'pdf') void baixarPdf();
    else if (formato === 'zip') void baixarZip([caminhoProjeto]);
    else if (formato === 'docx' || formato === 'md' || formato === 'html') void exportarDocumento(formato);
  }

  const FORMATOS_EXPORTACAO = {
    docx: { nome: 'Documento do Word', ext: 'docx' },
    md: { nome: 'Markdown', ext: 'md' },
    html: { nome: 'Página da web', ext: 'html' },
  } as const;

  /** Word a partir do PDF compilado (mesmo texto, fonte e paginas); Markdown e HTML pelo Pandoc. */
  async function exportarDocumento(formato: keyof typeof FORMATOS_EXPORTACAO) {
    const f = FORMATOS_EXPORTACAO[formato];
    try {
      await salvarTodas();
      const { save } = await import("@tauri-apps/plugin-dialog");
      const destino = await save({
        title: t('exportar.titulo').replace('{formato}', f.nome),
        defaultPath: `${nomeProjeto || 'documento'}.${f.ext}`,
        filters: [{ name: f.nome, extensions: [f.ext] }],
      });
      if (!destino) return;
      if (formato === 'docx') {
        // o Word sai do PDF: compila antes, para o arquivo ter o texto de agora
        while (compilando) await new Promise((r) => setTimeout(r, 200));
        await compilar();
        if (!pdfDados || statusCompilacao === 'erro') {
          alert(t('exportar.sem-pdf'));
          return;
        }
        const anterior = logTexto;
        logTexto = t('exportar.gerando-word');
        try {
          const tex = await invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: documentoPrincipal() }).catch(() => '');
          const { gerarWord } = await import('$lib/salvar-word');
          const pasta = documentoPrincipal().includes('/') ? documentoPrincipal().replace(/[^/]*$/, '') : '';
          const dados = await gerarWord(
            pdfDados,
            tex,
            (p, total) => {
              logTexto = t('exportar.gerando-word-pagina').replace('{p}', String(p)).replace('{total}', String(total));
            },
            (caminho) => invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: pasta + caminho }),
          );
          await invoke('cmd_gravar_bytes', { destino, dados });
        } finally {
          logTexto = anterior;
        }
      } else {
        await comTarefa(t('tarefa.exportando'), () => invoke('cmd_exportar_documento', {
          diretorio: caminhoProjeto,
          arquivo: documentoPrincipal(),
          formato,
          destino,
        }));
      }
      const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
      await revealItemInDir(destino).catch(() => {});
    } catch (e) {
      alert(`Não foi possível exportar: ${e}`);
    }
  }

  /** Abre a janela "Salvar como" do Windows e copia o PDF compilado para onde a pessoa escolher. */
  async function baixarPdf() {
    if (!pdfDados || !caminhoPdf) return;
    const normalizar = (c: string) => c.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
    const base = normalizar(caminhoProjeto);
    const pdf = caminhoPdf.replace(/\\/g, '/');
    const relativo = normalizar(pdf).startsWith(base + '/') ? pdf.slice(base.length + 1) : '';
    if (!relativo) {
      alert('O PDF compilado não está na pasta do projeto.');
      return;
    }
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const destino = await save({
        title: "Salvar PDF como",
        defaultPath: `${nomeProjeto || 'documento'}.pdf`,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (destino) {
        await comTarefa(t('tarefa.salvando-pdf'), () => invoke("cmd_copiar_para", { caminhoProjeto, caminhoRelativo: relativo.replace(/\\/g, '/'), destino }));
      }
    } catch (e) {
      alert(`Não foi possível salvar o PDF: ${e}`);
    }
  }

  // ---------- agente de IA: acesso ao projeto aberto ----------
  // O agente le pelo editor (arquivo aberto), pelo cache de arquivos ja abertos ou pelo disco.
  // Grava so quando a pessoa aceita uma proposta.
  const normalizarCaminho = (c: string) => c.replace(/\\/g, '/').replace(/^\/+/, '');
  const ehAtual = (c: string) => !!arquivoAtual && normalizarCaminho(c) === normalizarCaminho(arquivoAtual);
  const chaveCache = (c: string) =>
    Object.keys(arquivosConteudo).find((k) => normalizarCaminho(k) === normalizarCaminho(c));

  /** Aba de texto aberta para este caminho (o agente manda com ou sem a barra do inicio). */
  const abaDoAgente = (caminho: string) =>
    abasEstado.abas.find((a) => a.tipo === 'texto' && normalizarCaminho(a.caminho) === normalizarCaminho(caminho));

  async function lerParaAgente(caminho: string): Promise<string> {
    if (ehAtual(caminho)) return editorAtivo?.obterConteudo() ?? conteudoEditor;
    // aba aberta em segundo plano: a base da edicao e o texto dela, que pode ter digitacao ainda nao salva
    const aba = abaDoAgente(caminho);
    const daAba = aba ? dadosDasAbas.get(aba.caminho)?.estado.doc.toString() : undefined;
    if (daAba !== undefined) return daAba;
    const chave = chaveCache(caminho);
    if (chave !== undefined) return arquivosConteudo[chave];
    return await invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: normalizarCaminho(caminho) });
  }

  async function gravarParaAgente(caminho: string, texto: string) {
    const relativo = normalizarCaminho(caminho);
    const projeto = caminhoProjeto;
    // mesma fila do salvar: a IA e o salvamento da aba nunca gravam o mesmo arquivo juntos
    await naFila(comBarra(relativo), async () => {
      // a IA sobrescreve o arquivo: guarda antes o que esta no disco, mesmo dentro do intervalo de 5 min
      await invoke('cmd_versoes_gravar', { caminhoProjeto: projeto, caminhoRelativo: relativo, intervaloS: 0 }).catch(() => {});
      await invoke('cmd_salvar_arquivo', { caminhoProjeto: projeto, caminhoRelativo: relativo, conteudo: texto });
    });
    const chave = chaveCache(caminho);
    if (chave !== undefined) arquivosConteudo[chave] = texto;
    atualizarArquivo(chave ?? comBarra(normalizarCaminho(caminho)), texto);
    // aba aberta em segundo plano guarda o texto antigo: acompanha o que foi gravado, senao o proximo
    // salvamento dela desfaria a edicao da IA
    const aba = abaDoAgente(caminho);
    if (aba && aba.caminho !== arquivoAtual) acompanharDiscoNaAba(aba.caminho, texto);
  }

  const projetoAgente: ProjetoAgente = {
    arquivoAtual: () => normalizarCaminho(arquivoAtual),
    arvore: () => arquivosProjeto as unknown as NoArquivo[],
    lerArquivo: lerParaAgente,
    selecao: () => editorAtivo?.obterSelecao() ?? null,
    erros: () => erros,
    logTexto: () => logTexto,
    pasta: () => caminhoProjeto,
    inserirNoCursor: (t) => editorAtivo?.inserirTexto(t),
    async aplicarEdicao(caminho, original, novo) {
      if (ehAtual(caminho)) {
        if (!editorAtivo?.substituirTrecho(original, novo)) {
          throw new Error('O trecho mudou desde a proposta. Peca a IA para ler o arquivo de novo.');
        }
        return;
      }
      const texto = await lerParaAgente(caminho);
      const pos = texto.indexOf(original);
      if (pos < 0 || texto.indexOf(original, pos + 1) >= 0) {
        throw new Error('O trecho mudou desde a proposta. Peca a IA para ler o arquivo de novo.');
      }
      await gravarParaAgente(caminho, texto.slice(0, pos) + novo + texto.slice(pos + original.length));
    },
    async inserirNaLinha(caminho, linha, conteudo) {
      if (ehAtual(caminho)) {
        editorAtivo?.inserirNaLinha(linha, conteudo);
        return;
      }
      const linhas = (await lerParaAgente(caminho)).split('\n');
      const pos = Math.max(0, Math.min(linha - 1, linhas.length));
      linhas.splice(pos, 0, ...conteudo.replace(/\n$/, '').split('\n'));
      await gravarParaAgente(caminho, linhas.join('\n'));
    },
    async criarArquivo(caminho, conteudo) {
      // a leitura falha tambem em arquivo que existe mas nao e texto (png, pdf): a arvore
      // do projeto, recarregada agora, e quem diz se ele existe
      await recarregarArvore();
      const existe = existeNaArvore(arquivosProjeto, comBarra(normalizarCaminho(caminho)))
        || (await invoke<string>('cmd_ler_arquivo', { caminhoProjeto, caminhoRelativo: normalizarCaminho(caminho) })
          .then(() => true, () => false));
      if (existe) throw new Error('Esse arquivo já existe.');
      await invoke('cmd_salvar_arquivo', { caminhoProjeto, caminhoRelativo: normalizarCaminho(caminho), conteudo });
      await recarregarArvore();
    },
  };

  async function carregarConfigIA() {
    try {
      configIA = await invoke("cmd_ia_carregar_config");
    } catch (e) {
      console.error("Erro ao carregar config IA:", e);
    }
  }

  carregarInicio();
  carregarConfigIA();
</script>

<svelte:window onclick={() => (menuNovoAberto = false)} />

<Abertura />

<FeedbackModal />

{#if modo === 'inicio'}
  {#if pixInicioVisivel}
    <PixModal aoFechar={() => (pixInicioVisivel = false)} />
  {/if}
  <div class="inicio">
    <aside class="lateral">
      <div class="marca">
        <img src="/curupira-logo.png" alt="ÉguaTex" width="46" height="46" />
        <div>
          <div class="marca-nome">ÉguaTex</div>
          <div class="marca-sub">{t('app.descricao')}</div>
        </div>
      </div>

      <div class="novo">
        <button
          class="btn-novo"
          onclick={(e) => { e.stopPropagation(); menuNovoAberto = !menuNovoAberto; }}
        >
          <Plus size={16} />
          <span>Novo projeto</span>
          <ChevronDown size={14} />
        </button>
        {#if menuNovoAberto}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="menu" onclick={(e) => e.stopPropagation()}>
            {#each modelos as modelo}
              <button class="menu-item" onclick={() => escolherModelo(modelo)}>
                <span class="menu-nome">{modelo.nome}</span>
                <span class="menu-desc">{modelo.descricao}</span>
              </button>
            {/each}
            <div class="menu-sep"></div>
            <button class="menu-item" onclick={abrirPasta}>
              <span class="menu-nome"><FolderOpen size={14} /> Abrir pasta existente</span>
              <span class="menu-desc">Uma pasta com os arquivos .tex do seu trabalho</span>
            </button>
            <button class="menu-item" onclick={() => { menuNovoAberto = false; abrirDriveVisivel = true; }}>
              <span class="menu-nome"><IconeDrive size={14} /> {t('drive.abrir-botao')}</span>
              <span class="menu-desc">{t('drive.abrir-aviso')}</span>
            </button>
            <button class="menu-item" onclick={abrirZip}>
              <span class="menu-nome"><FileArchive size={14} /> Abrir arquivo .zip</span>
              <span class="menu-desc">Um projeto LaTeX compactado. Extrai em Documentos\EguaTex</span>
            </button>
          </div>
        {/if}
      </div>

      <nav class="nav">
        {#each FILTROS as f}
          <button
            class="nav-item"
            class:ativo={filtro === f.id}
            onclick={() => { filtro = f.id; selecionados = []; }}
          >
            <f.icone size={17} />
            <span>{f.nome}</span>
          </button>
        {/each}
      </nav>

      <div class="lateral-rodape">
        <CartoesApoio onfeedback={() => void enviarFeedback()} onapoiar={() => (pixInicioVisivel = true)} />
        <ContaUsuario />
        <BotaoTema comTexto />
        <button class="nav-item" onclick={() => openUrl(MAILTO_CONTATO)} title="Enviar e-mail">
          <Mail size={17} />
          <span>Contato: {EMAIL_CONTATO}</span>
        </button>
        <div class="versao">
          {versaoRust} | AGPL-3.0 | José Paulo de Almeida Junior e Jéssica Lopes Melo
          (<button class="link-perfil" onclick={() => openUrl(SITE)}>site</button>)
        </div>
      </div>
    </aside>

    <main class="principal">
      {#if semTex}
        <div class="aviso aviso-tex">
          <Info size={18} />
          <span>Nenhum compilador LaTeX foi encontrado neste computador. Sem ele você edita, mas não gera o PDF.</span>
          <button class="aviso-link" onclick={() => openUrl(SITE_INSTALAR_TEX)}>Como instalar</button>
        </div>
      {/if}
      {#if avisoVisivel}
        <div class="aviso">
          <Info size={18} />
          <span>Tudo funciona sem internet. Seus projetos ficam em <strong>{pastaProjetos || 'Documentos\\EguaTex'}</strong>, como pastas comuns.</span>
          <button class="aviso-fechar" onclick={fecharAviso} aria-label="Fechar aviso"><X size={16} /></button>
        </div>
      {/if}

      <h1 class="titulo">{filtroAtual.titulo}</h1>

      <div class="barra">
        <label class="busca">
          <Search size={16} />
          <input type="search" placeholder="Buscar nos projetos..." bind:value={busca} />
        </label>

        {#if selecionados.length > 0}
          <div class="selecao">
            <span class="selecao-conta">{selecionados.length} {selecionados.length === 1 ? 'selecionado' : 'selecionados'}</span>
            {#if filtro !== 'lixeira'}
              <button class="btn-sel" onclick={() => baixarZip(selecionados)}><Download size={15} /> Baixar</button>
            {/if}
            {#if filtro === 'todos'}
              <button class="btn-sel" onclick={() => mudarEstado(selecionados, 'arquivado')}><Archive size={15} /> Arquivar</button>
            {:else}
              <button class="btn-sel" onclick={() => mudarEstado(selecionados, 'ativo')}><RotateCcw size={15} /> Restaurar</button>
            {/if}
            {#if filtro === 'lixeira'}
              <button class="btn-sel perigo" onclick={() => tirarDaLista(selecionados)}><X size={15} /> Tirar da lista</button>
            {:else}
              <button class="btn-sel" onclick={() => mudarEstado(selecionados, 'lixeira')}><Trash2 size={15} /> Lixeira</button>
            {/if}
          </div>
        {/if}
      </div>

      {#if erroInicio}
        <div class="erro">{erroInicio}</div>
      {/if}

      <div class="tabela">
        <div class="linha cabecalho">
          <span class="celula-marcar">
            <input
              type="checkbox"
              class="marcar"
              checked={todosSelecionados}
              disabled={recentesFiltrados.length === 0}
              onchange={alternarTodos}
              aria-label={t('lista.selecionar-todos')}
              title={t('lista.selecionar-todos')}
            />
          </span>
          <span class="col-titulo">{t('lista.titulo')}</span>
          <span class="col-local">{t('lista.local')}</span>
          <span class="col-data">
            <button
              type="button"
              class="ordenar"
              title={ordemData === 'desc' ? t('lista.ordem-desc') : t('lista.ordem-asc')}
              onclick={() => (ordemData = ordemData === 'desc' ? 'asc' : 'desc')}
            >
              {t('lista.modificacao')}
              {#if ordemData === 'desc'}<ArrowDown size={13} />{:else}<ArrowUp size={13} />{/if}
            </button>
          </span>
          <span class="col-acoes"><span class="so-leitor">{t('lista.acoes')}</span></span>
        </div>
        {#each recentesFiltrados as projeto (projeto.caminho)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div
            class="linha projeto"
            class:selecionada={selecionados.includes(projeto.caminho)}
            onclick={() => entrarComBarra(projeto.caminho, projeto.nome)}
          >
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <span class="celula-marcar" onclick={(e) => e.stopPropagation()}>
              <input
                type="checkbox"
                class="marcar"
                checked={selecionados.includes(projeto.caminho)}
                onchange={() => alternarSelecao(projeto.caminho)}
                aria-label={tf('lista.selecionar', { nome: projeto.nome })}
              />
            </span>
            <span class="projeto-nome" title={projeto.nome}>{projeto.nome}</span>
            <span class="projeto-local" title={pastaDoProjeto(projeto.caminho)}>{localCurto(projeto.caminho)}</span>
            <span class="projeto-data" title={new Date(dataDoProjeto(projeto)).toLocaleString('pt-BR')}>{tempoRelativo(dataDoProjeto(projeto), agoraLista)}</span>
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <span class="acoes" onclick={(e) => e.stopPropagation()}>
              {#if filtro === 'lixeira'}
                <button class="acao" title="Restaurar" onclick={() => mudarEstado([projeto.caminho], 'ativo')}><RotateCcw size={15} /></button>
                <button class="acao perigo" title="Tirar da lista (a pasta continua no disco)" onclick={() => tirarDaLista([projeto.caminho])}><X size={15} /></button>
              {:else}
                <button class="acao" title="Fazer uma cópia" onclick={() => copiarProjeto(projeto)}><Copy size={15} /></button>
                <button class="acao" title="Baixar .zip" onclick={() => baixarZip([projeto.caminho])}><Download size={15} /></button>
                <button class="acao" title={t("drive.salvar-no-drive")} aria-label={t("drive.salvar-no-drive")} onclick={() => salvarProjetoNoDrive(projeto.caminho)}><CloudUpload size={15} /></button>
                {#if filtro === 'arquivados'}
                  <button class="acao" title="Desarquivar" onclick={() => mudarEstado([projeto.caminho], 'ativo')}><ArchiveRestore size={15} /></button>
                {:else}
                  <button class="acao" title="Arquivar" onclick={() => mudarEstado([projeto.caminho], 'arquivado')}><Archive size={15} /></button>
                {/if}
                <button class="acao" title="Mover para a lixeira" onclick={() => mudarEstado([projeto.caminho], 'lixeira')}><Trash2 size={15} /></button>
              {/if}
            </span>
          </div>
        {:else}
          <div class="vazio">
            {#if busca}
              Nenhum projeto com "{busca}".
            {:else if filtro === 'arquivados'}
              Nenhum projeto arquivado.
            {:else if filtro === 'lixeira'}
              A lixeira está vazia.
            {:else}
              Nenhum projeto ainda. Clique em <strong>Novo projeto</strong> para começar.
            {/if}
          </div>
        {/each}
      </div>

      {#if recentesFiltrados.length > 0}
        <p class="contagem">{textoMostrando(recentesFiltrados.length)}</p>
      {/if}
    </main>
  </div>

  {#if modeloEscolhido}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="fundo-modal" onclick={() => (modeloEscolhido = null)}>
      <form class="modal" onclick={(e) => e.stopPropagation()} onsubmit={(e) => { e.preventDefault(); criarProjeto(); }}>
        <h2>Novo projeto</h2>
        <p class="modal-modelo">{modeloEscolhido.nome}</p>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="modal-input" bind:value={nomeNovo} placeholder="Nome do projeto" autofocus />
        <p class="modal-destino">Pasta: {pastaProjetos}\{nomeDePasta(nomeNovo)}</p>
        {#if erroInicio}<p class="modal-erro">{erroInicio}</p>{/if}
        <div class="modal-acoes">
          <button type="button" class="btn-sec" onclick={() => (modeloEscolhido = null)}>Cancelar</button>
          <button type="submit" class="btn-criar" disabled={criando || !nomeNovo.trim()}>
            {criando ? 'Criando...' : 'Criar'}
          </button>
        </div>
      </form>
    </div>
  {/if}
{:else}
  {#snippet visualizadorImagem()}
    {#if abaAtiva?.tipo === 'imagem'}
      {#key abaAtiva.caminho}
        <VisualizadorImagem {caminhoProjeto} caminho={abaAtiva.caminho} onbaixar={baixarItem} />
      {/key}
    {/if}
  {/snippet}
  <Layout
    bind:this={layoutRef}
    oncompartilhar={() => (compartilharVisivel = true)}
    {estadoDrive}
    {detalheDrive}
    agente={projetoAgente}
    arquivos={arquivosProjeto}
    {arquivoAtual}
    {linha}
    {coluna}
    {statusCompilacao}
    {motorDetectado}
    {synctexDisponivel}
    ortografia={estadoOrtografia}
    destaque={destaquePdf}
    onpararcompilacao={pararCompilacao}
    onrecompilardozero={recompilarDoZero}
    onirparapdf={irParaPdf}
    onirparacodigo={irParaCodigo}
    {compilando}
    {logTexto}
    {tempoMs}
    {erros}
    {pdfDados}
    iaAtiva={configIA.ativa}
    {configIA}
    oncompilar={() => compilar()}
    onsalvar={() => void salvar()}
    oninicio={voltarAoInicio}
    onselecionararquivo={selecionarArquivo}
    onrenomear={renomearProjeto}
    {caminhoProjeto}
    onirparalinha={(n) => editorAtivo?.irParaLinha(n)}
    onconfigurar={() => abrirConfiguracoes()}
    onconfigurarortografia={() => abrirConfiguracoes('editor')}
    onconfigIA={(cfg) => (configIA = cfg)}
    oncriararquivo={criarArquivo}
    oncriarpasta={criarPasta}
    onenviararquivo={enviarArquivos}
    onrenomearitem={renomearItem}
    onexcluiritem={excluirItem}
    onbaixaritem={baixarItem}
    onatualizararquivos={() => void recarregarArvore()}
    oninserirtexto={(t) => editorAtivo?.inserir(t)}
    {nomeProjeto}
    conteudo={conteudoEditor}
    abas={abasEstado.abas}
    abaAtiva={abasEstado.ativa}
    abasSujas={[...sujas]}
    abaPrincipal={comBarra(documentoPrincipal())}
    onselecionaraba={(c) => void selecionarArquivo(c)}
    onfecharaba={(c) => void fecharAba(c)}
    onfecharoutras={(c) => void fecharOutrasAbas(c)}
    onmoveraba={moverAba}
    {profundidadeLista}
    imagem={visualizadorImagem}
    {acoes}
    estilo={estiloLinha}
    onbaixar={salvarComoFormato}
    ontraduzirpdf={traduzirDoPdf}
    modosEditor={{ visual: visualAtivo, modo: modoEdicao, sugestoes: qtdSugestoes, ...historicoEditor }}
  >
    <EditorPanel
      bind:this={editorRef}
      conteudo={conteudoEditor}
      onchange={aoMudarTexto}
      oncursor={(l, c) => { linha = l; coluna = c; }}
      onsalvar={() => void salvar()}
      oncompilar={() => compilar()}
      onestilo={(e) => (estiloLinha = e)}
      onhistorico={(h) => (historicoEditor = h)}
      onlista={(n) => (profundidadeLista = n)}
      onortografia={(e) => (estadoOrtografia = e)}
      ontraduzir={() => abrirTraducao()}
      onperguntarIA={(t) => layoutRef?.perguntarIA(t)}
      iaAtiva={configIA.ativa}
      visual={visualAtivo}
      revisandoAtivo={modoEdicao === 'revisando'}
      somenteLeitura={modoEdicao === 'visualizando'}
      arquivo={arquivoAtual}
      onrevisoes={(n) => (qtdSugestoes = n)}
    />
  </Layout>
  {#if driveAcessoPerdido}
    <div class="aviso-drive" role="alert">
      <span>{t('drive.acesso-perdido')} Seus arquivos neste computador continuam aqui.</span>
      <button type="button" onclick={reabrirPeloSeletor} disabled={reabrindoSeletor}>{t('drive.abrir-de-novo')}</button>
    </div>
  {/if}
  <CompartilharModal
    visivel={compartilharVisivel}
    {caminhoProjeto}
    {nomeProjeto}
    onfechar={() => (compartilharVisivel = false)}
    onpublicado={() => { driveAtivo = true; estadoDrive = 'Sincronizado'; }}
    onsincronizar={() => void sincronizarDrive()}
  />
  <TraducaoPainel texto={textoTraducao} onfechar={() => (textoTraducao = null)} onsubstituir={intervaloTraducao ? substituirPelaTraducao : undefined} />
{/if}

<AbrirDoDriveModal
  visivel={abrirDriveVisivel}
  destino="novo"
  direto
  {pastaProjetos}
  onfechar={() => (abrirDriveVisivel = false)}
  onconcluir={(r) => { abrirDriveVisivel = false; if (r.principal_sugerido) guardarPrincipal(r.caminho, r.principal_sugerido); void entrarComBarra(r.caminho, r.caminho.split(/[\\/]/).pop() || 'Projeto'); }}
/>

<AbrirDoDriveModal
  visivel={adicionarDriveVisivel}
  destino={caminhoProjeto}
  {pastaProjetos}
  onfechar={() => (adicionarDriveVisivel = false)}
  onconcluir={async (r) => {
    adicionarDriveVisivel = false;
    await recarregarArvore();
    if (r.principal_sugerido) {
      guardarPrincipal(caminhoProjeto, r.principal_sugerido);
      await selecionarArquivo(comBarra(r.principal_sugerido));
    }
    await conferirDrive(caminhoProjeto);
  }}
/>

<ConfiguracoesModal
  visivel={configuracoesVisivel}
  abaInicial={configuracoesAba}
  arquivosTex={caminhosTex(arquivosProjeto)}
  onfechar={() => (configuracoesVisivel = false)}
/>

<VersoesModal
  visivel={versoesVisivel}
  {caminhoProjeto}
  arquivo={arquivoAtual}
  sugestoes={versoesSugestoes}
  onfechar={() => (versoesVisivel = false)}
  onrestaurar={aoRestaurarVersao}
/>
<ContagemPalavrasModal visivel={contagemVisivel} texto={textoContagem} onfechar={() => (contagemVisivel = false)} />

<AssistenteTabela
  visivel={assistenteTabela.visivel}
  tamanho={assistenteTabela.tamanho}
  oninserir={(codigo) => editorAtivo?.inserirTexto(codigo)}
  onfechar={() => (assistenteTabela.visivel = false)}
/>

<style>
  :root {
    --primaria: #5E1A5B;
    --primaria-hover: #7D2A79;
    --primaria-suave: #F6E8F4;
    --acento: #B26A00;
    --acento-hover: #8F5500;
    --acento-suave: #0277BD;
    --destaque: #F57C00;
    --alerta: #F9A825;
    --perigo: #C62828;
    --perigo-suave: #FFCDD2;
    --sucesso-suave: #F6E8F4;
    --alerta-suave: #FFF9C4;

    --fundo: #F4F6F9;
    --superficie-0: #FFFFFF;
    --superficie-1: #F0F2F5;
    --superficie-2: #E4E8EE;
    --borda: #DCE1E8;
    --hover: rgba(94, 26, 91, 0.10);
    --ativo: rgba(60, 60, 70, 0.22);

    --texto: #1C2430;
    --texto-secundario: #4E5A69;
    --texto-terciario: #5F6B7A;

    --info-fundo: #E8F1FB;
    --info-borda: #B9D3EF;
    --info-texto: #1A4B7A;

    --sombra-1: 0 1px 3px rgba(0, 0, 0, 0.08), 0 1px 2px rgba(0, 0, 0, 0.06);
    --sombra-2: 0 4px 12px rgba(0, 0, 0, 0.08), 0 2px 4px rgba(0, 0, 0, 0.04);
    --sombra-3: 0 8px 24px rgba(0, 0, 0, 0.12), 0 4px 8px rgba(0, 0, 0, 0.06);

    --raio: 8px;
    --raio-sm: 6px;

    font-family: "Segoe UI", Inter, system-ui, -apple-system, sans-serif;
    font-size: 16px;
    line-height: 1.5;
    color: var(--texto);
    background-color: var(--fundo);
    -webkit-font-smoothing: antialiased;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-tema="claro"])) {
      --primaria: #A8479F;
      --primaria-hover: #C266B9;
      --primaria-suave: rgba(168, 71, 159, 0.20);
      --acento: #FFB74D;
      --acento-hover: #FFCC80;
      --acento-suave: #4FC3F7;
      --destaque: #FFB74D;
      --alerta: #FFF176;
      --perigo: #EF5350;

      --fundo: #2C3645;
      --superficie-0: #1B222C;
      --superficie-1: #232C38;
      --superficie-2: #34404F;
      --borda: #3B4757;
      --hover: rgba(168, 71, 159, 0.18);
      --ativo: rgba(200, 200, 210, 0.22);

      --texto: #E8ECF1;
      --texto-secundario: #B4BDC9;
      --texto-terciario: #A3AEBC;

      --info-fundo: #1F3B5C;
      --info-borda: #2D5A8A;
      --info-texto: #D6E6FA;

      --sombra-1: 0 1px 3px rgba(0, 0, 0, 0.3);
      --sombra-2: 0 4px 12px rgba(0, 0, 0, 0.4);
      --sombra-3: 0 8px 24px rgba(0, 0, 0, 0.5);
    }
  }

  :global(:root[data-tema="escuro"]) {
    --primaria: #A8479F;
    --primaria-hover: #C266B9;
    --primaria-suave: rgba(168, 71, 159, 0.20);
    --acento: #FFB74D;
    --acento-hover: #FFCC80;
    --acento-suave: #4FC3F7;
    --destaque: #FFB74D;
    --alerta: #FFF176;
    --perigo: #EF5350;
    --perigo-suave: #FFCDD2; /* texto da barra de status: opaco, como no tema do sistema */
    --sucesso-suave: #F6E8F4;
    --alerta-suave: #FFF9C4;

    --fundo: #2C3645;
    --superficie-0: #1B222C;
    --superficie-1: #232C38;
    --superficie-2: #34404F;
    --borda: #3B4757;
    --hover: rgba(168, 71, 159, 0.18);
      --ativo: rgba(200, 200, 210, 0.22);

    --texto: #E8ECF1;
    --texto-secundario: #B4BDC9;
    --texto-terciario: #A3AEBC;

    --info-fundo: #1F3B5C;
    --info-borda: #2D5A8A;
    --info-texto: #D6E6FA;

    --sombra-1: 0 1px 3px rgba(0, 0, 0, 0.3);
    --sombra-2: 0 4px 12px rgba(0, 0, 0, 0.4);
    --sombra-3: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  :global(button),
  :global(a) {
    transition: background-color 0.12s, color 0.12s, box-shadow 0.12s, transform 0.06s;
  }

  /* clique: camada cinza por cima de qualquer cor e o botao "afunda" 1px */
  :global(button:not(:disabled):active),
  :global(a:active) {
    box-shadow: inset 0 0 0 999px var(--ativo);
    transform: translateY(1px);
  }

  :global(button:focus-visible),
  :global(a:focus-visible),
  :global(input:focus-visible),
  :global(select:focus-visible) {
    outline: 2px solid var(--primaria);
    outline-offset: 1px;
  }

  .link-perfil {
    border: none;
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .link-perfil:hover {
    color: var(--primaria);
  }

  /* a janela nunca rola inteira: cada painel tem a propria barra de rolagem */
  :global(html),
  :global(body) {
    height: 100%;
    overflow: hidden;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    background: var(--fundo);
  }

  .inicio {
    display: grid;
    grid-template-columns: 264px 1fr;
    height: 100vh;
    background: var(--superficie-0);
    color: var(--texto);
  }

  .lateral {
    display: flex;
    flex-direction: column;
    padding: 18px 12px 12px;
    background: var(--superficie-0);
    min-height: 0;
  }

  .marca {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 22px;
  }

  .marca img {
    border-radius: 8px;
  }

  .marca-nome {
    font-size: 1.35rem;
    font-weight: 700;
    color: var(--primaria);
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  .marca-sub {
    font-size: 0.72rem;
    color: var(--texto-terciario);
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 12px;
    border: none;
    border-radius: var(--raio);
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 0.92rem;
    text-align: left;
    text-decoration: none;
    cursor: pointer;
    box-sizing: border-box;
  }

  .nav-item:hover {
    background: var(--hover);
  }

  .nav-item.ativo {
    background: var(--primaria);
    color: #fff;
    font-weight: 600;
  }

  .lateral-rodape {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-top: 1px solid var(--borda);
    padding-top: 10px;
  }

  .versao {
    padding: 10px 12px 0;
    font-size: 0.68rem;
    color: var(--texto-terciario);
  }

  .principal {
    background: var(--fundo);
    border-top-left-radius: 14px;
    margin-top: 10px;
    padding: 24px 28px;
    overflow-y: auto;
    min-height: 0;
  }

  .aviso-tex {
    border-color: var(--perigo, #c62828);
  }
  .aviso-link {
    margin-left: auto;
    padding: 6px 12px;
    border: 1px solid currentColor;
    border-radius: var(--raio);
    background: transparent;
    color: inherit;
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }
  .aviso {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    margin-bottom: 20px;
    border: 1px solid var(--info-borda);
    border-radius: var(--raio);
    background: var(--info-fundo);
    color: var(--info-texto);
    font-size: 0.9rem;
  }

  .aviso span {
    flex: 1;
  }

  .aviso-fechar {
    display: flex;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
  }

  .aviso-fechar:hover {
    background: var(--hover);
  }

  .titulo {
    margin: 0 0 16px;
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .barra {
    display: flex;
    gap: 16px;
    align-items: center;
    margin-bottom: 16px;
  }

  .busca {
    flex: 1;
    max-width: 900px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto-terciario);
  }

  .busca:focus-within {
    border-color: var(--primaria);
  }

  .busca input {
    flex: 1;
    padding: 9px 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 0.92rem;
  }

  .novo {
    position: relative;
    margin-bottom: 14px;
  }

  .btn-novo {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 9px 16px;
    border: none;
    border-radius: 999px;
    background: var(--primaria);
    color: #fff;
    font: inherit;
    font-size: 0.92rem;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }

  .btn-novo:hover {
    background: var(--primaria-hover);
  }

  .menu {
    position: absolute;
    left: 0;
    top: calc(100% + 6px);
    width: 300px;
    padding: 6px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--superficie-0);
    box-shadow: var(--sombra-3);
    z-index: 20;
  }

  .menu-item {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    color: var(--texto);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .menu-item:hover {
    background: var(--hover);
  }

  .menu-nome {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.9rem;
    font-weight: 500;
  }

  .menu-desc {
    font-size: 0.75rem;
    color: var(--texto-terciario);
  }

  .menu-sep {
    height: 1px;
    margin: 6px 4px;
    background: var(--borda);
  }

  .erro {
    margin-bottom: 12px;
    padding: 10px 14px;
    border-radius: var(--raio-sm);
    background: color-mix(in srgb, var(--perigo) 14%, transparent);
    color: var(--perigo);
    font-size: 0.88rem;
  }

  /* Lista como a do Overleaf: cabecalho fixo, linhas de 48 px, so divisorias horizontais e os
     icones de acao aparecendo no hover. O titulo e o local ficam numa linha com reticencias. */
  .tabela {
    container-type: inline-size;
    border-radius: var(--raio);
    background: var(--superficie-0);
    padding: 0 12px 4px;
  }

  .linha {
    display: grid;
    grid-template-columns: 40px minmax(0, 50fr) minmax(0, 25fr) minmax(8.5rem, 20fr) 160px;
    column-gap: 12px;
    align-items: center;
    min-height: 48px;
    padding: 0 4px;
    border-bottom: 1px solid color-mix(in srgb, var(--borda) 60%, transparent);
    font-size: 0.9rem;
  }

  .linha:last-child {
    border-bottom: none;
  }

  .cabecalho {
    position: sticky;
    top: 0;
    z-index: 2;
    min-height: 48px;
    background: var(--superficie-0);
    border-bottom-color: var(--borda);
    color: var(--texto-secundario);
    font-size: 0.82rem;
    font-weight: 600;
  }

  .cabecalho > span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .celula-marcar {
    display: flex;
    align-items: center;
    justify-content: center;
    align-self: stretch;
  }

  .ordenar {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
    padding: 4px 6px;
    margin-left: -6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }

  .ordenar:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .so-leitor {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .projeto {
    cursor: pointer;
  }

  .projeto:hover {
    background: color-mix(in srgb, var(--hover) 70%, transparent);
  }

  .projeto-nome {
    min-width: 0;
    font-weight: 600;
    color: var(--texto);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .projeto-local {
    min-width: 0;
    color: var(--texto-secundario);
    font-size: 0.84rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .projeto-data {
    min-width: 0;
    color: var(--texto-secundario);
    font-size: 0.84rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .projeto.selecionada {
    background: var(--primaria-suave);
  }

  .marcar {
    width: 15px;
    height: 15px;
    margin: 0;
    accent-color: var(--primaria);
    cursor: pointer;
  }

  .acoes {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.12s;
  }

  /* hover, foco pelo teclado ou linha marcada mostram as acoes; tela de toque mostra sempre */
  .projeto:hover .acoes,
  .projeto:focus-within .acoes,
  .projeto.selecionada .acoes {
    opacity: 1;
  }

  @media (hover: none) {
    .acoes {
      opacity: 1;
    }
  }

  .acao {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .acao:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .acao.perigo:hover {
    color: var(--perigo);
  }

  /* janela estreita: some o Local e depois a data. Cada linha e um grid proprio, entao as colunas
     sao sempre de largura fixa ou fr (nunca auto): assim as linhas ficam alinhadas entre si. */
  @container (max-width: 680px) {
    .linha {
      grid-template-columns: 40px minmax(0, 1fr) 9.5rem 160px;
    }
    .col-local,
    .projeto-local {
      display: none;
    }
  }

  @container (max-width: 440px) {
    .linha {
      grid-template-columns: 32px minmax(0, 1fr) 96px;
    }
    .col-data,
    .projeto-data {
      display: none;
    }
    .acao {
      width: 24px;
    }
  }

  .selecao {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
  }

  .selecao-conta {
    font-size: 0.85rem;
    color: var(--texto-secundario);
    margin-right: 4px;
  }

  .btn-sel {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 7px 12px;
    border: 1px solid var(--borda);
    border-radius: 999px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }

  .btn-sel:hover {
    background: var(--hover);
  }

  .btn-sel.perigo {
    color: var(--perigo);
  }

  .vazio {
    padding: 28px 6px;
    text-align: center;
    color: var(--texto-secundario);
    font-size: 0.92rem;
  }

  .contagem {
    text-align: left;
    color: var(--texto-terciario, var(--texto-secundario));
    font-size: 0.8rem;
    margin: 14px 0 0;
  }

  .fundo-modal {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .modal {
    width: 420px;
    padding: 22px;
    border: 1px solid var(--borda);
    border-radius: 12px;
    background: var(--superficie-0);
    color: var(--texto);
    box-shadow: var(--sombra-3);
  }

  .modal h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  .modal-modelo {
    margin: 2px 0 14px;
    font-size: 0.85rem;
    color: var(--texto-terciario);
  }

  .modal-input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--fundo);
    color: var(--texto);
    font: inherit;
    outline: none;
  }

  .modal-input:focus {
    border-color: var(--primaria);
  }

  .modal-destino {
    margin: 8px 0 0;
    font-size: 0.75rem;
    color: var(--texto-terciario);
    word-break: break-all;
  }

  .modal-erro {
    margin: 8px 0 0;
    font-size: 0.82rem;
    color: var(--perigo);
  }

  .modal-acoes {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }

  .btn-sec,
  .btn-criar {
    padding: 8px 16px;
    border-radius: var(--raio-sm);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }

  .btn-sec {
    border: 1px solid var(--borda);
    background: transparent;
    color: var(--texto);
  }

  .btn-sec:hover {
    background: var(--hover);
  }

  .btn-criar {
    border: none;
    background: var(--primaria);
    color: #fff;
    font-weight: 600;
  }

  .btn-criar:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .aviso-drive {
    position: fixed;
    left: 50%;
    bottom: 36px;
    transform: translateX(-50%);
    z-index: 70;
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: min(640px, calc(100vw - 24px));
    padding: 10px 14px;
    border-radius: var(--raio-sm);
    border: 1px solid var(--borda);
    background: var(--superficie-0);
    color: var(--texto);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.25);
    font-size: 0.875rem;
  }

  .aviso-drive button {
    flex: none;
    padding: 6px 12px;
    border: none;
    border-radius: var(--raio-sm);
    background: var(--primaria);
    color: #fff;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .aviso-drive button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
