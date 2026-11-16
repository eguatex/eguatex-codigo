<script lang="ts" module>
  export interface ErroLog {
    linha?: number | null;
    arquivo?: string | null;
    mensagem: string;
    tipo: string;
    /** Avisos agrupados no painel ("caixa-larga", "rotulo-repetido"...). */
    categoria?: string | null;
  }

  /** Abas do log, na ordem do cabecalho. O proprio painel desenha as abas e guarda a escolhida. */
  export const ABAS_LOG = [
    { id: "todos", chave: "log.aba.todos" },
    { id: "erros", chave: "log.aba.erros" },
    { id: "avisos", chave: "log.aba.avisos" },
    { id: "info", chave: "log.aba.info" },
  ] as const;
</script>

<script lang="ts">
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import errosConhecidos from "../../../dados/erros-latex.pt-BR.json";
  import { t, tf } from "$lib/i18n";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import LocateFixed from "@lucide/svelte/icons/locate-fixed";
  import { contagens, filtrarPorAba, linhasInfo, type AbaLog } from "$lib/filtro-log";

  type ErroConhecido = { padrao: string; explicacao: string; sugestao?: string };
  const CONHECIDOS = (errosConhecidos as ErroConhecido[]).filter((e) => e.padrao);

  /** Explicacao em portugues para as mensagens do LaTeX, que saem em ingles. */
  function explicacao(mensagem: string): string | null {
    const m = mensagem.toLowerCase();
    // alguns padroes sao expressoes ("Environment .* undefined"); os outros, texto puro
    const casa = (padrao: string) => {
      try {
        if (new RegExp(padrao, 'i').test(mensagem)) return true;
      } catch {
        // padrao com caractere especial solto: compara como texto
      }
      return m.includes(padrao.toLowerCase());
    };
    const e = CONHECIDOS.find((c) => casa(c.padrao)) ?? (/file `[^']+' not found/i.test(mensagem) ? CONHECIDOS.find((c) => c.padrao === 'File not found') : undefined);
    if (!e) return null;
    const texto = e.explicacao.replace(/'\{[a-z_]+\}'/g, 'desta linha').replace(/\{[a-z_]+\}/g, '');
    return e.sugestao ? `${texto} ${e.sugestao}` : texto;
  }

  interface Props {
    erros?: ErroLog[];
    log?: string;
    iaAtiva?: boolean;
    onirparalinha?: (linha: number, arquivo?: string | null) => void;
    onexplicarerro?: (erro: string) => void;
    /** Caminhos dos .log, .aux, .bbl e .blg do projeto. */
    outrosArquivos?: string[];
    onlimparcache?: () => void;
    onabrirarquivo?: (caminho: string) => void;
  }

  let {
    erros = [],
    log = "",
    iaAtiva = false,
    onirparalinha,
    onexplicarerro,
    outrosArquivos = [],
    onlimparcache,
    onabrirarquivo,
  }: Props = $props();

  /** Aba escolhida. Cada abertura do painel comeca em Todos. */
  let aba = $state<AbaLog>("todos");
  let logAberto = $state(false);
  let outrosAbertos = $state(false);
  const totais = $derived(contagens(erros, log));
  const infos = $derived(aba === "info" ? linhasInfo(log) : []);

  /** Avisos que o painel junta num grupo recolhido, na ordem em que aparecem. */
  const GRUPOS = [
    { id: "caixa-larga", um: "log.grupo.caixa-larga.um", varios: "log.grupo.caixa-larga.varios" },
    { id: "caixa-frouxa", um: "log.grupo.caixa-frouxa.um", varios: "log.grupo.caixa-frouxa.varios" },
    { id: "caixa-vertical", um: "log.grupo.caixa-vertical.um", varios: "log.grupo.caixa-vertical.varios" },
    { id: "rotulo-repetido", um: "log.grupo.rotulo-repetido.um", varios: "log.grupo.rotulo-repetido.varios" },
  ] as const;
  const idsAgrupados: string[] = GRUPOS.map((g) => g.id);

  const agrupado = (e: ErroLog) => e.tipo === "aviso" && !!e.categoria && idsAgrupados.includes(e.categoria);

  /** Erros e avisos soltos: ficam sempre a vista. */
  const visiveis = $derived(filtrarPorAba(erros, aba));
  const principais = $derived(visiveis.filter((e) => !agrupado(e)));

  /** Um grupo recolhido por categoria, com o titulo "12 avisos de linha larga". */
  const grupos = $derived(
    GRUPOS.map((g) => {
      const itens = visiveis.filter((e) => agrupado(e) && e.categoria === g.id);
      return { id: g.id, itens, rotulo: tf(itens.length === 1 ? g.um : g.varios, { n: itens.length }) };
    }).filter((g) => g.itens.length > 0),
  );

  let gruposAbertos = $state<Record<string, boolean>>({});

  function ehAviso(erro: ErroLog): boolean {
    return erro.tipo === "aviso";
  }

  function rotulo(tipo: string): string {
    if (tipo === "aviso") return t("log.tipo.aviso");
    if (tipo === "fatal") return t("log.tipo.fatal");
    return t("log.tipo.erro");
  }

  function temLinha(erro: ErroLog): boolean {
    return typeof erro.linha === "number" && erro.linha > 0;
  }

  // Texto no formato "arquivo, linha N"; cada parte so aparece se existir.
  function textoLocal(erro: ErroLog): string {
    const partes: string[] = [];
    const arquivo = erro.arquivo?.trim();
    if (arquivo) partes.push(arquivo);
    if (temLinha(erro)) partes.push(`linha ${erro.linha}`);
    return partes.join(", ");
  }

  /** Setas, Home e End trocam de aba, como no padrao WAI-ARIA para abas. */
  function teclarAba(ev: KeyboardEvent) {
    const i = ABAS_LOG.findIndex((a) => a.id === aba);
    let novo = i;
    if (ev.key === "ArrowRight") novo = (i + 1) % ABAS_LOG.length;
    else if (ev.key === "ArrowLeft") novo = (i - 1 + ABAS_LOG.length) % ABAS_LOG.length;
    else if (ev.key === "Home") novo = 0;
    else if (ev.key === "End") novo = ABAS_LOG.length - 1;
    else return;
    ev.preventDefault();
    aba = ABAS_LOG[novo].id;
    document.getElementById(`aba-${aba}`)?.focus();
  }

  function irParaLocal(erro: ErroLog) {
    if (typeof erro.linha === "number" && erro.linha > 0) {
      onirparalinha?.(erro.linha, erro.arquivo);
    }
  }
</script>

{#snippet entrada(erro: ErroLog)}
  {@const aviso = ehAviso(erro)}
  {@const local = textoLocal(erro)}
  <li class="entrada" class:aviso>
    <span class="icone" aria-hidden="true">
      {#if aviso}
        <TriangleAlert size={16} />
      {:else}
        <CircleAlert size={16} />
      {/if}
    </span>
    <div class="corpo">
      <p class="mensagem"><span class="tipo">{rotulo(erro.tipo)}</span> {erro.mensagem}</p>
      {#if explicacao(erro.mensagem)}
        <p class="explicacao">{explicacao(erro.mensagem)}</p>
      {/if}
      {#if local}
        {#if temLinha(erro)}
          <button
            type="button"
            class="local ligacao"
            title={t("log.ir-para-codigo")}
            onclick={() => irParaLocal(erro)}
          >
            <LocateFixed size={14} />
            <span>{local}</span>
          </button>
        {:else}
          <span class="local">{local}</span>
        {/if}
      {/if}
    </div>
    {#if iaAtiva}
      <button
        type="button"
        class="explicar"
        title={t("log.explicar-dica")}
        onclick={() => onexplicarerro?.(erro.mensagem)}
      >
        <Sparkles size={12} />
        <span>{t("log.explicar")}</span>
      </button>
    {/if}
  </li>
{/snippet}

<div class="logs">
  <div class="abas" role="tablist" aria-label={t("log.aba.rotulo")}>
    {#each ABAS_LOG as a (a.id)}
      <button
        type="button"
        role="tab"
        id="aba-{a.id}"
        class="aba"
        class:ativa={aba === a.id}
        class:tem-erro={a.id === "erros" && totais.erros > 0}
        aria-selected={aba === a.id}
        aria-controls="painel-log"
        tabindex={aba === a.id ? 0 : -1}
        onclick={() => (aba = a.id)}
        onkeydown={teclarAba}
      >
        <span>{t(a.chave)}</span>
        <span class="contador">{totais[a.id]}</span>
      </button>
    {/each}
  </div>

  <div class="conteudo" id="painel-log" role="tabpanel" aria-labelledby="aba-{aba}">
    {#if aba === "info"}
      {#if infos.length === 0}
        <p class="vazio">{t("log.info-vazio")}</p>
      {:else}
        <ul class="lista info" aria-label={t("log.aba.info")}>
          {#each infos as linha}
            <li class="entrada info">{linha}</li>
          {/each}
        </ul>
      {/if}
    {:else if principais.length === 0 && grupos.length === 0}
      <p class="vazio">{aba === "avisos" ? t("log.avisos-vazio") : t("log.vazio")}</p>
    {:else}
      <ul class="lista" aria-label={t("log.lista-rotulo")}>
        {#each principais as erro}
          {@render entrada(erro)}
        {/each}
      </ul>
    {/if}

    {#each grupos as grupo (grupo.id)}
      <section class="grupo">
        <button
          type="button"
          class="alternar-grupo"
          title={tf("log.grupo.dica")}
          aria-expanded={!!gruposAbertos[grupo.id]}
          aria-controls="grupo-{grupo.id}"
          onclick={() => (gruposAbertos[grupo.id] = !gruposAbertos[grupo.id])}
        >
          {#if gruposAbertos[grupo.id]}
            <ChevronDown size={14} />
          {:else}
            <ChevronRight size={14} />
          {/if}
          <TriangleAlert size={14} />
          <span>{grupo.rotulo}</span>
        </button>
        {#if gruposAbertos[grupo.id]}
          <ul id="grupo-{grupo.id}" class="lista" aria-label={grupo.rotulo}>
            {#each grupo.itens as erro}
              {@render entrada(erro)}
            {/each}
          </ul>
        {/if}
      </section>
    {/each}

    {#if aba === "todos"}
      <div class="log-completo">
        <button
          type="button"
          class="alternar"
          aria-expanded={logAberto}
          aria-controls="log-bruto"
          onclick={() => (logAberto = !logAberto)}
        >
          {#if logAberto}
            <ChevronDown size={14} />
          {:else}
            <ChevronRight size={14} />
          {/if}
          <span>{t("log.raw")}</span>
        </button>
        {#if logAberto}
          <pre id="log-bruto" class="bruto">{log || t("log.raw-vazio")}</pre>
        {/if}
      </div>
    {/if}

    <div class="rodape">
      {#if onlimparcache}
        <button type="button" class="botao-rodape" onclick={() => onlimparcache?.()}>
          <Trash2 size={13} />
          <span>{t("log.limpar-cache")}</span>
        </button>
      {/if}
      <button
        type="button"
        class="botao-rodape"
        aria-expanded={outrosAbertos}
        aria-controls="outros-arquivos"
        onclick={() => (outrosAbertos = !outrosAbertos)}
      >
        <FolderOpen size={13} />
        <span>{t("log.outros-arquivos")}</span>
        {#if outrosAbertos}
          <ChevronDown size={13} />
        {:else}
          <ChevronRight size={13} />
        {/if}
      </button>
    </div>
    {#if outrosAbertos}
      {#if outrosArquivos.length === 0}
        <p id="outros-arquivos" class="vazio">{t("log.outros-vazio")}</p>
      {:else}
        <ul id="outros-arquivos" class="outros" aria-label={t("log.outros-arquivos")}>
          {#each outrosArquivos as caminho (caminho)}
            <li>
              <button
                type="button"
                class="local ligacao"
                title={tf("log.abrir-arquivo", { arquivo: caminho })}
                onclick={() => onabrirarquivo?.(caminho)}
              >
                {caminho}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
</div>

<style>
  .logs {
    box-sizing: border-box;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--fundo);
    color: var(--texto);
    font-size: 0.8125rem;
  }

  /* Cabecalho fixo no topo, como .error-logs-tabs do Overleaf (logs.scss): aba com 3px embaixo. */
  .abas {
    display: flex;
    flex-shrink: 0;
    overflow-x: auto;
    scrollbar-width: none;
    background: var(--superficie-0);
    border-bottom: 1px solid var(--borda);
  }

  .aba {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border: none;
    border-bottom: 3px solid transparent;
    background: transparent;
    color: var(--texto-secundario);
    font: inherit;
    font-size: 0.8125rem;
    white-space: nowrap;
    cursor: pointer;
  }

  .aba:hover,
  .aba:focus-visible {
    color: var(--texto);
  }

  .aba.ativa {
    color: var(--texto);
    border-bottom-color: var(--primaria);
  }

  .aba:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .aba .contador {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    padding: 0 4px;
    box-sizing: border-box;
    border-radius: 9px;
    background: var(--superficie-1);
    font-size: 0.6875rem;
  }

  .aba.tem-erro .contador {
    color: var(--perigo);
  }

  .conteudo {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  /* Botoes de acao no rodape, a direita (logs-pane-actions do Overleaf). */
  .rodape {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    margin-top: auto;
    padding-top: 8px;
    border-top: 1px solid var(--borda);
  }

  .botao-rodape {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border: 1px solid var(--borda);
    border-radius: 4px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }

  .botao-rodape:hover {
    background: var(--hover);
  }

  .botao-rodape:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 2px;
  }

  .outros {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0 0 0 4px;
    list-style: none;
  }

  .entrada.info {
    --cor: var(--texto-terciario);
    font-family: "Cascadia Code", "Fira Code", Consolas, monospace;
    font-size: 0.75rem;
    color: var(--texto-secundario);
    overflow-wrap: anywhere;
  }

  .vazio {
    margin: 24px 0 8px;
    text-align: center;
    color: var(--texto-terciario);
  }

  .lista {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .entrada {
    --cor: var(--perigo);
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 10px;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-left: 3px solid var(--cor);
    border-radius: 6px;
  }

  .entrada.aviso {
    --cor: var(--acento);
  }

  .icone {
    display: inline-flex;
    flex-shrink: 0;
    padding-top: 1px;
    color: var(--cor);
  }

  .corpo {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .explicacao {
    margin: 2px 0 0;
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .mensagem {
    margin: 0;
    font-weight: 600;
    line-height: 1.4;
    color: var(--cor);
    overflow-wrap: anywhere;
  }

  .tipo {
    font-size: 0.6875rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    margin-right: 4px;
  }

  .local {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: transparent;
    font: inherit;
    font-size: 0.75rem;
    text-align: left;
    color: var(--texto-secundario);
    overflow-wrap: anywhere;
  }

  .ligacao {
    cursor: pointer;
    text-decoration: underline;
  }

  .ligacao:hover {
    color: var(--texto);
  }

  .ligacao:focus-visible,
  .explicar:focus-visible,
  .alternar:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 2px;
  }

  .explicar {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border: 1px solid var(--borda);
    border-radius: 4px;
    background: transparent;
    color: var(--acento);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .explicar:hover {
    background: var(--hover);
  }

  .alternar-grupo {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    padding: 6px 10px;
    border: 1px solid var(--borda);
    border-left: 3px solid var(--acento);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .alternar-grupo:hover {
    background: var(--hover);
  }

  .alternar-grupo:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 2px;
  }

  .grupo {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .log-completo {
    display: flex;
    flex-direction: column;
    margin-top: 4px;
    border-top: 1px solid var(--borda);
  }

  .alternar {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    align-self: flex-start;
    margin-top: 4px;
    padding: 6px 4px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .alternar:hover {
    color: var(--texto);
  }

  .bruto {
    margin: 4px 0 0;
    max-height: 320px;
    overflow: auto;
    padding: 8px;
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 6px;
    color: var(--texto-secundario);
    font-family: "Cascadia Code", "Fira Code", Consolas, monospace;
    font-size: 0.75rem;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .local.ligacao {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  /* Celular: alvos de 44 px, abas com rolagem horizontal e log bruto em 12 px com rolagem horizontal. */
  @media (pointer: coarse) {
    .aba {
      flex: 0 0 auto;
      min-height: 44px;
      padding: 0 16px;
    }

    .alternar-grupo,
    .alternar,
    .botao-rodape,
    .explicar,
    .local.ligacao {
      min-height: 44px;
    }

    .local.ligacao {
      min-width: 44px;
    }

    .bruto {
      font-size: 12px;
      white-space: pre;
      overflow-wrap: normal;
    }
  }
</style>
