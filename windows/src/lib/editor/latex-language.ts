import { StreamLanguage, type StreamParser } from "@codemirror/language";

interface EstadoLatex {
  inMath: boolean;
  inComment: boolean;
}

const parserLatex: StreamParser<EstadoLatex> = {
  startState() {
    return { inMath: false, inComment: false };
  },
  languageData: { commentTokens: { line: "%" } },
  token(stream, state) {
    // Símbolo escapado (\%, \$, \\, \{, \&): vem antes do % e do $.
    if (stream.match(/\\[^a-zA-Z@]/)) {
      return "string";
    }

    if (stream.match("%")) {
      stream.skipToEnd();
      return "comment";
    }

    if (stream.match("\\begin{") || stream.match("\\end{")) {
      stream.skipTo("}");
      stream.next();
      return "keyword";
    }

    if (stream.match(/\\[a-zA-Z@]+\*?/)) {
      return "keyword";
    }

    if (stream.match("$$")) {
      state.inMath = !state.inMath;
      return "atom";
    }

    if (stream.match("$")) {
      state.inMath = !state.inMath;
      return "atom";
    }

    if (state.inMath) {
      if (stream.match(/[a-zA-Z]+/)) return "variable";
      if (stream.match(/[0-9]+/)) return "number";
      if (stream.match(/[+\-*/=<>^_{}[\]()]/)) return "operator";
      stream.next();
      return "variable";
    }

    if (stream.match("{") || stream.match("}")) {
      return "bracket";
    }

    if (stream.match("[") || stream.match("]")) {
      return "bracket";
    }

    if (stream.match(/[0-9]+/)) {
      return "number";
    }

    stream.next();
    return null;
  },
};

const latexMode = StreamLanguage.define(parserLatex);

export { latexMode, parserLatex };
