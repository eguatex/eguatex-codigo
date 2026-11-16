// "Procurar atualização": o que a tela diz para cada resposta de `cmd_procurar_atualizacao`.
// A consulta só roda quando a pessoa clica no menu, nunca sozinha.

import { tf } from './i18n';

export interface ResultadoAtualizacao {
  /** nova, atualizado, sem-release, sem-internet ou erro. */
  estado: string;
  versao_atual: string;
  versao_nova?: string | null;
  link?: string | null;
}

export interface AvisoAtualizacao {
  texto: string;
  /** Endereço para baixar a versão nova. Só vem quando há versão nova. */
  link: string | null;
}

export function avisoDeAtualizacao(r: ResultadoAtualizacao): AvisoAtualizacao {
  const campos = { atual: r.versao_atual, nova: r.versao_nova ?? '' };
  switch (r.estado) {
    case 'nova':
      return { texto: tf('atualizacao.nova', campos), link: r.link ?? null };
    case 'atualizado':
      return { texto: tf('atualizacao.atualizado', campos), link: null };
    case 'sem-release':
      return { texto: tf('atualizacao.sem-release', campos), link: null };
    case 'sem-internet':
      return { texto: tf('atualizacao.sem-internet', campos), link: null };
    default:
      return { texto: tf('atualizacao.erro', campos), link: null };
  }
}
