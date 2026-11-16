<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { contarPalavras } from "$lib/contagem";

  interface Props {
    visivel?: boolean;
    texto?: string;
    onfechar?: () => void;
  }

  let { visivel = false, texto = '', onfechar }: Props = $props();

  const formato = new Intl.NumberFormat('pt-BR');

  // So conta quando a janela esta aberta, para nao recontar a cada tecla digitada.
  const resultado = $derived(visivel ? contarPalavras(texto) : null);

  const linhas = $derived(
    resultado
      ? [
          { rotulo: 'Palavras no texto', valor: resultado.palavrasTexto },
          { rotulo: 'Palavras nos títulos', valor: resultado.palavrasTitulos },
          { rotulo: 'Palavras fora do texto (legendas e notas)', valor: resultado.palavrasFora },
          { rotulo: 'Número de títulos', valor: resultado.titulos },
          { rotulo: 'Figuras e tabelas', valor: resultado.flutuantes },
          { rotulo: 'Equações em linha', valor: resultado.matematicaLinha },
          { rotulo: 'Equações em destaque', valor: resultado.matematicaDestaque },
        ]
      : [],
  );

  // Fecha so quando o clique comecou e terminou no fundo (arrastar a selecao para fora nao fecha).
  let pressionouNoFundo = false;

  function aoPressionarFundo(e: MouseEvent) {
    pressionouNoFundo = e.target === e.currentTarget;
  }

  function aoClicarFundo(e: MouseEvent) {
    if (pressionouNoFundo && e.target === e.currentTarget) onfechar?.();
    pressionouNoFundo = false;
  }

  function aoTeclar(e: KeyboardEvent) {
    if (visivel && e.key === 'Escape') {
      e.preventDefault();
      onfechar?.();
    }
  }

  // Leva o foco para a janela ao abrir e devolve ao elemento anterior ao fechar.
  function focarJanela(no: HTMLElement) {
    const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    no.focus();
    return {
      destroy() {
        anterior?.focus();
      },
    };
  }
</script>

<svelte:window onkeydown={aoTeclar} />

{#if visivel}
  <div class="fundo-modal" role="presentation" onmousedown={aoPressionarFundo} onclick={aoClicarFundo}>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="titulo-contagem"
      tabindex="-1"
      use:focarJanela
    >
      <header>
        <h2 id="titulo-contagem">Contagem de palavras</h2>
        <button class="fechar" onclick={onfechar} title="Fechar" aria-label="Fechar"><X size={18} /></button>
      </header>

      <div class="corpo">
        <table aria-label="Resultado da contagem">
          <tbody>
            {#each linhas as linha (linha.rotulo)}
              <tr>
                <th scope="row">{linha.rotulo}</th>
                <td>{formato.format(linha.valor)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="nota">Contagem aproximada do texto atual. Comentários e preâmbulo não entram.</p>
      </div>

      <footer>
        <button class="botao" onclick={onfechar}>Fechar</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .fundo-modal {
    position: fixed;
    inset: 0;
    z-index: 2000;
    display: flex;
    align-items: center;
    justify-content: center;
    /* escurece o que esta atras sem usar cor fixa */
    backdrop-filter: brightness(0.5);
  }

  .modal {
    width: min(520px, 92vw);
    max-height: 85vh;
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
    padding: 8px 18px 14px;
    overflow: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    padding: 10px 0;
    border-bottom: 1px solid var(--borda);
    font-size: 0.875rem;
  }

  th {
    text-align: left;
    font-weight: 400;
    color: var(--texto-secundario);
  }

  td {
    padding-left: 16px;
    text-align: right;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* a primeira linha e o numero principal */
  tr:first-child th,
  tr:first-child td {
    font-size: 1rem;
    color: var(--texto);
  }

  tr:last-child th,
  tr:last-child td {
    border-bottom: none;
  }

  .nota {
    margin: 10px 0 0;
    font-size: 0.75rem;
    color: var(--texto-terciario);
  }

  footer {
    display: flex;
    justify-content: flex-end;
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

  .fechar:focus-visible,
  .botao:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 1px;
  }
</style>
