/**
 * Conversa do assistente. Fica na memoria enquanto o programa estiver aberto (minimizar o
 * painel ou voltar para a tela inicial nao apaga) e, quando o chat sabe a pasta do projeto,
 * tambem em `.eguatex/chat.json` dentro dela. Essa pasta oculta nao entra no zip do Drive.
 */
import { prepararHistorico } from "./agente";

export const sessaoChat = {
  mensagens: [] as unknown[],
  historico: [] as unknown[],
  texto: "",
  /** Projeto a que a conversa em memoria pertence ("" = nao se sabe). */
  pasta: "",
  /** Saudacao do Curupira da conversa atual (1 a 8; 0 = ainda nao sorteada). So aparece na tela. */
  saudacao: 0,
};

export const ARQUIVO_CHAT = ".eguatex/chat.json";
const VERSAO_CHAT = 1;
const MAX_MENSAGENS_SALVAS = 200;

export interface ConversaSalva {
  mensagens: unknown[];
  historico: unknown[];
}

/** JSON gravado em .eguatex/chat.json: as mensagens da tela e o historico enviado ao Gemini. */
export function serializarConversa(mensagens: unknown[], historico: unknown[]): string {
  const conversa = {
    versao: VERSAO_CHAT,
    mensagens: mensagens.slice(-MAX_MENSAGENS_SALVAS),
    historico: prepararHistorico(historico as Parameters<typeof prepararHistorico>[0]),
  };
  return JSON.stringify(conversa);
}

/** Le o JSON gravado. Devolve null se estiver vazio, corrompido ou de outra versao. */
export function carregarConversa(texto: string): ConversaSalva | null {
  try {
    const j = JSON.parse(texto);
    if (!j || j.versao !== VERSAO_CHAT || !Array.isArray(j.mensagens) || !Array.isArray(j.historico)) return null;
    const mensagens = j.mensagens.filter(
      (m: unknown) => !!m && typeof m === "object" && typeof (m as { conteudo?: unknown }).conteudo === "string",
    );
    const historico = j.historico.filter(
      (c: unknown) => !!c && typeof c === "object" && Array.isArray((c as { parts?: unknown }).parts),
    );
    return { mensagens, historico };
  } catch {
    return null;
  }
}
