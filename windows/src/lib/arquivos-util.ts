export const EXTENSOES_AUXILIARES = [
  ".aux", ".log", ".out", ".toc", ".bbl", ".blg", ".fls", ".fdb_latexmk",
  ".synctex.gz", ".synctex", ".nav", ".snm", ".vrb", ".lof", ".lot", ".idx", ".ilg", ".ind", ".bcf", ".run.xml",
];

export function ehAuxiliar(nome: string): boolean {
  const n = nome.toLowerCase();
  return EXTENSOES_AUXILIARES.some((ext) => n.endsWith(ext));
}

export interface NoComTamanho {
  nome: string;
  tipo: "arquivo" | "pasta";
  tamanho?: number;
  filhos?: NoComTamanho[];
}

/** Remove os arquivos auxiliares da arvore, em qualquer pasta. Pastas ficam, mesmo vazias. */
export function semAuxiliares<T extends NoComTamanho>(nos: T[]): T[] {
  return nos
    .filter((n) => n.tipo === "pasta" || !ehAuxiliar(n.nome))
    .map((n) => (n.filhos ? { ...n, filhos: semAuxiliares(n.filhos) } : n));
}

export function contarAuxiliares(nos: NoComTamanho[]): number {
  let total = 0;
  for (const n of nos) {
    if (n.tipo === "arquivo" && ehAuxiliar(n.nome)) total++;
    if (n.filhos) total += contarAuxiliares(n.filhos);
  }
  return total;
}

/** 0 vira "0 B", 1536 vira "1,5 KB", 2 MB vira "2,0 MB". */
export function formatarTamanho(bytes: number | undefined | null): string {
  if (bytes === undefined || bytes === null || !Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(kb < 10 ? 1 : 0).replace(".", ",")} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(1).replace(".", ",")} MB`;
}
