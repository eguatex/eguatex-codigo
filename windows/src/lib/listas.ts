// Lista com marcadores ou numerada a partir do texto selecionado: um \item por linha.

export type AmbienteLista = 'itemize' | 'enumerate';

/**
 * Monta o ambiente de lista. Cada linha não vazia da seleção vira um \item e o recuo da primeira
 * linha vale para o bloco todo. Sem seleção, devolve o modelo com o cursor depois do primeiro \item.
 * `cursor` é a posição dentro de `texto`.
 */
export function listaDeLinhas(selecao: string, ambiente: AmbienteLista): { texto: string; cursor: number } {
  const linhas = selecao.split(/\r?\n/).filter((l) => l.trim() !== '');
  if (linhas.length === 0) {
    const inicio = `\\begin{${ambiente}}\n  \\item `;
    return { texto: `${inicio}\n\\end{${ambiente}}`, cursor: inicio.length };
  }
  const recuo = /^[ \t]*/.exec(linhas[0])![0];
  const itens = linhas.map((l) => {
    const limpo = l.trim();
    const conteudo = limpo.startsWith('\\item') ? limpo.replace(/^\\item\s*/, '') : limpo;
    return `${recuo}  \\item ${conteudo}`;
  });
  const texto = [`${recuo}\\begin{${ambiente}}`, ...itens, `${recuo}\\end{${ambiente}}`].join('\n');
  return { texto, cursor: texto.length };
}
