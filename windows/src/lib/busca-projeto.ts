// Busca de texto no projeto: suporte a maiúsculas/minúsculas, palavra inteira, regex, extensões (.tex, .bib, .txt, .md) e aviso de limite.

export const LIMITE_BUSCA = 500;

export interface Achado {
  arquivo: string;
  linha: number;
  texto: string;
}

export interface TextoDeArquivo {
  caminho: string;
  texto: string;
}

export interface OpcoesBusca {
  caseSensitive?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
  extensoes?: string[]; // ex: ['.tex', '.bib', '.txt', '.md']
}

/**
 * Procura `termo` nos arquivos fornecidos com opções avançadas.
 * `cortado` é true quando o número de ocorrências atinge o limite.
 */
export function buscarEmTexto(
  arquivos: TextoDeArquivo[],
  termo: string,
  opcoes: OpcoesBusca = {},
  limite = LIMITE_BUSCA,
): { achados: Achado[]; cortado: boolean } {
  const termoLimpo = termo.trim();
  if (!termoLimpo) return { achados: [], cortado: false };
  const extPermitidas = opcoes.extensoes || ['.tex', '.bib', '.txt', '.md'];

  let regex: RegExp;
  try {
    if (opcoes.regex) {
      regex = new RegExp(termo, opcoes.caseSensitive ? '' : 'i');
    } else {
      let padrao = termoLimpo.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      if (opcoes.wholeWord) {
        padrao = `\\b${padrao}\\b`;
      }
      regex = new RegExp(padrao, opcoes.caseSensitive ? '' : 'i');
    }
  } catch {
    const esc = termoLimpo.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    regex = new RegExp(esc, opcoes.caseSensitive ? '' : 'i');
  }

  const achados: Achado[] = [];
  for (const { caminho, texto } of arquivos) {
    const ext = caminho.substring(caminho.lastIndexOf('.'));
    if (extPermitidas.length > 0 && !extPermitidas.includes(ext.toLowerCase())) {
      continue;
    }
    const linhas = texto.split('\n');
    for (let i = 0; i < linhas.length; i++) {
      if (!regex.test(linhas[i])) continue;
      if (achados.length >= limite) return { achados, cortado: true };
      achados.push({ arquivo: caminho, linha: i + 1, texto: linhas[i].trim() });
    }
  }
  return { achados, cortado: false };
}
