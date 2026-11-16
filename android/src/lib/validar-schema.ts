// Validador minimo de JSON Schema, so com o que os schemas do EguaTex usam: type, enum, required,
// properties, additionalProperties (false ou schema), items, minItems, minLength, maxLength, pattern,
// oneOf e $ref local ("#/$defs/nome"). Sem dependencias, roda no app e nos testes.

type Schema = {
  $ref?: string;
  type?: 'object' | 'array' | 'string' | 'integer' | 'number' | 'boolean';
  enum?: unknown[];
  required?: string[];
  properties?: Record<string, Schema>;
  additionalProperties?: boolean | Schema;
  items?: Schema;
  minItems?: number;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  oneOf?: Schema[];
  $defs?: Record<string, Schema>;
};

// chaves que mexem no prototipo de um objeto: um JSON importado nunca pode traze-las
const CHAVES_RESERVADAS = new Set(['__proto__', 'constructor', 'prototype']);

function tipoDe(valor: unknown): string {
  if (valor === null) return 'null';
  if (Array.isArray(valor)) return 'array';
  if (typeof valor === 'number') return Number.isInteger(valor) ? 'integer' : 'number';
  return typeof valor;
}

function resolver(ref: string, raiz: Schema): Schema | undefined {
  if (!ref.startsWith('#/')) return undefined;
  let atual: unknown = raiz;
  for (const parte of ref.slice(2).split('/')) {
    if (typeof atual !== 'object' || atual === null) return undefined;
    if (!Object.hasOwn(atual, parte)) return undefined;
    atual = (atual as Record<string, unknown>)[parte];
  }
  return atual as Schema | undefined;
}

function conferir(valor: unknown, schema: Schema, raiz: Schema, caminho: string, erros: string[]): void {
  if (schema.$ref !== undefined) {
    const alvo = resolver(schema.$ref, raiz);
    if (!alvo) erros.push(`${caminho}: referencia ${schema.$ref} nao existe`);
    else conferir(valor, alvo, raiz, caminho, erros);
    return;
  }
  if (schema.oneOf) {
    const acertos = schema.oneOf.filter((s) => {
      const interno: string[] = [];
      conferir(valor, s, raiz, caminho, interno);
      return interno.length === 0;
    });
    if (acertos.length !== 1) {
      const detalhe: string[] = [];
      conferir(valor, schema.oneOf[schema.oneOf.length - 1], raiz, caminho, detalhe);
      erros.push(`${caminho}: nao corresponde a nenhum formato aceito${detalhe.length ? ` (${detalhe[0]})` : ''}`);
    }
    return;
  }
  const tipo = tipoDe(valor);
  if (schema.type && !(tipo === schema.type || (schema.type === 'number' && tipo === 'integer'))) {
    erros.push(`${caminho}: esperado ${schema.type}, veio ${tipo}`);
    return;
  }
  if (schema.enum && !schema.enum.includes(valor)) erros.push(`${caminho}: valor fora da lista permitida`);
  if (typeof valor === 'string') {
    if (schema.minLength !== undefined && valor.length < schema.minLength) erros.push(`${caminho}: texto curto demais`);
    if (schema.maxLength !== undefined && valor.length > schema.maxLength) erros.push(`${caminho}: texto longo demais`);
    if (schema.pattern !== undefined && !new RegExp(schema.pattern).test(valor)) erros.push(`${caminho}: formato invalido`);
  }
  if (Array.isArray(valor)) {
    if (schema.minItems !== undefined && valor.length < schema.minItems) erros.push(`${caminho}: lista com itens de menos`);
    if (schema.items) valor.forEach((item, i) => conferir(item, schema.items as Schema, raiz, `${caminho}[${i}]`, erros));
  }
  if (tipo === 'object') {
    const obj = valor as Record<string, unknown>;
    for (const chave of schema.required ?? []) {
      if (!Object.hasOwn(obj, chave)) erros.push(`${caminho}: falta o campo ${chave}`);
    }
    const props = schema.properties ?? {};
    for (const [chave, v] of Object.entries(obj)) {
      if (CHAVES_RESERVADAS.has(chave)) erros.push(`${caminho}: chave ${chave} reservada`);
      else if (Object.hasOwn(props, chave)) conferir(v, props[chave], raiz, `${caminho}.${chave}`, erros);
      else if (schema.additionalProperties === false) erros.push(`${caminho}: campo ${chave} nao e aceito`);
      else if (typeof schema.additionalProperties === 'object') conferir(v, schema.additionalProperties, raiz, `${caminho}.${chave}`, erros);
    }
  }
}

/** Lista de erros em texto; vazia quando o valor serve. `ref` valida contra uma parte do schema, ex.: "#/$defs/exportacao". */
export function validarSchema(valor: unknown, schema: object, ref?: string): string[] {
  const raiz = schema as Schema;
  const erros: string[] = [];
  conferir(valor, ref ? { $ref: ref } : raiz, raiz, '$', erros);
  return erros;
}
