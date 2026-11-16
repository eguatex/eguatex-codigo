<script lang="ts">
  // Tela de abertura: a mesma do site (logo, pegadas e frase do Curupira), por 2 segundos.
  import { onMount } from 'svelte';
  import { t } from '$lib/i18n';
  import { deveMostrarAbertura, marcarAberturaVista, DURACAO_ABERTURA_MS, SAIDA_ABERTURA_MS } from '$lib/abertura';

  let visivel = $state(deveMostrarAbertura());
  let saindo = $state(false);
  const nome = t('abertura.nome');
  const PEGADAS = [30, 70, 110, 150, 190, 230, 270, 310];

  onMount(() => {
    if (!visivel) return;
    marcarAberturaVista();
    const sair = setTimeout(() => (saindo = true), DURACAO_ABERTURA_MS);
    const fim = setTimeout(() => {
      visivel = false;
      document.documentElement.style.removeProperty('background');
    }, DURACAO_ABERTURA_MS + SAIDA_ABERTURA_MS);
    return () => {
      clearTimeout(sair);
      clearTimeout(fim);
    };
  });
</script>

{#if visivel}
  <div class="abertura" class:saindo role="status" aria-label={t('abertura.rotulo')}>
    <div class="centro">
      <img class="logo" src="/curupira-logo.png" alt="" width="190" height="190" />
      <div class="nome"><span class="inicial">{nome.slice(0, 1)}</span>{nome.slice(1)}</div>
      <svg class="pegadas" viewBox="0 0 360 64" aria-hidden="true">
        <defs>
          <g id="abertura-pe">
            <ellipse cx="0" cy="0" rx="9" ry="15" />
            <circle cx="-7" cy="19" r="3.2" />
            <circle cx="-1" cy="22" r="3.4" />
            <circle cx="5" cy="21" r="3" />
            <circle cx="10" cy="17" r="2.6" />
          </g>
        </defs>
        {#each PEGADAS as x, i (x)}
          <use href="#abertura-pe" transform="translate({x} {i % 2 ? 44 : 18}) rotate(90)" style="animation-delay: {i * 0.2}s" />
        {/each}
      </svg>
      <p class="texto">{t('abertura.texto')}</p>
      <p class="dica">{t('abertura.dica')}</p>
    </div>
  </div>
{/if}

<style>
  .abertura {
    --abertura-noite: #1c0a2b;
    --abertura-roxo: #3a1660;
    --abertura-texto: #f3eafb;
    --abertura-suave: #cdb8e4;
    --abertura-verde: #8fd47f;
    position: fixed;
    inset: 0;
    z-index: 10000;
    display: grid;
    place-items: center;
    padding: 24px;
    background: radial-gradient(circle at 50% 42%, var(--abertura-roxo), var(--abertura-noite) 62%);
    transition: opacity 0.4s ease;
  }
  .abertura.saindo {
    opacity: 0;
    pointer-events: none;
  }
  .centro {
    display: grid;
    justify-items: center;
    gap: 18px;
    text-align: center;
  }
  .logo {
    width: clamp(120px, 22vmin, 190px);
    height: auto;
    margin-bottom: -8px;
    filter: drop-shadow(0 0 28px rgba(255, 120, 40, 0.45));
    animation: respirar 2s ease-in-out infinite;
  }
  .nome {
    font-weight: 800;
    font-size: clamp(40px, 8vmin, 64px);
    letter-spacing: -0.03em;
    line-height: 1;
    color: var(--abertura-texto);
  }
  .inicial {
    background: linear-gradient(180deg, #ffb627, #ff5f15 50%, #e0243e);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .pegadas {
    width: min(300px, 62vw);
    height: auto;
    fill: var(--abertura-verde);
    overflow: visible;
  }
  .pegadas use {
    opacity: 0;
    animation: pisar 2s linear forwards;
  }
  .texto {
    margin: 0;
    font-weight: 500;
    font-size: 18px;
    color: var(--abertura-texto);
  }
  .dica {
    margin: -10px 0 0;
    max-width: 30ch;
    font-size: 14px;
    color: var(--abertura-suave);
  }
  @keyframes respirar {
    50% {
      filter: drop-shadow(0 0 46px rgba(255, 140, 40, 0.75));
      transform: translateY(-3px);
    }
  }
  @keyframes pisar {
    0% { opacity: 0; }
    8% { opacity: 1; }
    60% { opacity: 0.5; }
    100% { opacity: 0.35; }
  }
  @media (prefers-reduced-motion: reduce) {
    .logo,
    .pegadas use {
      animation: none;
    }
    .pegadas use {
      opacity: 0.8;
    }
    .abertura {
      transition: none;
    }
  }
</style>
