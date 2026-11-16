<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { t } from "$lib/i18n";

  interface Props {
    visivel?: boolean;
    onfechar?: () => void;
  }

  let { visivel = false, onfechar }: Props = $props();

  const passos: Array<{ titulo: string; itens: string[] }> = [
    {
      titulo: '1. Criar ou abrir um projeto',
      itens: [
        'Na tela inicial, clique em "Novo projeto" e escolha "Documento em branco".',
        'Para continuar um trabalho do Overleaf, baixe o .zip dele ("Download Source") e escolha "Abrir arquivo .zip".',
        'Cada projeto fica guardado no aplicativo. Use "Baixar .zip" para fazer backup ou levar o projeto ao computador.',
        'No celular, a barra de baixo troca entre Arquivos, Código e PDF. O botão Compilar gera o PDF sem internet.',
      ],
    },
    {
      titulo: '2. Escrever',
      itens: [
        'O texto fica à esquerda. Os comandos do LaTeX aparecem coloridos e o editor sugere comandos enquanto você digita.',
        'Use o menu Formatar ou a barra do editor para negrito, itálico, seções, listas e equações.',
        'A paleta de símbolos insere letras gregas, setas e operadores sem decorar os comandos.',
        'O arquivo é salvo sozinho 1 segundo depois que você para de digitar. Ctrl + S salva na hora.',
      ],
    },
    {
      titulo: '3. Compilar e ver o PDF',
      itens: [
        'Clique em "Compilar" ou aperte Ctrl + Enter. O PDF aparece à direita.',
        'Se o documento tiver bibliografia, o ÉguaTex roda o BibTeX ou o Biber e repete a compilação até as citações ficarem certas.',
        'Erros e avisos aparecem no painel de registros, em português, com a linha do problema. Clique no erro para ir até a linha.',
        'Dê dois cliques no PDF para ir ao trecho correspondente no texto.',
      ],
    },
    {
      titulo: '4. Organizar o projeto',
      itens: [
        'A barra lateral mostra os arquivos do projeto. Os botões no topo dela criam arquivos e pastas e enviam imagens para o projeto.',
        'A aba Estrutura lista capítulos e seções. Clique numa seção para ir até ela.',
        'O menu Layout mostra só o editor, só o PDF ou os dois lado a lado.',
      ],
    },
    {
      titulo: '5. Ajustar o programa',
      itens: [
        'O botão de sol e lua alterna entre o modo claro e o modo escuro.',
        'Em Configurações você escolhe o compilador (pdfLaTeX, XeLaTeX ou LuaLaTeX), o tamanho da fonte e outras opções.',
        t('ajuda.como-usar.ia'),
      ],
    },
  ];
</script>

<svelte:window onkeydown={(e) => { if (visivel && e.key === 'Escape') onfechar?.(); }} />

{#if visivel}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="fundo-modal" onclick={onfechar}>
    <div class="modal" role="dialog" aria-modal="true" aria-label="Como usar o ÉguaTex" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <header>
        <h2>Como usar o ÉguaTex</h2>
        <button class="fechar" onclick={onfechar} title="Fechar"><X size={18} /></button>
      </header>
      <div class="corpo">
        {#each passos as p}
          <section>
            <h3>{p.titulo}</h3>
            <ul>
              {#each p.itens as item}<li>{item}</li>{/each}
            </ul>
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
  .corpo {
    padding: 8px 18px 18px;
  }
  h3 {
    margin: 14px 0 6px;
    font-size: 0.9rem;
  }
  ul {
    margin: 0;
    padding-left: 18px;
  }
  li {
    padding: 3px 0;
    font-size: 0.8125rem;
    line-height: 1.45;
  }
</style>
