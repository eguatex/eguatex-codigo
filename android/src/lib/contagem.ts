// Contagem de palavras de um texto LaTeX, aproximando o texcount (que o Overleaf usa).
//
// Regras principais:
// - comentarios (% ate o fim da linha, mas nao \%) sao ignorados;
// - se existir \begin{document}, so conta o que esta entre ele e \end{document};
// - matematica ($...$, \( \), $$...$$, \[ \] e ambientes equation, align, gather, multline)
//   so e contada como equacao e nao entra nas palavras;
// - nomes de comandos (\algo) saem, mas o texto dos argumentos fica (\textbf{x} conta "x");
// - argumentos de \label, \ref, \cite, \usepackage, \includegraphics, \begin e \end nao contam;
// - palavras de \part ... \paragraph vao para os titulos; as de \caption e \footnote, para "fora".

export interface ResultadoContagem {
  palavrasTexto: number;
  palavrasTitulos: number;
  palavrasFora: number;
  titulos: number;
  flutuantes: number;
  matematicaLinha: number;
  matematicaDestaque: number;
}

type Contexto = 'texto' | 'titulo' | 'fora';

// Resultado de tratar um trecho que comeca em uma barra invertida ou em um cifrao:
// `prox` e onde a varredura continua e `texto` e o que fica no lugar (separador, letra etc.).
interface Passo {
  prox: number;
  texto: string;
}

const COMANDOS_TITULO = new Set([
  'part',
  'chapter',
  'section',
  'subsection',
  'subsubsection',
  'paragraph',
  'subparagraph',
]);

const COMANDOS_FORA = new Set(['caption', 'footnote', 'footnotetext', 'endnote']);

const AMBIENTES_FLUTUANTES = new Set(['figure', 'table']);

const AMBIENTES_MATEMATICA = new Set([
  'equation',
  'align',
  'gather',
  'multline',
  'displaymath',
  'eqnarray',
  'flalign',
  'alignat',
]);

// Ambientes cujo conteudo nao e texto corrido (codigo, desenhos, bibliografia).
const AMBIENTES_IGNORADOS = new Set([
  'verbatim',
  'lstlisting',
  'minted',
  'comment',
  'tikzpicture',
  'thebibliography',
  'filecontents',
]);

// Argumentos obrigatorios de formato que vem logo depois de \begin{ambiente}.
const ARGUMENTOS_AMBIENTE = new Map<string, number>([
  ['tabular', 1],
  ['tabular*', 2],
  ['tabularx', 2],
  ['tabulary', 2],
  ['longtable', 1],
  ['minipage', 1],
  ['multicols', 1],
  ['multicols*', 1],
  ['wrapfigure', 2],
  ['wraptable', 2],
]);

// Comandos cujos primeiros N argumentos (entre chaves) nao sao texto. O que vem depois
// continua sendo contado (por exemplo, o texto de \href{endereco}{texto}).
const ARGUMENTOS_COMANDO = new Map<string, number>([
  // referencias e citacoes
  ['label', 1],
  ['ref', 1],
  ['eqref', 1],
  ['pageref', 1],
  ['autoref', 1],
  ['cref', 1],
  ['Cref', 1],
  ['vref', 1],
  ['nameref', 1],
  ['cite', 1],
  ['citep', 1],
  ['citet', 1],
  ['citealp', 1],
  ['citealt', 1],
  ['citeauthor', 1],
  ['citeyear', 1],
  ['nocite', 1],
  ['parencite', 1],
  ['textcite', 1],
  ['autocite', 1],
  ['footcite', 1],
  // preambulo, arquivos e bibliografia
  ['documentclass', 1],
  ['usepackage', 1],
  ['RequirePackage', 1],
  ['includegraphics', 1],
  ['input', 1],
  ['include', 1],
  ['includeonly', 1],
  ['subfile', 1],
  ['bibliography', 1],
  ['bibliographystyle', 1],
  ['addbibresource', 1],
  ['graphicspath', 1],
  ['url', 1],
  ['index', 1],
  // definicoes, espacamento, estilo e cores
  ['newcommand', 2],
  ['renewcommand', 2],
  ['providecommand', 2],
  ['pagestyle', 1],
  ['thispagestyle', 1],
  ['pagenumbering', 1],
  ['vspace', 1],
  ['hspace', 1],
  ['setlength', 2],
  ['addtolength', 2],
  ['setcounter', 2],
  ['addtocounter', 2],
  ['color', 1],
  ['textcolor', 1],
  ['colorbox', 1],
  ['definecolor', 3],
  ['rule', 2],
  ['cline', 1],
  // tabelas e caixas: pula os argumentos de formato e mantem o texto
  ['multicolumn', 2],
  ['multirow', 2],
  ['parbox', 1],
  ['resizebox', 2],
  ['scalebox', 1],
  ['rotatebox', 1],
  // links e notas: so pula o argumento opcional (ou o endereco)
  ['href', 1],
  ['hyperref', 0],
  ['footnotemark', 0],
]);

// Acentos escritos como comando (\c{c}, \v{s}...): a letra seguinte continua a mesma palavra.
const COMANDOS_ACENTO = new Set(['c', 'v', 'u', 'H', 'k', 'r', 'd', 'b', 't']);

// Letras escritas como comando (\ss, \o, \i...): contam como uma letra da palavra.
const COMANDOS_LETRA = new Set(['ss', 'ae', 'AE', 'oe', 'OE', 'aa', 'AA', 'o', 'O', 'l', 'L', 'i', 'j']);

// Palavra: letras (com acentos) e numeros. Hifen, apostrofo e sublinhado entre letras ou
// numeros mantem uma palavra so (guarda-chuva); ponto e virgula so entre numeros (1.234,56).
const PALAVRA = /[\p{L}\p{N}]+(?:[-'’_][\p{L}\p{N}]+|(?<=\p{N})[.,]\p{N}+)*/gu;

const NOME_COMANDO = /[A-Za-z]+/y;

function zerar(): ResultadoContagem {
  return {
    palavrasTexto: 0,
    palavrasTitulos: 0,
    palavrasFora: 0,
    titulos: 0,
    flutuantes: 0,
    matematicaLinha: 0,
    matematicaDestaque: 0,
  };
}

// O % comeca um comentario quando nao esta escapado (um numero par de barras antes dele).
// O comentario leva junto a quebra de linha e o recuo da linha seguinte, como no TeX.
function removerComentarios(texto: string): string {
  return texto.replace(/((?:^|[^\\])(?:\\\\)*)%[^\n]*(?:\n[ \t]*)?/gm, '$1');
}

// Fica so o que esta entre \begin{document} e \end{document}, quando existirem.
function extrairCorpo(texto: string): string {
  const inicio = /\\begin\s*\{document\}/.exec(texto);
  const de = inicio ? inicio.index + inicio[0].length : 0;
  const fimDoc = /\\end\s*\{document\}/g;
  fimDoc.lastIndex = de;
  const fim = fimDoc.exec(texto);
  return texto.slice(de, fim ? fim.index : texto.length);
}

// Verdadeiro se em `j` ha uma quebra de linha seguida de uma linha em branco (fim de paragrafo).
function quebraParagrafo(f: string, j: number): boolean {
  if (f[j] !== '\n') return false;
  let k = j + 1;
  while (f[k] === ' ' || f[k] === '\t') k++;
  return f[k] === '\n';
}

// Pula espacos e, no maximo, uma quebra de linha (sem atravessar uma linha em branco).
function pularEspacos(f: string, de: number): number {
  let j = de;
  while (f[j] === ' ' || f[j] === '\t') j++;
  if (f[j] === '\n' && !quebraParagrafo(f, j)) {
    j++;
    while (f[j] === ' ' || f[j] === '\t') j++;
  }
  return j;
}

// Posicao logo depois do } que fecha o grupo aberto em `de`; -1 se o grupo nao fechar.
function fimDoGrupo(f: string, de: number): number {
  let profundidade = 0;
  for (let j = de; j < f.length; j++) {
    const c = f[j];
    if (c === '\\') j++;
    else if (c === '{') profundidade++;
    else if (c === '}' && --profundidade === 0) return j + 1;
  }
  return -1;
}

// Posicao logo depois do ] que fecha o argumento opcional aberto em `de`; -1 se nao fechar
// no mesmo paragrafo (entao o [ e tratado como texto comum).
function fimDoOpcional(f: string, de: number): number {
  let chaves = 0;
  for (let j = de + 1; j < f.length; j++) {
    const c = f[j];
    if (c === '\\') j++;
    else if (c === '{') chaves++;
    else if (c === '}') chaves--;
    else if (c === ']' && chaves <= 0) return j + 1;
    else if (quebraParagrafo(f, j)) return -1;
  }
  return -1;
}

// Acha `delimitador` (fora de barras de escape) sem atravessar um paragrafo; -1 se nao achar.
function acharFecho(f: string, de: number, delimitador: string): number {
  const comBarra = delimitador[0] === '\\';
  for (let j = de; j < f.length; j++) {
    if (f[j] === '\\') {
      if (comBarra && f.startsWith(delimitador, j)) return j;
      j++;
    } else if (quebraParagrafo(f, j)) {
      return -1;
    } else if (!comBarra && f.startsWith(delimitador, j)) {
      return j;
    }
  }
  return -1;
}

// Pula os argumentos opcionais e `obrigatorios` argumentos entre chaves depois de um comando.
// Com `obrigatorios` igual a 0, pula so um argumento opcional logo na frente.
function pularArgumentos(f: string, de: number, obrigatorios: number): number {
  let i = de;
  let restantes = obrigatorios;
  for (;;) {
    const j = pularEspacos(f, i);
    const aceitaOpcional = restantes > 0 || (obrigatorios === 0 && i === de);
    if (aceitaOpcional && f[j] === '[') {
      const fim = fimDoOpcional(f, j);
      if (fim < 0) break;
      i = fim;
    } else if (restantes > 0 && f[j] === '{') {
      const fim = fimDoGrupo(f, j);
      if (fim < 0) break;
      i = fim;
      restantes--;
    } else {
      break;
    }
  }
  return i;
}

// Le o argumento obrigatorio (entre chaves) de um comando, pulando os opcionais.
function lerArgumento(f: string, de: number): { ini: number; fim: number; apos: number } | null {
  let i = de;
  for (;;) {
    const j = pularEspacos(f, i);
    if (f[j] === '[') {
      const fim = fimDoOpcional(f, j);
      if (fim < 0) return null;
      i = fim;
    } else if (f[j] === '{') {
      const fim = fimDoGrupo(f, j);
      if (fim < 0) return null;
      return { ini: j + 1, fim: fim - 1, apos: fim };
    } else {
      return null;
    }
  }
}

function somarPalavras(texto: string, contexto: Contexto, r: ResultadoContagem): void {
  const quantidade = texto.match(PALAVRA)?.length ?? 0;
  if (contexto === 'texto') r.palavrasTexto += quantidade;
  else if (contexto === 'titulo') r.palavrasTitulos += quantidade;
  else r.palavrasFora += quantidade;
}

// $...$ e $$...$$. Se nao fechar no mesmo paragrafo, o cifrao e tratado como texto comum.
function tratarDolar(f: string, p: number, r: ResultadoContagem): Passo {
  const duplo = f[p + 1] === '$';
  const abre = duplo ? 2 : 1;
  const fecho = acharFecho(f, p + abre, duplo ? '$$' : '$');
  if (fecho < 0) return { prox: p + abre, texto: ' ' };
  if (duplo) r.matematicaDestaque++;
  else r.matematicaLinha++;
  return { prox: fecho + abre, texto: ' ' };
}

// Barra invertida seguida de algo que nao e letra: \( \[ \\ \% \' etc.
function tratarSimbolo(f: string, p: number, r: ResultadoContagem): Passo {
  const simbolo: string | undefined = f[p + 1];
  switch (simbolo) {
    case undefined:
      return { prox: p + 1, texto: '' };
    case '(': {
      const fecho = acharFecho(f, p + 2, '\\)');
      if (fecho < 0) return { prox: p + 2, texto: ' ' };
      r.matematicaLinha++;
      return { prox: fecho + 2, texto: ' ' };
    }
    case '[': {
      const fecho = acharFecho(f, p + 2, '\\]');
      if (fecho < 0) return { prox: p + 2, texto: ' ' };
      r.matematicaDestaque++;
      return { prox: fecho + 2, texto: ' ' };
    }
    case '\\': {
      // quebra de linha, com \\* e \\[2mm] opcionais
      const k = f[p + 2] === '*' ? p + 3 : p + 2;
      return { prox: pularArgumentos(f, k, 0), texto: ' ' };
    }
    case '_':
      return { prox: p + 2, texto: '_' }; // nome\_do\_arquivo continua sendo uma palavra
    case '-':
    case '/':
    case '@':
    case "'":
    case '`':
    case '^':
    case '"':
    case '~':
    case '=':
    case '.':
      // hifen opcional, correcoes de espaco e acentos: a palavra continua
      return { prox: p + 2, texto: '' };
    default:
      // \% \& \# \$ \{ \} \, \; \: \! e o espaco de controle separam palavras
      return { prox: p + 2, texto: ' ' };
  }
}

// \begin{ambiente}: conta flutuantes e equacoes, pula codigo e argumentos de formato.
function tratarInicioAmbiente(f: string, fimNome: number, r: ResultadoContagem): Passo {
  const j = pularEspacos(f, fimNome);
  if (f[j] !== '{') return { prox: fimNome, texto: ' ' };
  const aposNome = fimDoGrupo(f, j);
  if (aposNome < 0) return { prox: fimNome, texto: ' ' };

  const nome = f.slice(j + 1, aposNome - 1).trim();
  const base = nome.endsWith('*') ? nome.slice(0, -1) : nome;
  const matematica = AMBIENTES_MATEMATICA.has(base);

  if (matematica || AMBIENTES_IGNORADOS.has(base)) {
    const fechamento = `\\end{${nome}}`;
    const onde = f.indexOf(fechamento, aposNome);
    if (onde >= 0) {
      if (matematica) r.matematicaDestaque++;
      return { prox: onde + fechamento.length, texto: ' ' };
    }
  }

  if (AMBIENTES_FLUTUANTES.has(base)) r.flutuantes++;
  const obrigatorios = ARGUMENTOS_AMBIENTE.get(nome) ?? ARGUMENTOS_AMBIENTE.get(base) ?? 0;
  return { prox: pularArgumentos(f, aposNome, obrigatorios), texto: ' ' };
}

// Barra invertida seguida de letras: um comando.
function tratarComando(f: string, p: number, r: ResultadoContagem): Passo {
  NOME_COMANDO.lastIndex = p + 1;
  const achado = NOME_COMANDO.exec(f);
  if (!achado) return { prox: p + 1, texto: '' };

  const nome = achado[0];
  const fimNome = p + 1 + nome.length;
  const aposEstrela = f[fimNome] === '*' ? fimNome + 1 : fimNome;

  if (nome === 'begin') return tratarInicioAmbiente(f, fimNome, r);
  if (nome === 'end') return { prox: pularArgumentos(f, fimNome, 1), texto: ' ' };

  if (nome === 'verb') {
    // \verb|codigo|: o delimitador e o primeiro caractere depois do comando
    const delimitador = f[aposEstrela];
    if (delimitador !== undefined && !/[\sA-Za-z*]/.test(delimitador)) {
      const fecho = f.indexOf(delimitador, aposEstrela + 1);
      const fimLinha = f.indexOf('\n', aposEstrela + 1);
      if (fecho >= 0 && (fimLinha < 0 || fecho < fimLinha)) return { prox: fecho + 1, texto: ' ' };
    }
    return { prox: fimNome, texto: ' ' };
  }

  if (COMANDOS_TITULO.has(nome) || COMANDOS_FORA.has(nome)) {
    const argumento = lerArgumento(f, aposEstrela);
    if (!argumento) return { prox: aposEstrela, texto: ' ' };
    const interno = f.slice(argumento.ini, argumento.fim);
    if (COMANDOS_TITULO.has(nome)) {
      r.titulos++;
      varrer(interno, 'titulo', r);
    } else {
      varrer(interno, 'fora', r);
    }
    return { prox: argumento.apos, texto: ' ' };
  }

  const ignorados = ARGUMENTOS_COMANDO.get(nome);
  if (ignorados !== undefined) {
    return { prox: pularArgumentos(f, aposEstrela, ignorados), texto: ' ' };
  }

  if (COMANDOS_ACENTO.has(nome)) return { prox: pularEspacos(f, fimNome), texto: '' };
  if (COMANDOS_LETRA.has(nome)) return { prox: pularEspacos(f, fimNome), texto: 'a' };

  // qualquer outro comando: o nome sai e o texto dos argumentos fica
  return { prox: fimNome, texto: ' ' };
}

// Varre um trecho, trata comandos e matematica, e conta as palavras do que sobrar no `contexto`.
function varrer(f: string, contexto: Contexto, r: ResultadoContagem): void {
  const especiais = /[\\${}]/g;
  let texto = '';
  let i = 0;
  for (;;) {
    especiais.lastIndex = i;
    const achado = especiais.exec(f);
    if (!achado) break;
    const p = achado.index;
    texto += f.slice(i, p);

    const c = f[p];
    if (c === '\\') {
      const passo = /[A-Za-z]/.test(f[p + 1] ?? '') ? tratarComando(f, p, r) : tratarSimbolo(f, p, r);
      texto += passo.texto;
      i = passo.prox;
    } else if (c === '$') {
      const passo = tratarDolar(f, p, r);
      texto += passo.texto;
      i = passo.prox;
    } else {
      i = p + 1; // chaves de agrupamento nao aparecem no texto
    }
  }
  texto += f.slice(i);
  somarPalavras(texto, contexto, r);
}

export function contarPalavras(texto: string): ResultadoContagem {
  const resultado = zerar();
  const limpo = removerComentarios(texto.normalize('NFC').replace(/\r\n?/g, '\n'));
  varrer(extrairCorpo(limpo), 'texto', resultado);
  return resultado;
}
