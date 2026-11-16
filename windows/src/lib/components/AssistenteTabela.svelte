<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { untrack } from "svelte";
  import { gerarTabela, lerCsv, type Alinhamento, type Linhas } from "$lib/tabela";
  import { invoke } from "@tauri-apps/api/core";
  import EscolherArquivoGoogle from "./EscolherArquivoGoogle.svelte";
  import IconeDrive from "./IconeDrive.svelte";

  interface Props {
    visivel?: boolean;
    /** Tamanho inicial, como "3x4" (linhas x colunas). */
    tamanho?: string;
    oninserir?: (codigo: string) => void;
    onfechar?: () => void;
  }

  let { visivel = false, tamanho = '3x3', oninserir, onfechar }: Props = $props();

  const MAXIMO = 20;
  const limitar = (n: number) => Math.max(1, Math.min(MAXIMO, Math.round(n) || 1));

  let qtdLinhas = $state(3);
  let qtdColunas = $state(3);
  let celulas = $state<string[][]>([]);
  let alinhamentos = $state<Alinhamento[]>([]);
  let linhas = $state<Linhas>('simples');
  let cabecalhoNegrito = $state(true);
  let flutuante = $state(true);
  let legenda = $state('');
  let rotulo = $state('');

  // cada vez que abre, comeca do tamanho escolhido e com as celulas vazias
  // so `visivel` e `tamanho` disparam: ler qtdLinhas aqui dentro faria o efeito rodar de
  // novo a cada mudanca de tamanho e apagar o que a pessoa ja digitou
  $effect(() => {
    if (!visivel) return;
    const pedido = tamanho;
    untrack(() => {
      const [l, c] = pedido.split('x').map((n) => limitar(parseInt(n, 10)));
      qtdLinhas = l || 3;
      qtdColunas = c || 3;
      celulas = Array.from({ length: qtdLinhas }, () => Array(qtdColunas).fill(''));
      alinhamentos = Array(qtdColunas).fill('c');
      legenda = '';
      rotulo = '';
    });
  });

  // mudar o numero de linhas ou colunas preserva o que ja foi digitado
  function redimensionar(l: number, c: number) {
    qtdLinhas = limitar(l);
    qtdColunas = limitar(c);
    celulas = Array.from({ length: qtdLinhas }, (_, i) =>
      Array.from({ length: qtdColunas }, (_, j) => celulas[i]?.[j] ?? ''),
    );
    alinhamentos = Array.from({ length: qtdColunas }, (_, j) => alinhamentos[j] ?? 'c');
  }

  // Google Planilhas: a primeira aba vira as celulas (ate 20 x 20)
  let escolhendoPlanilha = $state(false);
  let avisoImportacao = $state('');
  async function importarPlanilha(arquivo: { id: string; nome: string }) {
    escolhendoPlanilha = false;
    avisoImportacao = `Lendo ${arquivo.nome}...`;
    try {
      const csv = await invoke<string>('cmd_drive_planilha_csv', { id: arquivo.id });
      const dados = lerCsv(csv);
      if (!dados.length) {
        avisoImportacao = 'A planilha está vazia.';
        return;
      }
      const colunas = Math.max(...dados.map((l) => l.length));
      const l = limitar(dados.length);
      const c = limitar(colunas);
      qtdLinhas = l;
      qtdColunas = c;
      celulas = Array.from({ length: l }, (_, i) => Array.from({ length: c }, (_, j) => dados[i]?.[j] ?? ''));
      alinhamentos = Array.from({ length: c }, (_, j) => alinhamentos[j] ?? 'c');
      if (!legenda) legenda = arquivo.nome;
      avisoImportacao =
        dados.length > MAXIMO || colunas > MAXIMO
          ? `A planilha tem ${dados.length} x ${colunas}; entraram as primeiras ${l} linhas e ${c} colunas.`
          : `Importado de ${arquivo.nome}.`;
    } catch (e) {
      avisoImportacao = `Não foi possível ler a planilha: ${e}`;
    }
  }

  const codigo = $derived(
    gerarTabela({ celulas, alinhamentos, linhas, cabecalhoNegrito, flutuante, legenda, rotulo }),
  );

  function inserir() {
    oninserir?.(codigo);
    onfechar?.();
  }

  const OPCOES_LINHAS: { valor: Linhas; rotulo: string }[] = [
    { valor: 'simples', rotulo: 'Linhas no topo, no cabeçalho e no fim' },
    { valor: 'grade', rotulo: 'Grade completa' },
    { valor: 'booktabs', rotulo: 'Estilo de livro (pacote booktabs)' },
    { valor: 'nenhuma', rotulo: 'Sem linhas' },
  ];
  const NOME_ALINHAMENTO: Record<Alinhamento, string> = { l: 'Esquerda', c: 'Centro', r: 'Direita' };

  let pressionouNoFundo = false;
  function aoPressionarFundo(e: MouseEvent) {
    pressionouNoFundo = e.target === e.currentTarget;
  }
  function aoClicarFundo(e: MouseEvent) {
    if (pressionouNoFundo && e.target === e.currentTarget) onfechar?.();
    pressionouNoFundo = false;
  }
  function aoTeclar(e: KeyboardEvent) {
    if (!visivel) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      onfechar?.();
    } else if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault();
      inserir();
    }
  }
  function focarPrimeira(no: HTMLElement) {
    const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    no.querySelector<HTMLInputElement>('.celula')?.focus();
    return { destroy: () => anterior?.focus() };
  }
</script>

<svelte:window onkeydown={aoTeclar} />

{#if visivel}
  <div class="fundo-modal" role="presentation" onmousedown={aoPressionarFundo} onclick={aoClicarFundo}>
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="titulo-tabela" tabindex="-1" use:focarPrimeira>
      <header>
        <h2 id="titulo-tabela">Assistente de tabela</h2>
        <button class="fechar" onclick={onfechar} title="Fechar" aria-label="Fechar"><X size={18} /></button>
      </header>

      <div class="corpo">
        <div class="tamanho">
          <label>Linhas
            <input type="number" min="1" max={MAXIMO} value={qtdLinhas} onchange={(e) => redimensionar(+e.currentTarget.value, qtdColunas)} />
          </label>
          <label>Colunas
            <input type="number" min="1" max={MAXIMO} value={qtdColunas} onchange={(e) => redimensionar(qtdLinhas, +e.currentTarget.value)} />
          </label>
          <button class="botao importar" onclick={() => (escolhendoPlanilha = true)} title="Trazer as células de uma planilha do Google Drive">
            <IconeDrive size={14} /> Importar do Google Planilhas
          </button>
        </div>
        {#if avisoImportacao}<p class="nota">{avisoImportacao}</p>{/if}

        <div class="grade-celulas" style:grid-template-columns="repeat({qtdColunas}, minmax(90px, 1fr))">
          {#each alinhamentos as _, j (j)}
            <select aria-label="Alinhamento da coluna {j + 1}" bind:value={alinhamentos[j]}>
              {#each ['l', 'c', 'r'] as const as a (a)}
                <option value={a}>{NOME_ALINHAMENTO[a]}</option>
              {/each}
            </select>
          {/each}
          {#each celulas as linha, i (i)}
            {#each linha as _, j (j)}
              <input
                class="celula"
                class:cabecalho={i === 0 && cabecalhoNegrito}
                aria-label="Linha {i + 1}, coluna {j + 1}"
                placeholder={i === 0 && cabecalhoNegrito ? 'Título' : ''}
                spellcheck="false"
                bind:value={celulas[i][j]}
              />
            {/each}
          {/each}
        </div>

        <div class="opcoes">
          <label class="campo">Linhas da tabela
            <select bind:value={linhas}>
              {#each OPCOES_LINHAS as o (o.valor)}
                <option value={o.valor}>{o.rotulo}</option>
              {/each}
            </select>
          </label>
          <label class="marcar"><input type="checkbox" bind:checked={cabecalhoNegrito} /> Primeira linha em negrito (cabeçalho)</label>
          <label class="marcar"><input type="checkbox" bind:checked={flutuante} /> Tabela numerada, com legenda e rótulo</label>
          {#if flutuante}
            <label class="campo">Legenda <input bind:value={legenda} placeholder="Ex.: Modos Raman do TiO$_2$" /></label>
            <label class="campo">Rótulo (para o \ref) <input bind:value={rotulo} placeholder="Ex.: modos-raman" spellcheck="false" /></label>
          {/if}
        </div>

        <details>
          <summary>Ver o código LaTeX</summary>
          <pre>{codigo}</pre>
        </details>
        {#if linhas === 'booktabs'}
          <p class="nota">O estilo de livro precisa de \usepackage{'{'}booktabs{'}'} no preâmbulo.</p>
        {/if}
      </div>

      <footer>
        <button class="botao" onclick={onfechar}>Cancelar</button>
        <button class="botao primario" onclick={inserir} title="Ctrl+Enter">Inserir tabela</button>
      </footer>
    </div>
  </div>
{/if}

<EscolherArquivoGoogle
  visivel={escolhendoPlanilha}
  tipo="planilha"
  onescolher={importarPlanilha}
  onfechar={() => (escolhendoPlanilha = false)}
/>

<style>
  .importar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    padding: 4px 10px;
    font-size: 0.8125rem;
  }

  .fundo-modal {
    position: fixed;
    inset: 0;
    z-index: 2000;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: brightness(0.5);
  }

  .modal {
    width: min(760px, 94vw);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    box-shadow: var(--sombra-3);
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid var(--borda);
  }

  h2 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
  }

  .fechar {
    display: flex;
    padding: 4px;
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .fechar:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .corpo {
    padding: 14px 18px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    font-size: 0.875rem;
  }

  .tamanho {
    display: flex;
    gap: 18px;
  }

  .tamanho label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--texto-secundario);
  }

  .tamanho input {
    width: 64px;
  }

  .grade-celulas {
    display: grid;
    gap: 4px;
    overflow-x: auto;
  }

  input,
  select {
    padding: 5px 8px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-1, var(--superficie-0));
    color: var(--texto);
    font: inherit;
    font-size: 0.8125rem;
    min-width: 0;
  }

  .grade-celulas select {
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .celula.cabecalho {
    font-weight: 600;
  }

  input:focus-visible,
  select:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 0;
  }

  .opcoes {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .campo {
    display: grid;
    grid-template-columns: 150px 1fr;
    align-items: center;
    gap: 10px;
    color: var(--texto-secundario);
  }

  .marcar {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  details summary {
    cursor: pointer;
    color: var(--texto-secundario);
  }

  pre {
    margin: 8px 0 0;
    padding: 10px;
    border-radius: var(--raio-sm);
    background: var(--superficie-2, var(--hover));
    font-size: 0.75rem;
    white-space: pre-wrap;
  }

  .nota {
    margin: 0;
    font-size: 0.75rem;
    color: var(--texto-terciario);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--borda);
  }

  .botao {
    padding: 7px 18px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .botao:hover {
    background: var(--hover);
  }

  .botao.primario {
    background: var(--primaria);
    border-color: var(--primaria);
    color: #fff;
  }

  .botao.primario:hover {
    filter: brightness(1.08);
  }

  .fechar:focus-visible,
  .botao:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 1px;
  }
</style>
