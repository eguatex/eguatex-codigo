import { EditorView } from "@codemirror/view";

export const temaClaro = EditorView.theme({
  "&": {
    backgroundColor: "#FFFFFF",
    color: "#212121",
    fontSize: "14px",
    height: "100%",
    fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
  },
  ".cm-content": {
    caretColor: "#5E1A5B",
    lineHeight: "1.6",
  },
  "&.cm-focused .cm-cursor": {
    borderLeftColor: "#5E1A5B",
    borderLeftWidth: "2px",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
    backgroundColor: "rgba(106, 27, 154, 0.15)",
  },
  ".cm-activeLine": {
    backgroundColor: "rgba(27, 94, 32, 0.06)",
  },
  ".cm-gutters": {
    backgroundColor: "#F5F7FA",
    color: "#9AA4B2",
    borderRight: "1px solid #E3E8EF",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "rgba(27, 94, 32, 0.1)",
    color: "#5E1A5B",
  },
  ".cm-matchingBracket": {
    backgroundColor: "rgba(249, 168, 37, 0.3)",
    outline: "1px solid #F9A825",
  },
  ".cm-searchMatch": {
    backgroundColor: "rgba(249, 168, 37, 0.4)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor: "rgba(2, 119, 189, 0.3)",
  },
});

export const temaEscuro = EditorView.theme({
  "&": {
    backgroundColor: "#1a1a1a",
    color: "#e8e8e8",
    fontSize: "14px",
    fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
  },
  ".cm-content": {
    caretColor: "#C266B9",
    lineHeight: "1.6",
  },
  "&.cm-focused .cm-cursor": {
    borderLeftColor: "#C266B9",
    borderLeftWidth: "2px",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
    backgroundColor: "rgba(142, 36, 170, 0.3)",
  },
  ".cm-activeLine": {
    backgroundColor: "rgba(255, 255, 255, 0.04)",
  },
  ".cm-gutters": {
    backgroundColor: "#1a1a1a",
    color: "#666",
    borderRight: "1px solid #333",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "rgba(168, 71, 159, 0.18)",
    color: "#C266B9",
  },
  ".cm-matchingBracket": {
    backgroundColor: "rgba(249, 168, 37, 0.25)",
    outline: "1px solid #F9A825",
  },
  ".cm-searchMatch": {
    backgroundColor: "rgba(249, 168, 37, 0.3)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor: "rgba(2, 119, 189, 0.4)",
  },
});
