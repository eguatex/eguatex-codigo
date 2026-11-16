<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { t } from "$lib/i18n";

  interface Props {
    visivel?: boolean;
    onfechar?: () => void;
  }

  let { visivel = false, onfechar }: Props = $props();

  const grupos: Array<{ titulo: string; itens: Array<[string, string]> }> = [
    {
      titulo: 'Geral',
      itens: [
        ['Ctrl + S', 'Salvar'],
        ['Ctrl + Enter', 'Compilar'],
      ],
    },
    {
      titulo: 'Edição',
      itens: [
        ['Ctrl + Z', 'Desfazer'],
        ['Ctrl + Y', 'Refazer'],
        ['Ctrl + A', 'Selecionar tudo'],
        ['Ctrl + /', 'Comentar ou descomentar linhas'],
        ['Tab', 'Aumentar recuo'],
        ['Shift + Tab', 'Diminuir recuo'],
      ],
    },
    {
      titulo: t('abas.grupo'),
      itens: [
        ['Ctrl + Tab', t('abas.proxima')],
        ['Ctrl + Shift + Tab', t('abas.anterior')],
        ['Ctrl + PageDown', t('abas.proxima')],
        ['Ctrl + PageUp', t('abas.anterior')],
        ['Ctrl + W', t('abas.fechar-atual')],
      ],
    },
    {
      titulo: 'Busca',
      itens: [
        ['Ctrl + F', 'Localizar no arquivo'],
        ['Ctrl + G', 'Próxima ocorrência'],
        ['Ctrl + Shift + G', 'Ocorrência anterior'],
        ['Esc', 'Fechar a busca'],
      ],
    },
  ];
</script>

<svelte:window onkeydown={(e) => { if (visivel && e.key === 'Escape') onfechar?.(); }} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="fundo-modal" onclick={onfechar}>
    <div class="modal" role="dialog" aria-modal="true" aria-label="Atalhos de teclado" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>Atalhos de teclado</h2>
        <button class="fechar" onclick={onfechar} title="Fechar"><X size={18} /></button>
      </header>
      <div class="grupos">
        {#each grupos as g}
          <section>
            <h3>{g.titulo}</h3>
            {#each g.itens as [tecla, acao]}
              <div class="linha"><kbd>{tecla}</kbd><span>{acao}</span></div>
            {/each}
          </section>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .fundo-modal {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .modal {
    width: min(720px, 92vw);
    max-height: 85vh;
    overflow: auto;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 8px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
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
  }

  .fechar {
    border: none;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
    display: flex;
  }

  .grupos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 18px;
    padding: 18px;
  }

  h3 {
    margin: 0 0 8px;
    font-size: 0.85rem;
    color: var(--texto-secundario);
  }

  .linha {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 4px 0;
    font-size: 0.8125rem;
  }

  kbd {
    min-width: 92px;
    padding: 2px 6px;
    border: 1px solid var(--borda);
    border-radius: 4px;
    background: var(--fundo);
    font-family: inherit;
    font-size: 0.75rem;
  }
</style>
