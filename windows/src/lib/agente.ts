// Agente de IA do ÉguaTeX: ferramentas que o Gemini pode chamar e propostas de alteracao.
// Regra central: o Gemini nunca grava nada. Ele so propoe; quem grava e o app, depois do "Aceitar".

export type NivelAgente = "conversa" | "leitura" | "sugerir";

export interface NoArquivo {
  nome: string;
  tipo: string;
  caminho: string;
  filhos?: NoArquivo[];
}

export interface ErroCompilacao {
  linha?: number | null;
  arquivo?: string | null;
  mensagem: string;
  tipo: string;
}

/** Acesso ao projeto aberto, entregue pela pagina principal. */
export interface ProjetoAgente {
  arquivoAtual(): string;
  arvore(): NoArquivo[];
  lerArquivo(caminho: string): Promise<string>;
  selecao(): { texto: string; linhaInicio: number; linhaFim: number } | null;
  erros(): ErroCompilacao[];
  logTexto(): string;
  aplicarEdicao(caminho: string, original: string, novo: string): Promise<void>;
  inserirNaLinha(caminho: string, linha: number, conteudo: string): Promise<void>;
  criarArquivo(caminho: string, conteudo: string): Promise<void>;
  /** Pasta do projeto aberto; com ela a conversa do chat fica salva em .eguatex/chat.json. */
  pasta?(): string;
  /** Insere texto na posicao do cursor do editor. Sem ela o chat nao mostra "Inserir no cursor". */
  inserirNoCursor?(texto: string): void;
}

export interface Proposta {
  id: number;
  tipo: "edicao" | "insercao" | "novo";
  caminho: string;
  motivo: string;
  original: string;
  novo: string;
  linha: number;
  estado: "pendente" | "aceita" | "rejeitada" | "falhou";
  erro?: string;
}

export const MAX_PROPOSTAS = 10;
const EXTENSOES = /\.(tex|bib|sty|cls|bst|txt|md)$/i;

export const NOMES_NIVEL: Record<NivelAgente, string> = {
  conversa: "Somente conversa",
  leitura: "Leitura do projeto",
  sugerir: "Sugerir alterações",
};

// ---------- declaracoes para o Gemini ----------

const LEITURA = [
  {
    name: "listar_arquivos",
    description: "Lista os arquivos do projeto LaTeX aberto (.tex, .bib, .sty, .cls).",
    parameters: { type: "object", properties: {} },
  },
  {
    name: "ler_arquivo",
    description: "Lê um arquivo do projeto e devolve o texto com o número de cada linha.",
    parameters: {
      type: "object",
      properties: {
        caminho: { type: "string", description: "Caminho relativo, como main.tex ou capitulos/cap1.tex" },
        linha_inicio: { type: "integer", description: "Primeira linha (opcional)" },
        linha_fim: { type: "integer", description: "Última linha (opcional)" },
      },
      required: ["caminho"],
    },
  },
  {
    name: "ler_selecao",
    description: "Devolve o trecho que a pessoa selecionou no editor e as linhas dele.",
    parameters: { type: "object", properties: {} },
  },
  {
    name: "ler_log_compilacao",
    description: "Devolve os erros e avisos da última compilação, já resumidos.",
    parameters: { type: "object", properties: {} },
  },
  {
    name: "buscar",
    description: "Procura um texto em todos os arquivos do projeto e devolve arquivo, linha e trecho.",
    parameters: {
      type: "object",
      properties: { texto: { type: "string", description: "Texto exato a procurar" } },
      required: ["texto"],
    },
  },
];

const ESCRITA = [
  {
    name: "propor_edicao",
    description:
      "Propõe trocar um trecho de um arquivo. trecho_original deve ser copiado exatamente do arquivo e aparecer uma única vez. A pessoa aceita ou rejeita.",
    parameters: {
      type: "object",
      properties: {
        caminho: { type: "string" },
        trecho_original: { type: "string" },
        trecho_novo: { type: "string" },
        motivo: { type: "string", description: "Explicação curta e simples para iniciantes" },
      },
      required: ["caminho", "trecho_original", "trecho_novo", "motivo"],
    },
  },
  {
    name: "propor_insercao",
    description: "Propõe inserir conteúdo antes de uma linha de um arquivo. A pessoa aceita ou rejeita.",
    parameters: {
      type: "object",
      properties: {
        caminho: { type: "string" },
        linha: { type: "integer" },
        conteudo: { type: "string" },
        motivo: { type: "string" },
      },
      required: ["caminho", "linha", "conteudo", "motivo"],
    },
  },
  {
    name: "propor_novo_arquivo",
    description: "Propõe criar um arquivo novo no projeto. A pessoa aceita ou rejeita.",
    parameters: {
      type: "object",
      properties: {
        caminho: { type: "string" },
        conteudo: { type: "string" },
        motivo: { type: "string" },
      },
      required: ["caminho", "conteudo", "motivo"],
    },
  },
];

export function ferramentas(nivel: NivelAgente) {
  if (nivel === "conversa") return [];
  return [{ functionDeclarations: nivel === "sugerir" ? [...LEITURA, ...ESCRITA] : LEITURA }];
}

export function instrucaoSistema(nivel: NivelAgente): string {
  const base =
    "Você e o Assistente LaTeX do ÉguaTeX, um editor LaTeX livre e offline. Responda em português do Brasil, " +
    "de forma direta e em linguagem simples para iniciantes. Siga a ABNT quando a pergunta envolver trabalhos " +
    "academicos brasileiros. Não invente pacotes, comandos ou opções de classe. Não use travessão.";
  if (nivel === "conversa") {
    return base + " Você não tem acesso aos arquivos do projeto; quando sugerir código, mostre um bloco LaTeX completo.";
  }
  if (nivel === "leitura") {
    return (
      base +
      " Use as ferramentas para ler o projeto antes de responder. Você não pode alterar arquivos: " +
      "quando sugerir uma mudança, mostre o trecho corrigido em um bloco de código e diga em que arquivo e linha ele vai."
    );
  }
  return (
    base +
    " Use as ferramentas para ler o projeto antes de responder. Para mudar o código, use propor_edicao, " +
    "propor_insercao ou propor_novo_arquivo: a pessoa vê a mudança e decide se aceita. Faca alterações minimas, " +
    "copie trecho_original exatamente como esta no arquivo (com poucas linhas, só o necessário) e explique o motivo. " +
    `No máximo ${MAX_PROPOSTAS} propostas por resposta. Nunca proponha apagar arquivos.`
  );
}

/** Contexto curto enviado junto com cada pergunta, para o Gemini nao precisar pedir o basico. */
export function contextoDaPergunta(projeto: ProjetoAgente | undefined, nivel: NivelAgente): string {
  if (!projeto || nivel === "conversa") return "";
  const partes: string[] = [];
  const atual = projeto.arquivoAtual();
  if (atual) partes.push(`Arquivo aberto no editor: ${atual}`);
  const sel = projeto.selecao();
  if (sel) partes.push(`Trecho selecionado (linhas ${sel.linhaInicio} a ${sel.linhaFim}):\n${sel.texto.slice(0, 4000)}`);
  const erros = projeto.erros().filter((e) => e.tipo !== "aviso");
  if (erros.length) partes.push(`Erros da última compilação:\n${resumirErros(erros)}`);
  return partes.length ? `\n\n[Contexto do ÉguaTeX]\n${partes.join("\n\n")}` : "";
}

function resumirErros(erros: ErroCompilacao[]): string {
  // erros primeiro: com dezenas de avisos de caixa, os 15 primeiros nao podem esconder o erro
  const ordenados = [...erros.filter((e) => e.tipo !== "aviso"), ...erros.filter((e) => e.tipo === "aviso")];
  return ordenados
    .slice(0, 15)
    .map((e) => `- ${e.arquivo ?? ""}${e.linha ? ` linha ${e.linha}` : ""}: ${e.mensagem}`)
    .join("\n");
}

function listar(nos: NoArquivo[], saida: string[] = []): string[] {
  for (const n of nos) {
    if (n.tipo === "pasta") listar(n.filhos ?? [], saida);
    else if (EXTENSOES.test(n.nome)) saida.push(n.caminho.replace(/^\/+/, ""));
  }
  return saida;
}

function numerar(texto: string, inicio = 1, fim?: number): string {
  const linhas = texto.split("\n");
  const ate = Math.min(fim ?? linhas.length, linhas.length, inicio + 399);
  const saida: string[] = [];
  for (let i = Math.max(1, inicio); i <= ate; i++) saida.push(`${i}: ${linhas[i - 1]}`);
  if (ate < linhas.length) saida.push(`... (arquivo tem ${linhas.length} linhas; peca as proximas com linha_inicio)`);
  return saida.join("\n");
}

function contar(texto: string, trecho: string): number {
  if (!trecho) return 0;
  let n = 0;
  let pos = texto.indexOf(trecho);
  while (pos >= 0) {
    n++;
    pos = texto.indexOf(trecho, pos + 1);
  }
  return n;
}

function limpar(caminho: unknown): string {
  return String(caminho ?? "").replace(/\\/g, "/").replace(/^\/+/, "");
}

let proximoId = 1;

/**
 * Executa uma chamada de ferramenta do Gemini e devolve o resultado em JSON.
 * Ferramentas de escrita so criam propostas (lista `propostas`); nada e gravado aqui.
 */
export async function executar(
  nome: string,
  args: Record<string, unknown>,
  projeto: ProjetoAgente,
  nivel: NivelAgente,
  propostas: Proposta[],
): Promise<Record<string, unknown>> {
  const escrita = nome.startsWith("propor_");
  if (nivel === "conversa" || (escrita && nivel !== "sugerir")) {
    return { erro: "Ferramenta não permitida no nível de acesso escolhido pela pessoa." };
  }
  try {
    switch (nome) {
      case "listar_arquivos":
        return { arquivos: listar(projeto.arvore()), aberto: projeto.arquivoAtual() };
      case "ler_arquivo": {
        const caminho = limpar(args.caminho);
        const texto = await projeto.lerArquivo(caminho);
        return { caminho, conteudo: numerar(texto, Number(args.linha_inicio) || 1, Number(args.linha_fim) || undefined) };
      }
      case "ler_selecao":
        return projeto.selecao() ?? { aviso: "Nada selecionado no editor." };
      case "ler_log_compilacao": {
        const erros = projeto.erros();
        if (!erros.length && !projeto.logTexto()) return { aviso: "Ainda não houve compilação." };
        const linhasLog = projeto
          .logTexto()
          .split("\n")
          .filter((l) => l.startsWith("!") || /^l\.\d+/.test(l) || /Warning/.test(l))
          .slice(0, 60);
        return { erros: resumirErros(erros) || "Sem erros.", trechos_do_log: linhasLog.join("\n") };
      }
      case "buscar": {
        const alvo = String(args.texto ?? "");
        if (!alvo) return { erro: "Informe o texto a procurar." };
        const achados: string[] = [];
        for (const caminho of listar(projeto.arvore())) {
          const linhas = (await projeto.lerArquivo(caminho)).split("\n");
          linhas.forEach((l, i) => {
            if (l.includes(alvo) && achados.length < 50) achados.push(`${caminho}:${i + 1}: ${l.trim()}`);
          });
        }
        return { ocorrencias: achados.length ? achados : "Nada encontrado." };
      }
    }

    if (propostas.length >= MAX_PROPOSTAS) {
      return { erro: `Limite de ${MAX_PROPOSTAS} propostas por resposta atingido.` };
    }
    const caminho = limpar(args.caminho);
    const motivo = String(args.motivo ?? "");

    if (nome === "propor_edicao") {
      const original = String(args.trecho_original ?? "");
      const novo = String(args.trecho_novo ?? "");
      const vezes = contar(await projeto.lerArquivo(caminho), original);
      if (vezes !== 1) {
        return {
          erro: vezes === 0
            ? "trecho_original não foi encontrado no arquivo. Releia o arquivo com ler_arquivo e copie o trecho exatamente."
            : `trecho_original aparece ${vezes} vezes. Inclua mais linhas vizinhas para ele ficar único.`,
        };
      }
      propostas.push({ id: proximoId++, tipo: "edicao", caminho, motivo, original, novo, linha: 0, estado: "pendente" });
      return { ok: "Proposta mostrada a pessoa, que vai aceitar ou rejeitar." };
    }
    if (nome === "propor_insercao") {
      await projeto.lerArquivo(caminho);
      propostas.push({
        id: proximoId++, tipo: "insercao", caminho, motivo, original: "",
        novo: String(args.conteudo ?? ""), linha: Number(args.linha) || 1, estado: "pendente",
      });
      return { ok: "Proposta mostrada a pessoa, que vai aceitar ou rejeitar." };
    }
    if (nome === "propor_novo_arquivo") {
      if (listar(projeto.arvore()).includes(caminho)) return { erro: "Esse arquivo já existe. Use propor_edicao." };
      propostas.push({
        id: proximoId++, tipo: "novo", caminho, motivo, original: "",
        novo: String(args.conteudo ?? ""), linha: 0, estado: "pendente",
      });
      return { ok: "Proposta mostrada a pessoa, que vai aceitar ou rejeitar." };
    }
    return { erro: `Ferramenta desconhecida: ${nome}` };
  } catch (e) {
    return { erro: String(e) };
  }
}

/** Aplica uma proposta aceita. O editor registra a troca como um passo do Ctrl+Z. */
export async function aplicarProposta(p: Proposta, projeto: ProjetoAgente): Promise<void> {
  if (p.tipo === "edicao") await projeto.aplicarEdicao(p.caminho, p.original, p.novo);
  else if (p.tipo === "insercao") await projeto.inserirNaLinha(p.caminho, p.linha, p.novo);
  else await projeto.criarArquivo(p.caminho, p.novo);
}

/** Texto curto que aparece no chat quando o Gemini usa uma ferramenta de leitura. */
export function descreverChamada(nome: string, args: Record<string, unknown>): string {
  switch (nome) {
    case "listar_arquivos": return "Viu a lista de arquivos";
    case "ler_arquivo": return `Leu ${limpar(args.caminho)}`;
    case "ler_selecao": return "Leu o trecho selecionado";
    case "ler_log_compilacao": return "Leu o log da compilação";
    case "buscar": return `Procurou "${String(args.texto ?? "")}"`;
    default: return "";
  }
}

// ---------------------------------------------------------------------------
// Historico enviado ao Gemini
// ---------------------------------------------------------------------------

/** Trocas completas (pergunta da pessoa ate a resposta final) mantidas no pedido. */
export const MAX_TROCAS = 8;
/** Teto do JSON enviado no campo `contents`, em caracteres. */
export const LIMITE_CORPO = 60_000;
/** Linhas de um ler_arquivo antigo que continuam no historico. */
const LINHAS_DE_ARQUIVO_ANTIGO = 20;

interface ConteudoHist {
  role: string;
  parts: Array<Record<string, unknown>>;
}

/** Vez da pessoa com texto (as respostas de ferramenta tambem tem role "user", sem texto). */
function iniciaTroca(c: ConteudoHist): boolean {
  return c.role === "user" && c.parts.some((p) => typeof p.text === "string");
}

function cortarArquivo(parte: Record<string, unknown>): Record<string, unknown> {
  const resp = parte.functionResponse as { name?: string; response?: Record<string, unknown> } | undefined;
  if (!resp || resp.name !== "ler_arquivo" || typeof resp.response?.conteudo !== "string") return parte;
  const linhas = resp.response.conteudo.split("\n");
  if (linhas.length <= LINHAS_DE_ARQUIVO_ANTIGO) return parte;
  const conteudo = linhas.slice(0, LINHAS_DE_ARQUIVO_ANTIGO).join("\n") + "\n[cortado]";
  return { ...parte, functionResponse: { ...resp, response: { ...resp.response, conteudo } } };
}

/**
 * Historico que vai no pedido: so as ultimas `maxTrocas` trocas completas, com o conteudo
 * de ler_arquivo das trocas antigas cortado nas primeiras linhas. A ultima troca fica
 * inteira (e a que a IA esta usando agora). Se ainda passar de `limite`, descarta as
 * trocas mais antigas. Nao altera o array recebido.
 */
export function prepararHistorico<T extends ConteudoHist>(historico: T[], maxTrocas = MAX_TROCAS, limite = LIMITE_CORPO): T[] {
  const inicios: number[] = [];
  historico.forEach((c, i) => {
    if (iniciaTroca(c)) inicios.push(i);
  });
  if (!inicios.length) return [...historico];
  let trocas = inicios.slice(-maxTrocas);
  const montar = (lista: number[]): T[] => {
    const corte = lista[0];
    const ultimaTroca = lista[lista.length - 1];
    return historico.slice(corte).map((c, k) =>
      corte + k >= ultimaTroca ? c : { ...c, parts: c.parts.map(cortarArquivo) },
    );
  };
  let saida = montar(trocas);
  while (trocas.length > 1 && JSON.stringify(saida).length > limite) {
    trocas = trocas.slice(1);
    saida = montar(trocas);
  }
  return saida;
}
