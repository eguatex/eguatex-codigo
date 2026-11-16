// Saudacao do Curupira no inicio de cada conversa do assistente. Aparece so na tela: nao vai
// para nenhuma API e nao entra no historico enviado. Os textos ficam em dados/i18n/pt-BR.json,
// nas chaves chat.saudacao.1 a chat.saudacao.8.

import type ptBR from "../../dados/i18n/pt-BR.json";

export const TOTAL_SAUDACOES = 8;

export type ChaveSaudacao = Extract<keyof typeof ptBR, `chat.saudacao.${number}`>;

/**
 * Sorteia o numero de 1 a TOTAL_SAUDACOES da saudacao da conversa nova, sem repetir `anterior`
 * (0 quando nao houve conversa antes). `sorteio` devolve um numero em [0, 1), como Math.random.
 */
export function sortearSaudacao(anterior: number, sorteio: () => number = Math.random): number {
  const opcoes: number[] = [];
  for (let n = 1; n <= TOTAL_SAUDACOES; n++) if (n !== anterior) opcoes.push(n);
  const valor = sorteio();
  const indice = Number.isFinite(valor) ? Math.floor(Math.abs(valor) * opcoes.length) : 0;
  return opcoes[Math.min(indice, opcoes.length - 1)];
}

/** Chave do i18n da saudacao `n`. */
export function chaveSaudacao(n: number): ChaveSaudacao {
  return `chat.saudacao.${n}` as ChaveSaudacao;
}
