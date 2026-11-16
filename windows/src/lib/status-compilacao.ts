// Textos curtos da barra de status: a causa do erro da compilacao ("pacote geometry nao
// encontrado na linha 42") e a sigla do corretor ortografico ("PT-BR").

import { t, tf } from './i18n';
import type { EstadoOrtografia } from './ortografia-idioma';

export interface ErroDaBarra {
  mensagem: string;
  linha?: number | null;
  tipo: string;
}

/** Mensagens que so dizem que a compilacao parou; a causa de verdade vem em outro erro. */
const GENERICAS = [/Emergency stop/i, /Fatal error occurred/i, /no output PDF file produced/i, /^\s*==>/];

const MAX_CAUSA = 60;

function semPrefixo(mensagem: string): string {
  return mensagem
    .replace(/^!\s*/, '')
    .replace(/^(LaTeX|Package \S+|Class \S+) Error:\s*/i, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Causa curta de uma mensagem do LaTeX, sem a linha. */
export function causaDaMensagem(mensagem: string): string {
  let m: RegExpMatchArray | null;
  if ((m = mensagem.match(/File [`']([^']+?)\.sty' not found/))) return tf('status.causa-pacote', { nome: m[1] });
  if ((m = mensagem.match(/File [`']([^']+?)\.cls' not found/))) return tf('status.causa-classe', { nome: m[1] });
  if ((m = mensagem.match(/File [`']([^']+)' not found/))) return tf('status.causa-arquivo', { nome: m[1] });
  if (/Undefined control sequence/i.test(mensagem)) return t('status.causa-comando');
  if (/Missing \$ inserted/i.test(mensagem)) return t('status.causa-cifrao');
  if (/Missing \\begin\{document\}/i.test(mensagem)) return t('status.causa-begin-document');
  if ((m = mensagem.match(/Environment (\S+) undefined/i))) return tf('status.causa-ambiente', { nome: m[1] });
  if (/Too many \}'s|Extra \}|Runaway argument|File ended while scanning|Missing \} inserted/i.test(mensagem)) {
    return t('status.causa-chave');
  }
  if (/requires either XeTeX or|XeTeX is required|requires LuaTeX/i.test(mensagem)) return t('status.causa-motor');
  if (/font .*(cannot be found|not loadable|not found)/i.test(mensagem)) return t('status.causa-fonte');
  // minted sem shell-escape ou sem o Pygments: o codigo sai sem cores
  if (/Package minted Error|\(minted\)|You must invoke LaTeX with the -shell-escape/i.test(mensagem)) return t('status.causa-minted');
  if (/Motor '.+' não encontrado/.test(mensagem)) return t('status.causa-sem-motor');
  const limpo = semPrefixo(mensagem);
  return limpo.length > MAX_CAUSA ? `${limpo.slice(0, MAX_CAUSA - 1).trimEnd()}…` : limpo;
}

/**
 * Texto do estado de erro: "Erro: pacote geometry não encontrado na linha 42". Usa o primeiro erro
 * de verdade (nao aviso), pulando os que so dizem que a compilacao parou. Sem erro na lista, "Erro".
 */
export function textoDoErro(erros: readonly ErroDaBarra[]): string {
  const reais = erros.filter((e) => e.tipo !== 'aviso' && e.mensagem.trim() !== '');
  const escolhido = reais.find((e) => !GENERICAS.some((g) => g.test(e.mensagem))) ?? reais[0];
  if (!escolhido) return t('status.erro');
  const causa = causaDaMensagem(escolhido.mensagem);
  const comLinha = escolhido.linha ? tf('status.erro-linha', { causa, linha: escolhido.linha }) : causa;
  return tf('status.erro-causa', { causa: comLinha });
}

/** Sigla do corretor ("PT-BR", "EN-US"; o idioma do % !TeX spellcheck quando ele desliga o corretor). */
export function siglaOrtografia(estado: EstadoOrtografia): string {
  if (!estado.ligada && estado.motivo === 'idioma') return estado.codigo.replace('_', '-').toUpperCase();
  if (estado.ligada && estado.idioma === 'en') return 'EN-US';
  return 'PT-BR';
}
