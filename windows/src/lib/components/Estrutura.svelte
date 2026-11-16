<!--
  Estrutura do documento em arvore: parte, capitulo, secao, subsecao e subsubsecao.
  Comportamento baseado no painel de estrutura do Overleaf (AGPL-3.0), reescrito em Svelte.
-->
<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";

  interface Props {
    conteudo?: string;
    linhaAtual?: number;
    onirparalinha?: (linha: number) => void;
  }

  let { conteudo = '', linhaAtual = 0, onirparalinha }: Props = $props();

  interface Secao {
    /** Posicao da secao na lista plana, de cima para baixo. */
    indice: number;
    /** Linha (a partir de 1) onde o comando comeca. */
    linha: number;
    titulo: string;
    nivel: number;
    /** Identifica a secao para lembrar se ela esta recolhida, mesmo quando as linhas mudam. */
    chave: string;
    /** Indice do pai na lista plana, ou -1 na raiz. */
    pai: number;
    filhos: Secao[];
  }

  interface Analise {
    raiz: Secao[];
    plana: Secao[];
  }

  const NIVEIS: Record<string, number> = {
    part: 1,
    chapter: 2,
    section: 3,
    subsection: 4,
    subsubsection: 5,
  };

  const COMANDO_SECAO = /(?<!\\)\\(part|chapter|section|subsection|subsubsection)\*?(?![A-Za-z@])/g;
  const COMENTARIO = /(^|[^\\])((?:\\\\)*)%[^\n]*/g;
  const AMBIENTE_LITERAL = /\\begin\{(verbatim\*?|lstlisting|minted|comment)\}[\s\S]*?\\end\{\1\}/g;
  const DEFINICAO = /\\(?:re)?newcommand|\\providecommand|\\def(?![A-Za-z@])|\\let(?![A-Za-z@])/;
  const LIMITE_TITULO = 300;

  /** Le um grupo entre delimitadores (chaves ou colchetes) com profundidade e escapes. */
  function lerGrupo(texto: string, inicio: number, abre: string, fecha: string): { interno: string; fim: number } | null {
    const limite = Math.min(texto.length, inicio + LIMITE_TITULO);
    let profundidade = 0;
    for (let i = inicio; i < limite; i++) {
      const c = texto[i];
      if (c === '\\') {
        i++;
      } else if (c === abre) {
        profundidade++;
      } else if (c === fecha) {
        profundidade--;
        if (profundidade === 0) return { interno: texto.slice(inicio + 1, i), fim: i + 1 };
      }
    }
    return null;
  }

  function pularEspacos(texto: string, inicio: number): number {
    let i = inicio;
    while (i < texto.length && /\s/.test(texto[i])) i++;
    return i;
  }

  /** Devolve o titulo longo (entre chaves) que vem depois do comando, ou null se nao houver. */
  function lerTitulo(texto: string, inicio: number): string | null {
    let i = pularEspacos(texto, inicio);
    if (texto[i] === '[') {
      const curto = lerGrupo(texto, i, '[', ']');
      if (!curto) return null;
      i = pularEspacos(texto, curto.fim);
    }
    if (texto[i] !== '{') return null;
    const grupo = lerGrupo(texto, i, '{', '}');
    if (grupo) return grupo.interno;
    // Chave sem fechamento: usa o resto da linha para a secao nao sumir da lista.
    const fimLinha = texto.indexOf('\n', i);
    return texto.slice(i + 1, fimLinha === -1 ? texto.length : fimLinha);
  }

  /** Tira rotulos e formatacao simples do titulo para ele ficar legivel na lista. */
  function limparTitulo(bruto: string): string {
    let t = bruto
      .replace(/\\label\s*\{[^{}]*\}/g, '')
      .replace(/\\texorpdfstring\s*\{(?:[^{}]|\{[^{}]*\})*\}\s*\{((?:[^{}]|\{[^{}]*\})*)\}/g, '$1');
    for (let i = 0; i < 4; i++) {
      const sem = t.replace(
        /\\(?:textbf|textit|emph|texttt|textsc|textrm|textsf|textsl|underline|mbox|text)\s*\{([^{}]*)\}/g,
        '$1',
      );
      if (sem === t) break;
      t = sem;
    }
    return t
      .replace(/\\([&%_#$])/g, '$1')
      .replace(/\\\\/g, ' ')
      .replace(/~/g, ' ')
      .replace(/[{}]/g, '')
      .replace(/\s+/g, ' ')
      .trim();
  }

  function analisar(texto: string): Analise {
    const raiz: Secao[] = [];
    const plana: Secao[] = [];
    if (!texto) return { raiz, plana };

    // Comentarios e ambientes literais nao contam: viram espacos, e as posicoes e as linhas continuam iguais.
    const limpo = texto
      .replace(
        COMENTARIO,
        (achado: string, antes: string, barras: string) =>
          antes + barras + ' '.repeat(achado.length - antes.length - barras.length),
      )
      .replace(AMBIENTE_LITERAL, (achado: string) => achado.replace(/[^\n]/g, ' '));

    const pilha: Secao[] = [];
    const repeticoes = new Map<string, number>();
    let linha = 1;
    let posicao = 0;

    for (const achado of limpo.matchAll(COMANDO_SECAO)) {
      const inicio = achado.index ?? 0;
      for (; posicao < inicio; posicao++) {
        if (limpo.charCodeAt(posicao) === 10) linha++;
      }

      // Definicoes como \newcommand{\x}{\section{...}} nao sao secoes do documento.
      const inicioLinha = limpo.lastIndexOf('\n', inicio - 1) + 1;
      if (DEFINICAO.test(limpo.slice(inicioLinha, inicio))) continue;

      const bruto = lerTitulo(limpo, inicio + achado[0].length);
      if (bruto === null) continue;

      const titulo = limparTitulo(bruto);
      const nivel = NIVEIS[achado[1]];
      const base = `${nivel}|${titulo}`;
      const vez = repeticoes.get(base) ?? 0;
      repeticoes.set(base, vez + 1);

      // O pai e a secao anterior mais proxima de nivel menor (igual ao aninhamento do Overleaf).
      while (pilha.length > 0 && pilha[pilha.length - 1].nivel >= nivel) pilha.pop();
      const pai = pilha.length > 0 ? pilha[pilha.length - 1] : null;

      const secao: Secao = {
        indice: plana.length,
        linha,
        titulo,
        nivel,
        chave: `${base}|${vez}`,
        pai: pai ? pai.indice : -1,
        filhos: [],
      };
      if (pai) pai.filhos.push(secao);
      else raiz.push(secao);
      pilha.push(secao);
      plana.push(secao);
    }

    return { raiz, plana };
  }

  const analise = $derived.by(() => analisar(conteudo));

  /** Ultima secao cuja linha e menor ou igual a linha do cursor. */
  const indiceAtivo = $derived.by(() => {
    let ativo = -1;
    for (const secao of analise.plana) {
      if (secao.linha <= linhaAtual) ativo = secao.indice;
      else break;
    }
    return ativo;
  });

  /** Ancestrais da secao ativa: se um deles estiver recolhido, ele recebe o destaque. */
  const ancestraisDoAtivo = $derived.by(() => {
    const conjunto = new Set<number>();
    let i = indiceAtivo >= 0 ? analise.plana[indiceAtivo].pai : -1;
    while (i >= 0) {
      conjunto.add(i);
      i = analise.plana[i].pai;
    }
    return conjunto;
  });

  const recolhidas = new SvelteSet<string>();

  function alternar(chave: string) {
    if (recolhidas.has(chave)) recolhidas.delete(chave);
    else recolhidas.add(chave);
  }

  let raizEl = $state<HTMLElement | null>(null);

  // Mantem a secao do cursor visivel quando a lista tem rolagem.
  $effect(() => {
    if (!raizEl || indiceAtivo < 0) return;
    raizEl.querySelector<HTMLElement>('[data-ativo="true"]')?.scrollIntoView({ block: 'nearest' });
  });
</script>

{#snippet no(secao: Secao)}
  {@const temFilhos = secao.filhos.length > 0}
  {@const recolhida = recolhidas.has(secao.chave)}
  {@const destacada = secao.indice === indiceAtivo || (recolhida && ancestraisDoAtivo.has(secao.indice))}
  <li>
    <div class="linha">
      {#if temFilhos}
        <button
          type="button"
          class="seta"
          aria-expanded={!recolhida}
          aria-label={recolhida ? 'Expandir' : 'Recolher'}
          title={recolhida ? 'Expandir' : 'Recolher'}
          onclick={() => alternar(secao.chave)}
        >
          {#if recolhida}
            <ChevronRight size={14} />
          {:else}
            <ChevronDown size={14} />
          {/if}
        </button>
      {:else}
        <span class="seta-vazia"></span>
      {/if}
      <button
        type="button"
        class="item"
        class:ativo={destacada}
        data-ativo={destacada}
        aria-current={destacada ? 'location' : undefined}
        title={secao.titulo}
        onclick={() => onirparalinha?.(secao.linha)}
      >
        <span class="texto">{secao.titulo || '(sem título)'}</span>
      </button>
    </div>
    {#if temFilhos && !recolhida}
      <ul class="lista aninhada">
        {#each secao.filhos as filho (filho.indice)}
          {@render no(filho)}
        {/each}
      </ul>
    {/if}
  </li>
{/snippet}

<div class="estrutura" bind:this={raizEl}>
  {#if analise.raiz.length > 0}
    <ul class="lista">
      {#each analise.raiz as secao (secao.indice)}
        {@render no(secao)}
      {/each}
    </ul>
  {:else}
    <p class="vazio">Nenhuma seção encontrada neste arquivo.</p>
  {/if}
</div>

<style>
  .estrutura {
    width: 100%;
    padding: 4px 0;
    box-sizing: border-box;
    font-size: 0.8125rem;
  }

  .lista {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .lista.aninhada {
    margin-left: 12px;
    padding-left: 3px;
    border-left: 1px solid var(--borda);
  }

  .linha {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .seta,
  .seta-vazia {
    flex: 0 0 18px;
    width: 18px;
    height: 24px;
  }

  .seta {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-terciario);
    cursor: pointer;
  }

  .seta:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .item {
    flex: 1 1 auto;
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto-secundario);
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    cursor: pointer;
  }

  .item:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .item.ativo {
    background: var(--primaria-suave);
    color: var(--primaria);
    font-weight: 600;
  }

  .seta:focus-visible,
  .item:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }

  .texto {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .vazio {
    margin: 0;
    padding: 10px 12px;
    color: var(--texto-terciario);
    font-style: italic;
    text-align: center;
  }
</style>
