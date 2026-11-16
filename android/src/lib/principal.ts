// Escolha do documento principal do projeto. A mesma regra do Rust (arquivos.rs:
// principal_entre), para o app e o backend sugerirem o mesmo arquivo. Como no Overleaf,
// so um .tex com \begin{document} pode ser principal.

/** Arquivo do projeto com o texto lido: `caminho` como a arvore informa (pode ter "/" na frente). */
export interface ArquivoTexto {
  caminho: string;
  texto: string;
}

/** Parte da linha antes do comentario (um % sem barra antes comenta o resto da linha). */
export function semComentario(linha: string): string {
  let anterior = '';
  for (let i = 0; i < linha.length; i++) {
    const ch = linha[i];
    if (ch === '%' && anterior !== '\\') return linha.slice(0, i);
    anterior = anterior === '\\' && ch === '\\' ? '' : ch;
  }
  return linha;
}

/** O texto tem \documentclass fora de comentario. */
export function temDocumentclass(texto: string): boolean {
  for (const linha of texto.split(/\r?\n/)) {
    const codigo = semComentario(linha);
    for (const achado of codigo.matchAll(/\\documentclass/g)) {
      const proximo = codigo.charAt((achado.index ?? 0) + achado[0].length);
      if (!/\p{L}/u.test(proximo)) return true;
    }
  }
  return false;
}

/** O texto tem \begin{document} fora de comentario (aceita espacos: \begin {document}). */
export function temBeginDocument(texto: string): boolean {
  return texto.split(/\r?\n/).some((linha) => semComentario(linha).replace(/\s+/g, '').includes('\\begin{document}'));
}

/** O documento usa a classe standalone (figura avulsa, compilada a parte). */
function classeStandalone(texto: string): boolean {
  return texto.split(/\r?\n/).some((linha) => {
    const codigo = semComentario(linha).replace(/\s+/g, '');
    const i = codigo.indexOf('\\documentclass');
    return i >= 0 && codigo.slice(i).includes('{standalone}');
  });
}

/**
 * O principal entre os .tex com \begin{document} fora de comentario (um preambulo so com
 * \documentclass nunca e principal). Empate: figura standalone por ultimo, depois quem tem
 * \documentclass no proprio arquivo, main.tex, o nome igual ao da pasta do projeto, o maior,
 * o mais raso e a ordem alfabetica. `null` se nenhum serve.
 */
export function escolherPrincipal(arquivos: ArquivoTexto[], nomePasta = ''): string | null {
  const limpo = (c: string) => c.replace(/\\/g, '/').replace(/^\/+/, '');
  const pasta = nomePasta.toLowerCase();
  const candidatos = arquivos
    .filter((a) => /\.tex$/i.test(a.caminho) && temBeginDocument(a.texto))
    .map((a) => {
      const rel = limpo(a.caminho);
      const nome = rel.split('/').pop()!.toLowerCase();
      return {
        original: a.caminho,
        rel,
        chave: [
          classeStandalone(a.texto) ? 1 : 0,
          temDocumentclass(a.texto) ? 0 : 1,
          nome === 'main.tex' ? 0 : 1,
          pasta && nome.replace(/\.tex$/, '') === pasta ? 0 : 1,
          -a.texto.length,
          rel.split('/').length - 1,
        ],
      };
    });
  if (!candidatos.length) return null;
  candidatos.sort((a, b) => {
    for (let i = 0; i < a.chave.length; i++) if (a.chave[i] !== b.chave[i]) return a.chave[i] - b.chave[i];
    return a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0;
  });
  return candidatos[0].original;
}

/** Chave do localStorage com o principal de um projeto (a chave leva o caminho do projeto). */
export function chavePrincipalDoProjeto(caminhoProjeto: string): string {
  return `eguatex.principal:${caminhoProjeto.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()}`;
}
