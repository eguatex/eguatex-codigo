<script lang="ts">
  // Mostra a tarefa mais recente do registro. Aparece so depois de 300 ms.
  // modo "rodape": rotulo, porcentagem e trilho de 3 px (dentro da StatusBar).
  // modo "faixa": so o trilho de 3 px (celular deitado, sem rodape).
  import { t } from "$lib/i18n";
  import { ATRASO_BARRA_MS, porcentagem, tarefaVisivel, tarefas } from "$lib/tarefas.svelte";

  interface Props {
    modo?: 'rodape' | 'faixa';
  }

  let { modo = 'rodape' }: Props = $props();

  let agora = $state(Date.now());

  // o relogio so roda enquanto ha tarefa; com a lista vazia nao ha timer
  $effect(() => {
    if (tarefas.lista.length === 0) return;
    agora = Date.now();
    const timer = setInterval(() => (agora = Date.now()), ATRASO_BARRA_MS / 3);
    return () => clearInterval(timer);
  });

  const tarefa = $derived(tarefaVisivel(tarefas.lista, agora));
  const pct = $derived(tarefa ? porcentagem(tarefa) : null);
</script>

{#if tarefa}
  <div
    class="barra-progresso"
    class:faixa={modo === 'faixa'}
    role="progressbar"
    aria-label={tarefa.rotulo}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={pct ?? undefined}
    title={t('tarefa.andamento')}
  >
    {#if modo === 'rodape'}
      <span class="rotulo">{tarefa.rotulo}</span>
      {#if pct !== null}<span class="pct">{pct}%</span>{/if}
    {/if}
    <span class="trilho">
      <span class="preenchimento" class:continua={pct === null} style={pct === null ? '' : `width: ${pct}%`}></span>
    </span>
  </div>
{/if}

<style>
  /* No rodape o trilho se ancora na borda de baixo da StatusBar (position: relative). */
  .barra-progresso {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .barra-progresso.faixa {
    display: block;
    padding: 0;
    width: 100%;
  }

  .trilho {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.2);
  }

  .faixa .trilho {
    position: relative;
    display: block;
    background: var(--borda, rgba(128, 128, 128, 0.3));
  }

  .preenchimento {
    display: block;
    height: 3px;
    background: var(--alerta-suave, #f5c542);
    transition: width 0.15s linear;
  }

  .faixa .preenchimento {
    background: var(--primaria);
  }

  .preenchimento.continua {
    width: 35%;
    animation: barra-indeterminada 1.2s ease-in-out infinite;
  }

  @keyframes barra-indeterminada {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .preenchimento.continua {
      width: 100%;
      animation: none;
      opacity: 0.6;
    }
  }
</style>
