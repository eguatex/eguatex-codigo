// Modo Revisando: cada edicao vira uma sugestao que pode ser aceita ou rejeitada.
// O texto do arquivo fica com a sugestao aplicada (o PDF mostra a versao sugerida).
// O trecho apagado nao fica no arquivo: ele e guardado aqui e aparece riscado.
import { Annotation, Facet, StateEffect, StateField, type EditorState, type Extension, type Range } from "@codemirror/state";
import { Decoration, EditorView, WidgetType, showTooltip, type Tooltip } from "@codemirror/view";
import { invertedEffects } from "@codemirror/commands";

export interface Revisao {
  id: string;
  tipo: "ins" | "del";
  /** ins: inicio do texto inserido; del: posicao onde estava o texto apagado. */
  de: number;
  /** ins: fim do texto inserido; del: igual a `de`. */
  ate: number;
  /** Texto apagado (so em del). */
  texto: string;
  data: number;
}

/** Liga o modo Revisando. */
export const revisando = Facet.define<boolean, boolean>({ combine: (v) => v.some(Boolean) });

/** Troca a lista inteira (abrir arquivo, desfazer). */
export const definirRevisoes = StateEffect.define<Revisao[]>();
const removerRevisoes = StateEffect.define<string[]>();
/** Marca as transacoes que aceitam ou rejeitam sugestoes: elas nao viram sugestao. */
const deRevisao = Annotation.define<boolean>();

let contador = 0;
const novoId = () => `${Date.now().toString(36)}-${(contador++).toString(36)}`;

function registrar(lista: Revisao[], tr: import("@codemirror/state").Transaction): Revisao[] {
  const antigas = tr.startState.field(campoRevisoes);
  const novas: Revisao[] = [];
  tr.changes.iterChanges((deA, ateA, deB, ateB) => {
    if (ateA > deA) {
      // o que ja era insercao sugerida simplesmente some; o resto vira exclusao
      let texto = "";
      let i = deA;
      const insercoes = antigas.filter((r) => r.tipo === "ins" && r.de < ateA && r.ate > deA).sort((a, b) => a.de - b.de);
      for (const r of insercoes) {
        if (r.de > i) texto += tr.startState.sliceDoc(i, r.de);
        i = Math.max(i, r.ate);
      }
      if (i < ateA) texto += tr.startState.sliceDoc(i, ateA);
      if (texto) {
        // junta com uma exclusao que ja estava no mesmo lugar (Backspace ou Delete seguidos)
        const vizinha = antigas.find((r) => r.tipo === "del" && r.de >= deA && r.de <= ateA);
        const atual = vizinha && lista.find((r) => r.id === vizinha.id);
        if (vizinha && atual) {
          atual.texto = ateA <= vizinha.de ? texto + atual.texto : atual.texto + texto;
        } else {
          novas.push({ id: novoId(), tipo: "del", de: deB, ate: deB, texto, data: Date.now() });
        }
      }
    }
    if (ateB > deB) {
      const junto = lista.find((r) => r.tipo === "ins" && (r.ate === deB || r.de === ateB));
      if (junto) {
        junto.de = Math.min(junto.de, deB);
        junto.ate = Math.max(junto.ate, ateB);
      } else {
        novas.push({ id: novoId(), tipo: "ins", de: deB, ate: ateB, texto: "", data: Date.now() });
      }
    }
  });
  return [...lista, ...novas];
}

export const campoRevisoes = StateField.define<Revisao[]>({
  create: () => [],
  update(lista, tr) {
    for (const e of tr.effects) if (e.is(definirRevisoes)) return e.value;
    let atual = lista;
    if (tr.docChanged) {
      atual = lista
        .map((r) =>
          r.tipo === "ins"
            ? { ...r, de: tr.changes.mapPos(r.de, 1), ate: tr.changes.mapPos(r.ate, -1) }
            : { ...r, de: tr.changes.mapPos(r.de, -1), ate: tr.changes.mapPos(r.de, -1) },
        )
        .filter((r) => r.tipo === "del" || r.ate > r.de);
      const doUsuario = tr.isUserEvent("input") || tr.isUserEvent("delete") || tr.isUserEvent("move");
      if (tr.startState.facet(revisando) && doUsuario && !tr.annotation(deRevisao)) {
        atual = registrar(atual, tr);
      }
    }
    for (const e of tr.effects) if (e.is(removerRevisoes)) atual = atual.filter((r) => !e.value.includes(r.id));
    return atual;
  },
});

// ---------- aparencia ----------

class Apagado extends WidgetType {
  constructor(readonly texto: string) {
    super();
  }
  eq(o: Apagado) {
    return o.texto === this.texto;
  }
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-rev-del";
    el.textContent = this.texto;
    el.title = "Trecho apagado (sugestão)";
    return el;
  }
}

const decoracoes = EditorView.decorations.compute([campoRevisoes], (state) => {
  const d: Range<Decoration>[] = [];
  for (const r of state.field(campoRevisoes)) {
    if (r.tipo === "ins") d.push(Decoration.mark({ class: "cm-rev-ins" }).range(r.de, r.ate));
    else d.push(Decoration.widget({ widget: new Apagado(r.texto), side: -1 }).range(r.de));
  }
  return Decoration.set(d, true);
});

// ---------- aceitar e rejeitar ----------

function desfazerSugestoes(state: EditorState, alvo: Revisao[]) {
  const changes = alvo.map((r) =>
    r.tipo === "ins" ? { from: r.de, to: r.ate, insert: "" } : { from: r.de, to: r.de, insert: r.texto },
  );
  return state.update({
    changes,
    effects: removerRevisoes.of(alvo.map((r) => r.id)),
    annotations: deRevisao.of(true),
    userEvent: "revisao",
  });
}

export function aceitar(view: EditorView, ids?: string[]) {
  const lista = view.state.field(campoRevisoes);
  const alvo = ids ?? lista.map((r) => r.id);
  if (alvo.length) view.dispatch({ effects: removerRevisoes.of(alvo), annotations: deRevisao.of(true) });
}

export function rejeitar(view: EditorView, ids?: string[]) {
  const lista = view.state.field(campoRevisoes);
  const alvo = ids ? lista.filter((r) => ids.includes(r.id)) : lista;
  if (alvo.length) view.dispatch(desfazerSugestoes(view.state, alvo));
}

function cartao(view: EditorView, r: Revisao): HTMLElement {
  const el = document.createElement("div");
  el.className = "cm-rev-cartao";
  const titulo = document.createElement("span");
  const hora = new Date(r.data).toLocaleString("pt-BR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
  titulo.textContent = `${r.tipo === "ins" ? "Inserção" : "Exclusão"} sugerida, ${hora}`;
  el.appendChild(titulo);
  for (const [rotulo, acao, classe] of [
    ["Aceitar", aceitar, "aceitar"],
    ["Rejeitar", rejeitar, "rejeitar"],
  ] as const) {
    const b = document.createElement("button");
    b.textContent = rotulo;
    b.className = classe;
    b.onmousedown = (e) => {
      e.preventDefault();
      acao(view, [r.id]);
    };
    el.appendChild(b);
  }
  return el;
}

const dica = StateField.define<Tooltip | null>({
  create: () => null,
  update(_, tr) {
    const pos = tr.state.selection.main.head;
    const r = tr.state.field(campoRevisoes).find((x) => pos >= x.de && pos <= x.ate);
    if (!r) return null;
    return { pos: r.de, above: true, create: (view) => ({ dom: cartao(view, r) }) };
  },
  provide: (f) => showTooltip.from(f),
});

const tema = EditorView.baseTheme({
  ".cm-rev-ins": {
    background: "rgba(46, 160, 67, 0.22)",
    textDecoration: "underline",
    textDecorationColor: "rgba(46, 160, 67, 0.9)",
  },
  ".cm-rev-del": {
    color: "#c0392b",
    textDecoration: "line-through",
    background: "rgba(192, 57, 43, 0.12)",
  },
  ".cm-rev-cartao": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "4px 6px 4px 10px",
    fontFamily: '"Segoe UI", system-ui, sans-serif',
    fontSize: "12px",
  },
  ".cm-rev-cartao button": {
    border: "none",
    borderRadius: "4px",
    padding: "3px 10px",
    cursor: "pointer",
    font: "inherit",
    fontWeight: "600",
  },
  ".cm-rev-cartao .aceitar": { background: "#2e7d32", color: "#fff" },
  ".cm-rev-cartao .rejeitar": { background: "transparent", color: "inherit", border: "1px solid currentColor" },
});

// o Ctrl+Z tambem desfaz as sugestoes criadas pela edicao
const historico = invertedEffects.of((tr) => {
  const antes = tr.startState.field(campoRevisoes);
  return antes !== tr.state.field(campoRevisoes) ? [definirRevisoes.of(antes)] : [];
});

export const revisoes: Extension = [campoRevisoes, decoracoes, dica, tema, historico];
