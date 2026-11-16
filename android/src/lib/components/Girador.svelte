<script lang="ts">
  // Unica animacao de espera do app. Para com prefers-reduced-motion (o icone fica parado).
  import Loader from "@lucide/svelte/icons/loader";
  import { t } from "$lib/i18n";

  interface Props {
    tamanho?: number;
    /** Texto lido por leitores de tela; padrao "Carregando...". */
    rotulo?: string;
  }

  let { tamanho = 14, rotulo = '' }: Props = $props();
</script>

<span class="girador" role="status" aria-label={rotulo || t('tarefa.carregando')}>
  <Loader size={tamanho} />
</span>

<style>
  .girador {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
  }

  .girador {
    animation: girador-girar 1s linear infinite;
  }

  @keyframes girador-girar {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .girador {
      animation: none;
    }
  }
</style>
