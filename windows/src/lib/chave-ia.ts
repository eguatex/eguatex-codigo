// Chave de API do Gemini (Google AI Studio) da versao 42_API. A chave nunca passa por aqui: vai
// direto do campo da tela para o comando cmd_ia_chave_ativar e fica so no cofre do Rust.
import type ptBR from "../../dados/i18n/pt-BR.json";

type Chave = keyof typeof ptBR;

/** Pagina do Google AI Studio em que a pessoa cria a chave, aberta no navegador. */
export const URL_CHAVE_GEMINI = "https://aistudio.google.com/apikey";

/** Codigos que cmd_ia_chave_ativar devolve. */
export const CODIGOS = ["ok", "invalida", "sem-internet", "limite", "tempo", "ocupado", "bloqueada", "erro", "cofre"] as const;
export type CodigoChave = (typeof CODIGOS)[number];

/** Chave do i18n da mensagem de um codigo. Codigo desconhecido vira "erro". */
export function chaveDoResultado(codigo: string): Chave {
  const c = (CODIGOS as readonly string[]).includes(codigo) ? codigo : "erro";
  return `ia.chave.resultado.${c}` as Chave;
}

/** O codigo deixa a IA ativada (a chave foi gravada). */
export function ativou(codigo: string): boolean {
  return codigo === "ok" || codigo === "limite";
}
