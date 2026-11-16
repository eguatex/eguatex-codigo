<script lang="ts">
  // Gerenciar atalhos e abas: categorias (renomear, reordenar, ocultar, apagar as do usuario) e, por categoria,
  // os botoes do usuario (adicionar, editar, remover, reordenar). Os botoes padrao nao se editam.
  import X from '@lucide/svelte/icons/x';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Plus from '@lucide/svelte/icons/plus';
  import Check from '@lucide/svelte/icons/check';
  import { t, tf } from '$lib/i18n';
  import { atalhos, alterarAtalhos, nomeDaCategoria } from '$lib/atalhos.svelte';
  import { preferencias, definirPreferencia } from '$lib/preferencias.svelte';
  import {
    adicionarBotao,
    alternarOculta,
    apagarCategoria,
    comandoValido,
    criarCategoria,
    editarBotao,
    LIMITE_NOME,
    montarCategorias,
    moverBotao,
    moverCategoria,
    removerBotao,
    renomearCategoria,
  } from '$lib/barra-matematica';

  interface Props {
    aoFechar: () => void;
    /** Abre ja no formulario de nova categoria (item "+ Nova categoria" do menu). */
    iniciarNova?: boolean;
  }

  let { aoFechar, iniciarNova = false }: Props = $props();

  const categorias = $derived(montarCategorias(atalhos.usuario));

  let selecionada = $state(preferencias.categoriaAtiva || 'matematica');
  // svelte-ignore state_referenced_locally
  let criando = $state(iniciarNova);
  let nomeNova = $state('');
  let renomeando = $state<string | null>(null);
  let nomeEditado = $state('');
  let rotulo = $state('');
  let codigo = $state('');
  let editando = $state<number | null>(null);
  let erroCodigo = $state(false);

  const atual = $derived(categorias.find((c) => c.id === selecionada) ?? categorias[0]);

  function criar(e: Event) {
    e.preventDefault();
    const r = criarCategoria(atalhos.usuario, nomeNova);
    if (!r) return;
    alterarAtalhos(() => r.usuario);
    selecionada = r.id;
    nomeNova = '';
    criando = false;
  }

  function iniciarRenomear(id: string, nome: string) {
    renomeando = id;
    nomeEditado = nome;
  }

  function confirmarRenomear(e: Event) {
    e.preventDefault();
    if (renomeando && nomeEditado.trim()) alterarAtalhos((u) => renomearCategoria(u, renomeando as string, nomeEditado));
    renomeando = null;
  }

  function apagar(id: string) {
    alterarAtalhos((u) => apagarCategoria(u, id));
    if (preferencias.categoriaAtiva === id) definirPreferencia('categoriaAtiva', '');
    if (selecionada === id) selecionada = 'matematica';
  }

  function limparFormulario() {
    rotulo = '';
    codigo = '';
    editando = null;
    erroCodigo = false;
  }

  function salvarBotao(e: Event) {
    e.preventDefault();
    if (!atual) return;
    if (!comandoValido(codigo.trim())) {
      erroCodigo = true;
      return;
    }
    const botao = { rotulo: rotulo.trim() || undefined, comando: codigo.trim() };
    const id = atual.id;
    const i = editando;
    alterarAtalhos((u) => (i === null ? adicionarBotao(u, id, botao) : editarBotao(u, id, i, botao)));
    limparFormulario();
  }

  function editarDe(i: number) {
    if (!atual) return;
    const b = atual.botoesProprios[i];
    editando = i;
    rotulo = b.rotulo ?? '';
    codigo = b.comando;
    erroCodigo = false;
  }

  function tecla(e: KeyboardEvent) {
    if (e.key === 'Escape') aoFechar();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-overlay" role="dialog" aria-modal="true" aria-label={t('atalhos.gerenciar-titulo')} tabindex="-1" onclick={aoFechar} onkeydown={tecla}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal-conteudo" role="document" onclick={(e) => e.stopPropagation()}>
    <div class="cabecalho">
      <h2>{t('atalhos.gerenciar-titulo')}</h2>
      <button type="button" class="icone" onclick={aoFechar} aria-label={t('atalhos.fechar')} title={t('atalhos.fechar')}><X size={18} /></button>
    </div>

    <div class="corpo">
      <section class="coluna">
        <h3>{t('atalhos.categorias')}</h3>
        <ul class="lista">
          {#each categorias as c, i (c.id)}
            <li class="linha" class:escolhida={atual?.id === c.id} class:oculta={c.oculta}>
              {#if renomeando === c.id}
                <form class="inline" onsubmit={confirmarRenomear}>
                  <!-- svelte-ignore a11y_autofocus -->
                  <input type="text" bind:value={nomeEditado} maxlength={LIMITE_NOME} aria-label={t('atalhos.nome-categoria')} autofocus />
                  <button type="submit" class="icone" aria-label={t('atalhos.salvar')} title={t('atalhos.salvar')}><Check size={16} /></button>
                </form>
              {:else}
                <button type="button" class="nome" onclick={() => (selecionada = c.id)}>
                  {nomeDaCategoria(c)}
                  {#if c.oculta}<small>{t('atalhos.etiqueta-oculta')}</small>{/if}
                </button>
                <button type="button" class="icone" disabled={i === 0} aria-label={t('atalhos.subir')} title={t('atalhos.subir')} onclick={() => alterarAtalhos((u) => moverCategoria(u, c.id, -1))}><ArrowUp size={16} /></button>
                <button type="button" class="icone" disabled={i === categorias.length - 1} aria-label={t('atalhos.descer')} title={t('atalhos.descer')} onclick={() => alterarAtalhos((u) => moverCategoria(u, c.id, 1))}><ArrowDown size={16} /></button>
                <button type="button" class="icone" aria-label={t('atalhos.renomear')} title={t('atalhos.renomear')} onclick={() => iniciarRenomear(c.id, nomeDaCategoria(c))}><Pencil size={16} /></button>
                {#if c.padrao}
                  <button type="button" class="icone" aria-label={c.oculta ? t('atalhos.mostrar') : t('atalhos.ocultar')} title={c.oculta ? t('atalhos.mostrar') : t('atalhos.ocultar')} onclick={() => alterarAtalhos((u) => alternarOculta(u, c.id))}>
                    {#if c.oculta}<Eye size={16} />{:else}<EyeOff size={16} />{/if}
                  </button>
                {:else}
                  <button type="button" class="icone" aria-label={t('atalhos.apagar')} title={t('atalhos.apagar')} onclick={() => apagar(c.id)}><Trash2 size={16} /></button>
                {/if}
              {/if}
            </li>
          {/each}
        </ul>

        {#if criando}
          <form class="nova" onsubmit={criar}>
            <label for="nome-nova-categoria">{t('atalhos.nova-categoria-titulo')}</label>
            <div class="inline">
              <!-- svelte-ignore a11y_autofocus -->
              <input id="nome-nova-categoria" type="text" bind:value={nomeNova} maxlength={LIMITE_NOME} placeholder={t('atalhos.nome-categoria')} autofocus />
              <button type="submit" class="primario" disabled={!nomeNova.trim()}>{t('atalhos.criar')}</button>
            </div>
          </form>
        {:else}
          <button type="button" class="secundario" onclick={() => (criando = true)}><Plus size={16} /> {t('atalhos.nova-categoria')}</button>
        {/if}
      </section>

      <section class="coluna">
        {#if atual}
          <h3>{tf('atalhos.botoes-de', { nome: nomeDaCategoria(atual) })}</h3>
          {#if atual.botoesPadrao.length > 0}
            <p class="nota">{tf('atalhos.botoes-padrao', { n: atual.botoesPadrao.length })}</p>
          {/if}
          <h4>{t('atalhos.seus-botoes')}</h4>
          <ul class="lista">
            {#each atual.botoesProprios as b, i (`${i}:${b.comando}`)}
              <li class="linha">
                <span class="amostra">{b.rotulo ?? b.comando}</span>
                <code class="cmd" title={b.comando}>{b.comando}</code>
                <button type="button" class="icone" disabled={i === 0} aria-label={t('atalhos.subir')} title={t('atalhos.subir')} onclick={() => alterarAtalhos((u) => moverBotao(u, atual.id, i, -1))}><ArrowUp size={16} /></button>
                <button type="button" class="icone" disabled={i === atual.botoesProprios.length - 1} aria-label={t('atalhos.descer')} title={t('atalhos.descer')} onclick={() => alterarAtalhos((u) => moverBotao(u, atual.id, i, 1))}><ArrowDown size={16} /></button>
                <button type="button" class="icone" aria-label={t('atalhos.editar')} title={t('atalhos.editar')} onclick={() => editarDe(i)}><Pencil size={16} /></button>
                <button type="button" class="icone" aria-label={t('atalhos.remover')} title={t('atalhos.remover')} onclick={() => { alterarAtalhos((u) => removerBotao(u, atual.id, i)); limparFormulario(); }}><Trash2 size={16} /></button>
              </li>
            {:else}
              <li class="nota">{t('atalhos.sem-proprios')}</li>
            {/each}
          </ul>

          <form class="formulario" onsubmit={salvarBotao}>
            <label>
              {t('atalhos.rotulo')}
              <input type="text" bind:value={rotulo} maxlength="40" />
            </label>
            <label>
              {t('atalhos.codigo')}
              <input type="text" bind:value={codigo} maxlength="2000" class:erro={erroCodigo} oninput={() => (erroCodigo = false)} spellcheck="false" />
            </label>
            {#if erroCodigo}<p class="mensagem-erro">{t('atalhos.erro-codigo')}</p>{/if}
            <div class="acoes">
              <button type="submit" class="primario" disabled={!codigo.trim()}>{editando === null ? t('atalhos.adicionar-botao') : t('atalhos.salvar')}</button>
              {#if editando !== null}<button type="button" class="secundario" onclick={limparFormulario}>{t('atalhos.cancelar')}</button>{/if}
            </div>
          </form>
        {/if}
      </section>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
  }

  .modal-conteudo {
    width: 100%;
    max-width: 880px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--superficie-0);
    color: var(--texto);
    border: 1px solid var(--borda);
    border-radius: 12px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
  }

  .cabecalho {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    border-bottom: 1px solid var(--borda);
  }

  h2 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
  }

  h3 {
    margin: 0 0 8px;
    font-size: 0.9375rem;
  }

  h4 {
    margin: 12px 0 6px;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
  }

  .corpo {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
    padding: 16px 20px 20px;
    overflow-y: auto;
  }

  .coluna {
    min-width: 0;
  }

  .lista {
    margin: 0 0 12px;
    padding: 0;
    list-style: none;
  }

  .linha {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 34px;
    padding: 0 4px;
    border-radius: 6px;
  }

  .linha.escolhida {
    background: var(--primaria-suave);
  }

  .linha.oculta .nome {
    opacity: 0.6;
  }

  .nome {
    flex: 1 1 auto;
    min-width: 0;
    padding: 6px 4px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .nome small {
    margin-left: 6px;
    color: var(--texto-secundario);
  }

  .amostra {
    flex: 0 0 auto;
    min-width: 40px;
    margin-right: 6px;
    padding: 3px 8px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    text-align: center;
    font-family: 'Noto Serif', 'Times New Roman', serif;
  }

  .cmd {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.75rem;
    color: var(--texto-secundario);
  }

  .icone {
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--texto-secundario);
    cursor: pointer;
  }

  .icone:hover:not(:disabled) {
    background: var(--hover);
    color: var(--texto);
  }

  .icone:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .inline {
    display: flex;
    flex: 1 1 auto;
    gap: 6px;
  }

  input[type='text'] {
    flex: 1 1 auto;
    min-width: 0;
    box-sizing: border-box;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--borda);
    border-radius: 6px;
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
  }

  input.erro {
    border-color: #c62828;
  }

  .formulario,
  .nova {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
  }

  .acoes {
    display: flex;
    gap: 8px;
  }

  .primario,
  .secundario {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 6px;
    font: inherit;
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .primario {
    border: none;
    background: var(--primaria);
    color: #fff;
  }

  .primario:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .secundario {
    border: 1px solid var(--borda);
    background: transparent;
    color: var(--texto);
  }

  .secundario:hover {
    background: var(--hover);
  }

  .nota {
    margin: 0 0 6px;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
  }

  .mensagem-erro {
    margin: 0;
    font-size: 0.8125rem;
    color: #c62828;
  }
</style>
