<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import Heart from "@lucide/svelte/icons/heart";
  import { PIX_APOIO, payloadPixValido } from '$lib/pix';
  import { t } from '$lib/i18n';
  import QRCode from 'qrcode';
  import { onMount } from 'svelte';
  import { registrarFechavel } from '$lib/voltar';

  interface Props {
    aoFechar: () => void;
  }

  let { aoFechar }: Props = $props();

  let copiado = $state<'' | 'chave' | 'codigo'>('');
  let copiaFalhou = $state(false);
  let caixaChave: HTMLElement | undefined = $state();
  let caixaCodigo: HTMLTextAreaElement | undefined = $state();
  let qrCodeSvg = $state('');

  // Texto exato do QR do banco (ver PIX_APOIO em pix.ts). Nunca montar de novo.
  const dadosPix = PIX_APOIO;
  const payload = payloadPixValido(PIX_APOIO.payload) ? PIX_APOIO.payload : '';

  onMount(async () => {
    try {
      qrCodeSvg = await QRCode.toString(payload, { type: 'svg', margin: 1, width: 160, color: { dark: '#000000', light: '#ffffff' } });
    } catch (e) {
      console.error('Erro ao gerar QR Code Pix:', e);
    }
  });

  // o botao Voltar do Android fecha o Pix em vez de sair do app (o modal so existe enquanto aberto)
  $effect(() => registrarFechavel(aoFechar));

  function selecionarTexto(alvo: HTMLElement | undefined) {
    if (!alvo) return;
    if (alvo instanceof HTMLTextAreaElement) {
      alvo.focus();
      alvo.select();
      return;
    }
    const selecao = window.getSelection();
    if (!selecao) return;
    const faixa = document.createRange();
    faixa.selectNodeContents(alvo);
    selecao.removeAllRanges();
    selecao.addRange(faixa);
  }

  // "Copiado!" so aparece quando a copia deu certo; na falha o texto fica selecionado
  async function copiarTexto(texto: string, qual: 'chave' | 'codigo', alvo: HTMLElement | undefined) {
    try {
      await navigator.clipboard.writeText(texto);
      copiaFalhou = false;
      copiado = qual;
      setTimeout(() => { if (copiado === qual) copiado = ''; }, 2500);
    } catch (e) {
      console.error('Erro ao copiar o texto do Pix:', e);
      copiado = '';
      copiaFalhou = true;
      selecionarTexto(alvo);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="modal-overlay"
  data-modal-fundo
  role="dialog"
  aria-modal="true"
  tabindex="-1"
  onclick={aoFechar}
  onkeydown={(e) => { if (e.key === 'Escape') aoFechar(); }}
>
  <div class="modal-conteudo" data-modal-caixa onclick={(e) => e.stopPropagation()}>
    <div class="modal-cabecalho" data-modal-cabecalho>
      <div class="titulo-com-icone">
        <Heart size={20} color="#f38ba8" fill="#f38ba8" />
        <h2>{t('pix.titulo')}</h2>
      </div>
      <button class="botao-fechar" data-modal-fechar onclick={aoFechar} aria-label={t('pix.fechar')}>
        <X size={18} />
      </button>
    </div>

    <div class="modal-corpo" data-modal-corpo>
      <p class="descricao">
        {t('pix.descricao')}
      </p>

      <div class="pix-info">
        <div class="banco-info area-banco">
          <strong>{t('pix.conta')}:</strong> {t('pix.banco-nome')}<br/>
          <strong>{t('pix.favorecido')}:</strong> {dadosPix.nome}
        </div>

        <div class="chave-container area-chave">
          <span class="rotulo-chave">{t('pix.chave-rotulo')}:</span>
          <div class="copia-box">
            <code bind:this={caixaChave}>{dadosPix.chave}</code>
            <button class="btn-copiar" onclick={() => copiarTexto(dadosPix.chave, 'chave', caixaChave)}>
              {#if copiado === 'chave'}
                <Check size={16} /> {t('pix.copiado')}
              {:else}
                <Copy size={16} /> {t('pix.copiar-chave')}
              {/if}
            </button>
          </div>
        </div>

        <div class="qrcode-section area-qr">
          <p class="payload-txt">{t('pix.qrcode-label')}</p>
          <div class="qr-container">
            {#if qrCodeSvg}
              {@html qrCodeSvg}
            {:else}
              <div class="qr-placeholder">{t('pix.gerando-qr')}</div>
            {/if}
          </div>
        </div>

        <div class="qrcode-placeholder area-payload">
          <p class="payload-txt">{t('pix.payload-label')}</p>
          <textarea readonly rows="2" class="payload-input" bind:this={caixaCodigo}>{payload}</textarea>
          <button class="btn-copiar btn-copiar-codigo" onclick={() => copiarTexto(payload, 'codigo', caixaCodigo)}>
            {#if copiado === 'codigo'}
              <Check size={16} /> {t('pix.copiado')}
            {:else}
              <Copy size={16} /> {t('pix.copiar-codigo')}
            {/if}
          </button>
          {#if copiaFalhou}
            <p class="copia-erro" role="alert">{t('pix.copiar-falhou')}</p>
          {/if}
        </div>
      </div>
    </div>

    <div class="modal-rodape">
      <button class="btn-fechar" onclick={aoFechar}>{t('pix.fechar')}</button>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    backdrop-filter: blur(4px);
  }

  .modal-conteudo {
    background: var(--superficie-0, #1e1e2e);
    border: 1px solid var(--borda, #313244);
    border-radius: 12px;
    width: 100%;
    max-width: 480px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    color: var(--texto, #cdd6f4);
    font-family: inherit;
  }

  .modal-cabecalho {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--borda, #313244);
  }

  .titulo-com-icone {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .modal-cabecalho h2 {
    font-size: 1.125rem;
    font-weight: 600;
    margin: 0;
  }

  .botao-fechar {
    background: transparent;
    border: none;
    color: var(--texto-secundario, #a6adc8);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .botao-fechar:hover {
    background: var(--hover, #313244);
    color: var(--texto, #cdd6f4);
  }

  .modal-corpo {
    padding: 20px;
    max-height: 75vh;
    overflow-y: auto;
  }

  .descricao {
    font-size: 0.875rem;
    color: var(--texto-secundario, #a6adc8);
    line-height: 1.5;
    margin-top: 0;
    margin-bottom: 16px;
  }

  .pix-info {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .banco-info {
    background: var(--superficie-1, #181825);
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 0.875rem;
    line-height: 1.4;
    border: 1px solid var(--borda, #313244);
  }

  .chave-container {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .rotulo-chave {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--texto-secundario, #a6adc8);
    text-transform: uppercase;
  }

  .copia-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--superficie-1, #181825);
    border: 1px solid var(--borda, #313244);
    border-radius: 8px;
    padding: 6px 10px;
  }

  .copia-box code {
    font-family: monospace;
    font-size: 0.875rem;
    font-weight: 600;
    color: #a6e3a1;
  }

  .btn-copiar {
    display: flex;
    align-items: center;
    gap: 6px;
    background: #89b4fa;
    color: #11111b;
    border: none;
    padding: 5px 10px;
    border-radius: 6px;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.2s;
  }

  .btn-copiar:hover {
    background: #b4befe;
  }

  .btn-copiar-codigo {
    align-self: flex-start;
  }

  .copia-erro {
    margin: 0;
    font-size: 0.75rem;
    color: #f38ba8;
  }

  .qrcode-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .qr-container {
    background: #ffffff;
    padding: 8px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  .qr-container :global(svg) {
    display: block;
    width: 140px;
    height: 140px;
  }

  .qr-placeholder {
    width: 140px;
    height: 140px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.75rem;
    color: #6c7086;
  }

  .qrcode-placeholder {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .payload-txt {
    font-size: 0.75rem;
    color: var(--texto-secundario, #a6adc8);
    margin: 0;
  }

  .payload-input {
    width: 100%;
    background: var(--superficie-1, #181825);
    border: 1px solid var(--borda, #313244);
    border-radius: 8px;
    padding: 6px;
    color: var(--texto-terciario, #6c7086);
    font-family: monospace;
    font-size: 0.75rem;
    resize: none;
  }

  .modal-rodape {
    flex: none;
    padding: 10px 20px;
    background: var(--superficie-1, #181825);
    border-top: 1px solid var(--borda, #313244);
    display: flex;
    justify-content: flex-end;
  }

  .btn-fechar {
    background: transparent;
    border: 1px solid var(--borda, #45475a);
    color: var(--texto, #cdd6f4);
    padding: 5px 14px;
    border-radius: 6px;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .btn-fechar:hover {
    background: var(--hover, #313245);
  }

  /* celular deitado (a tela tem 360 a 412 px de altura): o QR fica a esquerda, em ate 240 px, e a chave
     e o copia e cola ficam a direita. A regra comum de tela cheia esta em +page.svelte. */
  @media (max-height: 500px) and (min-width: 480px) {
    .modal-corpo {
      max-height: none;
      padding: 12px 16px;
    }

    .descricao {
      margin-bottom: 10px;
    }

    .pix-info {
      display: grid;
      grid-template-columns: min(60dvh, 240px) minmax(0, 1fr);
      grid-template-areas:
        'qr banco'
        'qr chave'
        'qr payload';
      grid-template-rows: auto auto 1fr;
      column-gap: 16px;
      row-gap: 10px;
      align-items: start;
    }

    .area-qr { grid-area: qr; }
    .area-banco { grid-area: banco; }
    .area-chave { grid-area: chave; }
    .area-payload { grid-area: payload; }

    .qr-container :global(svg),
    .qr-placeholder {
      width: calc(min(60dvh, 240px) - 16px);
      height: calc(min(60dvh, 240px) - 16px);
    }

    .copia-box {
      flex-wrap: wrap;
      gap: 6px;
    }

    .copia-box code {
      overflow-wrap: anywhere;
    }
  }

  /* toque: alvos de 44 px */
  @media (pointer: coarse) {
    .btn-copiar,
    .btn-fechar {
      min-height: 44px;
    }

    .payload-input {
      font-size: 0.8125rem;
    }
  }
</style>
