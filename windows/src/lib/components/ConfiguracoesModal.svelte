<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Hammer from "@lucide/svelte/icons/hammer";
  import CodeXml from "@lucide/svelte/icons/code-xml";
  import Palette from "@lucide/svelte/icons/palette";
  import { preferencias, definirPreferencia, type Preferencias } from "$lib/preferencias.svelte";
  import { t, tf } from "$lib/i18n";

  interface Props {
    visivel?: boolean;
    arquivosTex?: string[];
    /** Aba mostrada ao abrir (a sigla do corretor na barra de status abre a do editor). */
    abaInicial?: 'compilador' | 'editor' | 'aparencia';
    onfechar?: () => void;
  }

  let { visivel = false, arquivosTex = [], abaInicial = 'compilador', onfechar }: Props = $props();

  // Chaves das preferencias que sao liga e desliga; as demais usam lista de opcoes.
  type ChaveInterruptor = {
    [K in keyof Preferencias]: Preferencias[K] extends boolean ? K : never;
  }[keyof Preferencias];
  type ChaveSeletor = Exclude<keyof Preferencias, ChaveInterruptor>;

  interface Opcao<T> {
    valor: T;
    rotulo: string;
  }

  interface ItemInterruptor {
    tipo: 'interruptor';
    id: string;
    rotulo: string;
    descricao?: string;
    marcado: () => boolean;
    alterar: (valor: boolean) => void;
  }

  interface ItemSeletor {
    tipo: 'seletor';
    id: string;
    rotulo: string;
    descricao?: string;
    opcoes: () => Array<Opcao<string>>;
    atual: () => string;
    alterar: (valor: string) => void;
  }

  type Item = ItemInterruptor | ItemSeletor;

  interface Aba {
    id: 'compilador' | 'editor' | 'aparencia';
    titulo: string;
    icone: typeof Hammer;
    itens: Item[];
  }

  function interruptor(chave: ChaveInterruptor, rotulo: string, descricao?: string): ItemInterruptor {
    return {
      tipo: 'interruptor',
      id: `cfg-${chave}`,
      rotulo,
      descricao,
      marcado: () => preferencias[chave],
      alterar: (valor) => definirPreferencia(chave, valor),
    };
  }

  // Os valores das opcoes viram texto para o <select>; ao escolher, volta o valor tipado.
  function seletor<K extends ChaveSeletor>(
    chave: K,
    rotulo: string,
    opcoes: () => Array<Opcao<Preferencias[K]>>,
    descricao?: string,
  ): ItemSeletor {
    return {
      tipo: 'seletor',
      id: `cfg-${chave}`,
      rotulo,
      descricao,
      opcoes: () => opcoes().map((o) => ({ valor: String(o.valor), rotulo: o.rotulo })),
      atual: () => String(preferencias[chave]),
      alterar: (texto) => {
        const escolhida = opcoes().find((o) => String(o.valor) === texto);
        if (escolhida) definirPreferencia(chave, escolhida.valor);
      },
    };
  }

  const TAMANHOS: Array<Opcao<number>> = Array.from({ length: 15 }, (_, k) => ({
    valor: 10 + k,
    rotulo: `${10 + k} px`,
  }));

  const FAMILIAS: Array<Opcao<Preferencias['familiaFonte']>> = [
    { valor: 'padrao', rotulo: t('config.fonte-padrao') },
    { valor: 'Consolas', rotulo: 'Consolas' },
    { valor: 'Cascadia Code', rotulo: 'Cascadia Code' },
    { valor: 'Courier New', rotulo: 'Courier New' },
    { valor: 'Lucida Console', rotulo: 'Lucida Console' },
  ];

  const ALTURAS: Array<Opcao<Preferencias['alturaLinha']>> = [
    { valor: 'compacta', rotulo: t('config.altura-compacta') },
    { valor: 'normal', rotulo: t('config.altura-normal') },
    { valor: 'larga', rotulo: t('config.altura-larga') },
  ];

  const PDF_ESCURO: Array<Opcao<Preferencias['pdfEscuro']>> = [
    { valor: 'desligado', rotulo: t('config.pdf-escuro-desligado') },
    { valor: 'tema', rotulo: t('config.pdf-escuro-tema') },
    { valor: 'sempre', rotulo: t('config.pdf-escuro-sempre') },
  ];

  const ORTOGRAFIA: Array<Opcao<Preferencias['ortografia']>> = [
    { valor: 'desligada', rotulo: t('config.ortografia-desligada') },
    { valor: 'portugues', rotulo: t('config.ortografia-portugues') },
    { valor: 'ingles', rotulo: t('config.ortografia-ingles') },
    { valor: 'automatico', rotulo: t('config.ortografia-automatico') },
  ];

  const TEMAS: Array<Opcao<Preferencias['tema']>> = [
    { valor: 'sistema', rotulo: t('config.tema-sistema') },
    { valor: 'claro', rotulo: t('config.tema-claro') },
    { valor: 'escuro', rotulo: t('config.tema-escuro') },
  ];

  // Automatico + cada arquivo .tex. Se o documento salvo nao existe mais, ele continua na lista
  // (marcado) para o usuario ver o que esta configurado e poder trocar.
  function opcoesDocumento(): Array<Opcao<string>> {
    const lista: Array<Opcao<string>> = [{ valor: '', rotulo: t('config.documento-automatico') }];
    const vistos = new Set<string>(['']);
    for (const arquivo of arquivosTex) {
      if (vistos.has(arquivo)) continue;
      vistos.add(arquivo);
      lista.push({ valor: arquivo, rotulo: arquivo });
    }
    const salvo = preferencias.documentoPrincipal;
    if (!vistos.has(salvo)) lista.push({ valor: salvo, rotulo: tf('config.documento-nao-encontrado', { arquivo: salvo }) });
    return lista;
  }

  const abas: Aba[] = [
    {
      id: 'compilador',
      titulo: t('config.compilador'),
      icone: Hammer,
      itens: [
        seletor(
          'documentoPrincipal',
          t('config.documento-principal'),
          opcoesDocumento,
          t('config.documento-principal-descricao'),
        ),
        interruptor('autoSalvar', t('config.auto-salvar'), t('config.auto-salvar-descricao')),
        interruptor('avisarVersaoNova', t('config.avisar-versao'), t('config.avisar-versao-descricao')),
        interruptor('autoCompilar', t('config.compilacao-automatica'), t('config.compilacao-automatica-descricao')),
        interruptor(
          'pararNoPrimeiroErro',
          t('config.parar-no-primeiro-erro'),
          t('config.parar-no-primeiro-erro-descricao'),
        ),
        interruptor('synctexAposCompilar', t('config.synctex-apos-compilar'), t('config.synctex-apos-compilar-descricao')),
        interruptor('permitirShellEscape', t('compilacao.shell-escape'), t('compilacao.shell-escape-descricao')),
      ],
    },
    {
      id: 'editor',
      titulo: t('config.editor'),
      icone: CodeXml,
      itens: [
        interruptor('autocompletar', t('config.autocompletar'), t('config.autocompletar-descricao')),
        interruptor(
          'fecharColchetes',
          t('config.fechar-colchetes'),
          t('config.fechar-colchetes-descricao'),
        ),
        seletor('tamanhoFonte', t('config.tamanho-fonte'), () => TAMANHOS),
        seletor('familiaFonte', t('config.familia-fonte'), () => FAMILIAS),
        seletor('alturaLinha', t('config.altura-linha'), () => ALTURAS),
        interruptor('quebrarLinhas', t('config.quebrar-linhas')),
        interruptor('numerosLinha', t('config.numeros-linha')),
        interruptor('mostrarTrilha', t('config.mostrar-trilha'), t('config.mostrar-trilha-descricao')),
        interruptor('mostrarAbas', t('config.mostrar-abas'), t('config.mostrar-abas-descricao')),
        interruptor('previaEquacao', t('config.previa-equacao'), t('config.previa-equacao-descricao')),
        seletor('ortografia', t('config.ortografia'), () => ORTOGRAFIA, t('config.ortografia-descricao')),
        interruptor('avisosReferencias', t('config.avisos-referencias'), t('config.avisos-referencias-descricao')),
      ],
    },
    {
      id: 'aparencia',
      titulo: t('config.aparencia'),
      icone: Palette,
      itens: [
        seletor('tema', t('config.tema'), () => TEMAS, t('config.tema-descricao')),
        seletor('pdfEscuro', t('config.pdf-escuro'), () => PDF_ESCURO, t('config.pdf-escuro-descricao')),
      ],
    },
  ];

  let aba: Aba['id'] = $state('compilador');
  // Cada abertura comeca na aba pedida (a primeira, se ninguem pediu outra).
  $effect(() => {
    aba = visivel ? abaInicial : 'compilador';
  });

  function selecionarAba(id: Aba['id']) {
    aba = id;
  }

  function aoTeclarNasAbas(e: KeyboardEvent) {
    const indice = abas.findIndex((a) => a.id === aba);
    let novo: number;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') novo = (indice + 1) % abas.length;
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') novo = (indice - 1 + abas.length) % abas.length;
    else if (e.key === 'Home') novo = 0;
    else if (e.key === 'End') novo = abas.length - 1;
    else return;
    e.preventDefault();
    const destino = abas[novo];
    selecionarAba(destino.id);
    document.getElementById(`aba-${destino.id}`)?.focus();
  }

  // Fecha so quando o clique comecou e terminou no fundo (arrastar a selecao para fora nao fecha).
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

  // Leva o foco para a janela ao abrir e devolve ao elemento anterior ao fechar.
  function focarJanela(no: HTMLElement) {
    const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    no.focus();
    return {
      destroy() {
        anterior?.focus();
      },
    };
  }
</script>

<svelte:window onkeydown={aoTeclar} />

{#if visivel}
  <div
    class="fundo-modal"
    class:transparente={aba === 'aparencia'}
    role="presentation"
    onmousedown={aoPressionarFundo}
    onclick={aoClicarFundo}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="titulo-configuracoes"
      tabindex="-1"
      use:focarJanela
    >
      <header>
        <h2 id="titulo-configuracoes">{t('config.titulo')}</h2>
        <button class="fechar" onclick={onfechar} title={t('config.fechar')} aria-label={t('config.fechar')}><X size={18} /></button>
      </header>

      <div class="corpo">
        <div
          class="abas"
          role="tablist"
          aria-orientation="vertical"
          aria-label={t('config.categorias')}
          tabindex="-1"
          onkeydown={aoTeclarNasAbas}
        >
          {#each abas as a (a.id)}
            {@const Icone = a.icone}
            <button
              type="button"
              role="tab"
              id="aba-{a.id}"
              class="aba"
              class:ativa={aba === a.id}
              aria-selected={aba === a.id}
              aria-controls="painel-{a.id}"
              tabindex={aba === a.id ? 0 : -1}
              onclick={() => selecionarAba(a.id)}
            >
              <Icone size={18} />
              <span>{a.titulo}</span>
            </button>
          {/each}
        </div>

        <div class="paineis">
          {#each abas as a (a.id)}
            <div class="painel" role="tabpanel" id="painel-{a.id}" aria-labelledby="aba-{a.id}" hidden={aba !== a.id}>
                {#each a.itens as item (item.id)}
                  <div class="linha">
                    <div class="linha-texto">
                      <label for={item.id}>{item.rotulo}</label>
                      {#if item.descricao}
                        <p>{item.descricao}</p>
                      {/if}
                    </div>
                    <div class="linha-controle">
                      {#if item.tipo === 'interruptor'}
                        <input
                          id={item.id}
                          class="interruptor"
                          type="checkbox"
                          role="switch"
                          checked={item.marcado()}
                          onchange={(e) => item.alterar(e.currentTarget.checked)}
                        />
                      {:else}
                        <select id={item.id} class="seletor" value={item.atual()} onchange={(e) => item.alterar(e.currentTarget.value)}>
                          {#each item.opcoes() as opcao (opcao.valor)}
                            <option value={opcao.valor}>{opcao.rotulo}</option>
                          {/each}
                        </select>
                      {/if}
                    </div>
                  </div>
                {/each}
            </div>
          {/each}
        </div>
      </div>
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
    /* escurece o que esta atras sem usar cor fixa */
    backdrop-filter: brightness(0.5);
  }

  /* Na aba Aparencia o fundo fica claro, para ver o tema mudando atras da janela */
  .fundo-modal.transparente {
    backdrop-filter: none;
  }

  .modal {
    width: min(960px, 94vw);
    max-height: 92vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
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
    display: flex;
    height: min(540px, 76vh);
    min-height: 0;
  }

  .abas {
    flex-shrink: 0;
    width: 240px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px;
    border-right: 1px solid var(--borda);
    overflow-y: auto;
  }

  .aba {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    color: var(--texto);
    font: inherit;
    font-size: 0.875rem;
    text-align: left;
    cursor: pointer;
  }

  .aba:hover:not(.ativa) {
    background: var(--hover);
  }

  .aba.ativa {
    background: var(--primaria-suave);
    color: var(--primaria);
    font-weight: 600;
  }

  .paineis {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 8px 16px 16px;
  }

  .painel[hidden] {
    display: none;
  }

  .linha {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 48px;
    padding: 8px;
    border-radius: var(--raio-sm);
  }

  .linha-texto {
    min-width: 0;
  }

  .linha-texto label {
    font-size: 0.875rem;
    color: var(--texto);
  }

  .linha-texto p {
    margin: 2px 0 0;
    font-size: 0.8125rem;
    color: var(--texto-secundario);
  }

  .linha-controle {
    flex-shrink: 0;
    display: flex;
    justify-content: flex-end;
    min-width: 180px;
  }

  .seletor {
    min-width: 180px;
    max-width: 260px;
    padding: 6px 10px;
    border: 1px solid var(--borda);
    border-radius: var(--raio-sm);
    background: var(--superficie-0);
    color: var(--texto);
    font: inherit;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .seletor option {
    background: var(--superficie-0);
    color: var(--texto);
  }

  /* interruptor desenhado com CSS: verde quando ligado */
  .interruptor {
    appearance: none;
    -webkit-appearance: none;
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 22px;
    margin: 0;
    border-radius: 11px;
    background: var(--texto-terciario);
    cursor: pointer;
    transition: background 0.15s;
  }

  .interruptor::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--superficie-0);
    transition: transform 0.15s;
  }

  .interruptor:checked {
    background: var(--primaria);
  }

  .interruptor:checked::after {
    transform: translateX(18px);
  }

  .fechar:focus-visible,
  .aba:focus-visible,
  .seletor:focus-visible,
  .interruptor:focus-visible {
    outline: 2px solid var(--primaria);
    outline-offset: 2px;
  }

  @media (max-width: 640px) {
    .corpo {
      flex-direction: column;
    }

    .abas {
      width: auto;
      flex-direction: row;
      border-right: none;
      border-bottom: 1px solid var(--borda);
      overflow-x: auto;
      overflow-y: hidden;
    }

    .linha {
      flex-wrap: wrap;
    }
  }
</style>
