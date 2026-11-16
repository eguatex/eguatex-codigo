<script lang="ts">
  // Tela de ligar a IA com a chave do Gemini da propria pessoa (Google AI Studio).
  // A chave fica so na variavel do campo enquanto a pessoa cola; vai direto para o comando
  // cmd_ia_chave_ativar e mora no cofre do Rust. Nada aqui grava a chave no armazenamento do
  // navegador, em arquivo ou no console do depurador.
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t, tf } from "$lib/i18n";
  import { SITE_ASSISTENTE_IA, SITE_PRIVACIDADE, SITE_TERMOS } from "$lib/site";
  import { URL_CHAVE_GEMINI, chaveDoResultado, ativou } from "$lib/chave-ia";
  import PassosChave from "./PassosChave.svelte";
  import Girador from "./Girador.svelte";
  import Check from "@lucide/svelte/icons/check";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Trash2 from "@lucide/svelte/icons/trash-2";

  interface ConfigIA {
    ativa: boolean;
    provedor: string;
    modelo: string;
    nivel?: string;
  }

  interface Props {
    onconfigurar?: (config: ConfigIA) => void;
    /** Chamado quando a IA fica ativada com uma chave guardada. */
    onpronto?: () => void;
  }

  let { onconfigurar, onpronto }: Props = $props();

  let config: ConfigIA = $state({
    ativa: false,
    provedor: "gemini",
    modelo: "",
    nivel: "sugerir",
  });

  let comChave = $state(false);
  let chave = $state("");
  let mostrar = $state(false);
  let testando = $state(false);
  let resultado = $state("");
  let erroSalvar = $state(false);

  const lugar = $derived(t("ia.lugar.computador"));
  const ativoAqui = $derived(config.ativa && comChave);

  recarregar();

  async function recarregar() {
    try {
      const salvo = await invoke<ConfigIA>("cmd_ia_carregar_config");
      config = { ...salvo, nivel: salvo.nivel || "sugerir" };
      comChave = await invoke<boolean>("cmd_ia_chave_status");
    } catch {
      comChave = false;
    }
  }

  function avisar() {
    onconfigurar?.({ ...config });
    window.dispatchEvent(new Event("ia-chave-mudou"));
  }

  function criarChave() {
    openUrl(URL_CHAVE_GEMINI).catch(() => {});
  }

  async function ativar() {
    const texto = chave.trim();
    if (!texto || testando) return;
    testando = true;
    resultado = "";
    try {
      resultado = await invoke<string>("cmd_ia_chave_ativar", { chave: texto });
    } catch {
      resultado = "erro";
    } finally {
      testando = false;
    }
    if (ativou(resultado)) {
      chave = "";
      mostrar = false;
      await recarregar();
      avisar();
      onpronto?.();
    }
  }

  /** Volta a usar a chave que ja esta no cofre. */
  async function usarGuardada() {
    erroSalvar = false;
    const nova = { ...config, ativa: true, provedor: "gemini" };
    try {
      await invoke("cmd_ia_salvar_config", { config: nova });
    } catch {
      erroSalvar = true;
      return;
    }
    await recarregar();
    avisar();
    onpronto?.();
  }

  async function remover() {
    erroSalvar = false;
    resultado = "";
    try {
      await invoke("cmd_ia_chave_remover");
    } catch {
      erroSalvar = true;
    }
    await recarregar();
    avisar();
  }
</script>

<div class="overlay embutido">
  <div class="modal embutido">
    <div class="modal-corpo embutido">
      {#if ativoAqui}
        <div class="aviso-google pronto">
          <Check size={16} />
          <span>{t("ia.config.ativa")}</span>
        </div>
      {:else}
        <div class="secao">
          <p class="dica">{tf("ia.config.explicacao", { lugar })}</p>
          <PassosChave />
          <button class="btn-secundario" onclick={criarChave}>
            <ExternalLink size={13} />
            {t("ia.config.criar")}
          </button>
          <button class="link-politica" onclick={() => openUrl(SITE_ASSISTENTE_IA).catch(() => {})}>{t("ia.config.passo-a-passo")}</button>

          <label class="campo-rotulo" for="chave-api">{t("ia.config.campo")}</label>
          <div class="campo-chave">
            <input
              id="chave-api"
              type={mostrar ? "text" : "password"}
              bind:value={chave}
              placeholder={t("ia.config.campo-placeholder")}
              autocomplete="off"
              autocapitalize="off"
              spellcheck="false"
              disabled={testando}
              onkeydown={(e) => { if (e.key === "Enter") ativar(); }}
            />
            <button class="btn-secundario" onclick={() => (mostrar = !mostrar)} aria-pressed={mostrar}>
              {#if mostrar}<EyeOff size={13} /> {t("ia.config.esconder")}{:else}<Eye size={13} /> {t("ia.config.mostrar")}{/if}
            </button>
          </div>
          <p class="dica">{t("ia.config.dica-colar")}</p>

          <button class="btn-google" onclick={ativar} disabled={testando || !chave.trim()}>
            {#if testando}
              <Girador tamanho={14} />
              {t("ia.config.testando")}
            {:else}
              <Check size={14} />
              {t("ia.config.ativar")}
            {/if}
          </button>
          {#if comChave && !testando}
            <button class="btn-secundario" onclick={usarGuardada}>{t("ia.config.usar-guardada")}</button>
          {/if}
        </div>
      {/if}

      {#if resultado}
        <p class={ativou(resultado) ? "aviso-google pronto" : "erro-google"} role="status">{t(chaveDoResultado(resultado))}</p>
      {/if}
      {#if erroSalvar}
        <p class="erro-google" role="status">{t("ia.config.erro-salvar")}</p>
      {/if}

      {#if comChave}
        <button class="btn-perigo" onclick={remover}>
          <Trash2 size={13} />
          {t("ia.config.remover")}
        </button>
      {/if}

      <button class="link-politica" onclick={() => openUrl(SITE_PRIVACIDADE).catch(() => {})}>{t("ia.consentimento.politica")}</button>
      <button class="link-politica" onclick={() => openUrl(SITE_TERMOS).catch(() => {})}>{t("termos.link")}</button>
    </div>
  </div>
</div>

<style>
  .link-politica {
    align-self: flex-start;
    background: none;
    border: none;
    padding: 0 0 8px;
    color: var(--destaque, #0277bd);
    text-decoration: underline;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(2px);
  }

  .modal {
    background: var(--superficie-0);
    border-radius: 12px;
    width: 520px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--sombra-3);
    border: 1px solid var(--borda);
  }

  .modal-corpo {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .secao {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dica {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--texto-terciario);
    line-height: 1.5;
  }

  .campo-rotulo {
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--texto);
  }

  .campo-chave {
    display: flex;
    gap: 6px;
  }

  .campo-chave input {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
    font-family: var(--fonte-mono, monospace);
    font-size: 0.875rem;
  }

  .aviso-google {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 8px 12px;
    border-radius: var(--raio-sm);
    font-size: 0.8125rem;
  }

  .aviso-google.pronto {
    color: var(--primaria);
    background: color-mix(in srgb, var(--primaria) 10%, var(--superficie-1));
  }

  .btn-google {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 8px 16px;
    border: none;
    border-radius: var(--raio-sm);
    background: var(--primaria);
    color: white;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
    font-family: inherit;
  }

  .btn-google:hover:not(:disabled) {
    background: color-mix(in srgb, var(--primaria) 15%, var(--primaria));
  }

  .btn-google:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .erro-google {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--perigo);
    padding: 8px 12px;
    border-radius: var(--raio-sm);
    background: color-mix(in srgb, var(--perigo) 8%, var(--superficie-1));
  }

  .btn-secundario {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 8px 14px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
    font-size: 0.8125rem;
    cursor: pointer;
    font-family: inherit;
    white-space: nowrap;
    transition: all 0.15s;
  }

  .btn-secundario:hover:not(:disabled) {
    background: var(--hover);
  }

  .btn-perigo {
    align-self: flex-start;
    border: none;
    background: transparent;
    color: var(--perigo);
    cursor: pointer;
    padding: 6px 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .btn-perigo:hover {
    background: color-mix(in srgb, var(--perigo) 10%, transparent);
  }

  /* Modo embutido: sem fundo escuro, sem moldura de janela, ocupando a largura do lugar onde estiver */
  .overlay.embutido {
    position: static;
    display: block;
    background: transparent;
    backdrop-filter: none;
    z-index: auto;
  }

  .modal.embutido {
    width: auto;
    max-height: none;
    background: transparent;
    border: none;
    border-radius: 0;
    box-shadow: none;
  }

  .modal-corpo.embutido {
    overflow: visible;
    padding: 12px 8px;
  }
</style>
