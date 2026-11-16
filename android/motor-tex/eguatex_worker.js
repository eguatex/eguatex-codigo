// Web Worker do EguaTex Android: roda o motor LaTeX em WebAssembly (busytex).
// Mensagens recebidas:
//   { tipo: 'iniciar', base }  base = endereco onde estao os arquivos do motor
//   { tipo: 'compilar', id, arquivos, principal, motor, pararNoErro, diretorio, cache, memoria }
//   diretorio = pasta do projeto no aparelho; separa os auxiliares guardados de cada projeto
//   cache = { assinatura, arquivos: [{ caminho, base64 }] }: auxiliares gravados em disco pela
//   compilacao anterior (so entram se o Worker ainda nao tem os do projeto na memoria)
//   memoria = { dados, extras, pgf }: pacotes que a ultima compilacao boa do projeto usou
//   { tipo: 'preaquecer', chave, principal, texto, temClsSty, memoria }: carrega os pacotes de dados do projeto
//   e responde { tipo: 'preaquecido', chave, aceito } (aceito = false se recusou por estar compilando)
//   { tipo: 'limpar-cache' }
// Mensagens enviadas:
//   { tipo: 'progresso', texto } | { tipo: 'pronto' } | { tipo: 'falha', texto }
//   { tipo: 'resultado', id, pdf, log, logFinal, codigo, tempos, auxiliares, assinatura, memoria }
//   auxiliares, assinatura e memoria so vem quando a compilacao deu certo (senao null)

let pipeline = null;
let base = '';
// marcas de tempo (performance.now) do arranque do motor; so medem, nao mudam nada
const arranque = { inicio: 0, wasm: null, modulo: null, preaquecimento: null };
// compilacoes em andamento: o preaquecimento nao recria o modulo no meio de uma
let compilando = 0;

const PACOTES = [
  'texlive-basic.js',
  'ubuntu-texlive-latex-base.js',
  'ubuntu-texlive-latex-recommended.js',
  'ubuntu-texlive-latex-extra.js',
  'ubuntu-texlive-science.js',
  'ubuntu-texlive-fonts-recommended.js',
];

const DRIVERS = {
  pdflatex: 'pdftex_bibtex8',
  xelatex: 'xetex_bibtex8_dvipdfmx',
  lualatex: 'luahbtex_bibtex8',
};

function iniciar(endereco) {
  base = endereco.replace(/\/?$/, '/');
  arranque.inicio = performance.now();
  importScripts(base + 'busytex_pipeline.js');
  const url = (nome) => base + nome;
  pipeline = new BusytexPipeline(
    url('busytex.js'),
    url('busytex.wasm'),
    PACOTES.map(url),
    [url('texlive-basic.js')],
    [],
    (texto) => postMessage({ tipo: 'progresso', texto: String(texto) }),
    () => {
      arranque.modulo = Math.round(performance.now() - arranque.inicio);
      arranque.recargas = [...(pipeline.recargas || [])];
      postMessage({ tipo: 'pronto' });
    },
    true,
    BusytexPipeline.ScriptLoaderWorker,
  );
  pipeline.wasm_module_promise.then(
    () => { arranque.wasm = Math.round(performance.now() - arranque.inicio); },
    () => {},
  );
}

function binario(b64) {
  const s = atob(b64);
  const bytes = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) bytes[i] = s.charCodeAt(i);
  return bytes;
}

// abnTeX2 nao vem no busytex. Os arquivos dele (LPPL, copiados do TeX Live) vao em
// extras/abntex2 e entram ao lado do documento principal quando ele usa a classe.
let abntex2 = null;
async function arquivosAbntex2() {
  if (!abntex2) {
    const nomes = (await (await fetch(base + 'extras/abntex2.txt')).text()).split(/\s+/).filter(Boolean);
    abntex2 = await Promise.all(
      nomes.map(async (n) => ({ nome: n, texto: await (await fetch(base + 'extras/abntex2/' + n)).text() })),
    );
  }
  return abntex2;
}

// PGF/TikZ tambem nao vem no busytex, e o beamer depende dele. Os arquivos (GPL 2 ou
// LPPL 1.3c, copiados do TeX Live) vao em extras/pgf e entram quando o projeto usa
// tikz, pgf, pgfplots ou beamer.
const USA_PGF = /\b(tikz|pgf\w*|beamer\w*|tcolorbox|circuitikz|forest|smartdiagram)\b/;
let pgf = null;
async function arquivosPgf() {
  if (!pgf) {
    const nomes = (await (await fetch(base + 'extras/pgf.txt')).text()).split(/\s+/).filter(Boolean);
    pgf = await Promise.all(
      nomes.map(async (n) => ({ nome: n, texto: await (await fetch(base + 'extras/pgf/' + n)).text() })),
    );
  }
  return pgf;
}

// Pacotes e fontes que o busytex nao traz (qrcode, roboto, tgheros, arev, biblatex...),
// em extras/mais. Fontes sao binarias. O eguatex-fontes.map diz ao pdflatex onde estao
// as fontes Type 1 delas; o "=" troca linhas antigas do pdftex.map pelas novas.
const BINARIO = /\.(pfb|tfm|vf|otf|ttf)$/i;
let mais = null;
async function arquivosMais() {
  if (!mais) {
    const nomes = (await (await fetch(base + 'extras/mais.txt')).text()).split(/\s+/).filter(Boolean);
    mais = await Promise.all(
      nomes.map(async (n) => {
        const r = await fetch(base + 'extras/mais/' + n);
        return { nome: n, texto: BINARIO.test(n) ? new Uint8Array(await r.arrayBuffer()) : await r.text() };
      }),
    );
  }
  return mais;
}
// Classes novas (rpgtex) usam variantes "e" do expl3 (\tl_set:Ne, \tl_if_blank:eF) que o
// expl3 do busytex (2023-02) nao tem. extras/expl3-variantes.tex cria so as que faltam.
const USA_EXPL3_E = /\\(?:tl|str|seq|prop|clist|exp|msg)_[a-z_]+:[A-Za-z]*e/;
let expl3Variantes = null;
async function arquivoExpl3Variantes() {
  if (expl3Variantes === null) {
    const r = await fetch(base + 'extras/expl3-variantes.tex');
    expl3Variantes = r.ok ? await r.text() : '';
  }
  return { nome: 'expl3-variantes.tex', texto: expl3Variantes };
}

// abnTeX2: no LaTeX do busytex (2022-11) o \MakeTextUppercase vai para o sumario como
// "\MakeTextUppercase []{Introducao}" e a leitura do .toc para com "Argument of \MakeUppercase
// has an extra }" (TCC do Paulo). Sem o argumento opcional o sumario sai certo.
const ABNTEX2_CAIXA_ALTA = '\\AtBeginDocument{\\DeclareRobustCommand\\MakeTextUppercase[1]{\\MakeUppercase{#1}}}';

const MAPA_EXTRA = '\\ifdefined\\pdfmapfile\\pdfmapfile{+ec-para-lm.map}\\pdfmapfile{=eguatex-fontes.map}\\fi';

// Arquivos que entram em toda compilacao (ao lado do documento principal):
// babel-portuges (brazil, brazilian, portuges, portuguese .ldf), que o busytex nao traz.
let sempre = null;
async function arquivosSempre() {
  if (!sempre) {
    const nomes = (await (await fetch(base + 'extras/sempre.txt')).text()).split(/\s+/).filter(Boolean);
    sempre = await Promise.all(
      nomes.map(async (n) => ({ nome: n, texto: await (await fetch(base + 'extras/sempre/' + n)).text() })),
    );
  }
  return sempre;
}

// O busytex nao tem as fontes EC desenhadas (cm-super): \usepackage[T1]{fontenc} para com
// "Font ecrm1000 not found". Sem outro pacote de fonte no projeto, o Latin Modern (mesmo
// desenho) entra logo depois do fontenc, so na copia que vai para o motor.
const PACOTE_DE_FONTE = /\\usepackage(\[[^\]]*\])?\{[^}]*\b(lmodern|mlmodern|cfr-lm|mathptmx|times|newtx\w*|libertin\w*|palatino|mathpazo|fourier\w*|kpfonts|tg\w+|charter|bookman|newpx\w*|cmbright|ebgaramond|stix\w*|txfonts|pxfonts|helvet|avant|courier|lato|roboto|sourcesanspro|carlito|utopia|fontspec)\b/;
const FONTENC_T1 = /\\usepackage\[[^\]]*\bT1\b[^\]]*\]\{fontenc\}/;

function usarLatinModern(files, motor) {
  if (motor !== 'pdflatex' && DRIVERS[motor]) return;
  const textos = files.filter((f) => typeof f.contents === 'string' && /\.(tex|sty|cls)$/i.test(f.path));
  if (textos.some((f) => PACOTE_DE_FONTE.test(f.contents))) return;
  const alvo = textos.find((f) => FONTENC_T1.test(f.contents));
  if (alvo) alvo.contents = alvo.contents.replace(FONTENC_T1, (m) => m + '\\usepackage{lmodern}');
}

// O busytex so carrega os pacotes dos \usepackage do documento principal. Uma classe
// fora do nucleo (beamer, por exemplo) ou classes e pacotes do proprio projeto pedem
// outros por \LoadClass e \RequirePackage, que ele nao ve. Nesses casos carrega tudo.
const CLASSES_DO_NUCLEO = /^(article|report|book|letter|proc|slides|minimal)$/;

function precisaDeTodosOsPacotes(files, principal) {
  if (files.some((f) => /\.(cls|sty)$/i.test(f.path))) return true;
  const doc = files.find((f) => f.path === principal);
  const m = typeof doc?.contents === 'string' && /^[^%\n]*\\documentclass\s*(?:\[[^\]]*\])?\s*\{([^}]+)\}/m.exec(doc.contents);
  return !!m && !CLASSES_DO_NUCLEO.test(m[1].trim());
}

// Pacotes do TeX Live que o busytex nao traz, um arquivo .json por pacote em
// extras/pacotes, gerados por scripts/gerar-extras.py. O indice extras/pacotes.json diz
// quais pacotes cada \usepackage pede e de qual pacote vem cada arquivo.
let indice = null;
const carregados = new Map();
async function lerIndice() {
  if (!indice) {
    try {
      indice = await (await fetch(base + 'extras/pacotes.json')).json();
      indice.pgfSet = new Set(indice.pgf);
    } catch {
      indice = { gatilhos: {}, arquivos: {}, pacotes: [], pgfSet: new Set() };
    }
  }
  return indice;
}
async function pacoteExtra(nome) {
  if (!carregados.has(nome)) {
    const dados = await (await fetch(base + 'extras/pacotes/' + nome + '.json')).json();
    carregados.set(
      nome,
      Object.entries(dados).map(([n, v]) => ({ nome: n, texto: v.t ?? binario(v.b) })),
    );
  }
  return carregados.get(nome);
}
let mapaPacotes = null;
async function arquivoMapaPacotes() {
  if (mapaPacotes === null) {
    const r = await fetch(base + 'extras/pacotes-fontes.map');
    mapaPacotes = r.ok ? await r.text() : '';
  }
  return { nome: 'pacotes-fontes.map', texto: mapaPacotes };
}

const PEDE_PACOTE = /\\(?:usepackage|RequirePackage|RequirePackageWithOptions|documentclass|LoadClass|LoadClassWithOptions)\s*(?:\[[^\]]*\])?\s*\{([^}]*)\}/g;
const PEDE_TEMA = /\\use(|color|font|inner|outer)theme\s*(?:\[[^\]]*\])?\s*\{([^}]*)\}/g;

// Fontes pedidas ao fontspec pelo nome ou pelo arquivo: \setmainfont{Carlito}, \newfontfamily\x{Inria Sans}
const PEDE_FONTE = /\\(?:setmainfont|setsansfont|setmonofont|setmathfont|newfontfamily\s*\\[A-Za-z@]+|newfontface\s*\\[A-Za-z@]+|fontspec|setmathsfont|setmathrm)\s*(?:\[[^\]]*\])?\s*(?:\([^)]*\))?\s*(?:\[[^\]]*\])?\s*\{([^}#\\]+)\}/g;

/**
 * Chaves da fonte no indice, da mais exata para a mais larga: "Latin Modern Math" -> latinmodernmath;
 * "LibertinusMath-Regular.otf" -> libertinusmathregular e libertinusmath.
 */
function chavesDaFonte(nome) {
  const sem = nome.replace(/\.(otf|ttf)$/i, '');
  const limpa = (t) => t.toLowerCase().replace(/[^a-z0-9]/g, '');
  return [...new Set([limpa(sem), limpa(sem.split('-')[0])])].filter(Boolean);
}

/**
 * Fonte que nao existe no celular nem nos extras (Fira Code do Inria): a copia que vai ao motor
 * troca o nome por uma Latin Modern do mesmo tipo, para o resto do documento compilar. Sem isso
 * o mathspec parava a matematica inteira ("\\textfont 0 is undefined").
 */
function fonteSubstituta(nome) {
  if (/mono|code|courier|consol|typewriter/i.test(nome)) return 'Latin Modern Mono';
  if (/math/i.test(nome)) return 'Latin Modern Math';
  if (/sans|grotesk|helvet|arial/i.test(nome)) return 'Latin Modern Sans';
  return 'Latin Modern Roman';
}

/** Troca, nos textos do projeto, o nome da fonte que falta dentro das chaves dos comandos do fontspec. */
function trocarFontes(files, trocas) {
  if (trocas.size === 0) return;
  for (const f of files) {
    if (typeof f.contents !== 'string' || !/\.(tex|sty|cls)$/i.test(f.path)) continue;
    // so dentro dos comandos de fonte do fontspec (PEDE_FONTE): um \section{Fira Code} no texto fica igual
    f.contents = f.contents.replace(new RegExp(PEDE_FONTE.source, 'g'), (comando) => {
      let novo = comando;
      for (const [nome, nova] of trocas) novo = novo.split('{' + nome + '}').join('{' + nova + '}');
      return novo;
    });
  }
}

/** Pacote extra que traz a fonte pedida pelo nome ou pelo arquivo. */
function donoDaFonte(nome) {
  const fontes = indice.fontes || {};
  for (const c of chavesDaFonte(nome)) if (fontes[c]) return fontes[c];
  return undefined;
}

/** Pacotes extras que trazem as fontes pedidas nos textos. */
function pacotesDasFontes(textos) {
  const pacotes = new Set();
  for (const t of textos) {
    const limpo = t.replace(/(^|[^\\])%.*$/gm, '$1');
    for (const m of limpo.matchAll(PEDE_FONTE)) {
      const dono = donoDaFonte(m[1].trim());
      if (dono) pacotes.add(dono);
    }
  }
  return pacotes;
}

/** Nomes de pacotes e classes pedidos nos textos (sem os comentarios). */
function nomesPedidos(textos) {
  const nomes = new Set();
  for (const t of textos) {
    const limpo = t.replace(/(^|[^\\])%.*$/gm, '$1');
    for (const m of limpo.matchAll(PEDE_PACOTE)) for (const n of m[1].split(',')) if (n.trim()) nomes.add(n.trim());
    for (const m of limpo.matchAll(PEDE_TEMA)) for (const n of m[2].split(',')) if (n.trim()) nomes.add(`beamer${m[1]}theme${n.trim()}`);
  }
  return nomes;
}

// Arquivos pedidos fora do \usepackage: \input RoyalIn.fd (Royal.sty do cfr-initials) e as
// bibliotecas do TikZ (\usetikzlibrary{fill.image} pede tikzlibraryfill.image.code.tex, do tikzfill).
// Sem isto cada um custava uma tentativa inteira a mais (rpgtex passava de 4 minutos no emulador).
const PEDE_INPUT = /\\(?:input|InputIfFileExists)\s*\{?\s*([\w.-]+\.(?:fd|tex|sty|def|cfg))/g;
const PEDE_TIKZLIB = /\\use(?:tikz|pgf)library\s*(?:\[[^\]]*\])?\s*\{([^}]*)\}/g;

/** Pacotes extras que trazem os arquivos de \input e as bibliotecas do TikZ pedidos nos textos. */
function pacotesDosArquivos(textos) {
  const pacotes = new Set();
  for (const t of textos) {
    const limpo = t.replace(/(^|[^\\])%.*$/gm, '$1');
    for (const m of limpo.matchAll(PEDE_INPUT)) if (indice.arquivos[m[1]]) pacotes.add(indice.arquivos[m[1]]);
    for (const m of limpo.matchAll(PEDE_TIKZLIB)) {
      for (const n of m[1].split(',')) {
        const nome = n.trim();
        for (const a of [`tikzlibrary${nome}.code.tex`, `pgflibrary${nome}.code.tex`]) if (indice.arquivos[a]) pacotes.add(indice.arquivos[a]);
      }
    }
  }
  return pacotes;
}

/** Pacotes extras pedidos pelos nomes, e se o PGF/TikZ precisa entrar. */
function resolverExtras(nomes) {
  const pacotes = new Set();
  let pgf = false;
  for (const n of nomes) {
    for (const p of indice.gatilhos[n] ?? []) pacotes.add(p);
    if (indice.pgfSet.has(n) || USA_PGF.test(n)) pgf = true;
  }
  return { pacotes, pgf };
}

/**
 * Arquivo que o motor nao achou, tirado do log. Fonte do fontspec volta como "fonte:chave";
 * idioma do babel como o .ldf e estilo do biblatex como o .bbx.
 */
function arquivoQueFaltou(log) {
  return arquivosQueFaltaram(log)[0] ?? null;
}

/**
 * Todos os arquivos que faltaram, na ordem do log. O TeX quebra as linhas do log em 79 colunas
 * ("The font \"LibertinusMath-Regular\" can" + "not be found"): procura tambem no log sem quebras.
 */
function arquivosQueFaltaram(log) {
  const saida = [];
  const juntar = (v) => { if (v && !saida.includes(v)) saida.push(v); };
  for (const texto of [log, log.replace(/\r?\n/g, '')]) {
    for (const m of texto.matchAll(/The font "([^"]+)" cannot be found/g)) juntar('fonte:' + m[1]);
    for (const m of texto.matchAll(/Package babel Error: Unknown option '([^']+)'/g)) juntar(m[1] + '.ldf');
    for (const m of texto.matchAll(/Package biblatex Error: Style '([^']+)' not found/g)) juntar(m[1] + '.bbx');
    for (const m of texto.matchAll(/File `([^']+)' not found/g)) juntar(m[1].includes('.') ? m[1] : m[1] + '.tex');
    for (const m of texto.matchAll(/I can't find file `([^']+)'/g)) juntar(m[1].includes('.') ? m[1] : m[1] + '.tex');
    // quantificadores limitados: no log sem quebras um [^\n]* livre retrocedia em tempo quadratico (300 KB, minutos parado em 100%)
    for (const m of texto.matchAll(/Font \\[^=\s]{1,100}=(\S+)[\s\S]{0,300}?Metric \(TFM\) file not found/g)) juntar(m[1] + '.tfm');
    for (const m of texto.matchAll(/Font ([^\s]+) at \d+ not found/g)) juntar(m[1] + '.tfm');
    for (const m of texto.matchAll(/! Font [^=\n]{0,100}=(\S+) not loadable/g)) juntar(m[1] + '.tfm');
  }
  return saida;
}

// Idiomas do babel e estilos do biblatex pedidos nas opcoes: \usepackage[ngerman]{babel},
// \documentclass[ngerman]{...} (opcao global) e style=science, bibstyle=, citestyle=
const OPCOES_DE = /\\(?:usepackage|documentclass|RequirePackage|PassOptionsToPackage)\s*\[([^\]]*)\]/g;

/** Arquivos .ldf e .bbx/.cbx que as opcoes do documento pedem. */
function arquivosDasOpcoes(textos) {
  const saida = new Set();
  for (const t of textos) {
    const limpo = t.replace(/(^|[^\\])%.*$/gm, '$1');
    for (const m of limpo.matchAll(OPCOES_DE)) {
      for (const bruto of m[1].split(',')) {
        const op = bruto.trim();
        const estilo = /^(?:style|bibstyle|citestyle)\s*=\s*([\w-]+)$/.exec(op);
        if (estilo) {
          saida.add(estilo[1] + '.bbx');
          saida.add(estilo[1] + '.cbx');
        } else if (/^[a-z]+$/i.test(op)) saida.add(op + '.ldf');
      }
    }
  }
  return saida;
}

const MAPA_PACOTES = '\\ifdefined\\pdfmapfile\\pdfmapfile{=pacotes-fontes.map}\\fi';

function paraBase64(bytes) {
  let texto = '';
  for (let i = 0; i < bytes.length; i += 0x8000) texto += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(texto);
}

/**
 * Entrega ao pipeline os auxiliares que a compilacao anterior gravou em disco. So vale quando o
 * Worker ainda nao tem os auxiliares do projeto na memoria (a memoria e sempre mais nova).
 * Devolve quantos arquivos entraram.
 */
function semearCache(chave, cache) {
  const lista = cache && Array.isArray(cache.arquivos) ? cache.arquivos : [];
  if (lista.length === 0) return 0;
  pipeline.auxiliares = pipeline.auxiliares || new Map();
  pipeline.assinaturas_bib = pipeline.assinaturas_bib || new Map();
  if (pipeline.auxiliares.has(chave)) return 0;
  const entradas = lista.map((a) => [pipeline.project_dir + '/' + a.caminho, binario(a.base64)]);
  entradas.sort((a, b) => (a[0] < b[0] ? -1 : 1));
  pipeline.auxiliares.set(chave, entradas);
  if (typeof cache.assinatura === 'string') pipeline.assinaturas_bib.set(chave, cache.assinatura);
  return entradas.length;
}

/** Auxiliares guardados da compilacao boa, com caminhos relativos ao projeto, prontos para o disco. */
function auxiliaresParaDisco(chave) {
  const guardados = pipeline.auxiliares && pipeline.auxiliares.get(chave);
  if (!guardados || guardados.length === 0) return null;
  const prefixo = pipeline.project_dir + '/';
  return guardados
    .filter(([c]) => c.startsWith(prefixo))
    .map(([c, bytes]) => ({ caminho: c.slice(prefixo.length), base64: paraBase64(bytes) }));
}

/**
 * Pacotes de dados que o projeto pede, com a mesma regra de compilar() e do pipeline para decidir
 * `todos`: classe fora do nucleo, .cls ou .sty proprio, pacote extra ou PGF carregam os 6; um pacote
 * sem dono tambem. Com a lista lembrada da ultima compilacao boa, ela vale no lugar da regra.
 */
async function pacotesDeDados({ principal, texto, temClsSty, memoria }) {
  if (memoria && Array.isArray(memoria.dados)) {
    const lembrados = PACOTES.filter((n) => memoria.dados.includes(n));
    if (lembrados.length > 0) return lembrados;
  }
  if (typeof texto !== 'string') return [PACOTES[0]];
  await lerIndice();
  const files = [{ path: principal, contents: texto }];
  if (temClsSty) files.push({ path: 'projeto.sty', contents: '' });
  const { pacotes, pgf } = resolverExtras(nomesPedidos([texto]));
  if (precisaDeTodosOsPacotes(files, principal) || pacotes.size > 0 || pgf || USA_PGF.test(texto)) return PACOTES;
  const resolvidos = Object.values(await pipeline.data_package_resolver.resolve(files, principal, null)).filter((v) => v.used);
  if (resolvidos.some((v) => v.source == null)) return PACOTES;
  const nomes = new Set([PACOTES[0]]);
  for (const v of resolvidos) if (v.source && v.source !== 'local') nomes.add(String(v.source).split('/').pop());
  return PACOTES.filter((n) => nomes.has(n));
}

const MEMORIA_MINIMA_GB = 3;

/**
 * Aparelho com pouca memoria: os 6 pacotes de dados juntos pesam centenas de MB no WebAssembly.
 * Usa navigator.deviceMemory (em GB). O WebView do Android o expoe; sem ele o ambiente e incomum
 * e a memoria conta como baixa (so o texlive-basic; a compilacao carrega o resto).
 */
function memoriaBaixa() {
  const gb = typeof navigator !== 'undefined' ? navigator.deviceMemory : undefined;
  if (typeof gb === 'number' && gb > 0) return gb < MEMORIA_MINIMA_GB;
  return true;
}

/** Carrega com antecedencia os pacotes de dados do projeto aberto, sem desfazer o que ja esta carregado. */
async function preaquecerModulo(dados) {
  if (!pipeline || compilando) return false;
  const marca = performance.now();
  try {
    // pouca memoria: so o basico; o resto fica para a compilacao, que carrega o que o documento pede
    const nomes = memoriaBaixa() ? [PACOTES[0]] : await pacotesDeDados(dados);
    const urls = nomes.map((n) => base + n);
    const atual = await pipeline.Module;
    // uma compilacao pode ter comecado enquanto isto esperava: ela cuida do modulo
    if (compilando) return false;
    if (urls.every((u) => atual.data_packages_js.includes(u))) return true;
    pipeline.Module = pipeline.reload_module_if_needed(false, pipeline.env, pipeline.project_dir, urls);
    try {
      await pipeline.Module;
    } catch {
      // modulo nao nasceu: a proxima compilacao cria outro do zero
      pipeline.Module = null;
      return false;
    }
    arranque.preaquecimento = Math.round(performance.now() - marca);
    return true;
  } catch {
    // preaquecer e so adiantamento; qualquer falha fica para a compilacao
    return false;
  }
}

/** Preaquece e confirma ao frontend: aceito = false quando recusou (compilando) ou falhou, para ele tentar de novo depois. */
async function preaquecer(dados) {
  const aceito = await preaquecerModulo(dados);
  postMessage({ tipo: 'preaquecido', chave: dados.chave, aceito });
}

/** Pacotes extras pedidos pelos que ja estao no conjunto (dependencias), ate 4 voltas. Devolve se o PGF entrou. */
async function fecharDependencias(pacotes, pgf, fontes = false, porNome = null) {
  for (let volta = 0; volta < 4; volta++) {
    const antes = pacotes.size;
    const textos = [];
    for (const p of pacotes) for (const a of await pacoteExtra(p)) if (typeof a.texto === 'string' && /\.(sty|cls)$/.test(a.nome)) textos.push(a.texto);
    const extra = resolverExtras(nomesPedidos(textos));
    for (const p of extra.pacotes) pacotes.add(p);
    for (const p of pacotesDosArquivos(textos)) pacotes.add(p);
    // no xelatex, as fontes que as classes e pacotes extras pedem ao fontspec (o acmart pede
    // Inconsolatazi4 e LibertinusMath): sem isto cada uma custava uma tentativa inteira a mais
    if (fontes) for (const p of pacotesDasFontes(textos)) { pacotes.add(p); porNome?.add(p); }
    pgf ||= extra.pgf;
    if (pacotes.size === antes) break;
  }
  return pgf;
}

const ARQUIVO_DE_PACOTE = /\.(sty|cls|def|cfg|clo|fd|bst|bbx|cbx|lbx)$/;

/** O log mostra algum arquivo do pacote extra sendo aberto: "(./nome.sty". */
async function pacoteVistoNoLog(nome, log) {
  return (await pacoteExtra(nome)).some((a) => ARQUIVO_DE_PACOTE.test(a.nome) && log.includes('/' + a.nome));
}

async function pgfVistoNoLog(log) {
  return (await arquivosPgf()).some((a) => /\.(sty|cls)$/.test(a.nome) && log.includes('/' + a.nome));
}

/** Junta as marcas de uma chamada do pipeline nas marcas da compilacao (varias tentativas somam). */
function somarTempos(tempos, marcas) {
  if (!marcas) return;
  tempos.passadas.push(...marcas.passadas);
  tempos.recriacoes += marcas.recriacoes || 0;
  // cada recriacao do modulo: [pacotes de dados, carga do wasm e dos pacotes ms, montagem ms]
  tempos.recargas.push(...(marcas.recargas || []));
  // modulo, arquivos e PDF lido: ficam os valores da ultima tentativa, em ms desde o inicio dela
  tempos.modulo = marcas.modulo;
  tempos.arquivos = marcas.arquivos;
  tempos.pdfLido = marcas.pdf_lido;
  tempos.pacotesDados = marcas.pacotes_dados;
}

function ehPdflatexDe(motor) {
  return (DRIVERS[motor] ?? DRIVERS.pdflatex) === DRIVERS.pdflatex;
}

const FONTE_BINARIA = /\.(otf|ttf)$/i;

/**
 * Compila o projeto com o busytex. Tenta de novo, ate 6 vezes, quando falta um arquivo
 * que um pacote extra traz. Repete do zero, sem o cache de auxiliares, se um auxiliar
 * guardado causar erro apontado no log. Responde com `resultado` (pdf, log, logFinal, codigo).
 */
async function compilar({ id, arquivos, principal, motor, pararNoErro, diretorio, cache, memoria }) {
  const marcaCompilar = performance.now();
  const tempos = { passadas: [], tentativas: 0, recriacoes: 0, recargas: [] };
  await lerIndice();
  // os auxiliares guardados sao separados por projeto
  pipeline.chave_projeto = diretorio || '';
  const chave = (diretorio || '') + '|' + principal;
  tempos.cacheDisco = semearCache(chave, cache);
  const textosProjeto = arquivos.filter((a) => typeof a.texto === 'string').map((a) => a.texto);
  const nomes = nomesPedidos(textosProjeto);
  const { pacotes, pgf: pedePgf } = resolverExtras(nomes);
  // as fontes so servem ao xelatex (fontspec); o pdflatex usa as metricas dos pacotes
  // pacotes cujas fontes o fontspec procura pelo nome: so as .otf/.ttf deles vao para a pasta do
  // fontconfig. Sem cache (o fontconfig do busytex nao grava), cada passada do xelatex le todas as
  // fontes dessa pasta; com as 60 do libertine e do dejavu o ACM levava 18 s por passada
  const pacotesPorNome = new Set();
  if (!ehPdflatexDe(motor)) for (const p of pacotesDasFontes(textosProjeto)) { pacotes.add(p); pacotesPorNome.add(p); }
  for (const p of pacotesDosArquivos(textosProjeto)) pacotes.add(p);
  // idioma do babel e estilo do biblatex que um pacote extra traz (babel-german, biblatex-science)
  for (const a of arquivosDasOpcoes(textosProjeto)) if (indice.arquivos[a]) pacotes.add(indice.arquivos[a]);
  let usarPgf = pedePgf || textosProjeto.some((t) => USA_PGF.test(t));
  // o que o projeto pede por si (nomes, dependencias e o que as tentativas descobrirem): so isto vira memoria
  const usados = new Set(pacotes);
  let usadoPgf = usarPgf;
  // pacotes extras que a ultima compilacao boa do projeto precisou: evita descobri-los por tentativa e erro
  const lembrados = memoria && Array.isArray(memoria.extras) ? memoria.extras.filter((p) => indice.pacotes.includes(p)) : [];
  for (const p of lembrados) pacotes.add(p);
  usarPgf ||= !!(memoria && memoria.pgf);
  tempos.extrasLembrados = lembrados.length;
  const ehPdflatex = ehPdflatexDe(motor);
  let r = null;
  const tentados = new Set();
  /** Fontes que faltam no celular e o nome que vai no lugar delas (nome -> substituta). */
  const trocasDeFonte = new Map();

  // ate 6 tentativas: cada uma pode descobrir um pacote extra que faltava
  for (let tentativa = 0; tentativa < 6; tentativa++) {
    // dependencias dos pacotes extras ja escolhidos
    usarPgf = await fecharDependencias(pacotes, usarPgf, !ehPdflatex, pacotesPorNome);

    const files = arquivos.map((a) => ({ path: a.caminho, contents: a.texto ?? binario(a.base64) }));
    trocarFontes(files, trocasDeFonte);
    const todos = precisaDeTodosOsPacotes(files, principal) || pacotes.size > 0 || usarPgf;
    const pasta = principal.includes('/') ? principal.slice(0, principal.lastIndexOf('/') + 1) : '';
    const extras = [...(await arquivosSempre())];
    if (arquivos.some((a) => a.texto && /abntex2/.test(a.texto))) extras.push(...(await arquivosAbntex2()));
    if (usarPgf) extras.push(...(await arquivosPgf()));
    for (const p of pacotes) extras.push(...(await pacoteExtra(p)));
    extras.push(...(await arquivosMais()), await arquivoMapaPacotes());
    // na mesma linha 1, para os numeros de linha dos erros nao mudarem
    const doc = files.find((f) => f.path === principal);
    if (doc && typeof doc.contents === 'string' && ehPdflatex) doc.contents = MAPA_EXTRA + MAPA_PACOTES + doc.contents;
    if (doc && typeof doc.contents === 'string' && /\\documentclass\s*(?:\[[^\]]*\])?\s*\{abntex2\}/.test(doc.contents)) {
      doc.contents = ABNTEX2_CAIXA_ALTA + doc.contents;
    }
    if (doc && typeof doc.contents === 'string' && textosProjeto.some((t) => USA_EXPL3_E.test(t))) {
      const variantes = await arquivoExpl3Variantes();
      if (variantes.texto) {
        extras.push(variantes);
        doc.contents = '\\input{expl3-variantes.tex}' + doc.contents;
      }
    }
    // so conta o que esta na pasta do documento: um acmart.cls numa subpasta (acmart-primary/ do
    // ACM TechBrief) nao fica no caminho de busca do TeX, e o do pacote extra precisa entrar
    const presentes = new Set(files.map((f) => f.path));
    for (const extra of extras) {
      if (presentes.has(pasta + extra.nome)) continue;
      presentes.add(pasta + extra.nome);
      files.push({ path: pasta + extra.nome, contents: extra.texto, fonte: FONTE_BINARIA.test(extra.nome) && pacotesPorNome.has(indice.arquivos[extra.nome]) });
    }
    usarLatinModern(files, motor);
    const driver = DRIVERS[motor] ?? DRIVERS.pdflatex;
    // mapas de fontes que o xdvipdfmx le alem do dele (sem eles o MnSymbol do Inria parava o PDF)
    pipeline.mapas_xdvipdfmx = ['ec-para-lm.map', 'eguatex-fontes.map', 'pacotes-fontes.map'].filter((m) => presentes.has(pasta + m));
    tempos.tentativas++;
    r = await pipeline.compile(files, principal, null, 'silent', driver, null, pararNoErro, todos);
    somarTempos(tempos, r.marcas);
    // um .aux antigo pode quebrar a compilacao (pacote removido, por exemplo): repete do zero
    // so quando o erro aponta uma linha de um auxiliar ("./main.aux:12: ..."), formato do -file-line-error
    if (!(r.pdf && r.exit_code === 0) && pipeline.compilou_com_auxiliares && /\.(aux|toc|out|lof|lot|nav|snm|bbl|idx|ind|nlo|nls|loa|thm):\d+:/.test(r.log_final || '')) {
      tempos.tentativas++;
      r = await pipeline.compile(files, principal, null, 'silent', driver, null, pararNoErro, todos);
      somarTempos(tempos, r.marcas);
    }
    // faltou um arquivo que um pacote extra tem? carrega o pacote e tenta de novo, mesmo com PDF
    // (sem --halt-on-error o PDF sai, mas sem a fonte, o idioma ou o estilo que faltou)
    const nomesPgf = new Set((await arquivosPgf()).map((a) => a.nome));
    let achou = false;
    for (const falta of arquivosQueFaltaram(r.log_final || r.log || '')) {
      if (tentados.has(falta)) continue;
      tentados.add(falta);
      const dono = falta.startsWith('fonte:') ? donoDaFonte(falta.slice(6)) : indice.arquivos[falta];
      if (falta.startsWith('fonte:') && dono) pacotesPorNome.add(dono);
      if (falta.startsWith('fonte:') && !dono && !/^Latin Modern/.test(falta.slice(6))) {
        const nome = falta.slice(6);
        trocasDeFonte.set(nome, fonteSubstituta(nome));
        postMessage({ tipo: 'progresso', texto: `A fonte ${nome} não existe no celular; usando ${fonteSubstituta(nome)}...` });
        achou = true;
        break;
      }
      if (dono && !pacotes.has(dono)) {
        pacotes.add(dono);
        usados.add(dono);
        postMessage({ tipo: 'progresso', texto: `Carregando o pacote ${dono}...` });
        achou = true;
        break;
      } else if (!usarPgf && nomesPgf.has(falta)) {
        usarPgf = true;
        usadoPgf = true;
        achou = true;
        break;
      }
    }
    if (!achou) break;
  }

  const pdf = r.pdf ? r.pdf.buffer.slice(r.pdf.byteOffset, r.pdf.byteOffset + r.pdf.byteLength) : null;
  tempos.arranque = { wasm: arranque.wasm, modulo: arranque.modulo, recargas: arranque.recargas, preaquecimento: arranque.preaquecimento };
  // so a compilacao boa deixa cache em disco e lista de pacotes; falha apaga o cache no frontend
  const boa = !!(r.pdf && r.exit_code === 0);
  const auxiliares = boa ? auxiliaresParaDisco(chave) : null;
  const assinatura = auxiliares ? (pipeline.assinaturas_bib.get(chave) ?? null) : null;
  const dadosUsados = boa && tempos.pacotesDados ? [...new Set([PACOTES[0], ...tempos.pacotesDados])] : null;
  // a lista lembrada nao cresce sozinha: fica o que o projeto pede por si e o que o log mostra sendo aberto
  let lembrar = null;
  if (boa && dadosUsados) {
    const logFinal = r.log_final || '';
    usadoPgf = (await fecharDependencias(usados, usadoPgf)) || usadoPgf;
    const extrasUsados = [];
    for (const p of pacotes) if (usados.has(p) || (await pacoteVistoNoLog(p, logFinal))) extrasUsados.push(p);
    lembrar = { dados: dadosUsados, extras: extrasUsados, pgf: usadoPgf || (await pgfVistoNoLog(logFinal)) };
  }
  tempos.compilar = Math.round(performance.now() - marcaCompilar);
  tempos.fontesTrocadas = [...trocasDeFonte].map(([de, para]) => `${de} -> ${para}`);
  console.log('EguaTex tempos do worker', JSON.stringify(tempos));
  postMessage(
    { tipo: 'resultado', id, pdf, log: r.log, logFinal: r.log_final, codigo: r.exit_code, tempos, auxiliares, assinatura, memoria: lembrar },
    pdf ? [pdf] : [],
  );
}

onmessage = async ({ data }) => {
  try {
    if (data.tipo === 'iniciar') iniciar(data.base);
    else if (data.tipo === 'preaquecer') await preaquecer(data);
    else if (data.tipo === 'compilar') {
      compilando++;
      try {
        await compilar(data);
      } finally {
        compilando--;
      }
    } else if (data.tipo === 'limpar-cache') {
      // Limpar arquivos auxiliares: esquece os .aux e as assinaturas guardadas de todos os projetos
      pipeline.auxiliares?.clear();
      pipeline.assinaturas_bib?.clear();
    }
  } catch (e) {
    postMessage({ tipo: 'falha', id: data.id, texto: `${e}\n${e && e.stack ? e.stack : ''}` });
  }
};
