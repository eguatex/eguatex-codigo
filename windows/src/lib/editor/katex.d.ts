// Tipos minimos do KaTeX usados no modo Visual e nas equacoes do Word.
declare module "katex" {
  interface Opcoes {
    throwOnError?: boolean;
    displayMode?: boolean;
    strict?: string | boolean;
    output?: "html" | "mathml" | "htmlAndMathml";
    macros?: Record<string, string>;
  }
  const katex: {
    render(tex: string, alvo: HTMLElement, opcoes?: Opcoes): void;
    renderToString(tex: string, opcoes?: Opcoes): string;
  };
  export default katex;
}
