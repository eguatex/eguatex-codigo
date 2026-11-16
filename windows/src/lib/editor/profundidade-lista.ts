// Profundidade de lista no cursor: o botao de recuo da barra so aparece dentro de uma lista.
// Equivale ao listDepth do Overleaf (toolbar-items.tsx, AGPL-3.0), contado pelo texto em vez da arvore sintatica.

const AMBIENTES_DE_LISTA = 'itemize|enumerate|description';
const RE_AMBIENTE = new RegExp(String.raw`\\(begin|end)\{(?:${AMBIENTES_DE_LISTA})\}`, 'g');

/** Texto da linha sem o comentario (o primeiro % que nao vem depois de uma barra invertida). */
function semComentario(linha: string): string {
  for (let i = 0; i < linha.length; i++) {
    if (linha[i] === '\\') i++;
    else if (linha[i] === '%') return linha.slice(0, i);
  }
  return linha;
}

/**
 * Quantas listas (itemize, enumerate, description) estao abertas na posicao `posicao` do texto.
 * Linhas comentadas nao contam; fora de lista o valor e 0.
 */
export function profundidadeDaLista(texto: string, posicao: number): number {
  const ate = texto.slice(0, Math.max(0, posicao));
  let profundidade = 0;
  for (const linha of ate.split('\n')) {
    for (const m of semComentario(linha).matchAll(RE_AMBIENTE)) {
      profundidade = m[1] === 'begin' ? profundidade + 1 : Math.max(0, profundidade - 1);
    }
  }
  return profundidade;
}
