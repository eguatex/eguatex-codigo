// Ambientes matematicos do menu Inserir > Matematica: o texto que entra no editor e onde o cursor fica.
// Funcao pura, sem depender do editor, para testar sem abrir o app.

export type IdAmbiente = "equation" | "align" | "gather" | "cases" | "pmatrix-2x2" | "pmatrix-3x3" | "integral" | "fracao";

export const IDS_AMBIENTES: IdAmbiente[] = ["equation", "align", "gather", "cases", "pmatrix-2x2", "pmatrix-3x3", "integral", "fracao"];

export interface OpcoesAmbiente {
  /** Texto selecionado no editor. Entra no primeiro campo do modelo, e o cursor fica logo depois dele. */
  selecao?: string;
  /** O cursor ja esta dentro de uma formula ($...$, \[...\], equation...). Fora dela, os fragmentos ganham delimitadores. */
  emMatematica?: boolean;
  /** Texto da linha antes do cursor (ou da selecao). Se nao for vazio, o bloco comeca em linha nova. */
  antesNaLinha?: string;
  /** Texto da linha depois do cursor (ou da selecao). Se nao for vazio, o bloco termina em linha nova. */
  depoisNaLinha?: string;
}

export interface TextoInserido {
  texto: string;
  /** Posicao do cursor dentro de `texto` (0 e o inicio do texto inserido). */
  cursor: number;
}

// marca interna de onde o cursor vai (nunca aparece no resultado)
const MARCA = "\u0000";

type Forma =
  // ambiente completo: ja e uma formula, em linhas proprias
  | { tipo: "ambiente"; linhas: string[] }
  // pedaco de formula: fora da matematica vira \( ... \) (uma linha) ou \[ ... \] (varias)
  | { tipo: "fragmento"; linhas: string[] };

const MODELOS: Record<IdAmbiente, Forma> = {
  equation: { tipo: "ambiente", linhas: ["\\begin{equation}", `  ${MARCA}`, "\\end{equation}"] },
  align: { tipo: "ambiente", linhas: ["\\begin{align}", `  ${MARCA} &= \\\\`, "  &=", "\\end{align}"] },
  gather: { tipo: "ambiente", linhas: ["\\begin{gather}", `  ${MARCA} \\\\`, "\\end{gather}"] },
  cases: {
    tipo: "fragmento",
    linhas: ["\\begin{cases}", `  ${MARCA} & \\text{se } \\\\`, "  & \\text{caso contrário}", "\\end{cases}"],
  },
  "pmatrix-2x2": { tipo: "fragmento", linhas: ["\\begin{pmatrix}", `  ${MARCA} & \\\\`, "  &", "\\end{pmatrix}"] },
  "pmatrix-3x3": {
    tipo: "fragmento",
    linhas: ["\\begin{pmatrix}", `  ${MARCA} & & \\\\`, "  & & \\\\", "  & &", "\\end{pmatrix}"],
  },
  integral: { tipo: "fragmento", linhas: [`\\int_{${MARCA}}^{} \\, \\mathrm{d}x`] },
  fracao: { tipo: "fragmento", linhas: [`\\frac{${MARCA}}{}`] },
};

export function ehIdAmbiente(id: string): id is IdAmbiente {
  return (IDS_AMBIENTES as string[]).includes(id);
}

/** Texto e cursor do ambiente `id`. Quem chama confere o `id` com `ehIdAmbiente`. */
export function ambienteMatematica(id: IdAmbiente, opcoes: OpcoesAmbiente = {}): TextoInserido {
  const { selecao = "", emMatematica = false, antesNaLinha = "", depoisNaLinha = "" } = opcoes;
  const forma = MODELOS[id];
  let linhas = forma.linhas;
  let emBloco = forma.tipo === "ambiente";

  if (forma.tipo === "fragmento" && !emMatematica) {
    if (linhas.length === 1) {
      linhas = [`\\(${linhas[0]}\\)`];
    } else {
      linhas = ["\\[", ...linhas.map((l) => (l.trim() ? `  ${l}` : l)), "\\]"];
      emBloco = true;
    }
  }

  let texto = linhas.join("\n");
  const marca = texto.indexOf(MARCA);
  texto = texto.slice(0, marca) + selecao + texto.slice(marca + 1);
  let cursor = marca + selecao.length;

  // blocos em linha propria: quebra antes e depois quando ha texto ao redor
  if (emBloco) {
    if (antesNaLinha.trim()) {
      texto = "\n" + texto;
      cursor += 1;
    }
    if (depoisNaLinha.trim()) texto += "\n";
  }
  return { texto, cursor };
}
