// Confere se os textos oficiais do EguaTex (IDENTIDADE.md) estao iguais no app.
// Roda sozinho antes de cada compilacao (script "prebuild" do package.json).
// Se algum texto estiver diferente, a compilacao para e mostra o que corrigir.
// Os textos oficiais so mudam com autorizacao da Jessica Lopes Melo.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const raiz = fileURLToPath(new URL('..', import.meta.url));
const ler = (caminho) => readFileSync(join(raiz, caminho), 'utf8');
const problemas = [];

const identidade = ler('IDENTIDADE.md');
const pegar = (rotulo) => {
  const achado = identidade.match(new RegExp(`\\*\\*${rotulo}:\\*\\* "([^"]+)"`));
  if (!achado) throw new Error(`IDENTIDADE.md sem a linha "${rotulo}".`);
  return achado[1];
};
const LEMA = pegar('Lema');
const DESCRICAO = pegar('Descrição curta');
const DESCRICAO_LONGA = `ÉguaTex. ${DESCRICAO} ${LEMA}`;

const conferir = (onde, atual, esperado) => {
  if (atual !== esperado) problemas.push(`${onde}\n    esta:     ${atual}\n    deve ser: ${esperado}`);
};

const i18n = JSON.parse(ler('dados/i18n/pt-BR.json'));
conferir('dados/i18n/pt-BR.json, chave app.descricao', i18n['app.descricao'], DESCRICAO);
conferir('dados/i18n/pt-BR.json, chave tela-inicial.subtitulo', i18n['tela-inicial.subtitulo'], DESCRICAO);

conferir('package.json, campo description', JSON.parse(ler('package.json')).description, DESCRICAO);

const tauri = JSON.parse(ler('src-tauri/tauri.conf.json'));
conferir('src-tauri/tauri.conf.json, bundle.shortDescription', tauri.bundle?.shortDescription, DESCRICAO);
conferir('src-tauri/tauri.conf.json, bundle.longDescription', tauri.bundle?.longDescription, DESCRICAO_LONGA);

const cargo = ler('src-tauri/Cargo.toml').match(/^description\s*=\s*"([^"]*)"/m);
conferir('src-tauri/Cargo.toml, campo description', cargo?.[1], DESCRICAO);

// Variacoes antigas que nao devem voltar nos textos que a pessoa ve.
const PROIBIDAS = ['criado na Amazônia', 'Editor LaTeX livre', 'editor de LaTeX', 'offline e em português'];
const varrer = (pasta) => {
  for (const nome of readdirSync(join(raiz, pasta))) {
    const caminho = join(pasta, nome);
    if (statSync(join(raiz, caminho)).isDirectory()) { varrer(caminho); continue; }
    if (!/\.(svelte|json)$/.test(nome) || /\.test\./.test(nome)) continue;
    const texto = ler(caminho);
    for (const frase of PROIBIDAS) {
      if (texto.toLowerCase().includes(frase.toLowerCase())) problemas.push(`${caminho}\n    usa "${frase}". Troque pelo texto oficial do IDENTIDADE.md.`);
    }
  }
};
varrer('src');
varrer('dados/i18n');

if (problemas.length) {
  console.error('\nTextos oficiais do EguaTex diferentes do IDENTIDADE.md:\n');
  for (const p of problemas) console.error(`  - ${p}\n`);
  console.error('Corrija os textos acima seguindo o IDENTIDADE.md e compile de novo.');
  console.error('Os textos oficiais so mudam com autorizacao da Jessica Lopes Melo.\n');
  process.exit(1);
}
console.log('Textos oficiais conferidos: tudo igual ao IDENTIDADE.md.');
