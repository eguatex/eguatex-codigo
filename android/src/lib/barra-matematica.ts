// Barra Matematica: categorias padrao (dados/barra-matematica.json), categorias, botoes e macros do usuario
// (localStorage, chave eguatex.atalhos) e a mesclagem dos dois. Tudo aqui e funcao pura, sem Svelte.
// A parte reativa fica em atalhos.svelte.ts.

import padraoBruto from '../../dados/barra-matematica.json';
import schema from '../../dados/schemas/barra-matematica.schema.json';
import simbolosBrutos from '../../dados/simbolos.json';
import { validarSchema } from './validar-schema';

export const CHAVE_ATALHOS = 'eguatex.atalhos';

export interface BotaoUsuario {
  rotulo?: string;
  comando: string;
}

export interface Botao extends BotaoUsuario {
  /** Id de uma acao da barra do editor (ex.: ambiente-align). Sem ele, o botao insere `comando`. */
  acao?: string;
}

export interface CategoriaPadrao {
  id: string;
  botoes: Botao[];
}

export interface CategoriaUsuario {
  id: string;
  nome: string;
  botoes: BotaoUsuario[];
}

export interface Macro {
  texto: string;
  codigo: string;
}

export interface AtalhosUsuario {
  /** Categorias criadas pelo usuario. */
  categorias: CategoriaUsuario[];
  /** Botoes que o usuario acrescentou a uma categoria (padrao ou propria), por id. */
  extras: Record<string, BotaoUsuario[]>;
  /** Nome que o usuario deu a uma categoria padrao, por id. */
  nomes: Record<string, string>;
  /** Categorias padrao escondidas. */
  ocultas: string[];
  /** Ordem de todas as categorias (padrao e proprias). O que nao esta aqui vai para o fim. */
  ordem: string[];
  macros: Macro[];
}

export interface CategoriaEfetiva {
  id: string;
  /** true para as que vem do JSON padrao. */
  padrao: boolean;
  /** Nome dado pelo usuario; nas padrao sem renomear fica indefinido (o rotulo vem do i18n). */
  nome?: string;
  oculta: boolean;
  /** Botoes que vem do JSON padrao (vazio nas categorias do usuario). */
  botoesPadrao: Botao[];
  /** Botoes do usuario nesta categoria. */
  botoesProprios: BotaoUsuario[];
  /** Padrao primeiro, depois os do usuario. */
  botoes: Botao[];
}

export const PADRAO: { categorias: CategoriaPadrao[] } = padraoBruto as { categorias: CategoriaPadrao[] };
export const IDS_PADRAO: string[] = PADRAO.categorias.map((c) => c.id);

export const LIMITE_NOME = 40;

// ---------- grupos de simbolos.json (gerados em tempo de execucao) ----------

/** Grupo de simbolos.json que a barra mostra depois das categorias padrao. Nao e editavel nem aparece no gerenciador. */
export interface GrupoSimbolos {
  id: string;
  /** Nome do grupo no proprio simbolos.json. */
  nome: string;
  botoes: Botao[];
}

interface SimboloBruto {
  comando?: unknown;
  unicode?: unknown;
}

function idDoGrupo(nome: string): string {
  const base = nome
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `sim-${base || 'grupo'}`;
}

/**
 * Cria um grupo para cada grupo de simbolos.json, so com o que as categorias padrao ainda nao tem.
 * Grupo que as padrao ja cobrem por inteiro (Letras gregas, Operadores, Relacoes, Setas) nao vira grupo.
 * Um comando aparece uma vez so, nas padrao ou no primeiro grupo gerado que o tiver.
 */
export function gruposDeSimbolos(
  padrao: CategoriaPadrao[] = PADRAO.categorias,
  bruto: unknown = simbolosBrutos,
): GrupoSimbolos[] {
  const lista = (bruto as { categorias?: unknown } | null)?.categorias;
  if (!Array.isArray(lista)) return [];
  const usados = new Set<string>(padrao.flatMap((c) => c.botoes.map((b) => b.comando.trim())));
  const saida: GrupoSimbolos[] = [];
  for (const g of lista) {
    const { nome, simbolos } = (g ?? {}) as { nome?: unknown; simbolos?: unknown };
    if (typeof nome !== 'string' || !Array.isArray(simbolos)) continue;
    const botoes: Botao[] = [];
    for (const s of simbolos as SimboloBruto[]) {
      if (typeof s?.comando !== 'string' || !s.comando.trim() || usados.has(s.comando.trim())) continue;
      usados.add(s.comando.trim());
      botoes.push(typeof s.unicode === 'string' && s.unicode ? { rotulo: s.unicode, comando: s.comando } : { comando: s.comando });
    }
    if (botoes.length > 0) saida.push({ id: idDoGrupo(nome), nome, botoes });
  }
  return saida;
}

export const GRUPOS_SIMBOLOS: GrupoSimbolos[] = gruposDeSimbolos();

export function usuarioVazio(): AtalhosUsuario {
  return { categorias: [], extras: {}, nomes: {}, ocultas: [], ordem: [], macros: [] };
}

// ---------- validacao de comando ----------

/** Comando de botao: comeca por \ ou $ e tem chaves equilibradas (\{ e \} nao contam). */
export function comandoValido(comando: string): boolean {
  if (typeof comando !== 'string' || comando.length < 2) return false;
  if (!(comando.startsWith('\\') || comando.startsWith('$'))) return false;
  let nivel = 0;
  for (let i = 0; i < comando.length; i++) {
    const c = comando[i];
    if (c === '\\') {
      i++;
      continue;
    }
    if (c === '{') nivel++;
    else if (c === '}') {
      nivel--;
      if (nivel < 0) return false;
    }
  }
  return nivel === 0;
}

function botaoUsuarioValido(b: unknown): b is BotaoUsuario {
  if (typeof b !== 'object' || b === null) return false;
  const { comando, rotulo } = b as Record<string, unknown>;
  if (typeof comando !== 'string' || !comandoValido(comando) || comando.length > 2000) return false;
  return rotulo === undefined || (typeof rotulo === 'string' && rotulo.length >= 1 && rotulo.length <= 40);
}

function limparBotao(b: BotaoUsuario): BotaoUsuario {
  const rotulo = b.rotulo?.trim();
  return rotulo ? { rotulo, comando: b.comando } : { comando: b.comando };
}

export function macroValida(m: unknown): m is Macro {
  if (typeof m !== 'object' || m === null) return false;
  const { texto, codigo } = m as Record<string, unknown>;
  return typeof texto === 'string' && /^\S{1,40}$/.test(texto) && typeof codigo === 'string' && codigo.length >= 1 && codigo.length <= 2000;
}

const ID_VALIDO = /^[a-z0-9][a-z0-9-]{0,40}$/;

// ---------- normalizacao e armazenamento ----------

/** Aceita qualquer coisa vinda do disco e devolve so o que serve. Item ruim some, o resto fica. */
export function normalizarUsuario(bruto: unknown): AtalhosUsuario {
  const saida = usuarioVazio();
  if (typeof bruto !== 'object' || bruto === null) return saida;
  const o = bruto as Record<string, unknown>;
  const vistos = new Set<string>();
  if (Array.isArray(o.categorias)) {
    for (const c of o.categorias) {
      if (typeof c !== 'object' || c === null) continue;
      const { id, nome, botoes } = c as Record<string, unknown>;
      if (typeof id !== 'string' || !ID_VALIDO.test(id) || IDS_PADRAO.includes(id) || vistos.has(id)) continue;
      if (typeof nome !== 'string' || !nome.trim() || nome.length > LIMITE_NOME) continue;
      vistos.add(id);
      saida.categorias.push({ id, nome: nome.trim(), botoes: Array.isArray(botoes) ? botoes.filter(botaoUsuarioValido).map(limparBotao) : [] });
    }
  }
  const conhecidos = new Set([...IDS_PADRAO, ...saida.categorias.map((c) => c.id)]);
  if (typeof o.extras === 'object' && o.extras !== null && !Array.isArray(o.extras)) {
    for (const [id, lista] of Object.entries(o.extras)) {
      if (!conhecidos.has(id) || !Array.isArray(lista)) continue;
      const botoes = lista.filter(botaoUsuarioValido).map(limparBotao);
      if (botoes.length) saida.extras[id] = botoes;
    }
  }
  if (typeof o.nomes === 'object' && o.nomes !== null && !Array.isArray(o.nomes)) {
    for (const [id, nome] of Object.entries(o.nomes)) {
      if (IDS_PADRAO.includes(id) && typeof nome === 'string' && nome.trim() && nome.length <= LIMITE_NOME) saida.nomes[id] = nome.trim();
    }
  }
  const idsDe = (v: unknown): string[] =>
    Array.isArray(v) ? [...new Set(v.filter((x): x is string => typeof x === 'string' && conhecidos.has(x)))] : [];
  saida.ocultas = idsDe(o.ocultas);
  saida.ordem = idsDe(o.ordem);
  if (Array.isArray(o.macros)) {
    const porTexto = new Map<string, Macro>();
    for (const m of o.macros) if (macroValida(m)) porTexto.set(m.texto, { texto: m.texto, codigo: m.codigo });
    saida.macros = [...porTexto.values()];
  }
  return saida;
}

type Armazenamento = Pick<Storage, 'getItem' | 'setItem'>;

function armazenamentoPadrao(): Armazenamento | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

export function carregarUsuario(armazenamento: Armazenamento | null = armazenamentoPadrao()): AtalhosUsuario {
  try {
    const bruto = armazenamento?.getItem(CHAVE_ATALHOS);
    return bruto ? normalizarUsuario(JSON.parse(bruto)) : usuarioVazio();
  } catch {
    return usuarioVazio();
  }
}

export function gravarUsuario(usuario: AtalhosUsuario, armazenamento: Armazenamento | null = armazenamentoPadrao()): void {
  try {
    armazenamento?.setItem(CHAVE_ATALHOS, JSON.stringify(usuario));
  } catch {
    // sem armazenamento: a mudanca vale so nesta sessao
  }
}

// ---------- categorias efetivas ----------

/** Ids de todas as categorias na ordem de exibicao: a ordem do usuario, depois as padrao e as proprias que faltam. */
export function ordemCompleta(usuario: AtalhosUsuario, padrao: CategoriaPadrao[] = PADRAO.categorias): string[] {
  const existentes = [...padrao.map((c) => c.id), ...usuario.categorias.map((c) => c.id)];
  const ordem = usuario.ordem.filter((id) => existentes.includes(id));
  for (const id of existentes) if (!ordem.includes(id)) ordem.push(id);
  return ordem;
}

/** Junta o JSON padrao com o que o usuario guardou. Inclui as ocultas (marcadas) para o gerenciador. */
export function montarCategorias(usuario: AtalhosUsuario, padrao: CategoriaPadrao[] = PADRAO.categorias): CategoriaEfetiva[] {
  return ordemCompleta(usuario, padrao).map((id) => {
    const p = padrao.find((c) => c.id === id);
    const propria = usuario.categorias.find((c) => c.id === id);
    const extras = usuario.extras[id] ?? [];
    const botoesPadrao = p ? p.botoes : [];
    const botoesProprios = propria ? propria.botoes : extras;
    return {
      id,
      padrao: !!p,
      nome: p ? usuario.nomes[id] : propria?.nome,
      oculta: usuario.ocultas.includes(id),
      botoesPadrao,
      botoesProprios,
      botoes: [...botoesPadrao, ...botoesProprios],
    };
  });
}

export function categoriasVisiveis(usuario: AtalhosUsuario, padrao: CategoriaPadrao[] = PADRAO.categorias): CategoriaEfetiva[] {
  return montarCategorias(usuario, padrao).filter((c) => !c.oculta);
}

/** Categoria que a barra mostra para a categoria ativa: uma das visiveis ou um grupo gerado de simbolos.json. */
export function categoriaAtivaPorId(
  usuario: AtalhosUsuario,
  id: string,
  padrao: CategoriaPadrao[] = PADRAO.categorias,
  grupos: GrupoSimbolos[] = GRUPOS_SIMBOLOS,
): CategoriaEfetiva | undefined {
  const efetiva = categoriasVisiveis(usuario, padrao).find((c) => c.id === id);
  if (efetiva) return efetiva;
  const g = grupos.find((x) => x.id === id);
  if (!g) return undefined;
  return { id: g.id, padrao: true, nome: g.nome, oculta: false, botoesPadrao: g.botoes, botoesProprios: [], botoes: g.botoes };
}

// ---------- edicao (cada funcao devolve um objeto novo) ----------

function slug(nome: string): string {
  const base = nome
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 24);
  return `u-${base || 'categoria'}`;
}

export function criarCategoria(usuario: AtalhosUsuario, nome: string): { usuario: AtalhosUsuario; id: string } | null {
  const limpo = nome.trim();
  if (!limpo || limpo.length > LIMITE_NOME) return null;
  const ocupados = new Set([...IDS_PADRAO, ...usuario.categorias.map((c) => c.id)]);
  const base = slug(limpo);
  let id = base;
  for (let n = 2; ocupados.has(id); n++) id = `${base}-${n}`;
  return { usuario: { ...usuario, categorias: [...usuario.categorias, { id, nome: limpo, botoes: [] }], ordem: [...ordemCompleta(usuario), id] }, id };
}

export function renomearCategoria(usuario: AtalhosUsuario, id: string, nome: string): AtalhosUsuario {
  const limpo = nome.trim();
  if (!limpo || limpo.length > LIMITE_NOME) return usuario;
  if (IDS_PADRAO.includes(id)) return { ...usuario, nomes: { ...usuario.nomes, [id]: limpo } };
  return { ...usuario, categorias: usuario.categorias.map((c) => (c.id === id ? { ...c, nome: limpo } : c)) };
}

/** Volta uma categoria padrao ao nome de fabrica. */
export function restaurarNome(usuario: AtalhosUsuario, id: string): AtalhosUsuario {
  const nomes = { ...usuario.nomes };
  delete nomes[id];
  return { ...usuario, nomes };
}

/** So as categorias do usuario podem ser apagadas; as padrao so se escondem. */
export function apagarCategoria(usuario: AtalhosUsuario, id: string): AtalhosUsuario {
  if (!usuario.categorias.some((c) => c.id === id)) return usuario;
  const extras = { ...usuario.extras };
  delete extras[id];
  return {
    ...usuario,
    categorias: usuario.categorias.filter((c) => c.id !== id),
    extras,
    ocultas: usuario.ocultas.filter((o) => o !== id),
    ordem: usuario.ordem.filter((o) => o !== id),
  };
}

export function alternarOculta(usuario: AtalhosUsuario, id: string): AtalhosUsuario {
  if (!ordemCompleta(usuario).includes(id)) return usuario;
  const oculta = usuario.ocultas.includes(id);
  return { ...usuario, ocultas: oculta ? usuario.ocultas.filter((o) => o !== id) : [...usuario.ocultas, id] };
}

function mover<T>(lista: T[], indice: number, delta: number): T[] {
  const destino = indice + delta;
  if (indice < 0 || indice >= lista.length || destino < 0 || destino >= lista.length) return lista;
  const nova = [...lista];
  [nova[indice], nova[destino]] = [nova[destino], nova[indice]];
  return nova;
}

export function moverCategoria(usuario: AtalhosUsuario, id: string, delta: number): AtalhosUsuario {
  const ordem = ordemCompleta(usuario);
  return { ...usuario, ordem: mover(ordem, ordem.indexOf(id), delta) };
}

function atualizarLista(usuario: AtalhosUsuario, id: string, f: (lista: BotaoUsuario[]) => BotaoUsuario[]): AtalhosUsuario {
  if (usuario.categorias.some((c) => c.id === id)) {
    return { ...usuario, categorias: usuario.categorias.map((c) => (c.id === id ? { ...c, botoes: f(c.botoes) } : c)) };
  }
  if (IDS_PADRAO.includes(id)) {
    const lista = f(usuario.extras[id] ?? []);
    const extras = { ...usuario.extras };
    if (lista.length) extras[id] = lista;
    else delete extras[id];
    return { ...usuario, extras };
  }
  return usuario;
}

export function adicionarBotao(usuario: AtalhosUsuario, id: string, botao: BotaoUsuario): AtalhosUsuario {
  if (!botaoUsuarioValido(botao)) return usuario;
  return atualizarLista(usuario, id, (l) => [...l, limparBotao(botao)]);
}

export function editarBotao(usuario: AtalhosUsuario, id: string, indice: number, botao: BotaoUsuario): AtalhosUsuario {
  if (!botaoUsuarioValido(botao)) return usuario;
  return atualizarLista(usuario, id, (l) => l.map((b, i) => (i === indice ? limparBotao(botao) : b)));
}

export function removerBotao(usuario: AtalhosUsuario, id: string, indice: number): AtalhosUsuario {
  return atualizarLista(usuario, id, (l) => l.filter((_, i) => i !== indice));
}

export function moverBotao(usuario: AtalhosUsuario, id: string, indice: number, delta: number): AtalhosUsuario {
  return atualizarLista(usuario, id, (l) => mover(l, indice, delta));
}

/** Macro nova com o mesmo texto troca a antiga. Texto com espaco ou codigo vazio nao entra. */
export function definirMacro(usuario: AtalhosUsuario, macro: Macro): AtalhosUsuario {
  if (!macroValida(macro)) return usuario;
  const existe = usuario.macros.some((m) => m.texto === macro.texto);
  const macros = existe ? usuario.macros.map((m) => (m.texto === macro.texto ? { ...macro } : m)) : [...usuario.macros, { ...macro }];
  return { ...usuario, macros };
}

export function removerMacro(usuario: AtalhosUsuario, texto: string): AtalhosUsuario {
  return { ...usuario, macros: usuario.macros.filter((m) => m.texto !== texto) };
}

// ---------- exportar, importar, mesclar ----------

export interface Exportacao extends AtalhosUsuario {
  formato: 'eguatex-atalhos';
  versao: 1;
}

export function paraExportacao(usuario: AtalhosUsuario): Exportacao {
  return { formato: 'eguatex-atalhos', versao: 1, ...usuario };
}

export function serializarExportacao(usuario: AtalhosUsuario): string {
  return JSON.stringify(paraExportacao(usuario), null, 2) + '\n';
}

function uniaoBotoes(a: BotaoUsuario[], b: BotaoUsuario[]): BotaoUsuario[] {
  const vistos = new Set(a.map((x) => x.comando));
  return [...a, ...b.filter((x) => !vistos.has(x.comando))];
}

/** Soma o recebido ao que ja existe. Mesmo id: une os botoes sem repetir comando. Mesma macro: vale a recebida. */
export function mesclarUsuario(atual: AtalhosUsuario, recebido: AtalhosUsuario): AtalhosUsuario {
  const categorias = atual.categorias.map((c) => {
    const r = recebido.categorias.find((x) => x.id === c.id);
    return r ? { ...c, nome: r.nome, botoes: uniaoBotoes(c.botoes, r.botoes) } : c;
  });
  for (const r of recebido.categorias) if (!categorias.some((c) => c.id === r.id)) categorias.push(r);
  const extras: Record<string, BotaoUsuario[]> = { ...atual.extras };
  for (const [id, lista] of Object.entries(recebido.extras)) extras[id] = uniaoBotoes(extras[id] ?? [], lista);
  const ordem = [...atual.ordem];
  for (const id of recebido.ordem) if (!ordem.includes(id)) ordem.push(id);
  const macros = [...atual.macros];
  for (const m of recebido.macros) {
    const i = macros.findIndex((x) => x.texto === m.texto);
    if (i >= 0) macros[i] = m;
    else macros.push(m);
  }
  return normalizarUsuario({
    categorias,
    extras,
    nomes: { ...atual.nomes, ...recebido.nomes },
    ocultas: [...new Set([...atual.ocultas, ...recebido.ocultas])],
    ordem,
    macros,
  });
}

/** Codigo do erro quando o arquivo nem e JSON; os demais erros sao diagnosticos de caminho ($.campo: problema). */
export const ERRO_JSON = 'json-invalido';

export type ResultadoImportacao = { ok: true; usuario: AtalhosUsuario; recebido: AtalhosUsuario } | { ok: false; erros: string[] };

/** Le o texto de um arquivo de atalhos, valida no schema e mescla com `atual`. */
export function importarAtalhos(texto: string, atual: AtalhosUsuario): ResultadoImportacao {
  let bruto: unknown;
  try {
    bruto = JSON.parse(texto);
  } catch {
    return { ok: false, erros: [ERRO_JSON] };
  }
  if (bruto === null || typeof bruto !== 'object' || Array.isArray(bruto)) {
    return { ok: false, erros: [ERRO_JSON] };
  }
  const erros = validarSchema(bruto, schema, '#/$defs/exportacao');
  const cats = (bruto as { categorias?: unknown }).categorias;
  if (erros.length === 0 && Array.isArray(cats)) {
    for (const c of cats as { id: string }[]) {
      if (IDS_PADRAO.includes(c.id)) erros.push(`$.categorias.${c.id}: id reservado`);
    }
    for (const c of cats as { id: string; botoes: { comando: string }[] }[]) {
      for (const b of c.botoes) if (!comandoValido(b.comando)) erros.push(`$.categorias.${c.id}: comando ${b.comando}`);
    }
  }
  const extras = (bruto as { extras?: Record<string, { comando: string }[]> }).extras;
  if (erros.length === 0 && extras) {
    for (const [id, lista] of Object.entries(extras)) {
      for (const b of lista) if (!comandoValido(b.comando)) erros.push(`$.extras.${id}: comando ${b.comando}`);
    }
  }
  if (erros.length > 0) return { ok: false, erros };
  const recebido = normalizarUsuario(bruto);
  return { ok: true, usuario: mesclarUsuario(atual, recebido), recebido };
}

/** Valida o JSON padrao no schema (usado pelos testes e util para conferir um arquivo editado a mao). */
export function validarPadrao(valor: unknown): string[] {
  return validarSchema(valor, schema, '#/$defs/padrao');
}
