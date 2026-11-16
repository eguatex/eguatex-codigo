/** Prefixo que o Rust poe no erro quando o arquivo do Drive some ou o app perde o acesso (HTTP 404). */
export const ACESSO_PERDIDO = "ACESSO_PERDIDO:";

/** O erro veio de um 404 do Drive: a pessoa pode escolher o arquivo de novo no seletor do Google. */
export function acessoPerdido(erro: unknown): boolean {
  return String(erro).startsWith(ACESSO_PERDIDO);
}

/** Texto do erro para a tela, sem o prefixo interno. */
export function textoErro(erro: unknown): string {
  const texto = String(erro);
  return texto.startsWith(ACESSO_PERDIDO) ? texto.slice(ACESSO_PERDIDO.length) : texto;
}
