// Linha de tempos da compilacao no celular: so mede e formata, nao muda a compilacao.
// Os numeros do worker vem de performance.now() em eguatex_worker.js e busytex_pipeline.js.

/** Marcas devolvidas pelo worker, todas em milissegundos. */
export type TemposWorker = {
  /** Passadas na ordem em que rodaram: [programa, ms]. */
  passadas?: Array<[string, number]>;
  /** Quantas vezes o compilador do busytex foi chamado (1 sem repeticao). */
  tentativas?: number;
  /** Quantas vezes o modulo WebAssembly foi recriado durante a compilacao. */
  recriacoes?: number;
  /** Desde o inicio da ultima tentativa ate o modulo estar pronto. */
  modulo?: number;
  /** Desde o inicio da ultima tentativa ate os arquivos do projeto estarem gravados. */
  arquivos?: number;
  /** Desde o inicio da ultima tentativa ate o PDF ser lido. */
  pdfLido?: number;
  /** Tempo da compilacao inteira dentro do worker. */
  compilar?: number;
  /** Recriacoes do modulo durante a compilacao: [pacotes de dados, carga ms, montagem ms]. */
  recargas?: Recarga[];
  /** Arranque do motor, desde o inicio do worker. */
  arranque?: { wasm?: number | null; modulo?: number | null; recargas?: Recarga[]; preaquecimento?: number | null };
  /** Auxiliares vindos do cache em disco (arquivos entregues ao motor); 0 quando a memoria ja os tinha. */
  cacheDisco?: number;
  /** Pacotes extras lembrados da compilacao anterior do projeto. */
  extrasLembrados?: number;
  /** Fontes que faltavam no celular e a Latin Modern que entrou no lugar ("Fira Code -> Latin Modern Mono"). */
  fontesTrocadas?: string[];
  /** Pacotes de dados que esta compilacao usou (nomes de arquivo). */
  pacotesDados?: string[];
};

/** Uma criacao do modulo: quantos pacotes de dados, ms para carregar wasm e pacotes, ms para montar. */
export type Recarga = [number, number, number];

export type TemposMotor = {
  /** Tempo de cmd_motor_arquivos. */
  lerArquivos?: number;
  /** Tempo do postMessage que envia o projeto ao worker. */
  envio?: number;
  /** Tempo de converter o PDF para base64. */
  base64?: number;
  /** Tempo de gravar o PDF no disco. */
  gravacao?: number;
  /** Tempo de ler o cache em disco e a lista de pacotes lembrados. */
  lerCache?: number;
  /** Tempo de gravar o cache em disco e a lista de pacotes. */
  gravarCache?: number;
  /** Tempo de cmd_motor_erros. */
  erros?: number;
  /** Tempo de iniciar o motor (criar o worker e esperar o modulo). */
  iniciar?: number;
  total: number;
  worker?: TemposWorker;
};

const ms = (n: number | null | undefined): string | null =>
  typeof n === 'number' && Number.isFinite(n) ? `${Math.round(n)} ms` : null;

/** Separa a carga (wasm e pacotes de dados) da montagem do modulo. */
function textoRecarga(rotulo: string, [pacotes, carga, montagem]: Recarga): string[] {
  return [`${rotulo} carga wasm e ${pacotes} pacotes ${Math.round(carga)} ms`, `${rotulo} montagem ${Math.round(montagem)} ms`];
}

/** Monta a linha `ÉguaTeX: tempos: ...`, em milissegundos, com o total primeiro. Partes sem valor ficam de fora. */
export function formatarTempos(t: TemposMotor): string {
  const w = t.worker ?? {};
  const partes: Array<string | null> = [];
  const par = (nome: string, valor: number | null | undefined) => {
    const v = ms(valor);
    partes.push(v ? `${nome} ${v}` : null);
  };
  par('total', t.total);
  par('iniciar motor', t.iniciar);
  par('wasm compilado', w.arranque?.wasm);
  par('modulo pronto', w.arranque?.modulo);
  for (const r of w.arranque?.recargas ?? []) partes.push(...textoRecarga('arranque', r));
  par('preaquecimento', w.arranque?.preaquecimento);
  par('ler arquivos', t.lerArquivos);
  par('ler cache em disco', t.lerCache);
  if (typeof w.cacheDisco === 'number') partes.push(`auxiliares do disco ${w.cacheDisco}`);
  if (w.extrasLembrados) partes.push(`pacotes extras lembrados ${w.extrasLembrados}`);
  par('envio ao worker', t.envio);
  par('modulo da compilacao', w.modulo);
  par('arquivos gravados', w.arquivos);
  (w.recargas ?? []).forEach((r, i) => partes.push(...textoRecarga(`recriacao ${i + 1}`, r)));
  for (const [nome, dur] of w.passadas ?? []) par(`passada ${nome}`, dur);
  par('PDF lido', w.pdfLido);
  par('worker', w.compilar);
  if (typeof w.recriacoes === 'number') partes.push(`modulo recriado ${w.recriacoes}x`);
  if (typeof w.tentativas === 'number') partes.push(`tentativas ${w.tentativas}`);
  if (w.pacotesDados?.length) partes.push(`pacotes de dados ${w.pacotesDados.length}`);
  par('base64 do PDF', t.base64);
  par('gravar PDF', t.gravacao);
  par('gravar cache em disco', t.gravarCache);
  par('erros', t.erros);
  return `ÉguaTeX: tempos: ${partes.filter((p): p is string => p !== null).join(', ')}`;
}
