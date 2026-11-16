// Envio do feedback: monta o payload e valida. O caminho normal é o e-mail (FEEDBACK_URL vazia): abre o
// programa de e-mail com o texto pronto e a pessoa confirma lá. Se um dia FEEDBACK_URL for preenchida,
// o envio vira POST JSON e a fila local guarda o que falhar por rede (a fila só vale para o POST).
// Nada sai sozinho: só o que a pessoa escreveu no modal, mais a versão do app e o sistema.
// Obs.: @tauri-apps/plugin-http não está no projeto, então o POST usa o fetch comum da webview.

import { EMAIL_CONTATO, FEEDBACK_URL } from './site';
import { t, tf } from './i18n';
import { montarMailto } from './feedback';

export const LIMITE_MENSAGEM = 5000;
export const TEMPO_LIMITE_MS = 12000;
export const CHAVE_PENDENTES = 'eguatex.feedback.pendentes';
const MAX_PENDENTES = 20;

export const TIPOS = ['sugestao', 'problema', 'elogio', 'outro'] as const;
export type TipoFeedback = (typeof TIPOS)[number];

export interface CamposFeedback {
  tipo: TipoFeedback;
  mensagem: string;
  nome?: string;
}

export interface PayloadFeedback {
  tipo: TipoFeedback;
  mensagem: string;
  nome: string;
  versao: string;
  sistema: string;
}

export type ResultadoEnvio = 'enviado' | 'guardado' | 'mailto' | 'erro';

export interface OpcoesEnvio {
  url?: string;
  fetchFn?: typeof fetch;
  abrirLink?: (link: string) => Promise<unknown> | unknown;
  email?: string;
}

/** Chave da mensagem de erro em pt-BR, ou null se os campos servem. */
export function erroDosCampos(campos: CamposFeedback): 'feedback.erro-mensagem' | null {
  if (!campos.mensagem.trim()) return 'feedback.erro-mensagem';
  return null;
}

/** Payload puro: apara os textos, corta a mensagem em 5000 caracteres e rejeita (lança) campo inválido. */
export function montarPayload(campos: CamposFeedback, versao: string, sistema: string): PayloadFeedback {
  const erro = erroDosCampos(campos);
  if (erro) throw new Error(erro);
  return {
    tipo: TIPOS.includes(campos.tipo) ? campos.tipo : 'outro',
    mensagem: campos.mensagem.trim().slice(0, LIMITE_MENSAGEM),
    nome: (campos.nome ?? '').trim(),
    versao,
    sistema,
  };
}

/** Mailto com assunto "[ÉguaTex] {tipo}" e o payload no corpo: o envio normal. */
export function montarMailtoDoPayload(p: PayloadFeedback, email = EMAIL_CONTATO): string {
  const linhas = [`${t('feedback.tipo')}: ${t(`feedback.tipo-${p.tipo}` as 'feedback.tipo-outro')}`];
  if (p.nome) linhas.push(`${t('feedback.nome')}: ${p.nome}`);
  linhas.push('', p.mensagem, '', '---', `${t('feedback.versao')}: ${p.versao}`, `${t('feedback.sistema')}: ${p.sistema}`, '');
  const assunto = tf('feedback.assunto-modelo', { tipo: t(`feedback.tipo-${p.tipo}` as 'feedback.tipo-outro') });
  return montarMailto(email, assunto, linhas.join('\n'));
}

/** Resultado de um POST: ok, erro do servidor (5xx), erro de rede/tempo, ou recusa (4xx, não adianta repetir). */
type Tentativa = 'ok' | 'rede' | 'recusado';

async function postar(url: string, payload: PayloadFeedback, fetchFn: typeof fetch): Promise<Tentativa> {
  const controle = new AbortController();
  const timer = setTimeout(() => controle.abort(), TEMPO_LIMITE_MS);
  try {
    const r = await fetchFn(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
      signal: controle.signal,
    });
    if (r.ok) return 'ok';
    return r.status >= 500 ? 'rede' : 'recusado';
  } catch {
    return 'rede';
  } finally {
    clearTimeout(timer);
  }
}

export function lerPendentes(): PayloadFeedback[] {
  try {
    const bruto = JSON.parse(localStorage.getItem(CHAVE_PENDENTES) ?? '[]');
    return Array.isArray(bruto) ? bruto : [];
  } catch {
    return [];
  }
}

function gravarPendentes(lista: PayloadFeedback[]) {
  try {
    if (lista.length) localStorage.setItem(CHAVE_PENDENTES, JSON.stringify(lista.slice(-MAX_PENDENTES)));
    else localStorage.removeItem(CHAVE_PENDENTES);
  } catch {
    // sem localStorage a fila não existe; o feedback só se perde se a rede também falhou
  }
}

async function abrirPadrao(link: string) {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  await openUrl(link);
}

/**
 * Envia o feedback. Com URL: POST JSON (12 s); rede fora vai para a fila e devolve 'guardado'.
 * Sem URL (caminho normal): abre o e-mail e devolve 'mailto'. 'erro' quando o servidor recusa ou o e-mail não abre.
 */
export async function enviarFeedback(payload: PayloadFeedback, opcoes: OpcoesEnvio = {}): Promise<ResultadoEnvio> {
  const url = opcoes.url ?? FEEDBACK_URL;
  if (!url) {
    try {
      await (opcoes.abrirLink ?? abrirPadrao)(montarMailtoDoPayload(payload, opcoes.email));
      return 'mailto';
    } catch (e) {
      console.error('Erro ao abrir o e-mail:', e);
      return 'erro';
    }
  }
  const r = await postar(url, payload, opcoes.fetchFn ?? fetch);
  if (r === 'ok') return 'enviado';
  if (r === 'recusado') return 'erro';
  gravarPendentes([...lerPendentes(), payload]);
  return 'guardado';
}

/** Tenta de novo os feedbacks guardados. Sem URL ou sem fila não faz nada. Devolve quantos saíram. */
export async function reenviarPendentes(opcoes: OpcoesEnvio = {}): Promise<number> {
  const url = opcoes.url ?? FEEDBACK_URL;
  const fila = lerPendentes();
  if (!url || fila.length === 0) return 0;
  const sobra: PayloadFeedback[] = [];
  let enviados = 0;
  for (let i = 0; i < fila.length; i++) {
    const r = await postar(url, fila[i], opcoes.fetchFn ?? fetch);
    if (r === 'ok') enviados++;
    else if (r === 'rede') {
      // sem rede: guarda este e o resto sem tentar tudo de novo
      sobra.push(...fila.slice(i));
      break;
    }
    // 'recusado': o servidor não quer este, descarta
  }
  gravarPendentes(sobra);
  return enviados;
}
