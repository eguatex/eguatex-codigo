<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Download from '@lucide/svelte/icons/download';
  import { t, tf } from '$lib/i18n';
  import { formatarTamanho, tamanhoDoErroAcimaDoLimite, tipoMime } from '$lib/imagem';
  import { nomeDaAba } from '$lib/abas.svelte';

  interface Props {
    caminhoProjeto: string;
    /** Caminho da imagem no formato da arvore ("/figuras/foto.png"). */
    caminho: string;
    /** O mesmo "Baixar" do menu da arvore de arquivos. */
    onbaixar?: (caminho: string) => void;
  }

  let { caminhoProjeto, caminho, onbaixar }: Props = $props();

  let estado = $state<'carregando' | 'pronta' | 'grande' | 'erro'>('carregando');
  let url = $state('');
  let tamanho = $state(0);
  let erro = $state('');
  let tamanhoReal = $state(false);
  let largura = $state(0);
  let altura = $state(0);

  const nome = $derived(nomeDaAba(caminho));

  // Le os bytes pelo comando do projeto (limite de 25 MB) e cria o blob. O cleanup revoga a URL ao
  // trocar de imagem ou fechar a aba, para a imagem nao ficar na memoria.
  $effect(() => {
    const projeto = caminhoProjeto;
    const arquivo = caminho;
    let cancelado = false;
    let criada = '';
    estado = 'carregando';
    url = '';
    erro = '';
    tamanhoReal = false;
    largura = 0;
    altura = 0;
    void (async () => {
      try {
        const dados = await invoke<ArrayBuffer>('cmd_ler_bytes_projeto', { caminhoProjeto: projeto, caminhoRelativo: arquivo });
        if (cancelado) return;
        // SVG so entra como <img> com blob image/svg+xml: o navegador nao roda script de SVG carregado assim
        criada = URL.createObjectURL(new Blob([dados], { type: tipoMime(arquivo) }));
        tamanho = dados.byteLength;
        url = criada;
        estado = 'pronta';
      } catch (e) {
        if (cancelado) return;
        const grande = tamanhoDoErroAcimaDoLimite(e);
        if (grande !== null) {
          tamanho = grande;
          estado = 'grande';
        } else {
          erro = String(e);
          estado = 'erro';
        }
      }
    })();
    return () => {
      cancelado = true;
      if (criada) URL.revokeObjectURL(criada);
    };
  });
</script>

<div class="visualizador">
  <header>
    <button type="button" class="baixar" onclick={() => onbaixar?.(caminho)}>
      <Download size={16} /> {t('imagem.baixar')}
    </button>
    <span class="nome" title={caminho.replace(/^\//, '')}>{nome}</span>
    {#if estado === 'pronta' && largura > 0}
      <span class="info">{tf('imagem.dimensoes', { largura, altura })}</span>
    {/if}
    {#if estado === 'pronta' || estado === 'grande'}
      <span class="info">{formatarTamanho(tamanho)}</span>
    {/if}
  </header>

  <div class="area">
    {#if estado === 'carregando'}
      <p class="aviso">{t('imagem.carregando')}</p>
    {:else if estado === 'grande'}
      <p class="aviso">{tf('imagem.grande-demais', { tamanho: formatarTamanho(tamanho) })}</p>
    {:else if estado === 'erro'}
      <p class="aviso erro">{tf('imagem.erro', { erro })}</p>
    {:else}
      <button
        type="button"
        class="moldura"
        class:real={tamanhoReal}
        title={t('imagem.tamanho-real')}
        aria-label={t('imagem.tamanho-real')}
        aria-pressed={tamanhoReal}
        onclick={() => (tamanhoReal = !tamanhoReal)}
      >
        <img src={url} alt={nome} bind:naturalWidth={largura} bind:naturalHeight={altura} />
      </button>
    {/if}
  </div>
</div>

<style>
  /* Medidas de file-view.scss do Overleaf (AGPL-3.0): padding 12 px, botao centralizado no topo,
     imagem com no maximo 100% da largura e 90% da altura, borda de 1 px, sombra e fundo branco. */
  .visualizador {
    flex: 1;
    min-height: 0;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    padding: 12px;
    background: var(--superficie-1);
    color: var(--texto);
  }

  header {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
    gap: 6px 12px;
    font-size: 0.8125rem;
  }

  .nome {
    min-width: 0;
    max-width: 40ch;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--texto-secundario);
  }

  .info {
    color: var(--texto-secundario);
    white-space: nowrap;
  }

  .baixar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }

  .baixar:hover {
    background: var(--hover);
  }

  .area {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .aviso {
    margin: 24px auto;
    max-width: 480px;
    padding: 16px;
    text-align: center;
    color: var(--texto-secundario);
  }

  .erro {
    color: var(--perigo);
  }

  .moldura {
    display: block;
    width: 100%;
    height: 100%;
    padding: 0;
    border: none;
    background: transparent;
    cursor: zoom-in;
  }

  .moldura img {
    display: block;
    box-sizing: border-box;
    max-width: 100%;
    max-height: 90%;
    margin: 12px auto;
    border: 1px solid #677283;
    box-shadow: 0 2px 3px rgba(30, 37, 48, 0.2);
    background: #ffffff;
  }

  .moldura.real {
    width: max-content;
    min-width: 100%;
    height: auto;
    cursor: zoom-out;
  }

  .moldura.real img {
    max-width: none;
    max-height: none;
  }

  .moldura:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: -2px;
  }
</style>
