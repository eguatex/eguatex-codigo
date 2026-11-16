export type AbaLog = "todos" | "erros" | "avisos" | "info";

export interface EntradaFiltravel {
  tipo: string;
}

export function ehErro(e: EntradaFiltravel): boolean {
  return e.tipo !== "aviso";
}

export function filtrarPorAba<T extends EntradaFiltravel>(itens: T[], aba: AbaLog): T[] {
  if (aba === "erros") return itens.filter(ehErro);
  if (aba === "avisos") return itens.filter((e) => !ehErro(e));
  if (aba === "info") return [];
  return itens;
}

const PADRAO_INFO = /^(Package \S+ Info|LaTeX Info|Class \S+ Info|Output written|Document Class:|Transcript written)/;

/** Linhas informativas do log do TeX: "Package hyperref Info", "Output written on main.pdf". */
export function linhasInfo(log: string): string[] {
  return log
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => PADRAO_INFO.test(l));
}

export function contagens(itens: EntradaFiltravel[], log: string): Record<AbaLog, number> {
  const erros = itens.filter(ehErro).length;
  const avisos = itens.length - erros;
  return { todos: itens.length, erros, avisos, info: linhasInfo(log).length };
}
