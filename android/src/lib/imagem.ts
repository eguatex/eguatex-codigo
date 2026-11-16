// Imagens do projeto abertas numa aba: que arquivo e imagem, qual o tipo MIME e como mostrar o tamanho.

/** Acima disso o visualizador nao le o arquivo: mostra so nome, tamanho e o botao Baixar. */
export const LIMITE_IMAGEM_BYTES = 25 * 1024 * 1024;

/** Prefixo do erro do Rust (projetos.rs, PREFIXO_ACIMA_DO_LIMITE); depois dele vem o tamanho em bytes. */
export const PREFIXO_ACIMA_DO_LIMITE = 'acima-do-limite:';

const TIPOS: Record<string, string> = {
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  png: 'image/png',
  gif: 'image/gif',
  webp: 'image/webp',
  bmp: 'image/bmp',
  svg: 'image/svg+xml',
};

function extensao(nome: string): string {
  const base = nome.split(/[\\/]/).pop() ?? '';
  const ponto = base.lastIndexOf('.');
  // ".png" sozinho e nome de arquivo sem extensao, nao uma imagem
  return ponto > 0 ? base.slice(ponto + 1).toLowerCase() : '';
}

/** O nome (ou caminho) e de uma imagem que o visualizador abre. Maiuscula ou minuscula; .tex e .pdf nao. */
export function ehImagem(nome: string): boolean {
  return Object.hasOwn(TIPOS, extensao(nome));
}

/** Tipo MIME do blob da imagem; "application/octet-stream" para o que nao e imagem. */
export function tipoMime(nome: string): string {
  const ext = extensao(nome);
  return Object.hasOwn(TIPOS, ext) ? TIPOS[ext] : 'application/octet-stream';
}

/** "812 KB" ate 1 MB; acima, "3,4 MB". */
export function formatarTamanho(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1).replace('.', ',')} MB`;
}

/** Tamanho em bytes que o Rust informou ao recusar o arquivo grande; null se o erro e outro. */
export function tamanhoDoErroAcimaDoLimite(erro: unknown): number | null {
  const texto = typeof erro === 'string' ? erro : erro instanceof Error ? erro.message : '';
  if (!texto.startsWith(PREFIXO_ACIMA_DO_LIMITE)) return null;
  const n = Number(texto.slice(PREFIXO_ACIMA_DO_LIMITE.length));
  return Number.isFinite(n) ? n : null;
}
