import ptBR from "../../dados/i18n/pt-BR.json";

type Messages = typeof ptBR;

let current: Messages = ptBR;

export function t(key: keyof Messages): string {
  return current[key] ?? key;
}

/** Texto com campos: "Ola {nome}" com { nome: "Ana" } vira "Ola Ana". Campo sem valor fica como esta. */
export function tf(key: keyof Messages, campos: Record<string, string | number> = {}): string {
  return t(key).replace(/\{(\w+)\}/g, (inteiro, nome: string) => (nome in campos ? String(campos[nome]) : inteiro));
}

export function setLocale(messages: Messages) {
  current = messages;
}
