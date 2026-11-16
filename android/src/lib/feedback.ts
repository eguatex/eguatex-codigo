// "Enviar feedback": monta o e-mail com a versão do app e o sistema. Nada é enviado pelo app:
// o link mailto abre o programa de e-mail da pessoa, que lê, edita e envia (ou não).

import { EMAIL_CONTATO } from './site';
import { t } from './i18n';

/** Parte do user agent entre parênteses: "Windows NT 10.0; Win64; x64" ou "Linux; Android 14; Pixel 7". */
export function sistemaDoUserAgent(userAgent: string): string {
  const m = /\(([^)]+)\)/.exec(userAgent);
  return (m ? m[1] : userAgent).trim();
}

/** Link mailto: com assunto e corpo codificados. Os dois montadores de feedback passam por aqui. */
export function montarMailto(email: string, assunto: string, corpo: string): string {
  return `mailto:${email}?subject=${encodeURIComponent(assunto)}&body=${encodeURIComponent(corpo)}`;
}

/** Link mailto: com assunto e corpo já escritos. `versao` vem de `cmd_versao` ("ÉguaTex 1.0.0"). */
export function montarMailtoFeedback(versao: string, sistema: string, email = EMAIL_CONTATO): string {
  const corpo = [
    t('feedback.corpo'),
    '',
    '',
    '---',
    `${t('feedback.versao')}: ${versao}`,
    `${t('feedback.sistema')}: ${sistema}`,
    '',
  ].join('\n');
  return montarMailto(email, t('feedback.assunto'), corpo);
}
