<script lang="ts">
  // Itens do menu de opcoes de compilacao. Usado pela barra do PDF e pela barra unica do
  // celular deitado, para a logica ficar em um lugar so. Quem usa cuida do botao, da posicao
  // e do foco do menu; aqui so ficam os itens e o fechamento depois de cada escolha.
  import Check from "@lucide/svelte/icons/check";
  import { t } from "$lib/i18n";

  interface Props {
    autoCompilar?: boolean;
    pararNoPrimeiroErro?: boolean;
    compilando?: boolean;
    onautocompilar?: (ligado: boolean) => void;
    onpararnoprimeiroerro?: (ligado: boolean) => void;
    onpararcompilacao?: () => void;
    onrecompilardozero?: () => void;
    /** Com esta acao o menu ganha o item "Recompilar" no topo. */
    oncompilar?: () => void;
    /** Chamado antes de cada acao, para fechar o menu. */
    onescolher?: () => void;
  }

  let {
    autoCompilar = false,
    pararNoPrimeiroErro = false,
    compilando = false,
    onautocompilar,
    onpararnoprimeiroerro,
    onpararcompilacao,
    onrecompilardozero,
    oncompilar,
    onescolher,
  }: Props = $props();
</script>

{#if oncompilar}
  <button
    type="button"
    class="menu-item"
    role="menuitem"
    disabled={compilando}
    onclick={() => { onescolher?.(); oncompilar?.(); }}
  >
    <span class="marca"></span>
    <span>{t("compilar.recompilar")}</span>
  </button>
{/if}

<button
  type="button"
  class="menu-item"
  role="menuitemcheckbox"
  aria-checked={autoCompilar}
  onclick={() => { onescolher?.(); onautocompilar?.(!autoCompilar); }}
>
  <span class="marca">{#if autoCompilar}<Check size={14} />{/if}</span>
  <span>{t("compilacao.opcao-auto")}</span>
</button>
<button
  type="button"
  class="menu-item"
  role="menuitemcheckbox"
  aria-checked={pararNoPrimeiroErro}
  onclick={() => { onescolher?.(); onpararnoprimeiroerro?.(!pararNoPrimeiroErro); }}
>
  <span class="marca">{#if pararNoPrimeiroErro}<Check size={14} />{/if}</span>
  <span>{t("compilacao.opcao-parar-erro")}</span>
</button>
<div class="separador" role="separator"></div>
<button
  type="button"
  class="menu-item"
  role="menuitem"
  disabled={!compilando}
  onclick={() => { onescolher?.(); onpararcompilacao?.(); }}
>
  <span class="marca"></span>
  <span>{t("compilacao.opcao-parar")}</span>
</button>
<button
  type="button"
  class="menu-item"
  role="menuitem"
  disabled={compilando}
  onclick={() => { onescolher?.(); onrecompilardozero?.(); }}
>
  <span class="marca"></span>
  <span>{t("compilacao.opcao-zero")}</span>
</button>

<style>
  .menu-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: var(--texto);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  @media (pointer: coarse) {
    .menu-item {
      min-height: 44px;
    }
  }

  .menu-item:hover:not(:disabled),
  .menu-item:focus-visible {
    background: var(--hover);
    outline: none;
  }

  .menu-item:disabled {
    color: var(--texto-terciario);
    cursor: default;
  }

  .marca {
    display: inline-flex;
    width: 16px;
    flex-shrink: 0;
  }

  .separador {
    height: 1px;
    margin: 4px 0;
    background: var(--borda);
  }
</style>
