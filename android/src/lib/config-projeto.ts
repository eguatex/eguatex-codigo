// Configuracao de compilacao que vem dentro do projeto, como no Overleaf e no TeXstudio.
// Mesma regra do config_projeto.rs da versao Windows, adaptada ao motor do celular:
// - `latexmkrc` (ou `.latexmkrc`): `ensure_path('TEXMFHOME', ...)` e `ensure_path('TEXINPUTS', ...)`
//   dizem onde ficam classes e pacotes do projeto; `$pdflatex = 'lualatex ...'` e
//   `$pdf_mode = 4/5` escolhem o motor;
// - comentario magico `% !TeX program = lualatex` no documento principal (TeXstudio);
// - pastas `texmf`, `-texmf` ou `.texmf` na raiz, com uma subpasta `tex`.
// O motor WebAssembly nao aceita variaveis como TEXINPUTS. Por isso os arquivos dessas
// pastas sao copiados para a pasta do documento principal, onde o TeX sempre procura.

export type ArquivoProjeto = { caminho: string; texto?: string | null; base64?: string | null };

export type ConfigProjeto = {
  arquivos: ArquivoProjeto[];
  motor: string | null;
  notas: string[];
};

type Motor = 'pdflatex' | 'xelatex' | 'lualatex';

function motorDoNome(nome: string): Motor | null {
  const n = nome.trim().toLowerCase().replace(/\.exe$/, '');
  if (n.endsWith('lualatex')) return 'lualatex';
  if (n.endsWith('xelatex')) return 'xelatex';
  if (n.endsWith('pdflatex')) return 'pdflatex';
  return null;
}

function entreAspas(texto: string): string[] {
  return [...texto.matchAll(/'([^']*)'|"([^"]*)"/g)].map((m) => m[1] ?? m[2] ?? '');
}

/** Caminho do latexmkrc relativo a raiz. "/compile/" e a raiz do projeto no Overleaf. */
function resolver(caminho: string): string | null {
  let c = caminho.trim().replace(/\\/g, '/');
  if (c.startsWith('/compile')) c = c.slice('/compile'.length);
  else if (c.startsWith('/') || /^[a-z]:/i.test(c)) return null; // fora do projeto
  c = c.replace(/^(\.\/)+/, '').replace(/^\/+/, '').replace(/\/+$/, '');
  return c;
}

function lerLatexmkrc(texto: string, arvores: string[], entradas: string[]): Motor | null {
  let motor: Motor | null = null;
  let modoPdf: number | null = null;
  for (const bruta of texto.split(/\r?\n/)) {
    const linha = bruta.split('#')[0].trim();
    if (!linha) continue;
    const pos = linha.indexOf('ensure_path');
    if (pos >= 0) {
      const [variavel, ...valores] = entreAspas(linha.slice(pos));
      for (const v of valores) {
        const c = resolver(v);
        if (c === null) continue;
        if (variavel === 'TEXMFHOME') arvores.push(c);
        else if (variavel === 'TEXINPUTS') entradas.push(c);
      }
    } else if (linha.startsWith('$pdflatex')) {
      const cmd = entreAspas(linha)[0];
      if (cmd) motor = motorDoNome(cmd.split(/\s+/)[0]) ?? motor;
    } else if (linha.startsWith('$pdf_mode')) {
      const n = parseInt(linha.replace(/^\$pdf_mode[\s=]*/, ''), 10);
      if (!Number.isNaN(n)) modoPdf = n;
    }
  }
  if (modoPdf === 4) return 'lualatex';
  if (modoPdf === 5) return 'xelatex';
  return motor;
}

function comentarioMagico(texto: string): Motor | null {
  for (const linha of texto.split(/\r?\n/).slice(0, 30)) {
    const m = /^\s*%\s*!\s*T[eE]X\s+(?:TS-)?program\s*=\s*(\S+)/i.exec(linha);
    if (m) return motorDoNome(m[1]);
  }
  return null;
}

/** Conteudo de texto de um arquivo. Sem extensao conhecida (o latexmkrc), ele chega em base64. */
function textoDe(a: ArquivoProjeto | undefined): string {
  if (!a) return '';
  if (a.texto) return a.texto;
  if (!a.base64) return '';
  try {
    const bin = atob(a.base64);
    return new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
  } catch {
    return '';
  }
}

const nomeDe = (caminho: string) => caminho.slice(caminho.lastIndexOf('/') + 1);

/** Le a configuracao do projeto e devolve a lista de arquivos pronta para o motor. */
export function lerConfigProjeto(arquivos: ArquivoProjeto[], principal: string): ConfigProjeto {
  const notas: string[] = [];
  const arvores: string[] = [];
  const entradas: string[] = [];
  let motor: Motor | null = null;

  const pastaDoc = principal.includes('/') ? principal.slice(0, principal.lastIndexOf('/') + 1) : '';
  const porCaminho = new Map(arquivos.map((a) => [a.caminho, a]));

  for (const pasta of new Set(['', pastaDoc])) {
    for (const nome of ['latexmkrc', '.latexmkrc']) {
      const texto = textoDe(porCaminho.get(pasta + nome));
      if (!texto) continue;
      const m = lerLatexmkrc(texto, arvores, entradas);
      if (m) {
        motor = m;
        notas.push(`Motor ${m}, pedido pelo ${nome} do projeto.`);
      }
    }
  }

  const magico = comentarioMagico(textoDe(porCaminho.get(principal)));
  if (magico) {
    motor = magico;
    for (let i = notas.length - 1; i >= 0; i--) if (notas[i].startsWith('Motor ')) notas.splice(i, 1);
    notas.push(`Motor ${magico}, pedido pelo comentário % !TeX program do documento.`);
  }

  for (const nome of ['texmf', '-texmf', '.texmf']) {
    if (!arvores.includes(nome) && arquivos.some((a) => a.caminho.startsWith(nome + '/tex/'))) arvores.push(nome);
  }

  // pastas cujos arquivos o TeX precisa achar pelo nome
  const pastas = [
    ...arvores.flatMap((a) => [`${a}/tex/`, `${a}/bibtex/bib/`, `${a}/bibtex/bst/`]),
    ...entradas.map((e) => `${e}/`),
  ].map((p) => p.replace(/^\/+/, ''));

  const saida = [...arquivos];
  const nomesNaPasta = new Set(
    arquivos.filter((a) => a.caminho.startsWith(pastaDoc) && !a.caminho.slice(pastaDoc.length).includes('/')).map((a) => nomeDe(a.caminho)),
  );
  let copiados = 0;
  for (const a of arquivos) {
    if (!pastas.some((p) => a.caminho.startsWith(p))) continue;
    const nome = nomeDe(a.caminho);
    if (nomesNaPasta.has(nome)) continue; // o arquivo do documento vence, como no TEXINPUTS
    nomesNaPasta.add(nome);
    saida.push({ caminho: pastaDoc + nome, texto: a.texto, base64: a.base64 });
    copiados++;
  }
  const usadas = arvores.filter((a) => arquivos.some((x) => x.caminho.startsWith(a + '/')));
  for (const a of usadas) notas.push(`Classes e pacotes do projeto procurados também em ${a} (${copiados} arquivos).`);

  return { arquivos: saida, motor, notas };
}
