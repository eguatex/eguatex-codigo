// Arquivos e fontes que o LaTeX nao achou, lidos das mensagens de erro, para o app oferecer o
// download pelo tlmgr. "physics.sty" e um arquivo; "fonte:Fira Code" e uma fonte pedida ao fontspec.

const ARQUIVO = /File `([A-Za-z0-9._+-]+\.(?:sty|cls|def|cfg|fd|tex|bst|bbx|cbx|lbx|clo|ldf))' not found/;
// o TeX quebra a linha do log em 79 colunas: a mensagem pode vir com quebras no meio
const FONTE = /The\s+font\s+"([^"\n]{1,80})"\s+can\s*not\s+be\s+found/;
const METRICA = /Font\s+\\[^=\s]{1,100}=([A-Za-z0-9._+-]{1,80})(?:\s+at\s+[\d.]+pt)?\s+not\s+loadable:\s+Metric\s+\(TFM\)\s+file\s+not\s+found/;

/** O que falta para esta mensagem de erro: "x.sty", "y.tfm", "fonte:Nome" ou null. */
export function faltandoNaMensagem(mensagem: string): string | null {
  const arquivo = ARQUIVO.exec(mensagem);
  if (arquivo) return arquivo[1];
  const fonte = FONTE.exec(mensagem);
  if (fonte) return 'fonte:' + fonte[1].trim();
  const metrica = METRICA.exec(mensagem);
  if (metrica) return metrica[1] + '.tfm';
  return null;
}

/** Tudo o que falta numa lista de erros, sem repetir, na ordem em que aparece. */
export function faltandoNosErros(erros: ReadonlyArray<{ mensagem: string }>): string[] {
  const saida: string[] = [];
  for (const e of erros) {
    const f = faltandoNaMensagem(e.mensagem);
    if (f && !saida.includes(f)) saida.push(f);
  }
  return saida;
}

/** Nome para mostrar: "fonte:Fira Code" vira "a fonte Fira Code"; arquivos ficam como estao. */
export function nomeDoFaltando(item: string): string {
  return item.startsWith('fonte:') ? item.slice('fonte:'.length) : item;
}
