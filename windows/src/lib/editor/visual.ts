// Modo Visual do editor: o texto continua sendo o LaTeX do arquivo, mas os comandos
// ficam escondidos e o resultado aparece formatado (titulos, negrito, formulas).
// A linha onde esta o cursor volta a mostrar o codigo, para dar para editar.
import { EditorState, StateField, type Range, type Extension } from "@codemirror/state";
import { Decoration, EditorView, WidgetType, type DecorationSet } from "@codemirror/view";
import katex from "katex";
import "katex/dist/katex.min.css";

// ---------- widgets ----------

class Formula extends WidgetType {
  constructor(readonly tex: string, readonly destaque: boolean) {
    super();
  }
  eq(o: Formula) {
    return o.tex === this.tex && o.destaque === this.destaque;
  }
  toDOM() {
    const el = document.createElement(this.destaque ? "div" : "span");
    el.className = this.destaque ? "cm-v-formula cm-v-destaque" : "cm-v-formula";
    katex.render(this.tex, el, { throwOnError: false, displayMode: this.destaque, strict: "ignore" });
    return el;
  }
  ignoreEvent() {
    return false;
  }
}

class Etiqueta extends WidgetType {
  constructor(readonly texto: string, readonly classe: string) {
    super();
  }
  eq(o: Etiqueta) {
    return o.texto === this.texto && o.classe === this.classe;
  }
  toDOM() {
    const el = document.createElement("span");
    el.className = this.classe;
    el.textContent = this.texto;
    return el;
  }
  ignoreEvent() {
    return false;
  }
}

class Preambulo extends WidgetType {
  eq() {
    return true;
  }
  toDOM(view: EditorView) {
    const el = document.createElement("div");
    el.className = "cm-v-preambulo";
    el.textContent = "Preâmbulo (clique para ver o código)";
    el.onmousedown = (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: 0 } });
      view.focus();
    };
    return el;
  }
  ignoreEvent() {
    return true;
  }
}

// ---------- leitura do LaTeX ----------

/** Indice da chave que fecha a chave aberta em `i`, ou -1. */
function fecharChave(t: string, i: number): number {
  let nivel = 0;
  for (let j = i; j < t.length; j++) {
    const c = t[j];
    if (c === "\\") {
      j++;
      continue;
    }
    if (c === "{") nivel++;
    else if (c === "}" && --nivel === 0) return j;
  }
  return -1;
}

const TITULOS: Record<string, number> = {
  chapter: 1,
  section: 1,
  subsection: 2,
  subsubsection: 3,
  paragraph: 4,
  subparagraph: 4,
};

const FORMATOS: Record<string, string> = {
  textbf: "cm-v-negrito",
  textit: "cm-v-italico",
  emph: "cm-v-italico",
  underline: "cm-v-sublinhado",
  texttt: "cm-v-mono",
  textsc: "cm-v-versalete",
};

const AMBIENTES_MAT = /\\begin\{(equation\*?|align\*?|gather\*?|multline\*?|displaymath|eqnarray\*?)\}([\s\S]*?)\\end\{\1\}/g;

function construir(state: EditorState): DecorationSet {
  const t = state.doc.toString();
  const sel = state.selection.main;
  const decos: Range<Decoration>[] = [];
  // trechos ja tratados (formulas e comentarios): nada mais entra neles
  const ocupados: [number, number][] = [];
  const livre = (a: number, b: number) => !ocupados.some(([x, y]) => a < y && b > x);
  const tocaCursor = (a: number, b: number) => sel.from <= b && sel.to >= a;
  const esconder = (a: number, b: number) => {
    if (b > a) decos.push(Decoration.replace({}).range(a, b));
  };

  // comentarios
  for (const m of t.matchAll(/(^|[^\\])(%.*)/gm)) {
    const a = m.index! + m[1].length;
    const b = a + m[2].length;
    ocupados.push([a, b]);
    decos.push(Decoration.mark({ class: "cm-v-comentario" }).range(a, b));
  }

  // preambulo: tudo antes do fim da linha do \begin{document}
  const inicioDoc = t.indexOf("\\begin{document}");
  if (inicioDoc >= 0) {
    const fim = state.doc.lineAt(inicioDoc).to;
    ocupados.push([0, fim]);
    if (!tocaCursor(0, fim)) {
      decos.push(Decoration.replace({ widget: new Preambulo(), block: true }).range(0, fim));
    } else {
      for (let l = 1; l <= state.doc.lineAt(fim).number; l++) {
        decos.push(Decoration.line({ class: "cm-v-linha-preambulo" }).range(state.doc.line(l).from));
      }
    }
  }
  const fimDoc = t.indexOf("\\end{document}");
  if (fimDoc >= 0 && !tocaCursor(fimDoc, fimDoc + 14)) esconder(fimDoc, fimDoc + 14);

  // formulas
  const formula = (a: number, b: number, tex: string, destaque: boolean) => {
    if (!livre(a, b)) return;
    ocupados.push([a, b]);
    if (tocaCursor(a, b)) {
      decos.push(Decoration.mark({ class: "cm-v-codigo-formula" }).range(a, b));
      return;
    }
    const la = state.doc.lineAt(a);
    const lb = state.doc.lineAt(b);
    const bloco = destaque && a === la.from && b === lb.to;
    decos.push(Decoration.replace({ widget: new Formula(tex.trim(), destaque), block: bloco }).range(a, b));
  };
  for (const m of t.matchAll(AMBIENTES_MAT)) {
    const nome = m[1].replace("*", "");
    const tex = nome === "equation" || nome === "displaymath" ? m[2] : `\\begin{${nome}*}${m[2]}\\end{${nome}*}`;
    formula(m.index!, m.index! + m[0].length, tex.replace(/\\label\{[^}]*\}/g, ""), true);
  }
  for (const m of t.matchAll(/\$\$([\s\S]+?)\$\$|\\\[([\s\S]+?)\\\]/g)) {
    formula(m.index!, m.index! + m[0].length, m[1] ?? m[2], true);
  }
  for (const m of t.matchAll(/(?<!\\)\$([^$\n]+?)(?<!\\)\$|\\\(([\s\S]+?)\\\)/g)) {
    formula(m.index!, m.index! + m[0].length, m[1] ?? m[2], false);
  }

  // comandos com um argumento: titulos, formatos, citacoes
  for (const m of t.matchAll(/\\([a-zA-Z]+)(\*?)(\[[^\]]*\])?\{/g)) {
    const a = m.index!;
    const abre = a + m[0].length - 1;
    if (!livre(a, abre + 1)) continue;
    const nome = m[1];
    const nivel = TITULOS[nome];
    const formato = FORMATOS[nome];
    const especial = ["cite", "citep", "citet", "ref", "eqref", "autoref", "label", "includegraphics", "url", "href", "footnote"].includes(nome);
    if (!nivel && !formato && !especial) continue;
    const fecha = fecharChave(t, abre);
    if (fecha < 0) continue;
    const b = fecha + 1;
    const dentro = tocaCursor(a, b);

    if (nivel) {
      const linha = state.doc.lineAt(a);
      decos.push(Decoration.line({ class: `cm-v-titulo cm-v-h${nivel}` }).range(linha.from));
      if (!dentro) {
        esconder(a, abre + 1);
        esconder(fecha, b);
      }
    } else if (formato) {
      // \textbf{} vazio: o CodeMirror recusa marca vazia (RangeError) e a tecla se perde
      if (fecha > abre + 1) decos.push(Decoration.mark({ class: formato }).range(abre + 1, fecha));
      if (!dentro) {
        esconder(a, abre + 1);
        esconder(fecha, b);
      }
    } else if (!dentro && (nome === "footnote" || nome === "href")) {
      // so o colchete/comando some; o texto continua editavel
      if (fecha > abre + 1) decos.push(Decoration.mark({ class: "cm-v-nota" }).range(abre + 1, fecha));
    } else if (!dentro) {
      const arg = t.slice(abre + 1, fecha);
      const texto =
        nome.startsWith("cite") ? `[${arg}]`
        : nome === "label" ? `🏷 ${arg}`
        : nome === "includegraphics" ? `🖼 ${arg}`
        : nome === "url" ? arg
        : `↗ ${arg}`;
      ocupados.push([a, b]);
      decos.push(Decoration.replace({ widget: new Etiqueta(texto, "cm-v-etiqueta") }).range(a, b));
    }
  }

  // listas e ambientes comuns
  for (const m of t.matchAll(/\\item\b(\[[^\]]*\])?\s?/g)) {
    const a = m.index!;
    const b = a + m[0].length;
    if (!livre(a, b) || tocaCursor(a, b)) continue;
    decos.push(Decoration.replace({ widget: new Etiqueta(m[1] ? m[1].slice(1, -1) + " " : "•  ", "cm-v-marcador") }).range(a, b));
  }
  for (const m of t.matchAll(/\\(begin|end)\{(itemize|enumerate|description|center|figure\*?|table\*?|abstract|quote)\}(\[[^\]]*\])?/g)) {
    const a = m.index!;
    if (!livre(a, a + m[0].length)) continue;
    decos.push(Decoration.line({ class: "cm-v-ambiente" }).range(state.doc.lineAt(a).from));
  }
  for (const m of t.matchAll(/\\(maketitle|tableofcontents|newpage|clearpage|centering|noindent)\b/g)) {
    const a = m.index!;
    if (!livre(a, a + m[0].length)) continue;
    decos.push(Decoration.mark({ class: "cm-v-discreto" }).range(a, a + m[0].length));
  }

  return Decoration.set(decos, true);
}

const campoVisual = StateField.define<DecorationSet>({
  create: construir,
  update(valor, tr) {
    return tr.docChanged || tr.selection ? construir(tr.state) : valor;
  },
  provide: (f) => EditorView.decorations.from(f),
});

const temaVisual = EditorView.theme({
  ".cm-content": { fontFamily: '"Georgia", "Times New Roman", serif', fontSize: "1.08em" },
  ".cm-v-titulo": { fontWeight: "700", fontFamily: '"Segoe UI", system-ui, sans-serif' },
  ".cm-v-h1": { fontSize: "1.55em", paddingTop: "0.5em" },
  ".cm-v-h2": { fontSize: "1.3em", paddingTop: "0.35em" },
  ".cm-v-h3": { fontSize: "1.12em" },
  ".cm-v-h4": { fontSize: "1em" },
  ".cm-v-negrito": { fontWeight: "700" },
  ".cm-v-italico": { fontStyle: "italic" },
  ".cm-v-sublinhado": { textDecoration: "underline" },
  ".cm-v-mono": { fontFamily: "Consolas, monospace" },
  ".cm-v-versalete": { fontVariant: "small-caps" },
  ".cm-v-comentario": { opacity: "0.45", fontStyle: "italic" },
  ".cm-v-discreto, .cm-v-ambiente": { opacity: "0.45", fontSize: "0.85em", fontFamily: "Consolas, monospace" },
  ".cm-v-linha-preambulo": { opacity: "0.6", fontFamily: "Consolas, monospace", fontSize: "0.9em" },
  ".cm-v-codigo-formula": { fontFamily: "Consolas, monospace", background: "color-mix(in srgb, var(--primaria) 10%, transparent)" },
  ".cm-v-formula": { padding: "0 2px" },
  ".cm-v-destaque": { display: "block", textAlign: "center", padding: "6px 0" },
  ".cm-v-etiqueta": {
    padding: "0 6px",
    borderRadius: "4px",
    fontSize: "0.85em",
    fontFamily: '"Segoe UI", system-ui, sans-serif',
    background: "color-mix(in srgb, var(--primaria) 14%, transparent)",
  },
  ".cm-v-marcador": { fontWeight: "700", color: "var(--primaria)" },
  ".cm-v-nota": { fontSize: "0.85em", opacity: "0.8" },
  ".cm-v-preambulo": {
    margin: "4px 0 10px",
    padding: "6px 12px",
    borderRadius: "6px",
    cursor: "pointer",
    fontFamily: '"Segoe UI", system-ui, sans-serif',
    fontSize: "0.85em",
    opacity: "0.75",
    border: "1px dashed var(--borda)",
  },
});

/** Extensao do modo Visual (vazia no modo Codigo). */
export function modoVisual(ligado: boolean): Extension {
  return ligado ? [campoVisual, temaVisual] : [];
}
