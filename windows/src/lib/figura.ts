// Figuras do projeto: onde ficam e o bloco LaTeX que entra no editor.

/** Pasta do projeto onde as figuras importadas (do computador, do aparelho ou do Drive) ficam. */
export const PASTA_FIGURAS = "figuras";

/** Formatos aceitos nos seletores de figura. */
export const EXTENSOES_FIGURA = ["png", "jpg", "jpeg", "pdf", "eps"];

export function ehFigura(nome: string): boolean {
  const ponto = nome.lastIndexOf(".");
  return ponto > 0 && EXTENSOES_FIGURA.includes(nome.slice(ponto + 1).toLowerCase());
}

/** "/figuras/foto (2).png" (formato da arvore) vira "figuras/foto (2).png" (formato do LaTeX). */
export function caminhoDeFigura(relativo: string): string {
  return relativo.replace(/\\/g, "/").replace(/^\/+/, "");
}

/**
 * Bloco de figura inserido no cursor. Com `caminho` ("figuras/foto.png") o
 * \includegraphics ja aponta para o arquivo; sem ele fica vazio, para a pessoa preencher.
 */
export function blocoFigura(caminho = ""): string {
  return [
    "\\begin{figure}[h]",
    "  \\centering",
    `  \\includegraphics[width=0.8\\linewidth]{${caminho}}`,
    "  \\caption{}",
    "  \\label{fig:}",
    "\\end{figure}",
  ].join("\n");
}

/**
 * Garante que um bloco comece e termine em linha propria: quebra antes se ha texto antes do cursor
 * na linha, e depois se ha texto depois dele. `antes` e `depois` sao o texto da linha de cada lado
 * do cursor (ou da selecao).
 */
export function blocoEmLinhaPropria(bloco: string, antes: string, depois: string): string {
  return (antes.trim() ? "\n" : "") + bloco + (depois.trim() ? "\n" : "");
}
