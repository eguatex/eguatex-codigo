// Abertura do ÉguaTeX: a tela do Curupira aparece uma vez por sessão do app.
// Minimizar e voltar não mostra de novo; fechar e abrir o app mostra.

export const CHAVE_ABERTURA = 'eguatex-abertura-vista';
export const DURACAO_ABERTURA_MS = 2000;
export const SAIDA_ABERTURA_MS = 400;

type Armazenamento = Pick<Storage, 'getItem' | 'setItem'>;

function sessao(): Armazenamento | null {
  try {
    return typeof sessionStorage === 'undefined' ? null : sessionStorage;
  } catch {
    return null;
  }
}

/** Mostra a abertura se ela ainda não apareceu nesta sessão. Sem armazenamento, mostra (é a primeira vez). */
export function deveMostrarAbertura(armazenamento: Armazenamento | null = sessao()): boolean {
  try {
    return armazenamento?.getItem(CHAVE_ABERTURA) !== '1';
  } catch {
    return true;
  }
}

export function marcarAberturaVista(armazenamento: Armazenamento | null = sessao()): void {
  try {
    armazenamento?.setItem(CHAVE_ABERTURA, '1');
  } catch {
    // sem armazenamento a abertura só volta a aparecer se a página recarregar
  }
}
