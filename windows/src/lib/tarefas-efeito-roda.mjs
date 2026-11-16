// Roda em Node puro (o Vitest resolve o Svelte para a build de servidor, onde $effect nao faz nada).
// Compila o tarefas.svelte.ts real com o compilador do Svelte, monta um $effect.root com
// acompanharCompilacao (o efeito que o Layout usa) e imprime em JSON o que aconteceu.
// Uso: node tarefas-efeito-roda.mjs <raiz-do-projeto> <pasta-temporaria>
import { compileModule } from 'svelte/compiler';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';

const [raiz, tmp] = process.argv.slice(2);
const exigir = createRequire(raiz + '/package.json');
const internal = pathToFileURL(exigir.resolve('svelte/internal/client')).href;
// o pacote 'svelte' resolve para a build de servidor em Node; o efeito so executa na build de cliente
const indice = pathToFileURL(path.join(path.dirname(exigir.resolve('svelte/package.json')), 'src', 'index-client.js')).href;
const { stripTypeScriptTypes } = await import('node:module');

const consertar = (codigo) =>
  codigo.replaceAll("'svelte/internal/client'", `'${internal}'`).replaceAll("'svelte/internal/disclose-version'", `'${internal}'`).replaceAll("'svelte'", `'${indice}'`);

const tarefasJs = stripTypeScriptTypes(fs.readFileSync(raiz + '/src/lib/tarefas.svelte.ts', 'utf8'));
fs.writeFileSync(tmp + '/tarefas.mjs', consertar(compileModule(tarefasJs, { generate: 'client', filename: 'tarefas.svelte.js' }).js.code));

const teste = `
import { flushSync } from 'svelte';
import { acompanharCompilacao, tarefas } from './tarefas.mjs';
export function montar() {
  const estado = $state({ status: 'idle' });
  let leituras = 0;
  const desmontar = $effect.root(() => {
    acompanharCompilacao(() => { leituras++; return estado.status; }, 'Compilando...');
  });
  return { tarefas, desmontar, leituras: () => leituras, mudar(s) { estado.status = s; flushSync(); }, assentar: () => flushSync() };
}
`;
fs.writeFileSync(tmp + '/teste.mjs', consertar(compileModule(teste, { generate: 'client', filename: 'teste.svelte.js' }).js.code));

const { montar } = await import(pathToFileURL(tmp + '/teste.mjs').href);
const m = montar();
const saida = {};
m.assentar();
const antes = m.leituras();
saida.listaInicial = m.tarefas.lista.length;
m.mudar('compilando');
saida.execucoesAoCompilar = m.leituras() - antes;
saida.listaCompilando = m.tarefas.lista.length;
saida.canal = m.tarefas.lista[0]?.canal;
const n = m.leituras();
m.tarefas.lista.push({ id: 999, rotulo: 'outra', inicio: 0 });
m.assentar();
m.tarefas.lista.splice(1, 1);
m.assentar();
saida.execucoesAoMexerNaLista = m.leituras() - n;
m.mudar('sucesso');
saida.listaSucesso = m.tarefas.lista.length;
m.mudar('compilando');
saida.listaDeNovo = m.tarefas.lista.length;
m.desmontar();
saida.listaAposDesmontar = m.tarefas.lista.length;
console.log(JSON.stringify(saida));
