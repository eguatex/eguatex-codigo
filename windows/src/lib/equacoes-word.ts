// Equacoes do Word (OMML) a partir do .tex: o PDF so tem letras soltas, entao as equacoes
// destacadas saem do codigo LaTeX. KaTeX passa o LaTeX para MathML e este arquivo passa o
// MathML para OMML, o formato das equacoes editaveis do Word. Serve ao Windows e ao Android.

import katex from 'katex';

export interface EquacaoTex {
  /** OMML de cada equacao, ja dentro de <m:oMath> */
  omml: string;
  /** letras e algarismos da equacao, para achar a mesma equacao no PDF */
  letras: string;
}

// ---------- leitura do .tex ----------

function semComentarios(tex: string): string {
  return tex.replace(/(^|[^\\])%.*$/gm, '$1');
}

/** Grupo entre chaves que comeca em s[i] ('{'); devolve o conteudo e o indice depois dele. */
function grupo(s: string, i: number): [string, number] | null {
  if (s[i] !== '{') return null;
  let nivel = 0;
  for (let k = i; k < s.length; k++) {
    if (s[k] === '\\') {
      k++;
      continue;
    }
    if (s[k] === '{') nivel++;
    else if (s[k] === '}' && --nivel === 0) return [s.slice(i + 1, k), k + 1];
  }
  return null;
}

/** \newcommand, \renewcommand, \def e \DeclareMathOperator do documento, para o KaTeX. */
export function macrosDoTex(tex: string): Record<string, string> {
  const s = semComentarios(tex);
  const macros: Record<string, string> = {};
  const re = /\\(?:re)?newcommand\*?\s*(\{\\[A-Za-z]+\}|\\[A-Za-z]+)\s*(?:\[(\d)\])?\s*(?:\[[^\]]*\])?\s*/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(s))) {
    const g = grupo(s, re.lastIndex);
    if (!g) continue;
    macros[m[1].replace(/[{}]/g, '')] = g[0];
  }
  const def = /\\def\s*(\\[A-Za-z]+)\s*/g;
  while ((m = def.exec(s))) {
    const g = grupo(s, def.lastIndex);
    if (g) macros[m[1]] = g[0];
  }
  const op = /\\DeclareMathOperator(\*?)\s*\{(\\[A-Za-z]+)\}\s*/g;
  while ((m = op.exec(s))) {
    const g = grupo(s, op.lastIndex);
    if (g) macros[m[2]] = `\\operatorname${m[1]}{${g[0]}}`;
  }
  return macros;
}

/** Divide nas linhas do ambiente (\\ fora de chaves e de ambientes internos). */
function linhas(corpo: string): string[] {
  const saida: string[] = [];
  let nivel = 0;
  let amb = 0;
  let ini = 0;
  for (let k = 0; k < corpo.length; k++) {
    const c = corpo[k];
    if (c === '{') nivel++;
    else if (c === '}') nivel--;
    else if (c === '\\') {
      if (corpo.startsWith('\\begin', k)) amb++;
      else if (corpo.startsWith('\\end', k)) amb--;
      else if (corpo[k + 1] === '\\' && nivel === 0 && amb === 0) {
        saida.push(corpo.slice(ini, k));
        k++;
        ini = k + 1;
        continue;
      }
      k++;
    }
  }
  saida.push(corpo.slice(ini));
  return saida.filter((l) => l.replace(/\\(label|tag)\{[^}]*\}|\\(nonumber|notag)/g, '').trim());
}

// \x5C e a barra invertida do LaTeX (escrita assim para o transformador do Vite nao a trocar)
const AMBIENTES =
  /\x5Cbegin\{(equation|align|gather|multline|eqnarray|flalign|alignat|displaymath|math)(\*?)\}(\{\d+\})?([\s\S]*?)\x5Cend\{\1\2\}|\x5C\[([\s\S]*?)\x5C\]|\$\$([\s\S]*?)\$\$/g;

/** As equacoes destacadas do documento, na ordem do texto, prontas para o Word. */
export function equacoesDoTex(tex: string): EquacaoTex[] {
  const s = semComentarios(tex);
  const ini = s.indexOf('\\begin{document}');
  const corpo = ini >= 0 ? s.slice(ini) : s;
  const macros = macrosDoTex(ini >= 0 ? s.slice(0, ini) : '');
  const saida: EquacaoTex[] = [];
  let m: RegExpExecArray | null;
  AMBIENTES.lastIndex = 0;
  while ((m = AMBIENTES.exec(corpo))) {
    const nome = m[1] ?? '';
    let latex = (m[4] ?? m[5] ?? m[6] ?? '').replace(/\\(label|tag\*?)\{[^}]*\}|\\(nonumber|notag)\b/g, '');
    latex = latex.replace(/\\begin\{split\}/g, '\\begin{aligned}').replace(/\\end\{split\}/g, '\\end{aligned}');
    const rs = linhas(latex);
    if (/^(align|flalign|alignat)$/.test(nome)) latex = `\\begin{aligned}${rs.join('\\\\')}\\end{aligned}`;
    else if (/^(gather|multline)$/.test(nome) && rs.length > 1) latex = `\\begin{gathered}${rs.join('\\\\')}\\end{gathered}`;
    else if (nome === 'eqnarray') latex = `\\begin{array}{rcl}${rs.join('\\\\')}\\end{array}`;
    const eq = latexParaOmml(latex, macros);
    if (eq) saida.push(eq);
  }
  return saida;
}

// ---------- MathML (KaTeX) ----------

interface No {
  tag: string;
  attr: Record<string, string>;
  filhos: No[];
  texto: string;
}

function entidades(s: string): string {
  return s
    .replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(+d))
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, '&');
}

/** Leitor de XML minimo para a saida do KaTeX (sempre bem formada). */
function lerXml(xml: string): No {
  const raiz: No = { tag: '#raiz', attr: {}, filhos: [], texto: '' };
  const pilha = [raiz];
  const re = /<(\/?)([A-Za-z][\w:.-]*)([^>]*?)(\/?)>|([^<]+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(xml))) {
    const topo = pilha[pilha.length - 1];
    if (m[5] !== undefined) {
      topo.texto += entidades(m[5]);
      continue;
    }
    if (m[1]) {
      if (pilha.length > 1) pilha.pop();
      continue;
    }
    const attr: Record<string, string> = {};
    m[3].replace(/([\w:-]+)="([^"]*)"/g, (_, k, v) => ((attr[k] = entidades(v)), ''));
    const no: No = { tag: m[2], attr, filhos: [], texto: '' };
    topo.filhos.push(no);
    if (!m[4]) pilha.push(no);
  }
  return raiz;
}

// ---------- MathML para OMML ----------

const escXml = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

const NARIOS = new Set(['∑', '∏', '∐', '∫', '∬', '∭', '∮', '∯', '∰', '⋃', '⋂', '⋁', '⋀', '⨁', '⨂', '⨀', '⨄', '⨆']);
const RELACOES = /^[=<>≤≥≈≠≡∼≃≅∝→←↔⇒⇐⇔∈∉⊂⊃⊆⊇,;]$/;

let tamanhoAtual = 24; // meios pontos

function rPrWord(): string {
  return `<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:sz w:val="${tamanhoAtual}"/><w:szCs w:val="${tamanhoAtual}"/></w:rPr>`;
}

function corrida(texto: string, mPr = ''): string {
  if (!texto) return '';
  return `<m:r>${mPr ? `<m:rPr>${mPr}</m:rPr>` : ''}${rPrWord()}<m:t xml:space="preserve">${escXml(texto)}</m:t></m:r>`;
}

function estilo(no: No): string {
  const v = no.attr.mathvariant;
  const mapa: Record<string, string> = {
    normal: '<m:sty m:val="p"/>',
    bold: '<m:sty m:val="b"/>',
    italic: '<m:sty m:val="i"/>',
    'bold-italic': '<m:sty m:val="bi"/>',
    'double-struck': '<m:scr m:val="double-struck"/><m:sty m:val="p"/>',
    script: '<m:scr m:val="script"/><m:sty m:val="p"/>',
    'bold-script': '<m:scr m:val="script"/><m:sty m:val="b"/>',
    fraktur: '<m:scr m:val="fraktur"/><m:sty m:val="p"/>',
    'sans-serif': '<m:scr m:val="sans-serif"/><m:sty m:val="p"/>',
    monospace: '<m:scr m:val="monospace"/><m:sty m:val="p"/>',
  };
  if (v && mapa[v]) return mapa[v];
  // nome de funcao (sin, log) e texto de varias letras: em pe
  if (no.tag === 'mi' && [...no.texto].length > 1) return '<m:sty m:val="p"/>';
  return '';
}

function espaco(largura: string | undefined): string {
  const em = parseFloat(largura ?? '0');
  if (!(em > 0.05)) return '';
  if (em >= 1.9) return corrida('  ');
  if (em >= 0.9) return corrida(' ');
  if (em >= 0.27) return corrida(' ');
  if (em >= 0.2) return corrida(' ');
  return corrida(' ');
}

function fechaGrupo(no: No): boolean {
  return no.tag === 'mo' && no.attr.fence === 'true';
}

const APLICA = '⁡';
const ehAplica = (no: No | undefined) => !!no && no.tag === 'mo' && no.texto === APLICA;

/** \lim_{x\to 0} e \max_x: o nome da funcao vem dentro do munder, com a aplicacao no fim. */
function terminaEmAplicacao(no: No | undefined): boolean {
  if (!no) return false;
  if (no.tag === 'mrow') return ehAplica(no.filhos[no.filhos.length - 1]);
  if (/^(munder|mover|munderover|msub|msup|msubsup)$/.test(no.tag)) return terminaEmAplicacao(no.filhos[0]);
  return false;
}

/** Converte uma lista de irmaos (conteudo de mrow), juntando operadores grandes ao que segue. */
function sequencia(nos: No[]): string {
  let saida = '';
  for (let i = 0; i < nos.length; i++) {
    const no = nos[i];
    // \sin x, \lim f: funcao do Word, com o nome em pe e o argumento junto
    const nome = ehAplica(nos[i + 1]) ? 1 : terminaEmAplicacao(no) ? 0 : -1;
    if (nome >= 0 && i + 1 + nome < nos.length && !ehAplica(no)) {
      const arg = nos[i + 1 + nome];
      saida += `<m:func><m:funcPr><m:ctrlPr>${rPrWord()}</m:ctrlPr></m:funcPr><m:fName>${conv(no)}</m:fName><m:e>${conv(arg)}</m:e></m:func>`;
      i += 1 + nome;
      continue;
    }
    const base = no.filhos[0];
    const ehNario =
      (['munderover', 'msubsup', 'munder', 'msub', 'msup', 'mover'].includes(no.tag) && base?.tag === 'mo' && NARIOS.has(base.texto.trim())) ||
      (no.tag === 'mo' && NARIOS.has(no.texto.trim()));
    if (ehNario) {
      // o operando vai ate a proxima relacao (=, <, virgula) ou o fim
      const resto: No[] = [];
      while (i + 1 < nos.length && !(nos[i + 1].tag === 'mo' && RELACOES.test(nos[i + 1].texto.trim()))) resto.push(nos[++i]);
      saida += nario(no, sequencia(resto));
      continue;
    }
    saida += conv(no);
  }
  return saida;
}

function nario(no: No, operando: string): string {
  const op = no.tag === 'mo' ? no : no.filhos[0];
  let sub = '';
  let sup = '';
  if (no.tag === 'munderover' || no.tag === 'msubsup') {
    sub = conv(no.filhos[1]);
    sup = conv(no.filhos[2]);
  } else if (no.tag === 'munder' || no.tag === 'msub') sub = conv(no.filhos[1]);
  else if (no.tag === 'mover' || no.tag === 'msup') sup = conv(no.filhos[1]);
  const lim = no.tag.startsWith('mun') || no.tag === 'mover' ? 'undOvr' : 'subSup';
  return (
    `<m:nary><m:naryPr><m:chr m:val="${escXml(op.texto.trim())}"/><m:limLoc m:val="${lim}"/>` +
    (sub ? '' : '<m:subHide m:val="1"/>') +
    (sup ? '' : '<m:supHide m:val="1"/>') +
    `<m:ctrlPr>${rPrWord()}</m:ctrlPr></m:naryPr><m:sub>${sub}</m:sub><m:sup>${sup}</m:sup><m:e>${operando}</m:e></m:nary>`
  );
}

const ACENTOS: Record<string, string> = { '^': '̂', ˆ: '̂', '~': '̃', '˜': '̃', ˉ: '̄', '¯': '̄', '˙': '̇', '¨': '̈', ˇ: '̌', '˘': '̆', '´': '́', '`': '̀', '→': '⃗', '⃗': '⃗' };

function conv(no: No | undefined): string {
  if (!no) return '';
  const f = no.filhos;
  switch (no.tag) {
    case '#raiz':
    case 'math':
    case 'semantics':
      return sequencia(f.filter((x) => x.tag !== 'annotation'));
    case 'mrow':
    case 'mstyle':
    case 'mpadded':
    case 'mtd': {
      // \left( ... \right): delimitadores que crescem com o conteudo
      if (no.tag === 'mrow' && f.length >= 2 && fechaGrupo(f[0]) && fechaGrupo(f[f.length - 1])) {
        const abre = f[0].texto.trim();
        const fecha = f[f.length - 1].texto.trim();
        return (
          `<m:d><m:dPr><m:begChr m:val="${escXml(abre)}"/><m:endChr m:val="${escXml(fecha)}"/><m:ctrlPr>${rPrWord()}</m:ctrlPr></m:dPr>` +
          `<m:e>${sequencia(f.slice(1, -1))}</m:e></m:d>`
        );
      }
      return sequencia(f);
    }
    case 'mphantom':
    case 'annotation':
      return '';
    case 'mi':
    case 'mn':
      return corrida(no.texto, estilo(no));
    case 'mo': {
      const t = no.texto.replace(/⁡/g, '');
      return corrida(t, '<m:sty m:val="p"/>');
    }
    case 'mtext':
      return corrida(no.texto.replace(/\s/g, ' '), '<m:sty m:val="p"/><m:nor/>');
    case 'mspace':
      return espaco(no.attr.width);
    case 'msup':
      return `<m:sSup><m:e>${conv(f[0])}</m:e><m:sup>${conv(f[1])}</m:sup></m:sSup>`;
    case 'msub':
      return `<m:sSub><m:e>${conv(f[0])}</m:e><m:sub>${conv(f[1])}</m:sub></m:sSub>`;
    case 'msubsup':
      return `<m:sSubSup><m:e>${conv(f[0])}</m:e><m:sub>${conv(f[1])}</m:sub><m:sup>${conv(f[2])}</m:sup></m:sSubSup>`;
    case 'mfrac': {
      const semBarra = /^0(\.0*)?([a-z]+)?$/.test(no.attr.linethickness ?? '');
      return `<m:f>${semBarra ? '<m:fPr><m:type m:val="noBar"/></m:fPr>' : ''}<m:num>${conv(f[0])}</m:num><m:den>${conv(f[1])}</m:den></m:f>`;
    }
    case 'msqrt':
      return `<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>${sequencia(f)}</m:e></m:rad>`;
    case 'mroot':
      return `<m:rad><m:deg>${conv(f[1])}</m:deg><m:e>${conv(f[0])}</m:e></m:rad>`;
    case 'mover':
    case 'munder': {
      const marca = f[1];
      const simbolo = marca?.tag === 'mo' ? marca.texto.trim() : '';
      if (no.tag === 'mover' && (simbolo === '‾' || simbolo === '¯' || (simbolo === 'ˉ' && marca.attr.stretchy === 'true'))) {
        return `<m:bar><m:barPr><m:pos m:val="top"/></m:barPr><m:e>${conv(f[0])}</m:e></m:bar>`;
      }
      if (no.tag === 'munder' && (simbolo === '‾' || simbolo === '_')) {
        return `<m:bar><m:barPr><m:pos m:val="bot"/></m:barPr><m:e>${conv(f[0])}</m:e></m:bar>`;
      }
      if (no.tag === 'mover' && no.attr.accent === 'true' && simbolo) {
        const chr = ACENTOS[simbolo] ?? simbolo;
        return `<m:acc><m:accPr><m:chr m:val="${escXml(chr)}"/></m:accPr><m:e>${conv(f[0])}</m:e></m:acc>`;
      }
      if (simbolo && /^[⏞⏟︷︸]$/.test(simbolo)) {
        const pos = no.tag === 'mover' ? 'top' : 'bot';
        return `<m:groupChr><m:groupChrPr><m:chr m:val="${escXml(simbolo)}"/><m:pos m:val="${pos}"/></m:groupChrPr><m:e>${conv(f[0])}</m:e></m:groupChr>`;
      }
      const tag = no.tag === 'mover' ? 'limUpp' : 'limLow';
      return `<m:${tag}><m:e>${conv(f[0])}</m:e><m:lim>${conv(f[1])}</m:lim></m:${tag}>`;
    }
    case 'munderover':
      return `<m:limUpp><m:e><m:limLow><m:e>${conv(f[0])}</m:e><m:lim>${conv(f[1])}</m:lim></m:limLow></m:e><m:lim>${conv(f[2])}</m:lim></m:limUpp>`;
    case 'menclose':
      return `<m:borderBox><m:e>${sequencia(f)}</m:e></m:borderBox>`;
    case 'mtable': {
      const linhasT = f.filter((x) => x.tag === 'mtr' || x.tag === 'mlabeledtr');
      const ncol = Math.max(1, ...linhasT.map((r) => r.filhos.filter((c) => c.tag === 'mtd').length));
      const alin = (no.attr.columnalign ?? '').split(/\s+/);
      const jc = (k: number) => {
        const a = alin[Math.min(k, alin.length - 1)] || 'center';
        return a === 'right' ? 'right' : a === 'left' ? 'left' : 'center';
      };
      if (/^right left/.test(no.attr.columnalign ?? '')) {
        // align e aligned: matriz de equacoes do Word, alinhada em cada & (o "=" de cada linha)
        const linhasEq = linhasT.map((r) => {
          const cels = r.filhos.filter((c) => c.tag === 'mtd').map((c) => conv(c));
          return cels.map((x, k) => (k === 0 ? x : alinhar(x))).join('');
        });
        return `<m:eqArr><m:eqArrPr><m:ctrlPr>${rPrWord()}</m:ctrlPr></m:eqArrPr>${linhasEq.map((x) => `<m:e>${x}</m:e>`).join('')}</m:eqArr>`;
      }
      const cols = Array.from({ length: ncol }, (_, k) => `<m:mc><m:mcPr><m:count m:val="1"/><m:mcJc m:val="${jc(k)}"/></m:mcPr></m:mc>`).join('');
      // colunas do aligned (direita, esquerda) coladas, como no LaTeX
      const colado = /^(right left ?)+/.test(no.attr.columnalign ?? '');
      const mPr = `<m:mPr><m:baseJc m:val="center"/><m:plcHide m:val="1"/>${colado ? '<m:cGpRule m:val="3"/><m:cGp m:val="0"/>' : ''}<m:mcs>${cols}</m:mcs></m:mPr>`;
      const rows = linhasT
        .map((r) => {
          const cels = r.filhos.filter((c) => c.tag === 'mtd');
          while (cels.length < ncol) cels.push({ tag: 'mtd', attr: {}, filhos: [], texto: '' });
          return `<m:mr>${cels.map((c) => `<m:e>${conv(c)}</m:e>`).join('')}</m:mr>`;
        })
        .join('');
      return `<m:m>${mPr}${rows}</m:m>`;
    }
    default:
      return sequencia(f) + (no.texto.trim() ? corrida(no.texto) : '');
  }
}

/** Marca a primeira letra do trecho como ponto de alinhamento da matriz de equacoes. */
function alinhar(xml: string): string {
  const i = xml.indexOf('<m:r>');
  if (i < 0) return `<m:r><m:rPr><m:aln/></m:rPr>${rPrWord()}<m:t></m:t></m:r>` + xml;
  const resto = xml.slice(i + 5);
  if (resto.startsWith('<m:rPr>')) {
    const fim = resto.indexOf('</m:rPr>');
    return xml.slice(0, i + 5) + resto.slice(0, fim) + '<m:aln/>' + resto.slice(fim);
  }
  return xml.slice(0, i + 5) + '<m:rPr><m:aln/></m:rPr>' + resto;
}

function letrasDe(no: No): string {
  if (no.tag === 'annotation') return '';
  return (no.texto + no.filhos.map(letrasDe).join('')).normalize('NFKD').replace(/[^\p{L}\p{N}]/gu, '');
}

/** LaTeX de uma equacao destacada para OMML; null se o KaTeX nao entender ou se a conversao nao gerar nada. */
export function latexParaOmml(latex: string, macros: Record<string, string> = {}, tamanho = 12): EquacaoTex | null {
  let mathml: string;
  try {
    mathml = katex.renderToString(latex, { output: 'mathml', displayMode: true, throwOnError: true, strict: false, macros: { ...macros } });
  } catch {
    return null;
  }
  const raiz = lerXml(mathml);
  tamanhoAtual = Math.round(tamanho * 2);
  const omml = conv(raiz);
  if (!omml) return null;
  return { omml: `<m:oMath>${omml}</m:oMath>`, letras: letrasDe(raiz) };
}

/** Troca o tamanho da letra (pt) de um OMML ja pronto. */
export function comTamanho(omml: string, tamanho: number): string {
  const v = Math.max(2, Math.round(tamanho * 2));
  return omml.replace(/<w:sz w:val="\d+"\/><w:szCs w:val="\d+"\/>/g, `<w:sz w:val="${v}"/><w:szCs w:val="${v}"/>`);
}
