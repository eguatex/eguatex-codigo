<script lang="ts">
  // Macros: pares "texto" -> "codigo". No editor, o texto seguido de espaco vira o codigo (editor/macros.ts).
  import { onMount } from 'svelte';
  import X from '@lucide/svelte/icons/x';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import { t } from '$lib/i18n';
  import { atalhos, alterarAtalhos } from '$lib/atalhos.svelte';
  import { registrarFechavel } from '$lib/voltar';
  import { definirMacro, macroValida, removerMacro } from '$lib/barra-matematica';

  interface Props {
    aoFechar: () => void;
  }

  let { aoFechar }: Props = $props();

  let texto = $state('');
  let codigo = $state('');
  let erro = $state(false);

  function adicionar(e: Event) {
    e.preventDefault();
    const macro = { texto: texto.trim(), codigo };
    if (!macroValida(macro) || !codigo.trim()) {
      erro = true;
      return;
    }
    alterarAtalhos((u) => definirMacro(u, macro));
    texto = '';
    codigo = '';
    erro = false;
  }

  function tecla(e: KeyboardEvent) {
    if (e.key === 'Escape') aoFechar();
  }

  // o botao Voltar do Android fecha o modal em vez de sair do app (o modal so existe enquanto aberto)
  onMount(() => registrarFechavel(aoFechar));
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-overlay" data-modal-fundo role="dialog" aria-modal="true" aria-label={t('atalhos.macros-titulo')} tabindex="-1" onclick={aoFechar} onkeydown={tecla}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal-conteudo" data-modal-caixa role="document" onclick={(e) => e.stopPropagation()}>
    <div class="cabecalho" data-modal-cabecalho>
      <h2>{t('atalhos.macros-titulo')}</h2>
      <button type="button" class="icone" data-modal-fechar onclick={aoFechar} aria-label={t('atalhos.fechar')} title={t('atalhos.fechar')}><X size={18} /></button>
    </div>
    <div class="corpo" data-modal-corpo>
      <p class="nota">{t('atalhos.macros-descricao')}</p>
      <ul class="lista">
        {#each atalhos.usuario.macros as m (m.texto)}
          <li class="linha">
            <code>{m.texto}</code>
            <ArrowRight size={14} />
            <code class="codigo" title={m.codigo}>{m.codigo}</code>
            <button type="button" class="icone" aria-label={t('atalhos.remover')} title={t('atalhos.remover')} onclick={() => alterarAtalhos((u) => removerMacro(u, m.texto))}><Trash2 size={16} /></button>
          </li>
        {:else}
          <li class="nota">{t('atalhos.macros-vazio')}</li>
        {/each}
      </ul>
      <form class="formulario" onsubmit={adicionar}>
        <label>
          {t('atalhos.macro-texto')}
          <input type="text" bind:value={texto} maxlength="40" oninput={() => (erro = false)} spellcheck="false" />
        </label>
        <label>
          {t('atalhos.macro-codigo')}
          <input type="text" bind:value={codigo} maxlength="2000" oninput={() => (erro = false)} spellcheck="false" />
        </label>
        {#if erro}<p class="mensagem-erro">{t('atalhos.macro-erro')}</p>{/if}
        <div>
          <button type="submit" class="primario" disabled={!texto.trim() || !codigo.trim()}>{t('atalhos.macro-adicionar')}</button>
        </div>
      </form>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
  }

  .modal-conteudo {
    width: 100%;
    max-width: 520px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 12px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
  }

  .cabecalho {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    border-bottom: 1px solid var(--borda);
  }

  h2 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .corpo {
    padding: 16px 20px 20px;
    overflow-y: auto;
  }

  .nota {
    margin: 0 0 10px;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
  }

  .lista {
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
  }

  .linha {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
  }

  .codigo {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--texto-secundario);
  }

  .icone {
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .icone:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .formulario {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
  }

  input[type='text'] {
    box-sizing: border-box;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
  }

  .primario {
    padding: 6px 12px;
    border: none;
    border-radius: 6px;
    background: var(--primaria);
    color: #fff;
    font: inherit;
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .primario:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .mensagem-erro {
    margin: 0;
    font-size: 0.8125rem;
    color: #c62828;
  }

  /* toque: remover de 44 x 44, campos e botao com 44 px de altura */
  @media (pointer: coarse) {
    .linha {
      min-height: 48px;
    }

    .icone {
      width: 44px;
      height: 44px;
    }

    input[type='text'] {
      min-height: 44px;
    }

    .primario {
      min-height: 44px;
      padding: 0 16px;
    }
  }
</style>
