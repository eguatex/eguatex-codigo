// Compilacao no Android: o motor LaTeX em WebAssembly (busytex) roda num Web Worker.
// O Rust entrega os arquivos do projeto, o Worker compila e o PDF volta para o disco.
import { invoke } from '@tauri-apps/api/core';
import { lerConfigProjeto } from './config-projeto';
import { formatarTempos, type TemposWorker } from './tempos-motor';
import { aplicarProgressoRust } from './tarefas.svelte';

export const ehAndroid = typeof navigator !== 'undefined' && /Android/i.test(navigator.userAgent);

/** Endereco do protocolo busytex:// registrado no Rust. No Android vira http://busytex.localhost. */
const BASE = ehAndroid ? 'http://busytex.localhost/' : 'busytex://localhost/';

type ArquivoMotor = { caminho: string; texto?: string | null; base64?: string | null };
type ArquivoCache = { caminho: string; base64: string };
type MemoriaPacotes = { dados: string[]; extras: string[]; pgf: boolean };
type Resposta = {
  tipo: string;
  id?: number;
  pdf?: ArrayBuffer | null;
  log?: string;
  logFinal?: string;
  codigo?: number;
  texto?: string;
  tempos?: TemposWorker;
  /** So vem quando a compilacao deu certo. */
  auxiliares?: ArquivoCache[] | null;
  assinatura?: string | null;
  memoria?: MemoriaPacotes | null;
  /** Resposta do preaquecimento. */
  chave?: string;
  aceito?: boolean;
};

let worker: Worker | null = null;
let pronto: Promise<void> | null = null;
let proximoId = 1;
const pendentes = new Map<number, (r: Resposta) => void>();
/** Projetos (diretorio|principal) cujos auxiliares ja estao na memoria do Worker atual. */
const cacheQuente = new Set<string>();
/** Pacotes lembrados por projeto (diretorio|principal), lidos de .eguatex/cache-motor/pacotes.json. */
const memoriaPorChave = new Map<string, MemoriaPacotes | null>();
/** Projetos ja preaquecidos neste Worker (diretorio|principal). */
const preaquecidos = new Set<string>();
/** Preaquecimentos pedidos ao Worker que ainda nao responderam. */
const preaquecendo = new Set<string>();
/** Ultimo projeto usado: Limpar arquivos auxiliares apaga o cache em disco dele. */
let ultimoDiretorio = '';
let aoProgresso: ((texto: string) => void) | null = null;

/** Linhas "Preparing... (x/y)" e "Etapa x/y" do motor viram andamento da tarefa de compilar; todas seguem para o log. */
function repassarProgresso(texto: string): void {
  const m = /^Preparing\.\.\. \((\d+)\/(\d+)\)/.exec(texto) ?? /^Etapa (\d+)\/(\d+)/.exec(texto);
  if (m) aplicarProgressoRust({ id: 'compilar', feito: Number(m[1]), total: Number(m[2]) });
  aoProgresso?.(texto);
}

/** O Worker precisa nascer na mesma origem da pagina; o script vem do APK pelo protocolo. */
async function criarWorker(): Promise<Worker> {
  const codigo = await fetch(BASE + 'eguatex_worker.js').then((r) => {
    if (!r.ok) throw new Error('O motor LaTeX não foi encontrado no aplicativo.');
    return r.text();
  });
  const url = URL.createObjectURL(new Blob([codigo], { type: 'text/javascript' }));
  return new Worker(url);
}

function iniciar(): Promise<void> {
  if (pronto) return pronto;
  pronto = (async () => {
    worker = await criarWorker();
    await new Promise<void>((resolve, reject) => {
      worker!.onmessage = ({ data }: MessageEvent<Resposta>) => {
        if (data.tipo === 'pronto') resolve();
        else if (data.tipo === 'progresso') repassarProgresso(data.texto ?? '');
        else if (data.tipo === 'falha' && data.id === undefined) reject(new Error(data.texto));
        else if (data.id !== undefined) pendentes.get(data.id)?.(data);
      };
      worker!.onerror = (e) => reject(new Error(e.message || 'Falha ao iniciar o motor LaTeX'));
      worker!.postMessage({ tipo: 'iniciar', base: BASE });
    });
    worker!.onmessage = ({ data }: MessageEvent<Resposta>) => {
      if (data.tipo === 'progresso') repassarProgresso(data.texto ?? '');
      else if (data.tipo === 'preaquecido' && data.chave) {
        // so o Worker confirmando guarda a chave; recusa (estava compilando) deixa tentar de novo
        preaquecendo.delete(data.chave);
        if (data.aceito) preaquecidos.add(data.chave);
      } else if (data.id !== undefined) {
        pendentes.get(data.id)?.(data);
        pendentes.delete(data.id);
      }
    };
    // o Worker morreu (falta de memoria, por exemplo): as compilacoes esperando terminam com erro
    // e o Worker e descartado, senao a proxima compilacao espera o LIMITE_MS num Worker morto
    const morreu = (e: Event) =>
      reiniciarMotor(`O motor LaTeX parou: ${(e as ErrorEvent).message || 'falta de memória no aparelho'}`);
    worker!.onerror = morreu;
    worker!.onmessageerror = morreu;
  })().catch((e) => {
    pronto = null;
    worker?.terminate();
    worker = null;
    throw e;
  });
  return pronto;
}

/** Termina com erro todas as compilacoes que esperam resposta do Worker. */
function encerrarPendentes(texto: string) {
  for (const [id, responder] of [...pendentes]) responder({ tipo: 'falha', id, texto });
  pendentes.clear();
}

/** Descarta o motor depois de uma falha interna; a proxima compilacao cria outro. */
function reiniciarMotor(motivo = 'O motor LaTeX foi reiniciado.') {
  worker?.terminate();
  worker = null;
  pronto = null;
  // o Worker novo nasce sem auxiliares na memoria e sem pacotes preaquecidos
  cacheQuente.clear();
  preaquecidos.clear();
  preaquecendo.clear();
  encerrarPendentes(motivo);
}

/** Botao Parar no celular: nao ha processo para matar, entao o Worker inteiro e descartado. */
export function pararWeb() {
  paradas++;
  reiniciarMotor('Compilação interrompida.');
}

/** Conta os toques em Parar: a compilacao que comecou antes do toque nao segue para outro motor. */
let paradas = 0;

/** Limpar arquivos auxiliares: o Worker esquece os auxiliares guardados em memoria (sem Worker, nao ha cache). */
export function limparCacheMotor() {
  worker?.postMessage({ tipo: 'limpar-cache' });
  // o cache em disco tambem sai, senao a proxima compilacao o devolveria ao Worker
  cacheQuente.clear();
  // a lista de pacotes lembrada tambem sai, na memoria e no disco (cmd_motor_cache_apagar apaga os dois)
  memoriaPorChave.clear();
  if (ehAndroid && ultimoDiretorio) invoke('cmd_motor_cache_apagar', { diretorio: ultimoDiretorio, pacotes: true }).catch(() => {});
}

/** Tempo maximo de uma compilacao no celular. */
// 10 minutos: o TU Delft (beamer, XeLaTeX, biblioteca de fontes) e o rpgtex passavam de 4 minutos no
// emulador na primeira compilacao, sem laco nenhum; a segunda usa o cache e sai bem antes
const LIMITE_MS = 10 * 60 * 1000;

/** Carrega o motor em segundo plano, para a primeira compilacao sair mais rapido. */
export function preaquecer() {
  if (ehAndroid) iniciar().catch(() => {});
}

/**
 * Preaquece o motor com os pacotes do projeto aberto (classe, \usepackage, TikZ, beamer, abnTeX2),
 * usando a lista lembrada em .eguatex/cache-motor/pacotes.json quando ela existe. Repete na troca
 * de projeto; o Worker so recarrega o modulo se faltar algum pacote.
 */
export async function preaquecerProjeto(diretorio: string, arquivo: string): Promise<void> {
  if (!ehAndroid || !diretorio || !arquivo) return;
  const principal = arquivo.replace(/^\/+/, '');
  const chave = `${diretorio}|${principal}`;
  if (preaquecidos.has(chave) || preaquecendo.has(chave)) return;
  preaquecendo.add(chave);
  ultimoDiretorio = diretorio;
  try {
    const resumo: { texto: string | null; tem_cls_sty: boolean; pacotes: (MemoriaPacotes & { principal: string }) | null } = await invoke(
      'cmd_motor_resumo',
      { diretorio, principal },
    );
    memoriaPorChave.set(chave, resumo.pacotes);
    await iniciar();
    worker?.postMessage({ tipo: 'preaquecer', chave, principal, texto: resumo.texto, temClsSty: resumo.tem_cls_sty, memoria: resumo.pacotes });
  } catch {
    preaquecendo.delete(chave);
  }
}

export type ResultadoWeb = {
  sucesso: boolean;
  pdf: string | null;
  log: string;
  erros: Array<{ linha?: number | null; arquivo?: string | null; mensagem: string; tipo: string }>;
  motor_usado: string;
  /** Motor para lembrar ("xelatex@impressao") quando outro motor que nao o detectado gerou o PDF; null esquece. */
  motor_lembrar: string | null;
  tempo_ms: number;
};

function paraBase64(bytes: Uint8Array): string {
  let texto = '';
  for (let i = 0; i < bytes.length; i += 0x8000) texto += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(texto);
}

function juntarCaminho(dir: string, rel: string) {
  return `${dir.replace(/\/+$/, '')}/${rel.replace(/^\/+/, '')}`;
}

/** Resposta do cmd_detectar_motor: motor detectado, motivo, se o projeto o fixa e a impressao do documento. */
type MotorDoProjeto = { motor: string; motivo: string; fixo: boolean; impressao: string };

/**
 * Motor lembrado ("xelatex@impressao", o mesmo formato do Windows) que ainda vale: a impressao do
 * documento (preambulo, pacotes, estilos) e a mesma. Formato antigo, motor desconhecido ou impressao
 * diferente: null. LuaLaTeX vira XeLaTeX (nao roda no celular).
 */
export function motorDoLembrado(lembrado: string | null, impressao: string): string | null {
  if (!lembrado || !impressao) return null;
  const i = lembrado.lastIndexOf('@');
  if (i <= 0 || lembrado.slice(i + 1) !== impressao) return null;
  const motor = lembrado.slice(0, i);
  if (motor === 'lualatex') return 'xelatex';
  return motor === 'xelatex' || motor === 'pdflatex' ? motor : null;
}

/** Nome do motor como aparece na interface e no log. */
export function nomeDoMotor(motor: string): string {
  return motor === 'xelatex' ? 'XeLaTeX' : motor === 'lualatex' ? 'LuaLaTeX' : 'pdfLaTeX';
}

/**
 * O LuaLaTeX do busytex (luahbtex em WebAssembly) para com "memory access out of bounds" ao
 * carregar qualquer formato, ate o de um documento vazio (pilha de 64 KB do WebAssembly). Por
 * isso o celular compila com o XeLaTeX o que pede LuaLaTeX.
 */
export const AVISO_SEM_LUALATEX =
  'o LuaLaTeX não roda no motor do celular; o documento foi compilado com o XeLaTeX. ' +
  'Pacotes só do LuaLaTeX (luacode, \\directlua, luatexja) só funcionam no computador.';

/**
 * Motores tentados, nesta ordem: o escolhido sozinho (mesma regra do Windows) e, se ele terminar
 * sem PDF, o XeLaTeX. O LuaLaTeX entraria depois, mas nao roda no busytex (AVISO_SEM_LUALATEX).
 */
export function motoresDaCompilacao(detectado: string): string[] {
  const primeiro = detectado === 'lualatex' ? 'xelatex' : detectado === 'xelatex' ? 'xelatex' : 'pdflatex';
  return primeiro === 'xelatex' ? ['xelatex'] : ['pdflatex', 'xelatex'];
}

/**
 * Arquivos e fontes que o motor do celular nao tem, tirados do log, numa frase para o comeco do
 * log. O busytex e um TeX Live 2023 reduzido: um pacote que falta nele (e nos extras do app)
 * so compila no computador. `null` quando nada faltou.
 */
export function avisoDeFalta(log: string): string | null {
  const plano = log.replace(/\r?\n/g, '\n');
  const arquivos = new Set<string>();
  for (const m of plano.matchAll(/File `([^']+)' not found/g)) arquivos.add(m[1]);
  for (const m of plano.matchAll(/The font "([^"]+)" cannot be found/g)) arquivos.add(`fonte ${m[1]}`);
  for (const m of plano.matchAll(/Font ([^\s]+) at \d+ not found/g)) arquivos.add(`fonte ${m[1]}`);
  for (const m of plano.matchAll(/Package babel Error: Unknown option '([^']+)'/g)) arquivos.add(`idioma ${m[1]} do babel`);
  for (const m of plano.matchAll(/Package biblatex Error: Style '([^']+)' not found/g)) arquivos.add(`estilo ${m[1]} do biblatex`);
  if (arquivos.size === 0) return null;
  const lista = [...arquivos].slice(0, 8).join(', ');
  return `o motor do celular não tem: ${lista}. Ele traz um TeX Live 2023 reduzido; sem esses arquivos o documento só compila completo no computador.`;
}

/**
 * Compila como o cmd_compilar_completo da versao Windows e devolve o mesmo formato. O motor e
 * sempre o automatico (o celular nao tem seletor): `_motorPreferido` fica so pela assinatura.
 */
export async function compilarWeb(
  diretorio: string,
  arquivo: string,
  _motorPreferido: string,
  pararNoErro: boolean,
  progresso?: (texto: string) => void,
  motorLembrado?: string | null,
): Promise<ResultadoWeb> {
  const inicio = Date.now();
  const relogio0 = performance.now();
  const desde = (t0: number) => performance.now() - t0;
  let envio = 0;
  aoProgresso = progresso ?? null;
  const paradasNoInicio = paradas;
  progresso?.('Preparando o motor LaTeX (a primeira vez demora mais)...');
  await iniciar();
  const tIniciar = desde(relogio0);
  const tLer0 = performance.now();
  const principal = arquivo.replace(/^\/+/, '');
  const chave = `${diretorio}|${principal}`;
  ultimoDiretorio = diretorio;
  const lidos: ArquivoMotor[] = await invoke('cmd_motor_arquivos', { diretorio });
  const tLer = desde(tLer0);
  // auxiliares e pacotes da compilacao anterior, gravados em disco: valem na primeira compilacao depois de abrir o app
  const tCache0 = performance.now();
  const cache: { assinatura: string | null; arquivos: ArquivoCache[] } | null = cacheQuente.has(chave)
    ? null
    : await invoke<{ assinatura: string | null; arquivos: ArquivoCache[] }>('cmd_motor_cache_ler', { diretorio, principal }).catch(() => null);
  if (!memoriaPorChave.has(chave)) {
    const resumo = await invoke<{ pacotes: MemoriaPacotes | null }>('cmd_motor_resumo', { diretorio, principal }).catch(() => null);
    memoriaPorChave.set(chave, resumo?.pacotes ?? null);
  }
  const memoria = memoriaPorChave.get(chave) ?? null;
  const tCache = desde(tCache0);
  // latexmkrc, comentario % !TeX program e pastas texmf do proprio projeto
  const config = lerConfigProjeto(lidos, principal);
  const arquivos = config.arquivos;
  // motor automatico, pela regra do Windows (config_projeto.rs): % !TeX program, latexmkrc,
  // pacotes e comandos do principal, dos \input e das classes e pacotes locais
  const detectado = await invoke<MotorDoProjeto | null>('cmd_detectar_motor', { arquivo: principal, diretorio }).catch(() => null);
  const motorDetectado = detectado?.motor ?? config.motor ?? 'pdflatex';
  const motivo = detectado?.motivo ?? '';
  const fixo = detectado?.fixo ?? !!config.motor;
  const impressao = detectado?.impressao ?? '';
  const notasConfig = config.notas.filter((n) => !n.startsWith('Motor '));
  notasConfig.unshift(`Motor automático: ${nomeDoMotor(motorDetectado)}${motivo ? ` (${motivo})` : ''}.`);
  if (motorDetectado === 'lualatex') notasConfig.push(AVISO_SEM_LUALATEX);
  // o motor lembrado ("xelatex@impressao") roda primeiro enquanto o preambulo e os pacotes nao mudarem
  const lembrado = fixo ? null : motorDoLembrado(motorLembrado ?? null, impressao);
  if (motorLembrado && !fixo && !lembrado) notasConfig.push('o preâmbulo ou os pacotes mudaram; o motor lembrado foi esquecido e a detecção decide de novo.');
  const fila = lembrado && lembrado !== motoresDaCompilacao(motorDetectado)[0] ? [lembrado, ...motoresDaCompilacao(motorDetectado)] : motoresDaCompilacao(motorDetectado);
  const motorDaDeteccao = motoresDaCompilacao(motorDetectado)[0];
  let motorUsado = fila[0];
  // o celular nao tem biber: avisa em vez de deixar a bibliografia vazia
  const textos = arquivos.map((a) => a.texto ?? '').join('\n');
  if (/backend\s*=\s*biber/.test(textos) || (/\\usepackage(\[[^\]]*\])?\{biblatex\}/.test(textos) && !/backend\s*=\s*bibtex/.test(textos))) {
    notasConfig.push(
      'o celular não tem o biber. Para a bibliografia do biblatex aparecer, use \\usepackage[backend=bibtex]{biblatex} ou compile no computador.',
    );
  }
  // o makeindex do busytex monta o indice remissivo e o nomencl; o glossaries pede o
  // makeglossaries, que nao existe no celular
  if (/\\(printglossar|makeglossaries)/.test(textos)) {
    notasConfig.push('o celular não gera glossário (falta o makeglossaries). No computador ele sai normalmente.');
  }
  const rodar = () => {
    const id = proximoId++;
    return new Promise<Resposta>((resolve) => {
      if (!worker) {
        resolve({ tipo: 'falha', id, texto: 'O motor LaTeX não está pronto.' });
        return;
      }
      const relogio = setTimeout(
        () => reiniciarMotor(`A compilação passou de ${LIMITE_MS / 60000} minutos e foi interrompida. Procure um laço sem fim no documento.`),
        // limite total: soma as tentativas com outros motores e o reinicio do Worker
        Math.max(1000, LIMITE_MS - (Date.now() - inicio)),
      );
      pendentes.set(id, (r) => {
        clearTimeout(relogio);
        resolve(r);
      });
      const tEnvio0 = performance.now();
      worker.postMessage({ tipo: 'compilar', id, arquivos, principal, cache, memoria, motor: motorUsado, pararNoErro, diretorio });
      envio += desde(tEnvio0);
    });
  };
  // Parar antes do motor comecar (durante a leitura dos arquivos): termina como interrompida
  const paradasAntes = paradas;
  progresso?.('Compilando...');
  let aviso = '';
  /** Uma compilacao com o motor da vez; erro interno do WebAssembly recomeca o motor e tenta mais uma vez. */
  const rodarComReinicio = async (): Promise<Resposta> => {
    let r = await rodar();
    // Erro interno do WebAssembly ("memory access out of bounds") deixa o motor inutil
    if (r.tipo === 'falha' && /RuntimeError|out of bounds|unreachable/i.test(r.texto ?? '')) {
      reiniciarMotor();
      progresso?.('O motor LaTeX falhou. Tentando de novo...');
      await iniciar();
      aviso += `ÉguaTeX: o motor do celular falhou na primeira tentativa (${(r.texto ?? '').split('\n')[0]}) e foi reiniciado.\n`;
      r = await rodar();
    }
    return r;
  };
  const parou = () => paradas !== paradasAntes || paradas !== paradasNoInicio;
  let resposta: Resposta = parou() ? { tipo: 'falha', texto: 'Compilação interrompida.' } : await rodarComReinicio();
  // Sem PDF e com erro de motor (o mesmo erro_de_motor do Windows), os outros motores da fila tentam;
  // vale o primeiro que trouxer PDF. Projeto com motor fixo (% !TeX program, latexmkrc) nao tenta outro.
  const tentados = new Set([motorUsado]);
  const primeiraResposta = resposta;
  const primeiroMotor = motorUsado;
  while (!fixo) {
    const semPdf = resposta.tipo === 'falha' || !resposta.pdf || resposta.pdf.byteLength === 0;
    // Parar ou tempo esgotado (o Worker foi descartado) nao passam para outro motor
    if (!semPdf || parou() || (resposta.tipo === 'falha' && /interrompida|passou de/.test(resposta.texto ?? ''))) break;
    const logDaVez = resposta.logFinal || resposta.log || resposta.texto || '';
    const analise = await invoke<{ erro: boolean; motor_certo: string | null } | null>('cmd_erro_de_motor', { log: logDaVez, motor: motorUsado }).catch(() => null);
    // falha interna do WebAssembly sem log tambem conta: outro motor pode passar
    if (resposta.tipo !== 'falha' && !analise?.erro) break;
    const certo = analise?.motor_certo === 'lualatex' ? 'xelatex' : analise?.motor_certo;
    const proximo = [certo, motorDaDeteccao, ...fila, 'xelatex', 'pdflatex'].find((m): m is string => !!m && !tentados.has(m));
    if (!proximo) break;
    tentados.add(proximo);
    const anterior = resposta;
    const motorAnterior = motorUsado;
    motorUsado = proximo;
    progresso?.(`Sem PDF com o ${nomeDoMotor(motorAnterior)}. Tentando o ${nomeDoMotor(proximo)}...`);
    if (!worker) await iniciar();
    resposta = await rodarComReinicio();
    const trouxe = resposta.tipo !== 'falha' && !!resposta.pdf && resposta.pdf.byteLength > 0;
    if (trouxe) {
      notasConfig.push(`o ${nomeDoMotor(motorAnterior)} terminou sem PDF com erro de motor; compilado de novo com o ${nomeDoMotor(proximo)}.`);
      break;
    }
    resposta = anterior.tipo === 'falha' ? resposta : anterior;
    motorUsado = anterior.tipo === 'falha' ? proximo : motorAnterior;
  }
  if (tentados.size > 1 && (resposta.tipo === 'falha' || !resposta.pdf || resposta.pdf.byteLength === 0)) {
    // nenhum gerou PDF: fica o resultado do primeiro motor, com o erro original
    const outros = [...tentados].filter((m) => m !== primeiroMotor).map(nomeDoMotor).join(' e ');
    notasConfig.push(`tentado também com ${outros}, sem PDF; o log abaixo é o do ${nomeDoMotor(primeiroMotor)}.`);
    resposta = primeiraResposta;
    motorUsado = primeiroMotor;
  }
  const trocadas = resposta.tempos?.fontesTrocadas ?? [];
  if (trocadas.length > 0) {
    notasConfig.push(`fontes que não existem no celular foram trocadas na compilação (o arquivo não muda): ${trocadas.join(', ')}.`);
  }
  const notas = notasConfig.map((n) => `ÉguaTeX: ${n}\n`).join('');
  if (resposta.tipo === 'falha') {
    reiniciarMotor();
    // falha ou tempo esgotado do Worker: o auxiliar em disco pode ser o culpado, a proxima compilacao parte limpa
    cacheQuente.delete(chave);
    memoriaPorChave.delete(chave);
    await invoke('cmd_motor_cache_apagar', { diretorio, pacotes: true }).catch(() => {});
    throw new Error(resposta.texto);
  }

  // cache em disco: compilacao boa grava os auxiliares e a lista de pacotes; falha apaga o cache
  const tGravaCache0 = performance.now();
  if (resposta.auxiliares && resposta.auxiliares.length > 0) {
    await invoke('cmd_motor_cache_gravar', {
      diretorio,
      principal,
      assinatura: resposta.assinatura ?? null,
      arquivos: resposta.auxiliares,
    }).catch(() => {});
    cacheQuente.add(chave);
  } else {
    // erro de LaTeX do usuario (comum no celular): sai so o auxiliar; a lista de pacotes continua valendo
    await invoke('cmd_motor_cache_apagar', { diretorio, pacotes: false }).catch(() => {});
    cacheQuente.delete(chave);
  }
  if (resposta.memoria) {
    memoriaPorChave.set(chave, resposta.memoria);
    await invoke('cmd_motor_pacotes_gravar', { diretorio, principal, ...resposta.memoria }).catch(() => {});
  }
  const tGravaCache = desde(tGravaCache0);
  const logFinal = resposta.logFinal ?? '';
  const falta = avisoDeFalta(logFinal || resposta.log || '');
  const avisoFalta = falta ? `ÉguaTeX: ${falta}\n` : '';
  const base = principal.replace(/\.tex$/i, '');
  if (logFinal) await invoke('cmd_motor_gravar_texto', { caminho: juntarCaminho(diretorio, base + '.log'), conteudo: logFinal }).catch(() => {});
  let pdf: string | null = null;
  let tBase64 = 0;
  let tGravacao = 0;
  if (resposta.pdf && resposta.pdf.byteLength > 0) {
    pdf = juntarCaminho(diretorio, base + '.pdf');
    const tB0 = performance.now();
    const texto64 = paraBase64(new Uint8Array(resposta.pdf));
    tBase64 = desde(tB0);
    const tG0 = performance.now();
    await invoke('cmd_motor_gravar_base64', { caminho: pdf, base64: texto64 });
    tGravacao = desde(tG0);
  }
  const tErros0 = performance.now();
  const erros: ResultadoWeb['erros'] = await invoke('cmd_motor_erros', { log: logFinal, principal });
  const tErros = desde(tErros0);
  const graves = erros.filter((e) => e.tipo !== 'aviso');
  // so medicao: a linha vai no comeco do log e no logcat (adb logcat mostra o console.log)
  const linhaTempos = formatarTempos({
    total: desde(relogio0),
    iniciar: tIniciar,
    lerArquivos: tLer,
    lerCache: tCache,
    gravarCache: tGravaCache,
    envio,
    base64: tBase64,
    gravacao: tGravacao,
    erros: tErros,
    worker: resposta.tempos,
  });
  console.log(linhaTempos);
  return {
    sucesso: !!pdf && graves.length === 0,
    pdf,
    log: linhaTempos + '\n' + notas + avisoFalta + aviso + (logFinal || resposta.log || ''),
    erros,
    motor_usado: motorUsado,
    motor_lembrar: pdf && motorUsado !== motorDaDeteccao && impressao && !fixo ? `${motorUsado}@${impressao}` : null,
    tempo_ms: Date.now() - inicio,
  };
}
