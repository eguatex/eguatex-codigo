<script lang="ts">
  // Os dois cartoes de apoio ao projeto (feedback e Pix). Usados no Sobre, no popover da conta,
  // na barra lateral e na tela inicial, sempre com o mesmo desenho.
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import Heart from "@lucide/svelte/icons/heart";
  import { t } from "$lib/i18n";

  interface Props {
    onfeedback?: () => void;
    onapoiar?: () => void;
    /**
     * "auto" (padrao) empilha e passa a lado a lado quando a caixa tem 560 px ou mais;
     * "coluna" sempre empilha; "linha" sempre poe os dois lado a lado.
     */
    direcao?: 'auto' | 'coluna' | 'linha';
  }

  let { onfeedback, onapoiar, direcao = 'auto' }: Props = $props();
</script>

<div class="cartoes-apoio-caixa" class:auto={direcao === 'auto'}>
  <div class="cartoes-apoio {direcao}">
    <button type="button" class="cartao-apoio" onclick={() => onfeedback?.()}>
      <span class="icone"><MessageSquare size={18} /></span>
      <span class="textos">
        <strong>{t("sobre.feedback-titulo")}</strong>
        <span>{t("sobre.feedback-descricao")}</span>
      </span>
    </button>
    <button type="button" class="cartao-apoio" onclick={() => onapoiar?.()}>
      <span class="icone coracao"><Heart size={18} /></span>
      <span class="textos">
        <strong>{t("sobre.apoiar-titulo")}</strong>
        <span>{t("sobre.apoiar-descricao")}</span>
      </span>
    </button>
  </div>
</div>

<style>
  /* a largura da propria caixa decide o desenho: 560 px ou mais fica lado a lado */
  .cartoes-apoio-caixa.auto {
    container-type: inline-size;
  }

  .cartoes-apoio {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  @container (min-width: 560px) {
    .cartoes-apoio.auto {
      flex-direction: row;
    }

    .cartoes-apoio.auto .cartao-apoio {
      flex: 1 1 0;
    }
  }

  .cartoes-apoio.linha {
    flex-direction: row;
  }

  .cartoes-apoio.linha .cartao-apoio {
    flex: 1 1 0;
  }

  .cartao-apoio {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
    min-height: 56px;
    box-sizing: border-box;
    padding: 10px 12px;
    text-align: left;
    background: var(--superficie-1);
    border: 1px solid var(--borda);
    border-radius: 10px;
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }

  .cartao-apoio:hover {
    background: var(--hover);
    border-color: var(--primaria);
  }

  .icone {
    display: flex;
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--primaria);
  }

  .icone.coracao {
    color: #d6336c;
  }

  .textos {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: 13px;
  }

  .textos strong {
    font-weight: 600;
  }

  .textos span {
    font-size: 12px;
    color: var(--texto-secundario);
  }
</style>
