// Limites do KaTeX para o que vem de arquivos de terceiros (zip importado, projeto do Drive).
// maxSize: qualquer medida do usuario (\rule, \kern, \hspace) vira no maximo 20em; sem isso,
// `\rule{1e6em}{1e6em}` pede uma caixa de um milhao de em e trava a tela.
// maxExpand: o KaTeX para depois de 200 expansoes de macro (padrao: 1000), contra macros
// que se multiplicam como `\def\a{\a\a}\a`.
export const LIMITES_KATEX = { maxSize: 20, maxExpand: 200 } as const;
