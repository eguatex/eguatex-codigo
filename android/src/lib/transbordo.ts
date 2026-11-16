// Transbordo da barra do editor: decide quais grupos saem da barra e vao para o "mais".
// Regra do Overleaf (codemirror-toolbar.tsx, AGPL-3.0): os grupos saem de tras para frente
// ate o que sobra caber na largura disponivel.

export interface OpcoesTransbordo {
  /** Largura dos itens que nunca saem (desfazer, refazer, negrito, italico). */
  fixo?: number;
  /** Largura do botao "mais", que so existe quando algum grupo sai. */
  mais?: number;
}

/**
 * Devolve os indices (em ordem crescente) dos grupos que saem da barra.
 * `larguras` vem na ordem da tela; o ultimo grupo sai primeiro.
 * Se tudo cabe sem o "mais", nada sai. Se algum sai, o "mais" passa a ocupar espaco.
 */
export function calcularTransbordo(larguras: readonly number[], disponivel: number, opcoes: OpcoesTransbordo = {}): number[] {
  const fixo = opcoes.fixo ?? 0;
  const mais = opcoes.mais ?? 0;
  let total = fixo + larguras.reduce((soma, l) => soma + l, 0);
  if (total <= disponivel) return [];
  total += mais;
  const saem: number[] = [];
  for (let i = larguras.length - 1; i >= 0 && total > disponivel; i--) {
    total -= larguras[i];
    saem.unshift(i);
  }
  return saem;
}
