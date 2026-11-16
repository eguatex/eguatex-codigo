<script lang="ts">
  // Tres passos para criar a chave de API, com desenhos proprios (sem logo de empresa e sem texto
  // dentro do desenho). As legendas vem do i18n.
  import { t } from "$lib/i18n";
</script>

<section class="passos" aria-label={t("ia.passos.titulo")}>
  <h4>{t("ia.passos.titulo")}</h4>
  <ol>
    <li>
      <!-- celular com o navegador aberto e uma pessoa entrando na conta -->
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <rect x="18" y="6" width="28" height="52" rx="5" class="contorno" />
        <rect x="22" y="13" width="20" height="34" rx="2" class="tela" />
        <circle cx="32" cy="24" r="5" class="destaque" />
        <path d="M24 38c1.5-5 4.5-7 8-7s6.5 2 8 7" class="destaque-linha" />
        <circle cx="32" cy="52.5" r="2" class="contorno" />
      </svg>
      <span><b>1</b> {t("ia.passos.1")}</span>
    </li>
    <li>
      <!-- chave com dois quadrados sobrepostos (copiar) -->
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <circle cx="20" cy="26" r="9" class="destaque-linha" />
        <circle cx="20" cy="26" r="3" class="destaque" />
        <path d="M29 26h20M43 26v6M49 26v8" class="destaque-linha" />
        <rect x="30" y="38" width="16" height="16" rx="3" class="contorno" />
        <rect x="36" y="44" width="16" height="16" rx="3" class="tela-cheia" />
      </svg>
      <span><b>2</b> {t("ia.passos.2")}</span>
    </li>
    <li>
      <!-- campo com a chave escondida e o sinal de pronto -->
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <rect x="6" y="20" width="40" height="16" rx="4" class="contorno" />
        <circle cx="14" cy="28" r="2" class="destaque" />
        <circle cx="21" cy="28" r="2" class="destaque" />
        <circle cx="28" cy="28" r="2" class="destaque" />
        <circle cx="35" cy="28" r="2" class="destaque" />
        <circle cx="48" cy="44" r="11" class="ok" />
        <path d="M43 44l4 4 7-8" class="ok-linha" />
      </svg>
      <span><b>3</b> {t("ia.passos.3")}</span>
    </li>
  </ol>
</section>

<style>
  .passos h4 {
    margin: 0 0 8px;
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--texto-secundario);
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  li {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    text-align: center;
    font-size: 0.75rem;
    line-height: 1.35;
    color: var(--texto-secundario);
  }

  li b {
    color: var(--primaria);
  }

  svg {
    width: 56px;
    height: 56px;
  }

  .contorno {
    fill: none;
    stroke: var(--texto-terciario);
    stroke-width: 2.5;
  }

  .tela {
    fill: color-mix(in srgb, var(--primaria) 12%, transparent);
  }

  .tela-cheia {
    fill: color-mix(in srgb, var(--primaria) 25%, var(--superficie-0));
    stroke: var(--primaria);
    stroke-width: 2.5;
  }

  .destaque {
    fill: var(--primaria);
  }

  .destaque-linha {
    fill: none;
    stroke: var(--primaria);
    stroke-width: 3;
    stroke-linecap: round;
  }

  .ok {
    fill: color-mix(in srgb, var(--sucesso, #2e7d32) 18%, transparent);
    stroke: var(--sucesso, #2e7d32);
    stroke-width: 2.5;
  }

  .ok-linha {
    fill: none;
    stroke: var(--sucesso, #2e7d32);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  @media (max-width: 360px) {
    ol {
      grid-template-columns: 1fr;
    }
    li {
      flex-direction: row;
      text-align: left;
    }
  }
</style>
