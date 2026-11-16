<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import X from "@lucide/svelte/icons/x";
  import { t, tf } from "$lib/i18n";
  import { registrarFechavel } from "$lib/voltar";

  interface Versao {
    id: string;
    data_ms: number;
    tamanho: number;
    arquivo?: string;
    estado?: 'criado' | 'alterado' | string;
    delta?: number;
  }

  function textoDelta(v: Versao): string {
    if (v.estado !== 'alterado' || typeof v.delta !== 'number' || v.delta === 0) return '';
    const sinal = v.delta > 0 ? '+' : '-';
    return ` (${sinal}${tamanhoTexto(Math.abs(v.delta))})`;
  }

  interface Props {
    visivel?: boolean;
    caminhoProjeto?: string;
    arquivo?: string;
    /** Quantas sugestões de revisão o arquivo tem: restaurar as descarta e zera o Desfazer. */
    sugestoes?: number;
    onfechar?: () => void;
    /** Chamado depois de restaurar, com o texto que voltou para o arquivo. */
    onrestaurar?: (arquivo: string, texto: string) => void;
  }

  let { visivel = false, caminhoProjeto = '', arquivo = '', sugestoes = 0, onfechar, onrestaurar }: Props = $props();

  let versoes = $state<Versao[]>([]);
  let escolhida = $state<string | null>(null);
  let previa = $state('');
  let erro = $state('');
  let carregando = $state(false);

  const LIMITE_PREVIA = 6000;

  const dataFmt = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'medium' });

  function tamanhoTexto(bytes: number): string {
    return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1).replace('.', ',')} KB`;
  }

  $effect(() => {
    if (!visivel) return;
    const projeto = caminhoProjeto;
    const alvo = arquivo;
    erro = '';
    previa = '';
    escolhida = null;
    versoes = [];
    if (!projeto || !alvo) return;
    invoke<Versao[]>('cmd_versoes_listar', { caminhoProjeto: projeto, caminhoRelativo: alvo })
      .then((lista) => {
        versoes = lista;
        if (lista.length) void escolher(lista[0].id);
      })
      .catch((e) => (erro = String(e)));
  });

  async function escolher(id: string) {
    escolhida = id;
    previa = '';
    erro = '';
    try {
      const texto = await invoke<string>('cmd_versoes_ler', { caminhoProjeto, caminhoRelativo: arquivo, id });
      if (escolhida !== id) return;
      previa = texto.length > LIMITE_PREVIA ? texto.slice(0, LIMITE_PREVIA) + '\n...' : texto;
    } catch (e) {
      erro = String(e);
    }
  }

  async function restaurar() {
    if (!escolhida) return;
    if (sugestoes > 0 && !confirm(t('versoes.sugestoes-descartadas'))) return;
    carregando = true;
    erro = '';
    try {
      const texto = await invoke<string>('cmd_versoes_restaurar', {
        caminhoProjeto,
        caminhoRelativo: arquivo,
        id: escolhida,
      });
      onrestaurar?.(arquivo, texto);
      onfechar?.();
    } catch (e) {
      erro = String(e);
    } finally {
      carregando = false;
    }
  }

  let pressionouNoFundo = false;

  function aoPressionarFundo(e: MouseEvent) {
    pressionouNoFundo = e.target === e.currentTarget;
  }

  function aoClicarFundo(e: MouseEvent) {
    if (pressionouNoFundo && e.target === e.currentTarget) onfechar?.();
    pressionouNoFundo = false;
  }

  function aoTeclar(e: KeyboardEvent) {
    if (visivel && e.key === 'Escape') {
      e.preventDefault();
      onfechar?.();
    }
  }

  function focarJanela(no: HTMLElement) {
    const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    no.focus();
    return {
      destroy() {
        anterior?.focus();
      },
    };
  }

  // o botao Voltar do Android fecha o modal em vez de sair do app
  $effect(() => {
    if (visivel) return registrarFechavel(() => onfechar?.());
  });
</script>

<svelte:window onkeydown={aoTeclar} />

{#if visivel}
  <div class="fundo-modal" data-modal-fundo role="presentation" onmousedown={aoPressionarFundo} onclick={aoClicarFundo}>
    <div class="modal" data-modal-caixa role="dialog" aria-modal="true" aria-labelledby="titulo-versoes" tabindex="-1" use:focarJanela>
      <header data-modal-cabecalho>
        <h2 id="titulo-versoes">{tf('versoes.titulo', { arquivo: arquivo.split('/').pop() ?? arquivo })}</h2>
        <button class="fechar" data-modal-fechar onclick={onfechar} title={t('versoes.fechar')} aria-label={t('versoes.fechar')}><X size={18} /></button>
      </header>

      <div class="corpo" data-modal-corpo>
        {#if versoes.length === 0 && !erro}
          <p class="nota">{t('versoes.vazio')}</p>
        {:else}
          <ul class="lista" aria-label={t('versoes.lista')}>
            {#each versoes as v (v.id)}
              <li>
                <button class="versao" class:ativa={escolhida === v.id} onclick={() => escolher(v.id)}>
                  <span>{dataFmt.format(new Date(v.data_ms))}</span>
                  <small>{tamanhoTexto(v.tamanho)}</small>
                  <span class="mudanca">
                    <em>{v.estado === 'criado' ? t('versoes.criado') : t('versoes.alterado')}</em>
                    {v.arquivo || arquivo.split('/').pop()}{textoDelta(v)}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
          <pre class="previa" aria-label={t('versoes.previa')}>{previa}</pre>
        {/if}
        {#if erro}<p class="erro" role="alert">{erro}</p>{/if}
      </div>

      <footer>
        <p class="nota">{t('versoes.nota-restaurar')}</p>
        <button class="botao" onclick={onfechar}>{t('versoes.fechar')}</button>
        <button class="botao primario" disabled={!escolhida || carregando} onclick={restaurar}>{t('versoes.restaurar')}</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .fundo-modal {
    position: fixed;
    inset: 0;
    z-index: 2000;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: brightness(0.5);
  }

  .modal {
    width: min(760px, 94vw);
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    box-shadow: var(--sombra-3);
    outline: none;
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
    font-weight: 600;
  }

  .fechar {
    display: flex;
    padding: 4px;
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .fechar:hover {
    background: var(--hover);
    color: var(--texto);
  }

  .corpo {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 12px;
    padding: 12px 18px;
    min-height: 0;
    overflow: hidden;
  }

  .lista {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 55vh;
    overflow: auto;
  }

  .versao {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 0.8125rem;
    text-align: left;
    cursor: pointer;
  }

  .versao:hover {
    background: var(--hover);
  }

  .versao.ativa {
    background: var(--hover);
    font-weight: 600;
  }

  .versao small {
    color: var(--texto-terciario);
  }

  .versao {
    flex-wrap: wrap;
  }

  .mudanca {
    flex: 1 0 100%;
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .mudanca em {
    font-style: normal;
    font-weight: 600;
    margin-right: 4px;
  }

  .previa {
    margin: 0;
    padding: 10px;
    max-height: 55vh;
    overflow: auto;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    font-size: 0.78rem;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .nota {
    margin: 0;
    font-size: 0.75rem;
    color: var(--texto-terciario);
  }

  .erro {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 0.8125rem;
    color: var(--erro, #b3261e);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--borda);
  }

  footer .nota {
    margin-right: auto;
  }

  .botao {
    padding: 7px 18px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .botao:hover:not(:disabled) {
    background: var(--hover);
  }

  .botao:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .botao.primario {
    border-color: var(--primaria);
  }

  /* tela cheia no celular (regra comum em +page.svelte): uma coluna so */
  @media (max-width: 640px), (max-height: 500px) {
    .corpo {
      grid-template-columns: 1fr;
    }
  }

  .fechar:focus-visible,
  .versao:focus-visible,
  .botao:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 1px;
  }
</style>
