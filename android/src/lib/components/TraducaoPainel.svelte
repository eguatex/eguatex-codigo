<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ArrowLeftRight from "@lucide/svelte/icons/arrow-left-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Girador from "./Girador.svelte";
  import X from "@lucide/svelte/icons/x";

  interface Props {
    /** Trecho selecionado no editor; null fecha o painel. */
    texto: string | null;
    onfechar?: () => void;
    /** Troca a selecao do editor pela traducao. */
    onsubstituir?: (traducao: string) => void;
  }

  let { texto, onfechar, onsubstituir }: Props = $props();

  const IDIOMAS: [string, string][] = [
    ["pt", "português (Brasil)"],
    ["en", "inglês"],
    ["es", "espanhol"],
    ["fr", "francês"],
    ["de", "alemão"],
    ["it", "italiano"],
    ["nl", "holandês"],
    ["ru", "russo"],
    ["zh-CN", "chinês (simplificado)"],
    ["ja", "japonês"],
    ["ko", "coreano"],
    ["ar", "árabe"],
    ["hi", "hindi"],
    ["la", "latim"],
    ["gn", "guarani"],
  ];
  const nomeDe = (codigo: string) => IDIOMAS.find(([c]) => c === codigo)?.[1] ?? codigo;
  const CHAVE_DESTINO = "eguatex.traducao-destino";

  function lerDestino(): string {
    try {
      return localStorage.getItem(CHAVE_DESTINO) || "pt";
    } catch {
      return "pt";
    }
  }

  let origem = $state("auto");
  let destino = $state(lerDestino());
  let detectado = $state("");
  let traducao = $state("");
  let erro = $state("");
  let carregando = $state(false);
  let originalAberto = $state(false);
  let copiado = $state(false);
  // O Rust repete em 429; enquanto um pedido roda, o seguinte espera na fila (so o ultimo vale).
  let emAndamento = false;
  let pendente: { t: string; de: string; para: string } | null = null;

  async function traduzir(t: string, de: string, para: string) {
    if (emAndamento) {
      pendente = { t, de, para };
      return;
    }
    emAndamento = true;
    carregando = true;
    erro = "";
    try {
      const r = await invoke<{ traducao: string; origem: string }>("cmd_traduzir", { texto: t, origem: de, destino: para });
      traducao = r.traducao;
      detectado = r.origem;
    } catch (e) {
      traducao = "";
      erro = String(e);
    } finally {
      emAndamento = false;
      carregando = false;
    }
    if (pendente) {
      const p = pendente;
      pendente = null;
      void traduzir(p.t, p.de, p.para);
    }
  }

  // traduz ao abrir e sempre que um dos idiomas muda
  $effect(() => {
    if (texto) void traduzir(texto, origem, destino);
  });

  $effect(() => {
    if (!texto) {
      origem = "auto";
      originalAberto = false;
      traducao = "";
      erro = "";
    }
  });

  function mudarDestino(codigo: string) {
    destino = codigo;
    try {
      localStorage.setItem(CHAVE_DESTINO, codigo);
    } catch {}
  }

  function inverter() {
    const novaOrigem = destino;
    const novoDestino = origem === "auto" ? detectado || "en" : origem;
    origem = novaOrigem;
    mudarDestino(novoDestino);
  }

  async function copiar() {
    try {
      await navigator.clipboard.writeText(traducao);
      copiado = true;
      setTimeout(() => (copiado = false), 1500);
    } catch {
      erro = "Não foi possível copiar.";
    }
  }

  function tecla(e: KeyboardEvent) {
    if (texto && e.key === "Escape") onfechar?.();
  }
</script>

<svelte:window onkeydown={tecla} />

{#if texto}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="fundo" onclick={onfechar}>
    <div class="cartao" role="dialog" aria-label="Traduzir" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="idiomas">
        <label class="idioma">
          <select value={origem} onchange={(e) => (origem = e.currentTarget.value)} aria-label="Idioma de origem">
            <option value="auto">{origem === "auto" && detectado ? `${nomeDe(detectado)} (detectado)` : "detectar idioma"}</option>
            {#each IDIOMAS as [c, nome]}<option value={c}>{nome}</option>{/each}
          </select>
          <ChevronDown size={16} />
        </label>
        <button class="inverter" title="Inverter idiomas" aria-label="Inverter idiomas" onclick={inverter}>
          <ArrowLeftRight size={20} />
        </button>
        <label class="idioma">
          <select value={destino} onchange={(e) => mudarDestino(e.currentTarget.value)} aria-label="Idioma de destino">
            {#each IDIOMAS as [c, nome]}<option value={c}>{nome}</option>{/each}
          </select>
          <ChevronDown size={16} />
        </label>
        <button class="fechar" title="Fechar (Esc)" aria-label="Fechar" onclick={onfechar}><X size={16} /></button>
      </div>

      <div class="bloco">
        <div class="rotulo">{origem === "auto" ? (detectado ? `${nomeDe(detectado)} detectado` : "detectando...") : nomeDe(origem)}</div>
        <button class="original" class:aberto={originalAberto} onclick={() => (originalAberto = !originalAberto)} title={originalAberto ? "Recolher" : "Mostrar tudo"}>
          <span>{texto}</span>
          <ChevronDown size={16} />
        </button>
      </div>

      <div class="bloco">
        <div class="rotulo">{nomeDe(destino)}</div>
        {#if carregando}
          <p class="estado"><Girador tamanho={14} /> Traduzindo...</p>
        {:else if erro}
          <p class="estado erro">{erro}</p>
        {:else}
          <p class="traducao">{traducao}</p>
        {/if}
      </div>

      <div class="acoes">
        <span class="fonte">Google Tradutor. Precisa de internet.</span>
        {#if onsubstituir}<button class="acao" disabled={!traducao || carregando} onclick={() => onsubstituir?.(traducao)} title="Troca o texto selecionado pela tradução (Ctrl+Z desfaz)">Substituir no texto</button>{/if}
        <button class="acao principal" disabled={!traducao || carregando} onclick={copiar}>{copiado ? "Copiado" : "Copiar"}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .fundo {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding: 16px 16px 40px;
    background: color-mix(in srgb, #000 25%, transparent);
  }

  .cartao {
    width: min(620px, 100%);
    max-height: calc(100dvh - 80px);
    overflow: auto;
    padding: 18px 24px 16px;
    border-radius: 22px;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }

  .idiomas {
    display: grid;
    grid-template-columns: 1fr auto 1fr auto;
    align-items: center;
    gap: 8px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--borda);
  }

  .idioma {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    color: var(--texto-suave, var(--texto));
  }

  .idioma select {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 1.15rem;
    font-weight: 600;
    text-align: center;
    padding: 6px 4px;
    cursor: pointer;
    max-width: 100%;
  }

  .idioma select option {
    background: var(--superficie-0);
    color: var(--texto);
    font-weight: 400;
  }

  .inverter,
  .fechar {
    display: grid;
    place-items: center;
    border: none;
    background: transparent;
    color: var(--texto-suave, var(--texto));
    border-radius: 50%;
    width: 36px;
    height: 36px;
    cursor: pointer;
  }

  .inverter:hover,
  .fechar:hover {
    background: var(--superficie-2);
  }

  .bloco {
    padding: 14px 4px;
    border-bottom: 1px solid var(--borda);
  }

  .rotulo {
    font-size: 0.85rem;
    color: var(--texto-suave, var(--texto));
    opacity: 0.8;
    margin-bottom: 8px;
  }

  .original {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    border: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    text-align: left;
    padding: 0 0 0 8px;
    cursor: pointer;
  }

  .original span {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .original.aberto span {
    white-space: pre-wrap;
    max-height: 30vh;
    overflow: auto;
  }

  .original.aberto :global(svg) {
    transform: rotate(180deg);
  }

  .traducao {
    margin: 0;
    white-space: pre-wrap;
    font-size: 1.05rem;
    line-height: 1.55;
    max-height: 40vh;
    overflow: auto;
    user-select: text;
  }

  .estado {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .estado.erro {
    color: var(--perigo, #c0392b);
  }

  .acoes {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-top: 12px;
  }

  .fonte {
    flex: 1;
    font-size: 0.75rem;
    opacity: 0.6;
  }

  .acao {
    border: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-weight: 600;
    padding: 8px 14px;
    border-radius: 8px;
    cursor: pointer;
  }

  .acao.principal {
    font-size: 1.05rem;
  }

  .acao:hover:not(:disabled) {
    background: var(--superficie-2);
  }

  .acao:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
