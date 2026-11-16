<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { untrack } from "svelte";
  import PainelChave from "./ConfigIA.svelte";
  import Bot from "@lucide/svelte/icons/bot";
  import X from "@lucide/svelte/icons/x";
  import Minus from "@lucide/svelte/icons/minus";
  import { sessaoChat, ARQUIVO_CHAT, serializarConversa, carregarConversa } from "$lib/chat-sessao";
  import { t, tf } from "$lib/i18n";
  import { consentimentoIaDado, registrarConsentimentoIa } from "$lib/consentimento-ia";
  import { sortearSaudacao, chaveSaudacao } from "$lib/saudacao";
  import { SITE } from "$lib/site";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Send from "@lucide/svelte/icons/send-horizontal";
  import Girador from "./Girador.svelte";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import User from "@lucide/svelte/icons/user";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Power from "@lucide/svelte/icons/power";
  import Square from "@lucide/svelte/icons/square";
  import Check from "@lucide/svelte/icons/check";
  import Pencil from "@lucide/svelte/icons/pencil";
  import {
    ferramentas,
    instrucaoSistema,
    contextoDaPergunta,
    executar,
    aplicarProposta,
    descreverChamada,
    prepararHistorico,
    NOMES_NIVEL,
    type NivelAgente,
    type Proposta,
    type ProjetoAgente,
  } from "$lib/agente";

  interface ConfigIA {
    ativa: boolean;
    provedor: string;
    modelo: string;
    nivel?: string;
  }

  interface Mensagem {
    papel: "usuario" | "assistente" | "acao";
    conteudo: string;
    propostas?: Proposta[];
    /** Mensagem de erro que aceita o botao "Tentar de novo". */
    repetir?: boolean;
  }

  interface ParteGemini {
    text?: string;
    thought?: boolean;
    functionCall?: { id?: string; name: string; args?: Record<string, unknown> };
    [chave: string]: unknown;
  }

  interface ConteudoGemini {
    role: string;
    parts: ParteGemini[];
    /** Campo do ÉguaTex: qual modelo respondeu. Sai antes de ir para o historico. */
    modeloUsado?: string;
  }

  interface Props {
    visivel?: boolean;
    config?: ConfigIA;
    /** Acesso ao projeto aberto; sem ele o chat so conversa. */
    agente?: ProjetoAgente;
    onfechar?: () => void;
    /** A IA foi ligada ou a configuracao mudou: a pagina guarda a versao nova. */
    onconfigIA?: (config: ConfigIA) => void;
  }

  let {
    visivel = false,
    config = { ativa: false, provedor: "", modelo: "" },
    agente,
    onfechar,
    onconfigIA,
  }: Props = $props();

  const MAX_RODADAS = 12;

  // A conversa vem da sessao: minimizar ou sair do projeto nao apaga o que foi dito.
  let mensagens: Mensagem[] = $state(sessaoChat.mensagens as Mensagem[]);
  let historico = sessaoChat.historico as ConteudoGemini[];
  let inputTexto = $state(sessaoChat.texto);
  // Saudacao do Curupira: sorteada a cada conversa nova, sem repetir a anterior. Fica so na tela,
  // fora de `mensagens` e de `historico`: nada dela vai para a API.
  let saudacao = $state(sessaoChat.saudacao || novaSaudacao());
  function novaSaudacao(): number {
    sessaoChat.saudacao = sortearSaudacao(sessaoChat.saudacao);
    return sessaoChat.saudacao;
  }
  $effect(() => {
    sessaoChat.mensagens = $state.snapshot(mensagens) as Mensagem[];
    sessaoChat.texto = inputTexto;
  });
  let enviando = $state(false);
  // Texto que a IA ja mandou na resposta em andamento (stream); some quando a resposta fecha.
  let textoParcial = $state("");
  let idAtual = "";
  // Aviso de que o Gemini esta ocupado e o app esta repetindo o pedido sozinho.
  let avisoOcupado = $state("");
  // Conversa no momento do erro, para o botao "Tentar de novo" continuar do mesmo passo.
  let retomada: { historico: ConteudoGemini[]; tamanhoAntes: number; nivel: NivelAgente } | null = $state.raw(null);

  // A conversa de cada projeto fica em .eguatex/chat.json, dentro da pasta dele.
  let pasta = $derived(agente?.pasta?.() ?? "");
  // Conversa vazia so e gravada quando a pessoa pede ("Nova conversa"): assim um chat.json
        // corrompido ou ainda nao lido nunca e trocado por um arquivo vazio.
  let permitirSalvarVazio = false;
  let temporizadorSalvar: ReturnType<typeof setTimeout> | undefined;
  let salvarPendente: (() => Promise<void>) | null = null;

  async function salvarConversa(caminho: string, lista: unknown[], hist: unknown[]) {
    try {
      await invoke("cmd_salvar_arquivo", {
        caminhoProjeto: caminho,
        caminhoRelativo: ARQUIVO_CHAT,
        conteudo: serializarConversa(lista, hist),
      });
    } catch {
      // sem gravar, a conversa continua na memoria; nada mais quebra
    }
  }

  function agendarSalvar(caminho: string, lista: unknown[]) {
    const hist = [...historico];
    salvarPendente = () => salvarConversa(caminho, lista, hist);
    clearTimeout(temporizadorSalvar);
    temporizadorSalvar = setTimeout(() => {
      const f = salvarPendente;
      salvarPendente = null;
      void f?.();
    }, 600);
  }

  async function gravarPendente() {
    clearTimeout(temporizadorSalvar);
    const f = salvarPendente;
    salvarPendente = null;
    await f?.();
  }

  /** Abriu outro projeto: guarda a conversa do anterior e carrega a deste. */
  async function trocarDeProjeto(nova: string) {
    if (nova === sessaoChat.pasta) return;
    await gravarPendente();
    sessaoChat.pasta = nova;
    permitirSalvarVazio = false;
    if (!nova) return;
    retomada = null;
    mensagens = [];
    historico.length = 0;
    textoParcial = "";
    saudacao = novaSaudacao();
    try {
      const texto = await invoke<string>("cmd_ler_arquivo", { caminhoProjeto: nova, caminhoRelativo: ARQUIVO_CHAT });
      const conversa = carregarConversa(texto);
      if (conversa && sessaoChat.pasta === nova && !enviando) {
        mensagens = conversa.mensagens as Mensagem[];
        historico.splice(0, historico.length, ...(conversa.historico as ConteudoGemini[]));
      } else if (!conversa && texto.trim()) {
        // arquivo corrompido ou de outra versao: guarda uma copia antes de qualquer gravacao nova
        await invoke("cmd_salvar_arquivo", { caminhoProjeto: nova, caminhoRelativo: ARQUIVO_CHAT + ".bak", conteudo: texto }).catch(() => {});
      }
    } catch {
      // projeto sem conversa salva: comeca vazio
    }
  }

  $effect(() => {
    const nova = pasta;
    untrack(() => void trocarDeProjeto(nova));
  });

  $effect(() => {
    const lista = $state.snapshot(mensagens);
    const ocupado = enviando;
    const caminho = pasta;
    if (!caminho || ocupado || caminho !== sessaoChat.pasta) return;
    if (lista.length === 0 && !permitirSalvarVazio) return;
    agendarSalvar(caminho, lista);
  });
  // Cada envio ganha um numero; "Parar" avanca o contador e o laco antigo se encerra sozinho.
  let execucao = 0;
  let tamanhoAntesAtual = 0;
  let containerRef: HTMLDivElement | undefined = $state();
  let editando: number | null = $state(null);
  let versaoSeletores = $state(0);
  let entradaRef: HTMLTextAreaElement | undefined = $state();

  // Sem a chave do Gemini guardada, o painel mostra a tela da chave no lugar do chat.
  // Assim ninguem digita uma pergunta que seria recusada depois.
  let comChave = $state<boolean | null>(null);

  // LGPD: antes do primeiro envio a pessoa le e aceita o aviso uma vez.
  let consentido = $state(consentimentoIaDado());
  function aceitarConsentimento() {
    registrarConsentimentoIa();
    consentido = true;
  }

  async function conferirChave() {
    try {
      comChave = await invoke<boolean>("cmd_ia_chave_status");
    } catch {
      comChave = false;
    }
  }

  $effect(() => {
    if (visivel) conferirChave();
  });

  // Ativar ou remover a chave na tela da chave muda o que o chat mostra.
  $effect(() => {
    const aoMudar = () => conferirChave();
    window.addEventListener("ia-chave-mudou", aoMudar);
    return () => window.removeEventListener("ia-chave-mudou", aoMudar);
  });

  let nivel: NivelAgente = $derived(
    !agente ? "conversa" : ((["conversa", "leitura", "sugerir"].includes(config.nivel ?? "")
      ? config.nivel
      : "sugerir") as NivelAgente),
  );

  function adicionar(m: Mensagem) {
    mensagens = [...mensagens, m];
    rolarParaBaixo();
  }

  async function enviar() {
    const texto = inputTexto.trim();
    if (!texto || enviando || !consentido) return;

    inputTexto = "";
    adicionar({ papel: "usuario", conteudo: texto });
    enviando = true;

    const nivelDaVez = nivel;
    const tamanhoAntes = historico.length;
    tamanhoAntesAtual = tamanhoAntes;
    const minha = ++execucao;
    const pergunta = { text: texto + contextoDaPergunta(agente, nivelDaVez) };
    const ultimo = historico[historico.length - 1];
    // Se a rodada anterior parou no limite, a ultima vez e da pessoa: junta para nao repetir o papel.
    if (ultimo?.role === "user") historico[historico.length - 1] = { role: "user", parts: [...ultimo.parts, pergunta] };
    else historico.push({ role: "user", parts: [pergunta] });
    retomada = null;
    await rodar(nivelDaVez, tamanhoAntes, minha);
  }

  /** Repete o passo que falhou, com a conversa como estava na hora do erro. */
  async function tentarDeNovo() {
    if (enviando || !retomada) return;
    const r = retomada;
    retomada = null;
    mensagens = mensagens.filter((m) => !m.repetir);
    historico.splice(0, historico.length, ...r.historico);
    enviando = true;
    tamanhoAntesAtual = r.tamanhoAntes;
    await rodar(r.nivel, r.tamanhoAntes, ++execucao);
  }

  /** Rodadas do agente: pergunta a IA, executa as ferramentas e devolve os resultados, ate ela parar. */
  async function rodar(nivelDaVez: NivelAgente, tamanhoAntes: number, minha: number) {
    try {
      for (let rodada = 0; rodada < MAX_RODADAS; rodada++) {
        const corpo: Record<string, unknown> = {
          systemInstruction: { parts: [{ text: instrucaoSistema(nivelDaVez) }] },
          contents: prepararHistorico(historico),
          generationConfig: { temperature: 0.4, maxOutputTokens: 32768 },
        };
        const tools = ferramentas(nivelDaVez);
        if (tools.length) corpo.tools = tools;

        // Cada pedaco de texto chega pelo evento "ia-pedaco" e aparece na hora.
        idAtual = `chat-${Date.now()}-${minha}-${rodada}`;
        const idDaRodada = idAtual;
        const inicio = performance.now();
        let primeiro = true;
        textoParcial = "";
        const desligar = await listen<{ id: string; texto: string }>("ia-pedaco", (ev) => {
          if (ev.payload.id !== idDaRodada || minha !== execucao) return;
          if (primeiro) {
            primeiro = false;
            console.debug(`[ia] primeiro texto em ${Math.round(performance.now() - inicio)} ms`);
          }
          textoParcial += ev.payload.texto;
          rolarParaBaixo();
        });
        const desligarOcupado = await listen<{ id: string; tentativa: number; total: number }>("ia-ocupado", (ev) => {
          if (ev.payload.id !== idDaRodada || minha !== execucao) return;
          avisoOcupado = tf("chat.ocupado", { n: ev.payload.tentativa, total: ev.payload.total });
        });
        let bruta: ConteudoGemini;
        try {
          bruta = await invoke<ConteudoGemini>("cmd_ia_agente", {
            id: idDaRodada,
            modelo: config.modelo ?? "",
            corpo,
          });
        } finally {
          desligar();
          desligarOcupado();
          if (minha === execucao) avisoOcupado = "";
        }
        if (minha !== execucao) return;
        textoParcial = "";
        const { modeloUsado, ...resposta } = bruta;
        const pedido = config.modelo ?? "";
        if (pedido && modeloUsado && modeloUsado !== pedido) {
          adicionar({
            papel: "acao",
            conteudo: tf("chat.respondido-por", { modelo: nomeDoModelo(modeloUsado), pedido: nomeDoModelo(pedido) }),
          });
        }
        // A resposta volta inteira para o historico (o Gemini precisa das assinaturas dele).
        historico.push({ role: "model", parts: resposta.parts ?? [] });

        const texto = (resposta.parts ?? [])
          .filter((p) => p.text && !p.thought)
          .map((p) => p.text)
          .join("");
        const chamadas = (resposta.parts ?? []).filter((p) => p.functionCall);

        if (texto.trim()) adicionar({ papel: "assistente", conteudo: texto });
        if (!chamadas.length || !agente) break;

        const propostas: Proposta[] = [];
        const respostas: ParteGemini[] = [];
        for (const parte of chamadas) {
          const chamada = parte.functionCall!;
          const args = chamada.args ?? {};
          const resultado = await executar(chamada.name, args, agente, nivelDaVez, propostas);
          if (minha !== execucao) return;
          const acao = descreverChamada(chamada.name, args);
          if (acao) adicionar({ papel: "acao", conteudo: acao });
          respostas.push({
            functionResponse: {
              ...(chamada.id ? { id: chamada.id } : {}),
              name: chamada.name,
              response: resultado,
            },
          });
        }
        if (propostas.length) {
          adicionar({
            papel: "assistente",
            conteudo: propostas.length === 1 ? tf("chat.proposta-uma") : tf("chat.propostas-varias", { n: propostas.length }),
            propostas,
          });
        }
        historico.push({ role: "user", parts: respostas });
      }
    } catch (e) {
      if (minha !== execucao) return;
      // Remove a pergunta que falhou do historico (o mesmo array da sessao) para a proxima nao herdar um estado quebrado.
      retomada = { historico: historico.slice(), tamanhoAntes, nivel: nivelDaVez };
      historico.length = tamanhoAntes;
      adicionar({ papel: "assistente", conteudo: tf("chat.erro", { erro: String(e) }), repetir: true });
    } finally {
      if (minha === execucao) {
        enviando = false;
        textoParcial = "";
        rolarParaBaixo();
      }
    }
  }

  /** Solta a interface e descarta o que a rodada em andamento trouxer. O Rust corta o pedido em andamento. */
  function parar() {
    if (!enviando) return;
    execucao++;
    historico.length = tamanhoAntesAtual;
    enviando = false;
    textoParcial = "";
    avisoOcupado = "";
    // corta o stream no Rust; o erro "Interrompido." que ele devolve e ignorado acima
    invoke("cmd_ia_cancelar", { id: idAtual }).catch(() => {});
    adicionar({ papel: "acao", conteudo: tf("chat.interrompido") });
  }

  function nomeDoModelo(id: string): string {
    if (id.includes("pro")) return "Gemini Pro";
    if (id.includes("lite")) return "Gemini Flash Lite";
    if (id.includes("flash")) return "Gemini Flash";
    return id;
  }

  async function aceitar(p: Proposta) {
    if (!agente || p.estado !== "pendente") return;
    try {
      await aplicarProposta(p, agente);
      p.estado = "aceita";
    } catch (e) {
      p.estado = "falhou";
      p.erro = String(e instanceof Error ? e.message : e);
    }
    editando = null;
    mensagens = [...mensagens];
  }

  function rejeitar(p: Proposta) {
    if (p.estado !== "pendente") return;
    p.estado = "rejeitada";
    editando = null;
    mensagens = [...mensagens];
  }

  async function aceitarTodas(lista: Proposta[]) {
    for (const p of lista) await aceitar(p);
  }

  function rejeitarTodas(lista: Proposta[]) {
    for (const p of lista) rejeitar(p);
  }

  /** Linhas para o diff: removidas em vermelho, novas em verde. */
  function linhasDiff(p: Proposta): { tipo: "menos" | "mais"; texto: string }[] {
    const menos = p.original ? p.original.split("\n").map((t) => ({ tipo: "menos" as const, texto: t })) : [];
    const mais = p.novo ? p.novo.split("\n").map((t) => ({ tipo: "mais" as const, texto: t })) : [];
    return [...menos, ...mais];
  }

  function rotuloProposta(p: Proposta): string {
    if (p.tipo === "novo") return tf("chat.proposta.criar", { caminho: p.caminho });
    if (p.tipo === "insercao") return tf("chat.proposta.antes-da-linha", { caminho: p.caminho, linha: p.linha });
    return p.caminho;
  }

  export async function explicarErro(erro: string) {
    inputTexto = tf("chat.explicar-erro", { erro });
    await conferirChave();
    // Sem IA ligada o chat mostra a tela da chave; a pergunta fica na caixa para depois.
    if (comChave && config.ativa) enviar();
  }

  /** Menu do editor, "Perguntar à IA": deixa o trecho na caixa e espera a pergunta. */
  export function perguntarSobre(trecho: string) {
    inputTexto = tf("chat.sobre-trecho", { trecho: trecho.slice(0, 4000) });
    setTimeout(() => entradaRef?.focus(), 50);
  }

  /** Grava modelo ou nivel escolhido no cabecalho e avisa a pagina. */
  async function mudarConfig(parcial: Partial<ConfigIA>) {
    const nova: ConfigIA = { ...config, ...parcial, provedor: "gemini" };
    try {
      await invoke("cmd_ia_salvar_config", { config: nova });
      onconfigIA?.(nova);
    } catch (e) {
      // Redesenha os seletores com o valor que continua valendo.
      versaoSeletores++;
      adicionar({ papel: "assistente", conteudo: tf("chat.erro-config", { erro: String(e) }) });
    }
  }

  // "Automático": o Rust escolhe (Gemini Flash, com o Flash Lite de reserva). O Pro fica de fora:
  // nao tem cota gratuita.
  const MODELOS = [
    { valor: "", nome: t("chat.modelo-automatico") },
    { valor: "gemini-flash-latest", nome: "Gemini Flash" },
    { valor: "gemini-flash-lite-latest", nome: "Gemini Flash Lite" },
  ];

  function novaConversa() {
    if (enviando) parar();
    permitirSalvarVazio = true;
    retomada = null;
    mensagens = [];
    historico.length = 0;
    textoParcial = "";
    saudacao = novaSaudacao();
  }

  function rolarParaBaixo() {
    setTimeout(() => {
      if (containerRef) {
        containerRef.scrollTop = containerRef.scrollHeight;
      }
    }, 50);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      enviar();
    }
  }

  function escaparHtml(texto: string): string {
    return texto.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function formatarResposta(texto: string, comBotao = false): string {
    // Os blocos de codigo saem antes: dentro deles as quebras de linha ficam como estao.
    const blocos: string[] = [];
    const resto = texto.replace(/```\w*\n([\s\S]*?)```/g, (_tudo, codigo: string) => {
      blocos.push(codigo);
      return `\u0000${blocos.length - 1}\u0000`;
    });
    // Escapa o HTML antes de aplicar a formatacao
    return escaparHtml(resto)
      .replace(/`([^`]+)`/g, '<code class="code-inline">$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\n/g, "<br>")
      .replace(/\u0000(\d+)\u0000/g, (_tudo, n: string) => {
        const botao = comBotao
          ? `<button type="button" class="btn-inserir" data-inserir>${escaparHtml(tf("chat.inserir-no-cursor"))}</button>`
          : "";
        return `<div class="bloco-codigo"><pre class="code-block"><code>${escaparHtml(blocos[Number(n)])}</code></pre>${botao}</div>`;
      });
  }

  /** Clique no "Inserir no cursor" de um bloco de codigo da resposta. */
  function aoClicarResposta(e: MouseEvent) {
    const botao = (e.target as HTMLElement | null)?.closest?.("[data-inserir]");
    if (!botao) return;
    const codigo = botao.parentElement?.querySelector("code")?.textContent ?? "";
    if (codigo) agente?.inserirNoCursor?.(codigo);
  }
</script>

{#if visivel}
  <aside class="chat-painel">
    <div class="chat-cabecalho">
      <div class="chat-titulo">
        <Sparkles size={16} />
        <span>{t("chat.titulo")}</span>
      </div>
      <div class="chat-acoes">
        {#if comChave && config.ativa}
          <button class="chat-btn-icon" onclick={() => mudarConfig({ ativa: false })} title={t("chat.desligar")}>
            <Power size={14} />
          </button>
        {/if}
        <button class="chat-btn-icon" onclick={novaConversa} title={tf("chat.nova-conversa")}>
          <Trash2 size={14} />
        </button>
        <button class="chat-btn-icon" onclick={onfechar} title={t("chat.minimizar")}>
          <Minus size={14} />
        </button>
      </div>
    </div>

    {#if comChave && config.ativa}
      {#key versaoSeletores}
      <div class="chat-opcoes">
        <label>
          <span>{t("chat.modelo")}</span>
          <select
            value={config.modelo ?? ""}
            onchange={(e) => mudarConfig({ modelo: e.currentTarget.value })}
          >
            {#each MODELOS as m (m.valor)}
              <option value={m.valor}>{m.nome}</option>
            {/each}
            {#if config.modelo && !MODELOS.some((m) => m.valor === config.modelo)}
              <option value={config.modelo}>{config.modelo}</option>
            {/if}
          </select>
        </label>
        {#if agente}
          <label title={t("chat.acesso-dica")}>
            <span>{t("chat.acesso")}</span>
            <select
              value={nivel}
              onchange={(e) => mudarConfig({ nivel: e.currentTarget.value })}
            >
              {#each Object.entries(NOMES_NIVEL) as [id, nome] (id)}
                <option value={id}>{nome}</option>
              {/each}
            </select>
          </label>
        {/if}
      </div>
      {/key}
    {/if}

    {#if comChave === false || (comChave && !config.ativa)}
      <div class="chat-login">
        <p class="chat-login-texto">
          {comChave ? t("chat.desligada") : t("chat.sem-chave")}
        </p>
        <PainelChave onconfigurar={onconfigIA} onpronto={conferirChave} />
      </div>
    {:else if comChave === null}
      <div class="chat-aviso"><Girador tamanho={20} /></div>
    {:else}
      <div class="chat-mensagens" bind:this={containerRef}>
        {#if mensagens.length === 0}
          <div class="chat-vazio">
            <Sparkles size={20} />
            <p class="chat-saudacao">{t(chaveSaudacao(saudacao))}</p>
            <div class="sugestoes">
              {#if nivel !== "conversa"}
                <button class="sugestao" onclick={() => (inputTexto = t("chat.sugestao.corrigir-texto"))}>
                  {t("chat.sugestao.corrigir")}
                </button>
                <button class="sugestao" onclick={() => (inputTexto = t("chat.sugestao.revisar-texto"))}>
                  {t("chat.sugestao.revisar")}
                </button>
                <button class="sugestao" onclick={() => (inputTexto = t("chat.sugestao.abnt-texto"))}>
                  {t("chat.sugestao.abnt")}
                </button>
              {:else}
                <button class="sugestao" onclick={() => (inputTexto = t("chat.sugestao.tabela-texto"))}>
                  {t("chat.sugestao.tabela")}
                </button>
                <button class="sugestao" onclick={() => (inputTexto = t("chat.sugestao.abntex2-texto"))}>
                  {t("chat.sugestao.abntex2")}
                </button>
              {/if}
            </div>
          </div>
        {:else}
          {#each mensagens as msg, indice}
            {#if msg.papel === "acao"}
              <div class="msg-acao">{msg.conteudo}</div>
            {:else}
              <div class="msg" class:msg-usuario={msg.papel === "usuario"}>
                <div class="msg-avatar">
                  {#if msg.papel === "usuario"}
                    <User size={14} />
                  {:else}
                    <Bot size={14} />
                  {/if}
                </div>
                <div class="msg-conteudo">
                  {#if msg.papel === "usuario"}
                    <p>{msg.conteudo}</p>
                  {:else}
                    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                    <div class="msg-html" onclick={aoClicarResposta}>
                      {@html formatarResposta(msg.conteudo, !!agente?.inserirNoCursor)}
                    </div>
                    {#if msg.repetir && retomada && !enviando && indice === mensagens.length - 1}
                      <button class="btn-tentar" onclick={tentarDeNovo}>{t("chat.tentar-de-novo")}</button>
                    {/if}
                    {#if msg.propostas?.length}
                      {@const pendentes = msg.propostas.filter((p) => p.estado === "pendente")}
                      {#each msg.propostas as p (p.id)}
                        <div class="proposta" class:feita={p.estado !== "pendente"}>
                          <div class="proposta-cabecalho">
                            <span class="proposta-arquivo">{rotuloProposta(p)}</span>
                            {#if p.estado === "aceita"}
                              <span class="proposta-estado ok">{t("chat.proposta.aplicada")}</span>
                            {:else if p.estado === "rejeitada"}
                              <span class="proposta-estado">{t("chat.proposta.rejeitada")}</span>
                            {:else if p.estado === "falhou"}
                              <span class="proposta-estado erro">{t("chat.proposta.nao-aplicada")}</span>
                            {/if}
                          </div>
                          {#if p.motivo}
                            <p class="proposta-motivo">{p.motivo}</p>
                          {/if}
                          {#if editando === p.id}
                            <textarea class="proposta-editor" bind:value={p.novo} rows="6"></textarea>
                          {:else}
                            <pre class="proposta-diff">{#each linhasDiff(p) as l}<span class="diff-{l.tipo}">{l.tipo === "menos" ? "- " : "+ "}{l.texto}
</span>{/each}</pre>
                          {/if}
                          {#if p.erro}
                            <p class="proposta-erro">{p.erro}</p>
                          {/if}
                          {#if p.estado === "pendente"}
                            <div class="proposta-botoes">
                              <button class="btn-aceitar" onclick={() => aceitar(p)}>
                                <Check size={13} />
                                {t("chat.aceitar")}
                              </button>
                              <button class="btn-rejeitar" onclick={() => rejeitar(p)}>
                                <X size={13} />
                                {t("chat.rejeitar")}
                              </button>
                              <button
                                class="btn-rejeitar"
                                onclick={() => (editando = editando === p.id ? null : p.id)}
                              >
                                <Pencil size={13} />
                                {editando === p.id ? t("chat.ver-diferenca") : t("chat.editar")}
                              </button>
                            </div>
                          {/if}
                        </div>
                      {/each}
                      {#if pendentes.length > 1}
                        <div class="proposta-botoes">
                          <button class="btn-aceitar" onclick={() => aceitarTodas(pendentes)}>{t("chat.aceitar-todas")}</button>
                          <button class="btn-rejeitar" onclick={() => rejeitarTodas(pendentes)}>{t("chat.rejeitar-todas")}</button>
                        </div>
                      {/if}
                    {/if}
                  {/if}
                </div>
              </div>
            {/if}
          {/each}
          {#if enviando}
            <div class="msg">
              <div class="msg-avatar">
                <Girador tamanho={14} />
              </div>
              <div class="msg-conteudo">
                {#if textoParcial}
                  <div class="msg-html">
                    {@html formatarResposta(textoParcial)}
                  </div>
                {:else}
                  <p class="msg-pensando">{avisoOcupado || t("chat.pensando")}</p>
                {/if}
              </div>
            </div>
          {/if}
        {/if}
      </div>

      {#if !consentido}
        <div class="chat-consentimento">
          <h4>{t("ia.consentimento.titulo")}</h4>
          <p>{t("ia.consentimento.texto")}</p>
          <button class="link-politica" onclick={() => openUrl(`${SITE}/privacidade.html`)}>{t("ia.consentimento.politica")}</button>
          <button class="chat-aceitar" onclick={aceitarConsentimento}>{t("ia.consentimento.aceitar")}</button>
        </div>
      {:else}
      <div class="chat-input">
        <textarea
          class="chat-textarea"
          bind:value={inputTexto}
          placeholder={t("chat.placeholder")}
          onkeydown={onKeydown}
          rows="2"
          bind:this={entradaRef}
          disabled={enviando}
        ></textarea>
        {#if enviando}
          <button class="chat-enviar chat-parar" onclick={parar} title={t("chat.parar")}>
            <Square size={14} />
          </button>
        {:else}
          <button class="chat-enviar" onclick={enviar} disabled={!inputTexto.trim()} title={t("chat.enviar")}>
            <Send size={16} />
          </button>
        {/if}
      </div>
      {/if}
    {/if}
  </aside>
{/if}

<style>
  .chat-login {
    flex: 1;
    overflow: auto;
    padding: 12px;
  }
  .chat-consentimento {
    padding: 12px;
    border-top: 1px solid var(--borda);
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex-shrink: 0;
  }
  .chat-consentimento h4 {
    margin: 0;
    font-size: 0.9rem;
  }
  .chat-consentimento p {
    margin: 0;
    font-size: 0.8rem;
    color: var(--texto-secundario);
  }
  .link-politica {
    align-self: flex-start;
    background: none;
    border: none;
    padding: 0;
    color: var(--destaque, #0277bd);
    text-decoration: underline;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .chat-aceitar {
    padding: 8px 10px;
    border-radius: 6px;
    border: none;
    cursor: pointer;
    background: var(--destaque, #2e7d32);
    color: #fff;
    font-weight: 600;
  }
  .chat-login-texto {
    margin: 0 0 8px;
    font-size: 0.85rem;
    color: var(--texto-secundario);
  }
  .chat-painel {
    width: 340px;
    max-width: 100%;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--superficie-0);
    border-left: 1px solid var(--borda);
    height: 100%;
  }

  .chat-cabecalho {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px;
    border-bottom: 1px solid var(--borda);
    flex-shrink: 0;
  }

  .chat-titulo {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--acento);
  }

  .chat-acoes {
    display: flex;
    gap: 2px;
  }

  .chat-btn-icon {
    border: none;
    background: transparent;
    color: var(--texto-terciario);
    cursor: pointer;
    padding: 5px;
    border-radius: 5px;
    display: flex;
    align-items: center;
  }

  .chat-btn-icon:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .chat-aviso {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 24px;
    color: var(--texto-terciario);
    text-align: center;
  }

  .chat-opcoes {
    display: flex;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--borda);
    flex-shrink: 0;
  }

  .chat-opcoes label {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 0.68rem;
    color: var(--texto-terciario);
  }

  .chat-opcoes select {
    width: 100%;
    padding: 3px 4px;
    font-size: 0.75rem;
    font-family: inherit;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
  }

  .chat-mensagens {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .chat-vazio {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--texto-terciario);
    text-align: center;
    padding: 16px;
  }

  .chat-vazio p {
    margin: 0;
    font-size: 0.875rem;
  }

  .sugestoes {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    margin-top: 8px;
  }

  .btn-tentar {
    margin-top: 6px;
    min-height: 32px;
    padding: 6px 12px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: inherit;
    font-size: 0.75rem;
    font-family: inherit;
    cursor: pointer;
  }

  .btn-tentar:hover {
    background: var(--superficie-1, var(--superficie-0));
  }

  .sugestao {
    padding: 8px 12px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto-secundario);
    font-size: 0.75rem;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    transition: all 0.15s;
    line-height: 1.4;
  }

  .sugestao:hover {
    border-color: var(--acento);
    color: var(--texto);
    background: var(--hover);
  }

  .msg {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }

  .msg-usuario {
    flex-direction: row-reverse;
  }

  .msg-avatar {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--superficie-2);
    color: var(--texto-terciario);
  }

  .msg-usuario .msg-avatar {
    background: var(--primaria);
    color: white;
  }

  .msg-conteudo {
    max-width: 80%;
    padding: 8px 12px;
    border-radius: 12px;
    font-size: 0.8125rem;
    line-height: 1.5;
  }

  .msg-conteudo p {
    margin: 0;
  }

  .msg-usuario .msg-conteudo {
    background: var(--primaria);
    color: white;
    border-bottom-right-radius: 4px;
  }

  .msg:not(.msg-usuario) .msg-conteudo {
    background: var(--superficie-1);
    color: var(--texto);
    border-bottom-left-radius: 4px;
  }

  .msg-html {
    word-break: break-word;
  }

  .msg-html :global(.code-block) {
    background: var(--superficie-2);
    border-radius: 6px;
    padding: 8px 10px;
    margin: 6px 0;
    overflow-x: auto;
    font-family: "Cascadia Code", "Fira Code", monospace;
    font-size: 0.75rem;
    line-height: 1.4;
  }

  .msg-html :global(.code-inline) {
    background: var(--superficie-2);
    padding: 1px 4px;
    border-radius: 3px;
    font-family: "Cascadia Code", "Fira Code", monospace;
    font-size: 0.75rem;
  }

  .bloco-codigo {
    position: relative;
  }

  .msg-html :global(.btn-inserir) {
    display: block;
    margin: -2px 0 6px auto;
    border: 1px solid var(--borda);
    background: var(--superficie-1, var(--superficie-0));
    color: var(--texto-secundario);
    border-radius: 5px;
    padding: 3px 8px;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .msg-html :global(.btn-inserir:hover) {
    background: var(--hover);
    color: var(--texto);
  }

  .msg-pensando {
    color: var(--texto-terciario);
    font-style: italic;
  }

  .chat-input {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--borda);
    align-items: flex-end;
    flex-shrink: 0;
  }

  .chat-textarea {
    flex: 1;
    resize: none;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    padding: 8px 10px;
    font-size: 0.8125rem;
    font-family: inherit;
    background: var(--superficie-0);
    color: var(--texto);
    outline: none;
    line-height: 1.4;
    max-height: 100px;
  }

  .chat-textarea:focus {
    border-color: var(--acento);
  }

  .chat-textarea:disabled {
    opacity: 0.5;
  }

  .chat-enviar {
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    background: var(--acento);
    color: white;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .chat-enviar:hover:not(:disabled) {
    background: var(--acento-hover);
  }

  .chat-enviar:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }



  .msg-acao {
    font-size: 0.7rem;
    color: var(--texto-terciario);
    padding: 0 0 0 38px;
    font-style: italic;
  }

  .proposta {
    margin-top: 8px;
    border: 1px solid var(--borda);
    border-radius: 8px;
    padding: 8px;
    background: var(--superficie-1);
  }

  .proposta.feita {
    opacity: 0.7;
  }

  .proposta-cabecalho {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 0.75rem;
  }

  .proposta-arquivo {
    font-weight: 600;
    color: var(--texto);
    word-break: break-all;
  }

  .proposta-estado {
    color: var(--texto-terciario);
    white-space: nowrap;
  }

  .proposta-estado.ok {
    color: var(--primaria);
  }

  .proposta-estado.erro,
  .proposta-erro {
    color: var(--perigo);
  }

  .proposta-motivo,
  .proposta-erro {
    margin: 6px 0;
    font-size: 0.75rem;
  }

  .proposta-diff {
    margin: 6px 0;
    padding: 6px;
    max-height: 220px;
    overflow: auto;
    font-size: 0.7rem;
    line-height: 1.45;
    border-radius: 6px;
    background: var(--superficie-0);
    white-space: pre;
  }

  .diff-menos {
    display: block;
    color: var(--perigo);
    background: color-mix(in srgb, var(--perigo) 10%, transparent);
  }

  .diff-mais {
    display: block;
    color: var(--primaria);
    background: color-mix(in srgb, var(--primaria) 10%, transparent);
  }

  .proposta-editor {
    width: 100%;
    box-sizing: border-box;
    margin: 6px 0;
    font-family: monospace;
    font-size: 0.75rem;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 6px;
    padding: 6px;
  }

  .proposta-botoes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }

  .btn-aceitar,
  .btn-rejeitar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 0.75rem;
    cursor: pointer;
    font-family: inherit;
  }

  .btn-aceitar {
    border: none;
    background: var(--acento);
    color: white;
  }

  .btn-aceitar:hover {
    background: var(--acento-hover);
  }

  .btn-rejeitar {
    border: 1px solid var(--borda);
    background: transparent;
    color: var(--texto);
  }

  .btn-rejeitar:hover {
    background: var(--hover);
  }
</style>
