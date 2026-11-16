// Leitura do codigo LaTeX sem compilar: palavras para o corretor ortografico, rotulos
// (\label), usos de \ref e \cite e as entradas dos arquivos .bib.
// Todas as posicoes sao deslocamentos no texto original.

/** Troca os comentarios (de % ate o fim da linha) por espacos, mantendo as posicoes. */
export function semComentarios(texto: string): string {
  let saida = '';
  let i = 0;
  while (i < texto.length) {
    const c = texto[i];
    if (c === '\\') {
      saida += texto.slice(i, i + 2);
      i += 2;
      continue;
    }
    if (c === '%') {
      const fim = texto.indexOf('\n', i);
      const ate = fim < 0 ? texto.length : fim;
      saida += ' '.repeat(ate - i);
      i = ate;
      continue;
    }
    saida += c;
    i++;
  }
  return saida;
}

const AMBIENTES_MATEMATICA = new Set([
  'equation', 'equation*', 'align', 'align*', 'gather', 'gather*', 'multline', 'multline*',
  'eqnarray', 'eqnarray*', 'math', 'displaymath', 'flalign', 'flalign*', 'alignat', 'alignat*',
]);
const AMBIENTES_CODIGO = new Set([
  'verbatim', 'verbatim*', 'Verbatim', 'lstlisting', 'minted', 'comment', 'tikzpicture', 'filecontents',
  'filecontents*',
]);
/** Ambientes cujo primeiro argumento obrigatorio nao e texto (largura, colunas). */
const AMBIENTES_COM_ARGUMENTO: Record<string, number> = {
  tabular: 1, 'tabular*': 2, tabularx: 2, array: 1, longtable: 1, minipage: 1, multicols: 1,
  wrapfigure: 2, subfigure: 1, thebibliography: 1,
};

/** Comandos cujos argumentos nao sao texto: quantos argumentos {} pular. */
const COMANDOS_SEM_TEXTO: Record<string, number> = {
  label: 1, ref: 1, eqref: 1, pageref: 1, autoref: 1, nameref: 1, vref: 1, cref: 1, Cref: 1,
  cpageref: 1, nocite: 1, bibitem: 1, input: 1, include: 1, includeonly: 1, includegraphics: 1,
  includepdf: 1, usepackage: 1, RequirePackage: 1, documentclass: 1, bibliography: 1,
  bibliographystyle: 1, addbibresource: 1, url: 1, href: 1, graphicspath: 1, newcommand: 2,
  renewcommand: 2, providecommand: 2, newenvironment: 3, renewenvironment: 3,
  DeclareMathOperator: 2, setlength: 2, addtolength: 2, setcounter: 2, addtocounter: 2,
  hypersetup: 1, geometry: 1, pagestyle: 1, thispagestyle: 1, pagenumbering: 1, color: 1,
  textcolor: 1, colorbox: 1, definecolor: 3, hyphenation: 1, newtheorem: 2, lstset: 1, tikzset: 1,
  pgfplotsset: 1, numberwithin: 2, selectlanguage: 1, fontsize: 2, vspace: 1, hspace: 1,
  vskip: 0, hskip: 0, rule: 2, resizebox: 2, scalebox: 1, multicolumn: 2, multirow: 2,
  cline: 1, cmidrule: 1, setmainfont: 1, setsansfont: 1, setmonofont: 1, newacronym: 2,
  addcontentsline: 2, counterwithin: 2, setcitestyle: 1, citestyle: 1, bibliographyunit: 1,
  inputminted: 2, lstinputlisting: 1, mintinline: 1, includestandalone: 1, sisetup: 1, SI: 2,
  si: 1, num: 1, ang: 1, qty: 2, unit: 1, ce: 1, pu: 1,
};

const RE_LETRA = /\p{L}/u;
const ehLetra = (c: string | undefined) => c !== undefined && RE_LETRA.test(c);
const ehLetraAscii = (c: string | undefined) => c !== undefined && /[a-zA-Z@]/.test(c);

/** Comandos de citacao: \cite, \citep, \parencite, \citeonline (abnTeX2), \textcite... */
export const ehComandoCitacao = (nome: string) => /cite/i.test(nome) && !/^(?:setcitestyle|citestyle)$/.test(nome);
const COMANDOS_REFERENCIA = new Set([
  'ref', 'eqref', 'pageref', 'autoref', 'nameref', 'vref', 'cref', 'Cref', 'cpageref', 'labelcref',
]);
export const ehComandoReferencia = (nome: string) => COMANDOS_REFERENCIA.has(nome);

/** Fim do grupo que abre em `i` ("{" ou "["), contando o aninhamento. Devolve o indice depois do fechamento. */
function fimDoGrupo(texto: string, i: number): number {
  const abre = texto[i];
  const fecha = abre === '{' ? '}' : ']';
  let nivel = 0;
  for (let j = i; j < texto.length; j++) {
    const c = texto[j];
    if (c === '\\') {
      j++;
      continue;
    }
    if (c === abre) nivel++;
    else if (c === fecha && --nivel === 0) return j + 1;
  }
  return texto.length;
}

/** Pula espacos (sem atravessar linha em branco). */
function pularEspacos(texto: string, i: number): number {
  let quebras = 0;
  while (i < texto.length && /\s/.test(texto[i])) {
    if (texto[i] === '\n' && ++quebras > 1) break;
    i++;
  }
  return i;
}

/** Pula opcionais [..] e `quantos` argumentos {..}. Devolve a nova posicao. */
function pularArgumentos(texto: string, i: number, quantos: number): number {
  let restantes = quantos;
  let j = i;
  while (j < texto.length) {
    const k = pularEspacos(texto, j);
    if (texto[k] === '[') {
      j = fimDoGrupo(texto, k);
      continue;
    }
    if (restantes > 0 && texto[k] === '{') {
      j = fimDoGrupo(texto, k);
      restantes--;
      continue;
    }
    // \newcommand\x{...}: argumento sem chaves
    if (restantes > 0 && texto[k] === '\\') {
      let m = k + 1;
      while (ehLetraAscii(texto[m])) m++;
      j = m === k + 1 ? k + 2 : m;
      restantes--;
      continue;
    }
    break;
  }
  return j;
}

export interface Palavra {
  palavra: string;
  de: number;
  ate: number;
}

/**
 * Palavras de texto corrido que o corretor deve conferir. Ficam de fora os comandos, os
 * argumentos que nao sao texto (\label, \cite, \usepackage...), a matematica, os
 * ambientes de codigo, os comentarios, as siglas e as palavras com letras acentuadas
 * no estilo antigo (\'a).
 */
export function palavrasParaVerificar(original: string): Palavra[] {
  const texto = semComentarios(original);
  const saida: Palavra[] = [];
  let i = 0;
  const n = texto.length;

  const pularAte = (marca: string, de: number) => {
    const k = texto.indexOf(marca, de);
    return k < 0 ? n : k + marca.length;
  };

  while (i < n) {
    const c = texto[i];
    if (c === '\\') {
      const prox = texto[i + 1];
      if (prox === '(') {
        i = pularAte('\\)', i + 2);
        continue;
      }
      if (prox === '[') {
        i = pularAte('\\]', i + 2);
        continue;
      }
      if (!ehLetraAscii(prox)) {
        i += 2;
        continue;
      }
      let j = i + 1;
      while (ehLetraAscii(texto[j])) j++;
      const nome = texto.slice(i + 1, j);
      if (texto[j] === '*') j++;
      if (nome === 'verb') {
        const delim = texto[j];
        i = delim ? pularAte(delim, j + 1) : n;
        continue;
      }
      if (nome === 'begin' || nome === 'end') {
        const k = pularEspacos(texto, j);
        if (texto[k] !== '{') {
          i = j;
          continue;
        }
        const fim = fimDoGrupo(texto, k);
        const ambiente = texto.slice(k + 1, fim - 1).trim();
        if (nome === 'begin' && (AMBIENTES_MATEMATICA.has(ambiente) || AMBIENTES_CODIGO.has(ambiente))) {
          i = pularAte(`\\end{${ambiente}}`, fim);
          continue;
        }
        i = nome === 'begin' ? pularArgumentos(texto, fim, AMBIENTES_COM_ARGUMENTO[ambiente] ?? 0) : fim;
        continue;
      }
      const pular = ehComandoCitacao(nome) ? 1 : COMANDOS_SEM_TEXTO[nome];
      i = pular !== undefined ? pularArgumentos(texto, j, pular) : j;
      continue;
    }
    if (c === '$') {
      if (texto[i + 1] === '$') {
        i = pularAte('$$', i + 2);
        continue;
      }
      let j = i + 1;
      while (j < n && !(texto[j] === '$' && texto[j - 1] !== '\\')) j++;
      i = j + 1;
      continue;
    }
    if (ehLetra(c)) {
      let j = i;
      while (j < n && (ehLetra(texto[j]) || ((texto[j] === '-' || texto[j] === "'" || texto[j] === '’') && ehLetra(texto[j + 1])))) j++;
      const anterior = texto[i - 1];
      // acento no estilo antigo dentro da palavra: Fun\c{c}\~ao
      const acentoDepois = /^\\(?:['`~^"=.]|[cvuHkr]\s*\{)/.test(texto.slice(j, j + 5));
      const colado = acentoDepois || anterior === '}' || anterior === '\\' || (texto[i - 2] === '\\' && /['`~^"=.]/.test(anterior)) || /\d/.test(texto[j] ?? '') || /\d/.test(anterior ?? '');
      if (!colado) {
        // palavra com hifen: cada parte e conferida sozinha
        let inicio = i;
        for (const parte of texto.slice(i, j).split(/[-'’]/)) {
          const de = texto.indexOf(parte, inicio);
          inicio = de + parte.length;
          if (parte.length < 2) continue;
          // siglas e nomes com maiusculas no meio (DNA, TiO2, LaTeX) ficam de fora
          if (/\p{Lu}/u.test(parte.slice(1))) continue;
          saida.push({ palavra: parte, de, ate: de + parte.length });
        }
      }
      i = j;
      continue;
    }
    i++;
  }
  return saida;
}

export interface RegiaoMatematica {
  de: number;
  ate: number;
  /** O codigo da formula, sem os delimitadores. */
  tex: string;
  /** Formula em destaque (linha propria) ou em linha. */
  destaque: boolean;
  /** Ambiente (equation, align...) quando a formula vem de um. */
  ambiente?: string;
}

/** Trechos de matematica do texto: $...$, $$...$$, \(...\), \[...\] e os ambientes de equacao. */
export function regioesMatematica(original: string): RegiaoMatematica[] {
  const texto = semComentarios(original);
  const saida: RegiaoMatematica[] = [];
  const n = texto.length;
  let i = 0;
  const fechar = (de: number, inicioTex: number, marca: string, destaque: boolean, ambiente?: string) => {
    const k = texto.indexOf(marca, inicioTex);
    const fimTex = k < 0 ? n : k;
    const ate = k < 0 ? n : k + marca.length;
    saida.push({ de, ate, tex: texto.slice(inicioTex, fimTex).trim(), destaque, ambiente });
    return ate;
  };
  while (i < n) {
    const c = texto[i];
    if (c === '\\') {
      const prox = texto[i + 1];
      if (prox === '(') {
        i = fechar(i, i + 2, '\\)', false);
        continue;
      }
      if (prox === '[') {
        i = fechar(i, i + 2, '\\]', true);
        continue;
      }
      if (texto.startsWith('\\verb', i) && !ehLetraAscii(texto[i + 5])) {
        const delim = texto[i + 5];
        const k = delim ? texto.indexOf(delim, i + 6) : -1;
        i = k < 0 ? n : k + 1;
        continue;
      }
      if (texto.startsWith('\\begin', i)) {
        const k = pularEspacos(texto, i + 6);
        if (texto[k] === '{') {
          const fim = fimDoGrupo(texto, k);
          const ambiente = texto.slice(k + 1, fim - 1).trim();
          if (AMBIENTES_MATEMATICA.has(ambiente)) {
            i = fechar(i, fim, `\\end{${ambiente}}`, true, ambiente);
            continue;
          }
          if (AMBIENTES_CODIGO.has(ambiente)) {
            const e = texto.indexOf(`\\end{${ambiente}}`, fim);
            i = e < 0 ? n : e;
            continue;
          }
        }
      }
      i += 2;
      continue;
    }
    if (c === '$') {
      if (texto[i + 1] === '$') {
        i = fechar(i, i + 2, '$$', true);
        continue;
      }
      let j = i + 1;
      while (j < n && !(texto[j] === '$' && texto[j - 1] !== '\\')) j++;
      saida.push({ de: i, ate: Math.min(n, j + 1), tex: texto.slice(i + 1, j).trim(), destaque: false });
      i = j + 1;
      continue;
    }
    i++;
  }
  return saida;
}

// ---------------- rotulos, referencias e citacoes ----------------

export interface Rotulo {
  nome: string;
  arquivo: string;
  linha: number;
  de: number;
  ate: number;
  /** figura, tabela, equacao, secao, item... */
  tipo: string;
  /** Legenda da figura ou tabela, titulo da secao ou a propria equacao. */
  descricao: string;
}

export interface Uso {
  /** Nome do comando, sem a barra. */
  comando: string;
  chave: string;
  de: number;
  ate: number;
}

const RE_COMANDO_COM_CHAVES = /\\([a-zA-Z]+)\*?\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}/g;

function linhaDe(texto: string, pos: number): number {
  let linha = 1;
  for (let i = 0; i < pos; i++) if (texto.charCodeAt(i) === 10) linha++;
  return linha;
}

/** Argumento entre chaves que comeca logo depois de `i` (pulando espacos e opcionais). */
function argumentoDepois(texto: string, i: number): string {
  let k = i;
  for (;;) {
    k = pularEspacos(texto, k);
    if (texto[k] === '[') k = fimDoGrupo(texto, k);
    else break;
  }
  if (texto[k] !== '{') return '';
  return texto.slice(k + 1, fimDoGrupo(texto, k) - 1).trim();
}

const TIPO_DO_AMBIENTE: Record<string, string> = {
  figure: 'figura', 'figure*': 'figura', wrapfigure: 'figura', subfigure: 'figura',
  table: 'tabela', 'table*': 'tabela', longtable: 'tabela',
  equation: 'equacao', align: 'equacao', gather: 'equacao', multline: 'equacao', eqnarray: 'equacao',
  enumerate: 'item', theorem: 'teorema', lemma: 'lema', definition: 'definicao', lstlisting: 'codigo',
};
const SECOES = /\\(part|chapter|section|subsection|subsubsection|paragraph)\*?\s*(?:\[[^\]]*\])?\s*\{/g;

/** Contador de linhas que so anda para frente: perguntas em posicoes crescentes custam
 *  uma unica passada pelo texto inteiro. */
function contadorDeLinhas(texto: string) {
  let pos = 0;
  let linha = 1;
  return (alvo: number) => {
    for (; pos < alvo; pos++) if (texto.charCodeAt(pos) === 10) linha++;
    return linha;
  };
}

/** Pilha de ambientes que anda para frente: devolve o ambiente aberto mais interno em
 *  `pos` (posicoes crescentes). */
function ambientesAteh(texto: string) {
  const pilha: { nome: string; inicio: number }[] = [];
  const re = /\\(begin|end)\s*\{([^}]+)\}/g;
  let pendente: RegExpExecArray | null = re.exec(texto);
  return (pos: number): { nome: string; inicio: number; fim: number } | null => {
    while (pendente && pendente.index < pos) {
      const nome = pendente[2].trim();
      if (pendente[1] === 'begin') pilha.push({ nome, inicio: pendente.index });
      else {
        let k = pilha.length - 1;
        while (k >= 0 && pilha[k].nome !== nome) k--;
        if (k >= 0) pilha.length = k;
      }
      pendente = re.exec(texto);
    }
    const aberto = pilha.at(-1);
    if (!aberto) return null;
    const fecha = texto.indexOf(`\\end{${aberto.nome}}`, pos);
    return { ...aberto, fim: fecha < 0 ? texto.length : fecha };
  };
}

/** Ultimo titulo de secao antes de `pos` (posicoes crescentes). */
function secoesAteh(texto: string) {
  const re = new RegExp(SECOES.source, 'g');
  let ultima: RegExpExecArray | null = null;
  let pendente: RegExpExecArray | null = re.exec(texto);
  return (pos: number) => {
    while (pendente && pendente.index < pos) {
      ultima = pendente;
      pendente = re.exec(texto);
    }
    return ultima;
  };
}

/** Corta a descricao sem deixar um comando ou um grupo pela metade. */
function cortar(descricao: string, limite = 400): string {
  if (descricao.length <= limite) return descricao;
  let corte = descricao.slice(0, limite);
  const barra = corte.lastIndexOf('\\');
  if (barra > limite - 40) corte = corte.slice(0, barra);
  let abertas = 0;
  for (const c of corte) abertas += c === '{' ? 1 : c === '}' ? -1 : 0;
  return corte + '}'.repeat(Math.max(0, abertas)) + ' …';
}

/** Os \label do texto, com o tipo e uma descricao curta do que foi rotulado.
 *  Uma unica passada: linhas, ambientes e secoes avancam junto com os rotulos. */
export function extrairRotulos(original: string, arquivo: string): Rotulo[] {
  const texto = semComentarios(original);
  const saida: Rotulo[] = [];
  const re = /\\label\s*\{([^}]*)\}/g;
  const linhaEm = contadorDeLinhas(original);
  const ambienteEm = ambientesAteh(texto);
  const secaoAntes = secoesAteh(texto);
  let m: RegExpExecArray | null;
  while ((m = re.exec(texto))) {
    const nome = m[1].trim();
    if (!nome) continue;
    const de = m.index + m[0].indexOf('{') + 1;
    const ate = de + m[1].length;
    let tipo = 'texto';
    let descricao = '';
    const amb = ambienteEm(m.index);
    const base = amb?.nome.replace(/\*$/, '');
    const ultima = secaoAntes(m.index);
    if (amb && base && TIPO_DO_AMBIENTE[base]) {
      tipo = TIPO_DO_AMBIENTE[base];
      const miolo = texto.slice(amb.inicio, amb.fim);
      if (tipo === 'figura' || tipo === 'tabela') {
        const k = miolo.indexOf('\\caption');
        if (k >= 0) descricao = argumentoDepois(miolo, k + '\\caption'.length);
      } else if (tipo === 'equacao') {
        descricao = miolo
          .replace(/^\\begin\s*\{[^}]+\}/, '')
          .replace(/\\label\s*\{[^}]*\}/g, '')
          .replace(/\\nonumber|\\notag/g, '')
          .trim();
      }
    } else if (ultima) {
      // rotulo logo depois de \section{...}: e a secao
      const fimTitulo = fimDoGrupo(texto, ultima.index + ultima[0].length - 1);
      if (texto.slice(fimTitulo, m.index).trim() === '') {
        tipo = { part: 'parte', chapter: 'capitulo' }[ultima[1]] ?? 'secao';
        descricao = texto.slice(ultima.index + ultima[0].length, fimTitulo - 1).trim();
      }
    }
    saida.push({ nome, arquivo, linha: linhaEm(m.index), de, ate, tipo, descricao: cortar(descricao) });
  }
  return saida;
}

/** Usos de \ref (e parentes) e de \cite (e parentes), uma entrada por chave. */
export function extrairUsos(original: string): { referencias: Uso[]; citacoes: Uso[] } {
  const texto = semComentarios(original);
  const referencias: Uso[] = [];
  const citacoes: Uso[] = [];
  RE_COMANDO_COM_CHAVES.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = RE_COMANDO_COM_CHAVES.exec(texto))) {
    const comando = m[1];
    const lista = ehComandoReferencia(comando) ? referencias : ehComandoCitacao(comando) ? citacoes : null;
    if (!lista) continue;
    const inicioArg = m.index + m[0].lastIndexOf('{') + 1;
    let pos = inicioArg;
    for (const parte of m[2].split(',')) {
      const chave = parte.trim();
      const de = pos + parte.indexOf(chave);
      pos += parte.length + 1;
      if (!chave || chave === '*') continue;
      lista.push({ comando, chave, de, ate: de + chave.length });
    }
  }
  return { referencias, citacoes };
}

// ---------------- bibliografia ----------------

export interface EntradaBib {
  chave: string;
  tipo: string;
  arquivo: string;
  linha: number;
  autor: string;
  titulo: string;
  ano: string;
  onde: string;
}

/** Tira chaves e comandos simples de um campo do .bib: "{\'E}gua {Tex}" vira "Égua Tex". */
export function limparCampo(valor: string): string {
  const acentos: Record<string, Record<string, string>> = {
    "'": { a: 'á', e: 'é', i: 'í', o: 'ó', u: 'ú', A: 'Á', E: 'É', I: 'Í', O: 'Ó', U: 'Ú' },
    '`': { a: 'à', A: 'À' },
    '~': { a: 'ã', o: 'õ', n: 'ñ', A: 'Ã', O: 'Õ' },
    '^': { a: 'â', e: 'ê', o: 'ô', A: 'Â', E: 'Ê', O: 'Ô' },
    '"': { u: 'ü', o: 'ö', a: 'ä', U: 'Ü', O: 'Ö', A: 'Ä' },
  };
  return valor
    .replace(/\\c\s*\{?\s*([cC])\s*\}?/g, (_, l: string) => (l === 'c' ? 'ç' : 'Ç'))
    .replace(/\\(['`~^"])\s*\{?\s*\\?([a-zA-Z])\s*\}?/g, (todo, a: string, l: string) => acentos[a]?.[l] ?? todo)
    .replace(/\\[a-zA-Z]+\s*/g, '')
    .replace(/[{}]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Entradas de um arquivo .bib (as @string, @preamble e @comment ficam de fora). */
export function extrairBib(texto: string, arquivo: string): EntradaBib[] {
  const saida: EntradaBib[] = [];
  const re = /@\s*([a-zA-Z]+)\s*([{(])\s*([^,\s]+)\s*,/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(texto))) {
    const tipo = m[1].toLowerCase();
    if (tipo === 'string' || tipo === 'preamble' || tipo === 'comment') continue;
    const inicio = m.index + m[0].indexOf(m[2]);
    const fim = m[2] === '{' ? fimDoGrupo(texto, inicio) : texto.indexOf(')\n', inicio) + 1 || texto.length;
    const corpo = texto.slice(m.index + m[0].length, fim - 1);
    const campos: Record<string, string> = {};
    const reCampo = /([a-zA-Z-]+)\s*=\s*/g;
    let c: RegExpExecArray | null;
    while ((c = reCampo.exec(corpo))) {
      const k = c.index + c[0].length;
      let valor: string;
      let depois: number;
      if (corpo[k] === '{') {
        depois = fimDoGrupo(corpo, k);
        valor = corpo.slice(k + 1, depois - 1);
      } else if (corpo[k] === '"') {
        let j = k + 1;
        while (j < corpo.length && !(corpo[j] === '"' && corpo[j - 1] !== '\\')) j++;
        valor = corpo.slice(k + 1, j);
        depois = j + 1;
      } else {
        const fimValor = corpo.slice(k).search(/[,\n}]/);
        depois = fimValor < 0 ? corpo.length : k + fimValor;
        valor = corpo.slice(k, depois);
      }
      campos[c[1].toLowerCase()] = limparCampo(valor);
      reCampo.lastIndex = depois;
    }
    saida.push({
      chave: m[3],
      tipo,
      arquivo,
      linha: linhaDe(texto, m.index),
      autor: campos.author ?? campos.editor ?? '',
      titulo: campos.title ?? '',
      ano: campos.year ?? (campos.date ?? '').slice(0, 4),
      onde: campos.journal ?? campos.journaltitle ?? campos.booktitle ?? campos.publisher ?? campos.school ?? campos.institution ?? '',
    });
    re.lastIndex = fim;
  }
  return saida;
}

/** Itens de \begin{thebibliography} no proprio .tex (\bibitem{chave}). */
export function extrairBibitems(original: string, arquivo: string): EntradaBib[] {
  const texto = semComentarios(original);
  const saida: EntradaBib[] = [];
  const re = /\\bibitem\s*(?:\[[^\]]*\])?\s*\{([^}]+)\}([^\n]*)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(texto))) {
    saida.push({
      chave: m[1].trim(),
      tipo: 'bibitem',
      arquivo,
      linha: linhaDe(original, m.index),
      autor: '',
      titulo: limparCampo(m[2]).slice(0, 200),
      ano: '',
      onde: '',
    });
  }
  return saida;
}
