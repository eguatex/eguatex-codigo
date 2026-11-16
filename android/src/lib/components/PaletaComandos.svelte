<script lang="ts">
  import { tick } from 'svelte';
  import { listarComandos, executar, type Acoes } from '$lib/comandos';

  interface Props {
    visivel?: boolean;
    acoes?: Acoes;
    onfechar?: () => void;
  }

  let { visivel = false, acoes = {}, onfechar }: Props = $props();

  const todos = listarComandos();
  let busca = $state('');
  let indice = $state(0);
  let campo: HTMLInputElement | undefined = $state();

  const resultados = $derived.by(() => {
    const termo = busca.trim().toLowerCase();
    return todos.filter(
      (c) => acoes[c.id] && (!termo || c.nome.toLowerCase().includes(termo) || c.caminho.toLowerCase().includes(termo))
    );
  });

  $effect(() => {
    if (visivel) {
      busca = '';
      indice = 0;
      tick().then(() => campo?.focus());
    }
  });

  function rodar(id: string) {
    onfechar?.();
    executar(acoes, id);
  }

  function tecla(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      indice = Math.min(indice + 1, resultados.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      indice = Math.max(indice - 1, 0);
    } else if (e.key === 'Enter' && resultados[indice]) {
      e.preventDefault();
      rodar(resultados[indice].id);
    } else if (e.key === 'Escape') {
      onfechar?.();
    }
  }
</script>

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="fundo" onclick={onfechar}>
    <div class="paleta" role="dialog" aria-modal="true" aria-label="Paleta de comandos" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <input
        bind:this={campo}
        bind:value={busca}
        oninput={() => (indice = 0)}
        onkeydown={tecla}
        placeholder="Digite um comando..."
        aria-label="Buscar comando"
      />
      <div class="lista" role="listbox">
        {#each resultados as c, i}
          <button
            class="opcao"
            class:selecionada={i === indice}
            role="option"
            aria-selected={i === indice}
            onmouseenter={() => (indice = i)}
            onclick={() => rodar(c.id)}
          >
            <span class="nome">{c.nome}</span>
            <span class="caminho">{c.caminho}</span>
            {#if c.atalho}<kbd>{c.atalho}</kbd>{/if}
          </button>
        {:else}
          <p class="vazio">Nenhum comando encontrado</p>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .fundo {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    z-index: 2000;
  }

  .paleta {
    width: min(560px, 92vw);
    background: var(--superficie-0);
    border: 1px solid var(--borda);
    border-radius: 8px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }

  input {
    width: 100%;
    box-sizing: border-box;
    padding: 12px 14px;
    border: none;
    border-bottom: 1px solid var(--borda);
    background: transparent;
    color: var(--texto);
    font-size: 0.9rem;
    font-family: inherit;
    outline: none;
  }

  .lista {
    max-height: 50vh;
    overflow: auto;
    padding: 4px 0;
  }

  .opcao {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 14px;
    border: none;
    background: transparent;
    color: var(--texto);
    font-size: 0.8125rem;
    text-align: left;
    cursor: pointer;
    font-family: inherit;
  }

  .opcao.selecionada {
    background: var(--hover);
  }

  .nome {
    flex-shrink: 0;
  }

  .caminho {
    flex: 1;
    color: var(--texto-terciario);
    font-size: 0.75rem;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  kbd {
    padding: 1px 6px;
    border: 1px solid var(--borda);
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
    color: var(--texto-secundario);
  }

  .vazio {
    margin: 10px 14px;
    color: var(--texto-secundario);
    font-size: 0.8125rem;
  }
</style>
