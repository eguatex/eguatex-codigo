// Preferencias do usuario (compilador, editor e tema), guardadas em localStorage.
// Arquivo .svelte.ts: usa runes fora de componentes. Nunca reatribua `preferencias`;
// altere apenas as propriedades (de preferencia por definirPreferencia).

import { normalizarModo, type ModoOrtografia } from './ortografia-idioma';

export interface Preferencias {
  /** Sempre 'Automatico': o motor sai do documento (% !TeX program, latexmkrc, pacotes), como no Windows.
   * Os outros valores so existem em preferencias antigas; o celular nao tem seletor de motor. */
  motor: 'Automatico' | 'pdfLaTeX' | 'XeLaTeX' | 'LuaLaTeX';
  documentoPrincipal: string; // caminho relativo; '' = automatico (main.tex)
  autoCompilar: boolean;
  pararNoPrimeiroErro: boolean;
  autocompletar: boolean;
  fecharColchetes: boolean;
  tamanhoFonte: number; // 10 a 24
  familiaFonte: 'padrao' | 'Consolas' | 'Cascadia Code' | 'Courier New' | 'Lucida Console';
  alturaLinha: 'compacta' | 'normal' | 'larga';
  tema: 'sistema' | 'claro' | 'escuro';
  mostrarTrilha: boolean;
  mostrarAbas: boolean;
  previaEquacao: boolean;
  /** Celular deitado: parte da largura que o PDF ocupa no modo dividido (0,3 a 0,7). */
  proporcaoPdf: number;
  ortografia: ModoOrtografia;
  pdfEscuro: 'desligado' | 'tema' | 'sempre';
  quebrarLinhas: boolean;
  numerosLinha: boolean;
  autoSalvar: boolean;
  /** Categoria aberta na barra Matematica ('' = barra fechada). */
  categoriaAtiva: string;
}

const CHAVE_ARMAZENAMENTO = 'eguatex.preferencias';

const FONTE_MINIMA = 10;
const FONTE_MAXIMA = 24;

const PADROES: Preferencias = {
  motor: 'Automatico',
  documentoPrincipal: '',
  autoCompilar: false,
  pararNoPrimeiroErro: false,
  autocompletar: true,
  fecharColchetes: true,
  tamanhoFonte: 14,
  familiaFonte: 'padrao',
  alturaLinha: 'normal',
  tema: 'sistema',
  // desligada na instalacao nova, como no Overleaf; quem ja tem preferencias salvas mantem a trilha ligada (ver carregar)
  mostrarTrilha: false,
  mostrarAbas: true,
  previaEquacao: true,
  proporcaoPdf: 0.5,
  ortografia: 'automatico',
  pdfEscuro: 'desligado',
  quebrarLinhas: true,
  numerosLinha: true,
  autoSalvar: true,
  categoriaAtiva: '',
};

// Cada normalizador devolve o valor valido ou undefined quando o valor nao serve.
type Normalizadores = { [K in keyof Preferencias]: (valor: unknown) => Preferencias[K] | undefined };

function booleano(valor: unknown): boolean | undefined {
  return typeof valor === 'boolean' ? valor : undefined;
}

function umDe<T extends string>(permitidos: readonly T[]): (valor: unknown) => T | undefined {
  return (valor) => permitidos.find((p) => p === valor);
}

const normalizadores: Normalizadores = {
  motor: umDe(['Automatico', 'pdfLaTeX', 'XeLaTeX', 'LuaLaTeX']),
  documentoPrincipal: (valor) => (typeof valor === 'string' ? valor : undefined),
  autoCompilar: booleano,
  pararNoPrimeiroErro: booleano,
  autocompletar: booleano,
  fecharColchetes: booleano,
  tamanhoFonte: (valor) =>
    typeof valor === 'number' && Number.isFinite(valor)
      ? Math.min(FONTE_MAXIMA, Math.max(FONTE_MINIMA, Math.round(valor)))
      : undefined,
  familiaFonte: umDe(['padrao', 'Consolas', 'Cascadia Code', 'Courier New', 'Lucida Console']),
  alturaLinha: umDe(['compacta', 'normal', 'larga']),
  tema: umDe(['sistema', 'claro', 'escuro']),
  mostrarTrilha: booleano,
  mostrarAbas: booleano,
  previaEquacao: booleano,
  ortografia: normalizarModo,
  pdfEscuro: umDe(['desligado', 'tema', 'sempre']),
  quebrarLinhas: booleano,
  numerosLinha: booleano,
  autoSalvar: booleano,
  categoriaAtiva: (valor) => (typeof valor === 'string' && /^[a-z0-9-]{0,41}$/.test(valor) ? valor : undefined),
  proporcaoPdf: (valor) =>
    typeof valor === 'number' && Number.isFinite(valor) ? Math.min(0.7, Math.max(0.3, valor)) : undefined,
};

// Grava `valor` em `alvo[chave]` se for valido (ajustando o que for preciso). Devolve se gravou.
function aplicar<K extends keyof Preferencias>(alvo: Preferencias, chave: K, valor: unknown): boolean {
  const normalizado = normalizadores[chave](valor);
  if (normalizado === undefined) return false;
  alvo[chave] = normalizado;
  return true;
}

function carregar(): Preferencias {
  const resultado: Preferencias = { ...PADROES };
  try {
    const bruto = localStorage.getItem(CHAVE_ARMAZENAMENTO);
    if (!bruto) return resultado;
    const salvo: unknown = JSON.parse(bruto);
    if (typeof salvo !== 'object' || salvo === null) return resultado;
    const lido = salvo as Record<string, unknown>;
    // quem ja tinha preferencias salvas antes do padrao mudar continua com a trilha ligada
    resultado.mostrarTrilha = true;
    for (const chave of Object.keys(PADROES) as Array<keyof Preferencias>) {
      aplicar(resultado, chave, lido[chave]);
    }
    // antes de 10/11/2026 "Mostrar abas" desligado deixava so a aba ativa; agora esconde a barra inteira.
    // Quem gravou antes dessa mudanca volta a ver as abas uma vez; a escolha nova fica marcada por abasV2.
    if (lido.abasV2 !== true) resultado.mostrarAbas = true;
    // desde 12/11/2026 o motor e invisivel, como no Overleaf e no Windows: qualquer motor gravado antes
    // vira Automatico. O projeto ainda fixa um motor com % !TeX program ou com o latexmkrc.
    resultado.motor = 'Automatico';
  } catch {
    // armazenamento indisponivel ou conteudo invalido: valem os padroes
  }
  return resultado;
}

function gravar(): void {
  try {
    localStorage.setItem(CHAVE_ARMAZENAMENTO, JSON.stringify({ ...$state.snapshot(preferencias), abasV2: true }));
  } catch {
    // sem armazenamento: a preferencia vale apenas nesta sessao
  }
}

export const preferencias: Preferencias = $state(carregar());

export function definirPreferencia<K extends keyof Preferencias>(chave: K, valor: Preferencias[K]): void {
  if (!aplicar(preferencias, chave, valor)) return;
  gravar();
}
