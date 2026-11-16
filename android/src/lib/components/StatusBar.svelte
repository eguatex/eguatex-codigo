<script lang="ts">
  // Barra de status minimalista, como a do Overleaf e a do VS Code.
  // Esquerda: um estado por vez (tarefa em andamento, compilando, concluida por 3 s, erro, ocioso).
  // Direita: so indicadores curtos (linha e coluna, sigla do corretor, motor que rodou).
  // Compilar e baixar pacotes ficam na barra do PDF e na lista de erros; o motor e automatico.
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import CircleX from "@lucide/svelte/icons/circle-x";
  import BarraProgresso from "./BarraProgresso.svelte";
  import { t, tf } from "$lib/i18n";
  import { ATRASO_BARRA_MS, tarefaVisivel, tarefas } from "$lib/tarefas.svelte";
  import { siglaOrtografia, textoDoErro, type ErroDaBarra } from "$lib/status-compilacao";
  import type { EstadoOrtografia } from "$lib/ortografia-idioma";

  interface Props {
    linha?: number;
    coluna?: number;
    statusCompilacao?: 'idle' | 'compilando' | 'sucesso' | 'parcial' | 'erro';
    /** Erros da ultima compilacao: o estado de erro mostra a causa do primeiro. */
    erros?: ErroDaBarra[];
    /** Todas as abas confirmadas pelo disco: o ocioso diz "Salvo"; senao "Pronto". */
    salvo?: boolean;
    /** Motor que rodou na ultima compilacao ("XeLaTeX"); vazio nao aparece. */
    motor?: string;
    ortografia?: EstadoOrtografia;
    /** Clique no erro: abre o log da compilacao. */
    onabrirlog?: () => void;
    /** Clique na sigla do corretor: abre a configuracao dele. */
    onortografia?: () => void;
    /** Celular deitado: a barra so aparece enquanto ha algo a mostrar (compilando, tarefa, sucesso, erro). */
    soQuandoAtivo?: boolean;
  }

  let {
    linha = 1,
    coluna = 1,
    statusCompilacao = 'idle',
    erros = [],
    salvo = false,
    motor = '',
    ortografia = { ligada: true },
    onabrirlog,
    onortografia,
    soQuandoAtivo = false,
  }: Props = $props();

  /** Quanto tempo "Compilação concluída" fica na barra antes de voltar ao ocioso. */
  const SUCESSO_MS = 3000;

  let mostrarSucesso = $state(false);
  $effect(() => {
    if (statusCompilacao !== 'sucesso') {
      mostrarSucesso = false;
      return;
    }
    mostrarSucesso = true;
    const timer = setTimeout(() => (mostrarSucesso = false), SUCESSO_MS);
    return () => clearTimeout(timer);
  });

  // tarefa longa (baixar pacote, abrir zip, Drive): mesma regra da BarraProgresso, depois de 300 ms
  let agora = $state(Date.now());
  $effect(() => {
    if (tarefas.lista.length === 0) return;
    agora = Date.now();
    const timer = setInterval(() => (agora = Date.now()), ATRASO_BARRA_MS / 3);
    return () => clearInterval(timer);
  });
  const haTarefa = $derived(tarefaVisivel(tarefas.lista, agora) !== undefined);

  const estado = $derived<'tarefa' | 'compilando' | 'sucesso' | 'erro' | 'ocioso'>(
    haTarefa
      ? 'tarefa'
      : statusCompilacao === 'compilando'
        ? 'compilando'
        : mostrarSucesso
          ? 'sucesso'
          : statusCompilacao === 'erro' || statusCompilacao === 'parcial'
            ? 'erro'
            : 'ocioso',
  );

  const textoErro = $derived(textoDoErro(erros));
  const sigla = $derived(siglaOrtografia(ortografia));
  const tituloOrtografia = $derived(
    ortografia.ligada
      ? t(ortografia.idioma === 'en' ? 'status.ortografia-ligada-en' : 'status.ortografia-ligada')
      : ortografia.motivo === 'idioma'
        ? tf('status.ortografia-clique', { motivo: tf('status.ortografia-idioma', { idioma: ortografia.codigo }) })
        : ortografia.motivo === 'nao-portugues'
          ? tf('status.ortografia-clique', { motivo: t('status.ortografia-nao-portugues') })
          : t('status.ortografia-desligada'),
  );
</script>

{#if !(soQuandoAtivo && estado === 'ocioso')}
<footer class="statusbar" class:so-ativo={soQuandoAtivo}>
  <div class="statusbar-esquerda" role="status" aria-live="polite">
    {#if estado === 'tarefa'}
      <BarraProgresso modo="rodape" />
    {:else if estado === 'compilando'}
      <span class="statusbar-item compilando">{t('status.compilando')}</span>
      <span class="trilho" aria-hidden="true"><span class="preenchimento"></span></span>
    {:else if estado === 'sucesso'}
      <span class="statusbar-item sucesso"><CircleCheck size={12} />{t('status.compilado')}</span>
    {:else if estado === 'erro'}
      <button type="button" class="statusbar-item erro" title={t('status.erro-titulo')} onclick={() => onabrirlog?.()}>
        <CircleX size={12} />
        <span class="erro-texto">{textoErro}</span>
      </button>
    {:else}
      <span class="statusbar-item ocioso">{salvo ? t('status.salvo') : t('status.pronto')}</span>
    {/if}
  </div>
  <div class="statusbar-direita">
    <span class="statusbar-item posicao" title={t('status.posicao-titulo')}>Ln {linha}, Col {coluna}</span>
    <button
      type="button"
      class="statusbar-item sigla"
      class:desligada={!ortografia.ligada}
      aria-label={tituloOrtografia}
      onclick={() => onortografia?.()}
    >{sigla}</button>
    {#if motor}
      <span class="statusbar-item motor" title={tf('status.motor-usado', { motor })}>{motor}</span>
    {/if}
  </div>
</footer>
{/if}

<style>
  .statusbar {
    position: relative;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    background: var(--primaria);
    color: rgba(255, 255, 255, 0.9);
    font-size: 0.6875rem;
    min-height: 22px;
    letter-spacing: 0.01em;
  }

  .statusbar-esquerda,
  .statusbar-direita {
    display: flex;
    gap: 14px;
    align-items: center;
    min-width: 0;
  }

  .statusbar-esquerda {
    flex: 1 1 auto;
    overflow: hidden;
  }

  .statusbar-direita {
    flex: 0 0 auto;
  }

  .statusbar-item {
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  button.statusbar-item {
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    padding: 1px 4px;
    margin: 0 -4px;
    border-radius: 3px;
    cursor: pointer;
  }

  button.statusbar-item:hover,
  button.statusbar-item:focus-visible {
    background: rgba(255, 255, 255, 0.15);
  }

  .statusbar-item.erro {
    min-width: 0;
    color: var(--perigo-suave);
    font-weight: 600;
  }

  .erro-texto {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .statusbar-item.sucesso {
    color: var(--sucesso-suave);
  }

  .statusbar-item.compilando {
    color: var(--alerta-suave);
  }

  .statusbar-item.ocioso,
  .statusbar-item.motor,
  .statusbar-item.posicao {
    opacity: 0.85;
  }

  .sigla {
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .sigla.desligada {
    opacity: 0.45;
  }

  /* compilando: trilho indeterminado de 3 px na borda de baixo, como o da BarraProgresso */
  .trilho {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.2);
  }

  .preenchimento {
    display: block;
    width: 35%;
    height: 3px;
    background: var(--alerta-suave, #f5c542);
    animation: barra-compilando 1.2s ease-in-out infinite;
  }

  @keyframes barra-compilando {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .preenchimento {
      width: 100%;
      animation: none;
      opacity: 0.6;
    }
  }
</style>
