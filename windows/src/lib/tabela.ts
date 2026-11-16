// Gera o codigo LaTeX de uma tabela a partir das opcoes do assistente de tabela.
// Ideia do assistente de tabela do TeXstudio (GPL); codigo escrito para o EguaTex.

export type Alinhamento = 'l' | 'c' | 'r';
export type Linhas = 'nenhuma' | 'simples' | 'grade' | 'booktabs';

export interface OpcoesTabela {
  celulas: string[][];
  alinhamentos: Alinhamento[];
  linhas: Linhas;
  cabecalhoNegrito: boolean;
  /** Dentro de \begin{table} com legenda e rotulo (numerada). */
  flutuante: boolean;
  legenda: string;
  rotulo: string;
}

/** Protege os caracteres especiais do LaTeX (& % # _ ^ ~ e \ solta) fora de trechos
 *  $...$, sem mexer nos ja protegidos. Chaves ficam como estao, para 	extbf{x} funcionar;
 *  matematica entre $ tambem. */
export function protegerCelula(texto: string): string {
  return texto
    .split(/(\$[^$]*\$)/)
    .map((parte, i) => (i % 2 === 1 ? parte : protegerTexto(parte)))
    .join('')
    .trim();
}

function protegerTexto(t: string): string {
  let saida = '';
  for (let i = 0; i < t.length; i++) {
    const c = t[i];
    const proximo = t[i + 1] ?? '';
    if (c === '\\') {
      // comando (\textbf) ou caractere ja protegido (\%) continua igual
      if (/[A-Za-z&%#_{}$~^\\]/.test(proximo)) {
        saida += c + proximo;
        i++;
      } else saida += '\\textbackslash{}';
    } else if ('&%#_$'.includes(c)) saida += '\\' + c;
    else if (c === '~') saida += '\\textasciitilde{}';
    else if (c === '^') saida += '\\textasciicircum{}';
    else saida += c;
  }
  return saida;
}

/** Rotulo do \label: sem espacos nem caracteres que o LaTeX nao aceita. */
function limparRotulo(r: string): string {
  return r.trim().replace(/\s+/g, '-').replace(/[^A-Za-z0-9:._-]/g, '');
}

export function gerarTabela(o: OpcoesTabela): string {
  const colunas = o.alinhamentos.length;
  const grade = o.linhas === 'grade';
  const especificacao = grade ? '|' + o.alinhamentos.join('|') + '|' : o.alinhamentos.join('');
  const recuo = o.flutuante ? '    ' : '  ';
  const regra = (tipo: 'topo' | 'meio' | 'fim'): string | null => {
    if (o.linhas === 'booktabs') return { topo: '\\toprule', meio: '\\midrule', fim: '\\bottomrule' }[tipo];
    if (o.linhas === 'grade' || o.linhas === 'simples') return '\\hline';
    return null;
  };

  const corpo: string[] = [];
  const topo = regra('topo');
  if (topo) corpo.push(recuo + topo);
  o.celulas.forEach((linha, i) => {
    const textos = Array.from({ length: colunas }, (_, c) => {
      const t = protegerCelula(linha[c] ?? '');
      return i === 0 && o.cabecalhoNegrito && t ? `\\textbf{${t}}` : t;
    });
    corpo.push(recuo + textos.join(' & ') + ' \\\\');
    const ultima = i === o.celulas.length - 1;
    if (ultima) return;
    // simples e booktabs: linha so depois do cabecalho; grade: entre todas
    if (grade || (i === 0 && o.linhas !== 'nenhuma')) {
      const meio = regra('meio');
      if (meio) corpo.push(recuo + meio);
    }
  });
  const fim = regra('fim');
  if (fim) corpo.push(recuo + fim);

  const tabular = [
    `${o.flutuante ? '  ' : ''}\\begin{tabular}{${especificacao}}`,
    ...corpo,
    `${o.flutuante ? '  ' : ''}\\end{tabular}`,
  ];
  if (!o.flutuante) return tabular.join('\n');
  const rotulo = limparRotulo(o.rotulo);
  return [
    '\\begin{table}[htbp]',
    '  \\centering',
    `  \\caption{${protegerCelula(o.legenda)}}`,
    ...(rotulo ? [`  \\label{${rotulo.startsWith('tab:') ? rotulo : 'tab:' + rotulo}}`] : []),
    ...tabular,
    '\\end{table}',
  ].join('\n');
}

/** Le um CSV (o que o Google Planilhas exporta): virgula, aspas duplas e "" dentro de
 *  aspas. Linhas totalmente vazias no fim sao descartadas. */
export function lerCsv(texto: string): string[][] {
  const linhas: string[][] = [];
  let linha: string[] = [];
  let campo = '';
  let entreAspas = false;
  for (let i = 0; i < texto.length; i++) {
    const c = texto[i];
    if (entreAspas) {
      if (c === '"' && texto[i + 1] === '"') {
        campo += '"';
        i++;
      } else if (c === '"') entreAspas = false;
      else campo += c;
    } else if (c === '"') entreAspas = true;
    else if (c === ',') {
      linha.push(campo);
      campo = '';
    } else if (c === '\n' || c === '\r') {
      if (c === '\r' && texto[i + 1] === '\n') i++;
      linha.push(campo);
      linhas.push(linha);
      linha = [];
      campo = '';
    } else campo += c;
  }
  if (campo || linha.length) {
    linha.push(campo);
    linhas.push(linha);
  }
  while (linhas.length && linhas.at(-1)!.every((c) => !c.trim())) linhas.pop();
  return linhas;
}
