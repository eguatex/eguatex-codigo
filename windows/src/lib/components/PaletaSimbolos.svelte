<script lang="ts">
  // Paleta de simbolos. Estrutura (abas por categoria, busca, grade e botao de fechar) inspirada em
  // services/web/frontend/stylesheets/modules/symbol-palette.scss (Overleaf, AGPL-3.0).
  import { tick } from 'svelte';
  import X from '@lucide/svelte/icons/x';
  import Search from '@lucide/svelte/icons/search';
  import simbolosBrutos from '../../../dados/simbolos.json';

  interface Props {
    visivel?: boolean;
    oninserir?: (latex: string) => void;
    onfechar?: () => void;
  }

  let { visivel = false, oninserir, onfechar }: Props = $props();

  interface Simbolo {
    comando: string;
    glifo: string;
    descricao: string;
    chave: string;
  }

  interface Categoria {
    nome: string;
    simbolos: Simbolo[];
  }

  // marcas de acento (bloco Unicode "combining diacritical marks", de 0x300 a 0x36f)
  const RE_ACENTOS = new RegExp('[' + String.fromCharCode(0x300) + '-' + String.fromCharCode(0x36f) + ']', 'g');

  // sem acento e em minusculas, para a busca ignorar acentos e caixa
  function normalizar(texto: string): string {
    return texto.normalize('NFD').replace(RE_ACENTOS, '').toLowerCase();
  }

  // Aceita { categorias: [...] } ou a lista direta; a descricao pode vir como "descricao" ou "nome".
  function lerCategorias(dados: unknown): Categoria[] {
    const lista: unknown = Array.isArray(dados) ? dados : (dados as { categorias?: unknown } | null)?.categorias;
    if (!Array.isArray(lista)) return [];
    const saida: Categoria[] = [];
    for (const cat of lista) {
      if (typeof cat !== 'object' || cat === null) continue;
      const bruta = cat as { nome?: unknown; simbolos?: unknown };
      if (typeof bruta.nome !== 'string' || !Array.isArray(bruta.simbolos)) continue;
      const simbolos: Simbolo[] = [];
      for (const item of bruta.simbolos) {
        if (typeof item !== 'object' || item === null) continue;
        const r = item as Record<string, unknown>;
        if (typeof r.comando !== 'string' || r.comando === '') continue;
        const glifo = typeof r.unicode === 'string' ? r.unicode : '';
        const descricao = typeof r.descricao === 'string' ? r.descricao : typeof r.nome === 'string' ? r.nome : '';
        simbolos.push({ comando: r.comando, glifo, descricao, chave: normalizar(`${r.comando} ${descricao}`) });
      }
      if (simbolos.length > 0) saida.push({ nome: bruta.nome, simbolos });
    }
    return saida;
  }

  const CATEGORIAS = lerCategorias(simbolosBrutos);

  let ativa = $state(0);
  let termo = $state('');
  let destaque = $state<Simbolo | null>(null);
  let abas = $state<(HTMLButtonElement | null)[]>([]);
  let campoBusca = $state<HTMLInputElement | null>(null);

  // ao abrir (pela calculadora ou por Inserir > Simbolo), o cursor ja cai na busca
  $effect(() => {
    if (visivel) void tick().then(() => campoBusca?.focus());
  });

  const palavras = $derived(normalizar(termo).split(/\s+/).filter(Boolean));
  const buscando = $derived(palavras.length > 0);

  // Com busca, procura em todas as categorias (sem repetir o mesmo comando).
  const lista = $derived.by(() => {
    if (!buscando) return CATEGORIAS[ativa]?.simbolos ?? [];
    const colado = termo.trim();
    const vistos = new Set<string>();
    const achados: Simbolo[] = [];
    for (const cat of CATEGORIAS) {
      for (const s of cat.simbolos) {
        if (vistos.has(s.comando)) continue;
        if (s.glifo === colado || palavras.every((p) => s.chave.includes(p))) {
          vistos.add(s.comando);
          achados.push(s);
        }
      }
    }
    return achados;
  });

  function inserir(s: Simbolo) {
    oninserir?.(s.comando);
  }

  // o clique nao tira o foco do editor: a selecao dele continua valendo para a insercao
  function manterFoco(e: MouseEvent) {
    e.preventDefault();
  }

  async function escolherAba(i: number, focar = false) {
    ativa = i;
    termo = '';
    destaque = null;
    await tick();
    if (focar) abas[i]?.focus();
    abas[i]?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }

  function teclaAbas(e: KeyboardEvent) {
    const n = CATEGORIAS.length;
    if (n === 0) return;
    if (e.key === 'ArrowRight') escolherAba((ativa + 1) % n, true);
    else if (e.key === 'ArrowLeft') escolherAba((ativa + n - 1) % n, true);
    else if (e.key === 'Home') escolherAba(0, true);
    else if (e.key === 'End') escolherAba(n - 1, true);
    else return;
    e.preventDefault();
  }

  const PASSOS: Record<string, number | undefined> = { ArrowRight: 1, ArrowLeft: -1 };

  // setas movem o foco entre os simbolos da grade
  function teclaGrade(e: KeyboardEvent) {
    const grade = e.currentTarget as HTMLElement;
    const alvo = e.target as HTMLElement;
    if (!alvo.classList.contains('simbolo')) return;
    const colunas = Math.max(1, getComputedStyle(grade).gridTemplateColumns.split(' ').length);
    const passo = e.key === 'ArrowDown' ? colunas : e.key === 'ArrowUp' ? -colunas : PASSOS[e.key];
    if (passo === undefined) return;
    const botoes = Array.from(grade.querySelectorAll<HTMLElement>('.simbolo'));
    const proximo = Math.min(botoes.length - 1, Math.max(0, botoes.indexOf(alvo) + passo));
    botoes[proximo]?.focus();
    e.preventDefault();
  }

  function teclaPainel(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    if (termo !== '' && (e.target as HTMLElement).tagName === 'INPUT') termo = '';
    else onfechar?.();
  }
</script>

{#if visivel}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <section class="paleta" aria-label="Paleta de símbolos" onkeydown={teclaPainel}>
    <header class="cabecalho">
      <div class="abas" role="tablist" aria-label="Categorias de símbolos" tabindex="-1" onkeydown={teclaAbas}>
        {#each CATEGORIAS as cat, i (cat.nome)}
          <button
            type="button"
            role="tab"
            id="paleta-aba-{i}"
            class="aba"
            aria-selected={!buscando && i === ativa}
            aria-controls="paleta-corpo"
            tabindex={!buscando && i === ativa ? 0 : -1}
            bind:this={abas[i]}
            onmousedown={manterFoco}
            onclick={() => escolherAba(i)}
          >
            {cat.nome}
          </button>
        {/each}
      </div>

      <label class="busca">
        <Search size={14} />
        <input
          type="text"
          placeholder="Buscar símbolo ou comando"
          aria-label="Buscar símbolo ou comando"
          autocomplete="off"
          spellcheck="false"
          bind:value={termo}
          bind:this={campoBusca}
        />
      </label>

      <button type="button" class="fechar" title="Fechar" aria-label="Fechar paleta de símbolos" onmousedown={manterFoco} onclick={() => onfechar?.()}>
        <X size={16} />
      </button>
    </header>

    <div class="corpo" id="paleta-corpo" role="tabpanel" aria-labelledby={buscando ? undefined : `paleta-aba-${ativa}`}>
      {#if lista.length > 0}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div class="grade" role="group" aria-label="Símbolos" onkeydown={teclaGrade}>
          {#each lista as s, i}
            <button
              type="button"
              class="simbolo"
              title={s.descricao ? `${s.comando} (${s.descricao})` : s.comando}
              aria-label={s.descricao ? `${s.descricao}, ${s.comando}` : s.comando}
              tabindex={i === 0 ? 0 : -1}
              onmousedown={manterFoco}
              onmouseenter={() => (destaque = s)}
              onmouseleave={() => (destaque = null)}
              onfocus={() => (destaque = s)}
              onblur={() => (destaque = null)}
              onclick={() => inserir(s)}
            >
              {#if s.glifo}
                <span class="glifo">{s.glifo}</span>
              {:else}
                <span class="rotulo">{s.comando}</span>
              {/if}
            </button>
          {/each}
        </div>
      {:else}
        <p class="vazio">{buscando ? 'Nenhum símbolo encontrado.' : 'Nenhum símbolo disponível.'}</p>
      {/if}
    </div>

    <footer class="rodape">
      {#if destaque}
        <code>{destaque.comando}</code>
        <span>{destaque.descricao}</span>
      {:else}
        <span>Clique em um símbolo para inserir no texto.</span>
      {/if}
      {#if buscando}
        <span class="contagem">{lista.length} {lista.length === 1 ? 'resultado' : 'resultados'}</span>
      {/if}
    </footer>
  </section>
{/if}

<style>
  .paleta {
    height: min(220px, 40dvh);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    background: var(--superficie-0);
    color: var(--texto);
    border-top: 1px solid var(--borda);
    font-size: 0.8125rem;
  }

  .cabecalho {
    height: 36px;
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    background: var(--superficie-1);
    border-bottom: 1px solid var(--borda);
  }

  .abas {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
  }

  .aba {
    flex: 0 0 auto;
    padding: 0 12px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--texto-secundario);
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }

  .aba:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .aba[aria-selected='true'] {
    background: var(--superficie-0);
    color: var(--primaria);
    border-bottom-color: var(--primaria);
  }

  .aba:focus-visible,
  .fechar:focus-visible,
  .simbolo:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .busca {
    flex: 0 0 200px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 5px 6px;
    padding: 0 8px;
    background: var(--superficie-0);
    color: var(--texto-terciario);
    border: 1px solid var(--borda);
    border-radius: 6px;
  }

  .busca:focus-within {
    border-color: var(--primaria);
    color: var(--primaria);
  }

  .busca input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
  }

  .busca input::placeholder {
    color: var(--texto-terciario);
  }

  .fechar {
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    align-self: center;
    margin-right: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .fechar:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .corpo {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 10px;
  }

  .grade {
    display: grid;
    grid-template-columns: repeat(auto-fill, 40px);
    gap: 6px;
  }

  .simbolo {
    width: 40px;
    height: 40px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--superficie-1);
    color: var(--texto);
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
  }

  .simbolo:hover {
    background: var(--primaria-suave);
    border-color: var(--primaria);
  }

  .glifo {
    font-family: 'Cambria Math', 'STIX Two Math', 'Segoe UI Symbol', 'Times New Roman', serif;
    font-size: 1.375rem;
    line-height: 1;
  }

  .rotulo {
    padding: 2px;
    overflow: hidden;
    overflow-wrap: anywhere;
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 9px;
    line-height: 1.1;
  }

  .vazio {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    color: var(--texto-secundario);
  }

  .rodape {
    height: 24px;
    box-sizing: border-box;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    overflow: hidden;
    white-space: nowrap;
    background: var(--superficie-1);
    border-top: 1px solid var(--borda);
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .rodape code {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-weight: 600;
    color: var(--texto);
  }

  .contagem {
    margin-left: auto;
    color: var(--texto-terciario);
  }
</style>
