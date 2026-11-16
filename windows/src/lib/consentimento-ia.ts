// Consentimento para enviar dados ao provedor de IA escolhido (LGPD). Fica so neste aparelho, em localStorage.
// Mudou o texto da politica de forma que exija novo aceite: troque VERSAO.

const CHAVE = 'eguatex.consentimento-ia';
const VERSAO = '2026-11-15-chave-api';

export function consentimentoIaDado(): boolean {
  try {
    return localStorage.getItem(CHAVE) === VERSAO;
  } catch {
    return false;
  }
}

export function registrarConsentimentoIa(): void {
  try {
    localStorage.setItem(CHAVE, VERSAO);
  } catch {
    // sem armazenamento: o aceite nao fica gravado e a pergunta volta na proxima abertura
  }
}
