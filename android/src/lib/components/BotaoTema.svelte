<script lang="ts">
  // Alterna entre modo claro e escuro. Grava a escolha nas preferencias.
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import { preferencias, definirPreferencia } from '$lib/preferencias.svelte';

  interface Props {
    comTexto?: boolean;
  }
  let { comTexto = false }: Props = $props();

  let sistemaEscuro = $state(
    typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches,
  );

  $effect(() => {
    const consulta = window.matchMedia('(prefers-color-scheme: dark)');
    const mudar = (e: MediaQueryListEvent) => (sistemaEscuro = e.matches);
    consulta.addEventListener('change', mudar);
    return () => consulta.removeEventListener('change', mudar);
  });

  let escuro = $derived(
    preferencias.tema === 'escuro' || (preferencias.tema === 'sistema' && sistemaEscuro),
  );
  let rotulo = $derived(escuro ? 'Mudar para o modo claro' : 'Mudar para o modo escuro');

  function alternar() {
    definirPreferencia('tema', escuro ? 'claro' : 'escuro');
  }
</script>

<button class="botao-tema" class:com-texto={comTexto} onclick={alternar} title={rotulo} aria-label={rotulo}>
  {#if escuro}<Sun size={comTexto ? 17 : 15} />{:else}<Moon size={comTexto ? 17 : 15} />{/if}
  {#if comTexto}<span>{escuro ? 'Modo claro' : 'Modo escuro'}</span>{/if}
</button>

<style>
  .botao-tema {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 28px;
    min-width: 28px;
    padding: 0 6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .botao-tema:hover {
    background: var(--hover);
  }
  .com-texto {
    justify-content: flex-start;
    width: 100%;
    height: auto;
    padding: 9px 12px;
    font-size: 0.92rem;
  }
</style>
