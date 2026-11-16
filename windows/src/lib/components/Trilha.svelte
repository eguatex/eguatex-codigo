<script lang="ts">
  // Trilha do arquivo e da secao atual, no estilo dos breadcrumbs do editor do Overleaf
  // (services/web/frontend/js/features/source-editor/extensions/breadcrumbs.tsx, AGPL-3.0).
  import { t } from '$lib/i18n';

  interface Props {
    arquivo?: string;
    secao?: string;
  }

  let { arquivo = '', secao = '' }: Props = $props();

  const completo = $derived(arquivo && secao ? `${arquivo} > ${secao}` : arquivo || secao);
</script>

<nav class="trilha" aria-label={t('trilha.rotulo')} title={completo}>
  {#if arquivo}
    <span class="arquivo">{arquivo}</span>
  {/if}
  {#if arquivo && secao}
    <span class="separador" aria-hidden="true">&gt;</span>
  {/if}
  {#if secao}
    <span class="secao">{secao}</span>
  {/if}
</nav>

<style>
  .trilha {
    /* medidas de breadcrumbs-panel.ts do Overleaf: 28 px, padding 4 px, gap 2 px */
    height: 28px;
    box-sizing: border-box;
    padding: 4px;
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    overflow: hidden;
    white-space: nowrap;
    font-size: 0.75rem;
    line-height: 1;
    color: var(--texto-secundario);
    background: var(--superficie-0);
    border-bottom: 1px solid var(--borda);
  }

  .arquivo {
    flex: 0 1 auto;
    min-width: 0;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .separador {
    flex-shrink: 0;
    color: var(--texto-terciario);
  }

  .secao {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
