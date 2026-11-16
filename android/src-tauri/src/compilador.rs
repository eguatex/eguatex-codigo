use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoCompilacao {
    pub sucesso: bool,
    pub pdf: Option<String>,
    pub log: String,
    pub erros: Vec<ErroCompilacao>,
    pub motor_usado: String,
    pub tempo_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErroCompilacao {
    pub linha: Option<u32>,
    pub arquivo: Option<String>,
    pub mensagem: String,
    pub tipo: String,
    /// So nos avisos agrupados no painel ("caixa-larga", "rotulo-repetido"...). Vazio nos demais.
    #[serde(default)]
    pub categoria: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorDetectado {
    pub nome: String,
    pub caminho: String,
    pub versao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoMotores {
    pub distribuicao: Option<String>,
    pub motores: Vec<MotorDetectado>,
}

/// Pasta do TinyTeX que vem junto com o instalador, relativa aos recursos do app.
const TEX_EMBUTIDO: &str = "tex/TinyTeX/bin/windows";

/// Coloca o TinyTeX embutido no fim do PATH. Um MiKTeX ou TeX Live ja instalado
/// continua com prioridade; o embutido so e usado quando nao ha outro.
/// Chamada uma vez no setup, antes de qualquer compilacao.
pub fn usar_tex_embutido(recursos: &Path) {
    // O instalador move o TinyTeX para %PROGRAMDATA%\EguaTex, pasta sem acento.
    // A pasta de recursos so vale no desenvolvimento.
    let mut candidatos: Vec<PathBuf> = Vec::new();
    if let Some(dados) = std::env::var_os("PROGRAMDATA") {
        candidatos.push(PathBuf::from(dados).join("EguaTex").join("TinyTeX").join("bin").join("windows"));
    }
    candidatos.push(recursos.join(TEX_EMBUTIDO));
    let Some(bin) = candidatos.into_iter().find(|b| b.join("pdflatex.exe").exists()) else {
        return;
    };
    let mut caminhos: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if caminhos.contains(&bin) {
        return;
    }
    caminhos.push(bin);
    if let Ok(novo) = std::env::join_paths(caminhos) {
        // SAFETY: roda no setup do Tauri, antes de criar as threads que leem o PATH.
        unsafe { std::env::set_var("PATH", novo) };
    }
}

fn detectar_executavel(nome: &str) -> Option<PathBuf> {
    which::which(nome).ok()
}

fn obter_versao(caminho: &Path) -> String {
    let mut comando = Command::new(caminho);
    comando.arg("--version").stdin(Stdio::null());
    ocultar_janela(&mut comando);
    comando
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

fn detectar_distribuicao() -> Option<String> {
    if detectar_executavel("miktex-pdflatex").is_some()
        || detectar_executavel("mpm").is_some()
    {
        return Some("MiKTeX".to_string());
    }
    if detectar_executavel("tlmgr").is_some() {
        return Some("TeX Live".to_string());
    }
    None
}

pub fn detectar_motores() -> InfoMotores {
    let nomes = ["pdflatex", "xelatex", "lualatex", "latexmk", "bibtex", "biber"];
    let mut motores = Vec::new();

    for nome in &nomes {
        if let Some(caminho) = detectar_executavel(nome) {
            let versao = obter_versao(&caminho);
            motores.push(MotorDetectado {
                nome: nome.to_string(),
                caminho: caminho.display().to_string(),
                versao,
            });
        }
    }

    InfoMotores {
        distribuicao: detectar_distribuicao(),
        motores,
    }
}

/// Frase que aparece no log quando o usuario interrompe a compilacao.
const MSG_INTERROMPIDA: &str = "Compilação interrompida pelo usuário.";

/// PID do processo filho em execucao (motor LaTeX ou bibtex/biber), se houver.
static PID_ATUAL: Mutex<Option<u32>> = Mutex::new(None);

/// Ligada por `cmd_parar_compilacao` quando mata o processo em execucao. A compilacao
/// le este valor depois que o processo termina, para saber que a saida foi cortada
/// de proposito e nao rodar as etapas seguintes.
static INTERROMPIDA: AtomicBool = AtomicBool::new(false);

/// Uma compilacao por vez. Os comandos de compilar sao assincronos e o frontend pode
/// pedir uma segunda enquanto a primeira roda; dois LaTeX na mesma pasta estragariam
/// os arquivos auxiliares um do outro (e so existe um PID guardado).
static UMA_POR_VEZ: Mutex<()> = Mutex::new(());

/// Nos testes, quem compila de verdade segura esta trava, para que um teste que
/// interrompe a compilacao nunca mate o processo de outro teste.
#[cfg(test)]
static TESTE_COMPILACAO: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn travar_teste() -> MutexGuard<'static, ()> {
    travar(&TESTE_COMPILACAO)
}

/// Pega a trava mesmo que outra thread tenha dado panic com ela na mao.
fn travar<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|erro| erro.into_inner())
}

fn foi_interrompida() -> bool {
    INTERROMPIDA.load(Ordering::SeqCst)
}

/// No Windows, impede que o processo filho abra uma janela de console.
fn ocultar_janela(comando: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        comando.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = comando;
}

/// Roda o comando e espera o fim, guardando o PID enquanto ele executa para que
/// `cmd_parar_compilacao` consiga interrompe-lo.
fn executar(comando: &mut Command) -> std::io::Result<Output> {
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    ocultar_janela(comando);
    let filho = comando.spawn()?;
    *travar(&PID_ATUAL) = Some(filho.id());
    let saida = filho.wait_with_output();
    *travar(&PID_ATUAL) = None;
    saida
}

/// Mata o processo e a arvore de filhos dele (o motor LaTeX chama outros programas).
fn matar_arvore(pid: u32) -> bool {
    let (programa, argumentos): (&str, Vec<String>) = if cfg!(windows) {
        (
            "taskkill",
            vec![
                "/PID".to_string(),
                pid.to_string(),
                "/T".to_string(),
                "/F".to_string(),
            ],
        )
    } else {
        ("kill", vec!["-9".to_string(), pid.to_string()])
    };
    let mut comando = Command::new(programa);
    comando
        .args(argumentos)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ocultar_janela(&mut comando);
    comando
        .status()
        .map(|estado| estado.success())
        .unwrap_or(false)
}

/// Interrompe o processo da compilacao em andamento (motor LaTeX ou bibtex/biber) e
/// cancela as etapas que viriam depois. Devolve true se havia um processo rodando e ele
/// foi interrompido; false se nao havia nada rodando.
#[tauri::command]
pub fn cmd_parar_compilacao() -> bool {
    // A trava do PID fica presa enquanto o kill roda: a compilacao so confere o sinal de
    // interrupcao depois de soltar o PID, entao nunca o ve pela metade.
    let guarda = travar(&PID_ATUAL);
    let Some(pid) = *guarda else {
        return false;
    };
    INTERROMPIDA.store(true, Ordering::SeqCst);
    let interrompeu = matar_arvore(pid);
    if !interrompeu {
        INTERROMPIDA.store(false, Ordering::SeqCst);
    }
    interrompeu
}

/// Resultado de uma compilacao interrompida: falha, sem PDF, sem erros do LaTeX e com a
/// frase combinada no log (uma vez so).
fn interrompida(mut resultado: ResultadoCompilacao) -> ResultadoCompilacao {
    resultado.sucesso = false;
    resultado.pdf = None;
    resultado.erros.clear();
    if !resultado.log.contains(MSG_INTERROMPIDA) {
        resultado.log.push_str(&format!("\n{}\n", MSG_INTERROMPIDA));
    }
    resultado
}

/// Compila uma vez. Com `parar_no_erro` o motor recebe `-halt-on-error` e para no
/// primeiro erro; sem ele continua apesar dos erros e ainda gera o PDF.
pub fn compilar(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    let _vez = travar(&UMA_POR_VEZ);
    INTERROMPIDA.store(false, Ordering::SeqCst);
    let (pasta, nome, sub) = separar_pasta(arquivo, diretorio);
    let resultado = compilar_uma_vez(&nome, &pasta, motor, parar_no_erro);
    com_subpasta(resultado, &sub)
}

/// O LaTeX procura .sty, .cls, .bib e imagens na pasta onde roda. Quando o documento
/// principal esta numa subpasta do projeto (ex.: template-latex/main.tex), a compilacao
/// roda dentro dela. Devolve (pasta onde rodar, nome do arquivo, subpasta relativa).
fn separar_pasta(arquivo: &str, diretorio: &str) -> (String, String, String) {
    let caminho = Path::new(arquivo);
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| arquivo.to_string());
    match caminho.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(sub) => (
            Path::new(diretorio).join(sub).to_string_lossy().to_string(),
            nome,
            sub.to_string_lossy().replace('\\', "/"),
        ),
        None => (diretorio.to_string(), nome, String::new()),
    }
}

/// Os erros saem com caminhos relativos a pasta onde o LaTeX rodou. Acrescenta a
/// subpasta para que o clique no erro abra o arquivo certo do projeto.
pub(crate) fn com_subpasta(mut resultado: ResultadoCompilacao, sub: &str) -> ResultadoCompilacao {
    if sub.is_empty() {
        return resultado;
    }
    for erro in &mut resultado.erros {
        if let Some(a) = &erro.arquivo {
            let limpo = a.trim_start_matches("./");
            if !Path::new(limpo).is_absolute() {
                erro.arquivo = Some(format!("./{sub}/{limpo}"));
            }
        }
    }
    resultado
}

fn compilar_uma_vez(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    let inicio = std::time::Instant::now();
    let dir = Path::new(diretorio);
    let arquivo_path = Path::new(arquivo);

    let nome_motor = match motor {
        "pdflatex" | "pdfLaTeX" => "pdflatex",
        "xelatex" | "XeLaTeX" => "xelatex",
        "lualatex" | "LuaLaTeX" => "lualatex",
        _ => "pdflatex",
    };

    let executavel = match detectar_executavel(nome_motor) {
        Some(p) => p,
        None => {
            return ResultadoCompilacao {
                sucesso: false,
                pdf: None,
                log: format!("Motor '{}' não encontrado no sistema. Instale o MiKTeX ou o TeX Live.", nome_motor),
                erros: vec![ErroCompilacao {
                    linha: None,
                    arquivo: None,
                    mensagem: format!("Motor '{}' não encontrado", nome_motor),
                    tipo: "fatal".to_string(),
                    categoria: None,
                }],
                motor_usado: nome_motor.to_string(),
                tempo_ms: inicio.elapsed().as_millis() as u64,
            };
        }
    };

    let mut comando = Command::new(&executavel);
    comando.current_dir(dir).arg("-interaction=nonstopmode");
    if parar_no_erro {
        comando.arg("-halt-on-error");
    }
    // O motor nunca roda programas externos (\write18): um projeto baixado nao executa comandos.
    // No Android o busytex nao tem shell; esta flag vale para a compilacao nativa (testes no computador).
    comando
        .arg("-no-shell-escape")
        .arg("-file-line-error")
        .arg("-synctex=1")
        .arg(arquivo_path);
    let resultado = executar(&mut comando);

    let tempo_ms = inicio.elapsed().as_millis() as u64;

    match resultado {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let log_completo = format!("{}\n{}", stdout, stderr);

            if foi_interrompida() {
                return interrompida(ResultadoCompilacao {
                    sucesso: false,
                    pdf: None,
                    log: log_completo,
                    erros: Vec::new(),
                    motor_usado: nome_motor.to_string(),
                    tempo_ms,
                });
            }

            let principal = arquivo_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let erros = extrair_erros(&log_completo, &principal);

            let nome_base = arquivo_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let pdf_path = dir.join(format!("{}.pdf", nome_base));
            let pdf = if pdf_path.exists() {
                Some(pdf_path.display().to_string())
            } else {
                None
            };

            ResultadoCompilacao {
                sucesso: output.status.success() && pdf.is_some(),
                pdf,
                log: log_completo,
                erros,
                motor_usado: nome_motor.to_string(),
                tempo_ms,
            }
        }
        Err(e) => ResultadoCompilacao {
            sucesso: false,
            pdf: None,
            log: format!("Falha ao executar {}: {}", nome_motor, e),
            erros: vec![ErroCompilacao {
                linha: None,
                arquivo: None,
                mensagem: e.to_string(),
                tipo: "fatal".to_string(),
                categoria: None,
            }],
            motor_usado: nome_motor.to_string(),
            tempo_ms,
        },
    }
}

/// Sufixos dos arquivos que o LaTeX grava numa passada e le na seguinte (referencias,
/// sumario, listas, bibliografia, paginas do backref). Se nenhum mudou numa passada, a
/// proxima daria o mesmo PDF.
const SUFIXOS_ENTRE_PASSADAS: &[&str] = &[
    ".aux", ".toc", ".lof", ".lot", ".out", ".brf", ".bbl", ".nav", ".snm", ".ind", ".loa",
    ".lol",
];

/// Maximo de passadas do motor numa compilacao completa (o mesmo limite do latexmk).
const MAX_PASSADAS: u32 = 5;

/// Hash do conteudo de cada arquivo auxiliar da pasta e das subpastas (sem pastas ocultas).
fn retrato_auxiliares(dir: &Path) -> BTreeMap<PathBuf, u64> {
    let mut retrato = BTreeMap::new();
    juntar_auxiliares(dir, 0, &mut retrato);
    retrato
}

fn juntar_auxiliares(pasta: &Path, nivel: u32, retrato: &mut BTreeMap<PathBuf, u64>) {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let Ok(tipo) = entrada.file_type() else {
            continue;
        };
        let nome = entrada.file_name().to_string_lossy().to_lowercase();
        if tipo.is_dir() {
            if nivel < 6 && !nome.starts_with('.') {
                juntar_auxiliares(&entrada.path(), nivel + 1, retrato);
            }
        } else if tipo.is_file() && SUFIXOS_ENTRE_PASSADAS.iter().any(|s| nome.ends_with(s)) {
            if let Ok(bytes) = std::fs::read(entrada.path()) {
                let mut hash = DefaultHasher::new();
                bytes.hash(&mut hash);
                retrato.insert(entrada.path(), hash.finish());
            }
        }
    }
}

/// Programa que monta a bibliografia: bibtex quando algum .aux tem \bibdata (o
/// \bibliography do documento), biber quando o documento usa biblatex com biber (marcas
/// \abx@aux no .aux principal e o .bcf ao lado), nenhum quando nao ha bibliografia.
fn ferramenta_bibliografia(
    dir: &Path,
    nome_base: &str,
    auxiliares: &BTreeMap<PathBuf, u64>,
) -> Option<&'static str> {
    let ler = |caminho: &Path| String::from_utf8_lossy(&std::fs::read(caminho).unwrap_or_default()).into_owned();
    let pede_bibtex = auxiliares
        .keys()
        .filter(|c| c.extension().is_some_and(|e| e.eq_ignore_ascii_case("aux")))
        .any(|c| ler(c).contains("\\bibdata"));
    if pede_bibtex {
        return Some("bibtex");
    }
    let usa_biblatex = ler(&dir.join(format!("{nome_base}.aux"))).contains("\\abx@aux");
    if usa_biblatex && dir.join(format!("{nome_base}.bcf")).is_file() {
        return Some("biber");
    }
    None
}

/// Roda bibtex ou biber na pasta do projeto e devolve a saida para o log.
fn rodar_bibliografia(programa: &str, dir: &Path, nome_base: &str) -> String {
    let Some(executavel) = detectar_executavel(programa) else {
        return format!("{programa} não encontrado no sistema: a bibliografia não foi montada.\n");
    };
    let mut comando = Command::new(&executavel);
    comando.current_dir(dir).arg(nome_base);
    match executar(&mut comando) {
        Ok(saida) => format!(
            "{}{}",
            String::from_utf8_lossy(&saida.stdout),
            String::from_utf8_lossy(&saida.stderr)
        ),
        Err(e) => format!("Falha ao executar {programa}: {e}\n"),
    }
}

/// Compilacao completa, como o latexmk: roda o motor, monta a bibliografia (bibtex ou
/// biber, se o documento tiver uma) e repete o motor enquanto os arquivos auxiliares
/// mudarem, ate MAX_PASSADAS. Documento sem mudanca compila uma vez so. Com `parar_no_erro`
/// desligado, o ciclo segue apesar dos erros sempre que sai PDF, para o sumario e as
/// citacoes ficarem certos. Se o usuario interromper em qualquer etapa, as seguintes nao
/// rodam.
pub fn compilar_com_bibtex(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    let _vez = travar(&UMA_POR_VEZ);
    INTERROMPIDA.store(false, Ordering::SeqCst);
    let (pasta, nome, sub) = separar_pasta(arquivo, diretorio);
    let resultado = compilar_ciclo(&nome, &pasta, motor, parar_no_erro);
    com_subpasta(resultado, &sub)
}

fn compilar_ciclo(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    let inicio = std::time::Instant::now();

    let dir = Path::new(diretorio);
    let nome_base = Path::new(arquivo)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut antes = retrato_auxiliares(dir);
    let mut resultado = compilar_uma_vez(arquivo, diretorio, motor, parar_no_erro);
    let mut log = resultado.log.clone();
    let mut passadas = 1;

    loop {
        if foi_interrompida() {
            resultado.log = log;
            return interrompida(resultado);
        }
        // sem PDF novo nao ha o que repetir: motor ausente, erro fatal ou parou no erro
        let sem_pdf_novo =
            resultado.pdf.is_none() || resultado.log.contains("no output PDF file produced");
        if sem_pdf_novo || (parar_no_erro && !resultado.sucesso) {
            break;
        }
        let mut depois = retrato_auxiliares(dir);
        if passadas == 1 {
            if let Some(programa) = ferramenta_bibliografia(dir, &nome_base, &depois) {
                log.push_str(&format!("\n--- {programa} ---\n"));
                log.push_str(&rodar_bibliografia(programa, dir, &nome_base));
                if foi_interrompida() {
                    resultado.log = log;
                    return interrompida(resultado);
                }
                depois = retrato_auxiliares(dir);
            }
        }
        if depois == antes || passadas >= MAX_PASSADAS {
            break;
        }
        antes = depois;
        resultado = compilar_uma_vez(arquivo, diretorio, motor, parar_no_erro);
        passadas += 1;
        log.push_str(&format!("\n--- Passada {passadas} ---\n"));
        log.push_str(&resultado.log);
    }

    resultado.log = log;
    resultado.tempo_ms = inicio.elapsed().as_millis() as u64;
    resultado
}

/// Avisos que o painel agrupa e recolhe (campo `categoria` do `ErroCompilacao`).
const CATEGORIA_CAIXA_LARGA: &str = "caixa-larga";
const CATEGORIA_CAIXA_FROUXA: &str = "caixa-frouxa";
const CATEGORIA_CAIXA_VERTICAL: &str = "caixa-vertical";
const CATEGORIA_ROTULO_REPETIDO: &str = "rotulo-repetido";

/// Aviso de caixa ou de rotulo repetido lido de uma linha do log.
struct AvisoLido {
    categoria: &'static str,
    linha: Option<u32>,
}

/// Reconhece `Overfull \hbox (Xpt too wide) ... at lines A--B`, `Underfull \hbox (badness N) ...`,
/// as mesmas com `\vbox` e `detected at line N`, e `LaTeX Warning: Label `x' multiply defined`.
fn aviso_de_caixa_ou_rotulo(linha: &str) -> Option<AvisoLido> {
    let categoria = if linha.starts_with("Overfull \\hbox (") {
        CATEGORIA_CAIXA_LARGA
    } else if linha.starts_with("Underfull \\hbox (") {
        CATEGORIA_CAIXA_FROUXA
    } else if linha.starts_with("Overfull \\vbox (") || linha.starts_with("Underfull \\vbox (") {
        CATEGORIA_CAIXA_VERTICAL
    } else if linha.starts_with("LaTeX Warning: Label `") && linha.contains("multiply defined") {
        // a linha chega inteira: `juntar_linhas_quebradas` ja uniu o nome longo quebrado em 79 colunas
        return Some(AvisoLido { categoria: CATEGORIA_ROTULO_REPETIDO, linha: None });
    } else {
        return None;
    };
    Some(AvisoLido { categoria, linha: primeira_linha_citada(linha) })
}

/// `... at lines 12--14` e `... detected at line 7` dao 12 e 7. Sem numero, `None`.
fn primeira_linha_citada(texto: &str) -> Option<u32> {
    let (_, resto) = texto.split_once(" at lines ").or_else(|| texto.split_once(" at line "))?;
    let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();
    digitos.parse().ok()
}

/// Acompanha quais arquivos o LaTeX tem abertos enquanto o log e lido: cada `(./x.tex` abre
/// um arquivo e o `)` correspondente fecha. Parenteses que nao abrem arquivo (`(12pt too wide)`)
/// entram como `None` so para o `)` fechar o par certo.
#[derive(Default)]
struct RastroDeArquivos {
    abertos: Vec<Option<String>>,
    /// A linha seguinte ao cabecalho de uma caixa horizontal e o texto da caixa (do documento,
    /// com parenteses soltos): nao conta parenteses. Vale para o log em arquivo, que tem ` []` e
    /// uma linha em branco depois, e para o do console, que segue direto para o proximo `)`.
    pular_conteudo: bool,
}

/// Largura em que o TeX quebra as linhas do log (`max_print_line`), em bytes.
const COLUNAS_DO_LOG: usize = 79;

/// O TeX quebra o log em 79 colunas no meio de nomes de arquivo, de rotulos e de textos.
/// Uma linha com exatamente 79 bytes continua na seguinte: as duas viram uma so.
fn juntar_linhas_quebradas(log: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual: Option<String> = None;
    for linha in log.lines() {
        let mut texto = atual.take().unwrap_or_default();
        texto.push_str(linha);
        if linha.len() == COLUNAS_DO_LOG {
            atual = Some(texto);
        } else {
            saida.push(texto);
        }
    }
    saida.extend(atual);
    saida
}

/// Cabecalho do relatorio de caixa horizontal: Overfull, Underfull, Loose ou Tight seguido de
/// `\hbox (`. O TeX imprime o texto da caixa numa linha so (quebrada em 79 colunas,
/// que `juntar_linhas_quebradas` ja juntou) logo depois do cabecalho.
fn cabecalho_de_caixa_horizontal(linha: &str) -> bool {
    ["Overfull", "Underfull", "Loose", "Tight"]
        .iter()
        .any(|p| linha.strip_prefix(p).is_some_and(|r| r.starts_with(" \\hbox (")))
}

/// Nome do arquivo logo depois de um `(`. Com aspas, vai ate a aspa de fechamento. Sem aspas,
/// vai ate o proximo parentese e pode ter espacos (`./pasta com espaco/capitulo tres.tex`) se
/// o trecho todo comecar como caminho e terminar em extensao; senao vale a primeira palavra.
/// Devolve o nome e quantos bytes de `resto` ele ocupa.
fn nome_apos_parentese(resto: &str) -> (&str, usize) {
    if let Some(sem_aspa) = resto.strip_prefix('"') {
        if let Some(fim) = sem_aspa.find('"') {
            return (&sem_aspa[..fim], fim + 2);
        }
    }
    let ate_parentese = resto.find(['(', ')']).unwrap_or(resto.len());
    let trecho = resto[..ate_parentese].trim_end();
    let parece_caminho = trecho.starts_with("./") || trecho.starts_with("../") || trecho.starts_with('/') || trecho.as_bytes().get(1) == Some(&b':');
    if parece_caminho && trecho.contains(' ') && tem_extensao(trecho) {
        return (trecho, trecho.len());
    }
    let fim = resto.find(|c: char| c.is_whitespace() || c == '(' || c == ')').unwrap_or(resto.len());
    (&resto[..fim], fim)
}

impl RastroDeArquivos {
    fn ler_linha(&mut self, linha: &str) {
        if self.pular_conteudo {
            self.pular_conteudo = false;
            return;
        }
        if cabecalho_de_caixa_horizontal(linha) {
            self.pular_conteudo = true;
            return;
        }
        // contexto de erro e os proprios avisos trazem parenteses soltos do texto do documento
        const PULAR: [&str; 7] = ["[]", "\\", "!", "l.", "<", "Overfull", "Underfull"];
        if PULAR.iter().any(|p| linha.starts_with(p)) {
            return;
        }
        let mut resto = linha;
        while let Some(pos) = resto.find(['(', ')']) {
            let fecha = resto.as_bytes()[pos] == b')';
            resto = &resto[pos + 1..];
            if fecha {
                self.abertos.pop();
                continue;
            }
            let (token, usados) = nome_apos_parentese(resto);
            self.abertos.push(tem_extensao(token).then(|| token.to_string()));
            resto = &resto[usados..];
        }
    }

    /// O arquivo aberto mais recente, se for um `.tex` do projeto. Se o aviso nasceu num `.aux`,
    /// `.toc`, `.bbl` ou classe, os numeros de linha nao valem para o texto do usuario: `None`.
    fn arquivo_tex_aberto(&self) -> Option<&str> {
        self.abertos
            .iter()
            .rev()
            .find_map(|a| a.as_deref())
            .filter(|a| a.to_ascii_lowercase().ends_with(".tex"))
    }
}

/// O nome (sem a pasta) termina em `.ext` de 1 a 5 letras ou digitos.
fn tem_extensao(token: &str) -> bool {
    let nome = token.rsplit(['/', '\\']).next().unwrap_or(token);
    nome.rsplit_once('.')
        .is_some_and(|(base, ext)| !base.is_empty() && (1..=5).contains(&ext.len()) && ext.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// `principal` e o nome do documento principal: onde caem os avisos sem arquivo `.tex` aberto.
pub(crate) fn extrair_erros(log: &str, principal: &str) -> Vec<ErroCompilacao> {
    let mut erros = Vec::new();
    let mut rastro = RastroDeArquivos::default();

    for line in juntar_linhas_quebradas(log).iter().map(String::as_str) {
        if let Some(resto) = line.strip_prefix('!') {
            erros.push(ErroCompilacao {
                linha: None,
                arquivo: None,
                mensagem: resto.trim().to_string(),
                tipo: "erro".to_string(),
                categoria: None,
            });
        } else if let Some((arquivo, num, msg)) = arquivo_linha_mensagem(line) {
            erros.push(ErroCompilacao {
                linha: Some(num),
                arquivo: Some(arquivo.to_string()),
                mensagem: msg.to_string(),
                tipo: if msg.to_lowercase().contains("warning") {
                    "aviso".to_string()
                } else {
                    "erro".to_string()
                },
                categoria: None,
            });
        } else if let Some(aviso) = aviso_de_caixa_ou_rotulo(line) {
            let (arquivo, linha) = match rastro.arquivo_tex_aberto() {
                Some(a) => (Some(a.to_string()), aviso.linha),
                None if principal.is_empty() => (None, None),
                None => (Some(format!("./{principal}")), None),
            };
            erros.push(ErroCompilacao {
                linha,
                arquivo,
                mensagem: line.trim().to_string(),
                tipo: "aviso".to_string(),
                categoria: Some(aviso.categoria.to_string()),
            });
        }
        rastro.ler_linha(line);
    }

    erros
}

/// Separa `arquivo:linha: mensagem` (formato do -file-line-error). O arquivo pode ter
/// ":" na letra da unidade (`C:/ProgramData/.../x.cls:12: ...`): vale o primeiro ":"
/// seguido de numero e ":" cujo trecho anterior termine num arquivo com extensao.
fn arquivo_linha_mensagem(linha: &str) -> Option<(&str, u32, &str)> {
    for (pos, _) in linha.match_indices(':') {
        let antes = &linha[..pos];
        let depois = &linha[pos + 1..];
        let digitos = depois.bytes().take_while(u8::is_ascii_digit).count();
        if digitos == 0 || !depois[digitos..].starts_with(':') {
            continue;
        }
        let nome = antes.rsplit(['/', '\\']).next().unwrap_or(antes);
        let tem_extensao = nome.rsplit_once('.').is_some_and(|(base, ext)| {
            !base.is_empty() && !ext.is_empty() && ext.len() <= 8 && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        });
        if !tem_extensao {
            continue;
        }
        let msg = depois[digitos + 1..].trim();
        let num = depois[..digitos].parse().ok()?;
        return (!msg.is_empty()).then_some((antes, num, msg));
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Log de exemplo: main.tex abre cap1.tex, a classe e o .aux; os avisos de caixa vem de arquivos diferentes.
    const LOG_AVISOS: &str = "This is pdfTeX, Version 3.141592653-2.6-1.40.27 (TeX Live 2026)
(./main.tex
LaTeX2e <2024-11-01>
(/usr/share/texlive/article.cls
Document Class: article 2024/06/29 v1.4n Standard LaTeX document class
(/usr/share/texlive/size10.clo))
(./main.aux)
(./cap1.tex
Overfull \\hbox (12.5pt too wide) in paragraph at lines 12--14
[]\\T1/cmr/m/n/10 palavra (longa
 []

Underfull \\hbox (badness 10000) in paragraph at lines 20--21

Underfull \\vbox (badness 10000) detected at line 30
)
Overfull \\hbox (3.0pt too wide) in alignment at lines 40--41
LaTeX Warning: Label `eq:a' multiply defined.
./main.tex:50: Undefined control sequence.
! Undefined control sequence.
";

    fn so_erros(erros: &[ErroCompilacao]) -> Vec<&ErroCompilacao> {
        erros.iter().filter(|e| e.tipo != "aviso").collect()
    }

    #[test]
    fn avisos_de_caixa_e_rotulo_saem_com_arquivo_linha_e_categoria() {
        let erros = extrair_erros(LOG_AVISOS, "main.tex");
        let avisos: Vec<_> = erros.iter().filter(|e| e.categoria.is_some()).collect();
        assert_eq!(avisos.len(), 5, "{erros:?}");
        assert!(avisos.iter().all(|e| e.tipo == "aviso"), "{avisos:?}");

        // Overfull \hbox ... at lines 12--14 dentro de cap1.tex
        assert_eq!(avisos[0].categoria.as_deref(), Some("caixa-larga"));
        assert_eq!(avisos[0].arquivo.as_deref(), Some("./cap1.tex"));
        assert_eq!(avisos[0].linha, Some(12));
        assert_eq!(avisos[0].mensagem, "Overfull \\hbox (12.5pt too wide) in paragraph at lines 12--14");
        // Underfull \hbox
        assert_eq!(avisos[1].categoria.as_deref(), Some("caixa-frouxa"));
        assert_eq!(avisos[1].linha, Some(20));
        // Underfull \vbox ... detected at line 30
        assert_eq!(avisos[2].categoria.as_deref(), Some("caixa-vertical"));
        assert_eq!(avisos[2].arquivo.as_deref(), Some("./cap1.tex"));
        assert_eq!(avisos[2].linha, Some(30));
        // depois do ")" que fecha cap1.tex, volta a valer o main.tex
        assert_eq!(avisos[3].categoria.as_deref(), Some("caixa-larga"));
        assert_eq!(avisos[3].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(avisos[3].linha, Some(40));
        // rotulo repetido: sem linha, no documento principal
        assert_eq!(avisos[4].categoria.as_deref(), Some("rotulo-repetido"));
        assert_eq!(avisos[4].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(avisos[4].linha, None);
    }

    #[test]
    fn avisos_de_caixa_nao_mudam_a_contagem_de_erros() {
        let com = extrair_erros(LOG_AVISOS, "main.tex");
        let sem_avisos: String = LOG_AVISOS
            .lines()
            .filter(|l| !l.starts_with("Overfull") && !l.starts_with("Underfull") && !l.contains("multiply defined"))
            .map(|l| format!("{l}\n"))
            .collect();
        let sem = extrair_erros(&sem_avisos, "main.tex");
        assert_eq!(so_erros(&com).len(), 2, "{com:?}");
        assert_eq!(so_erros(&com).len(), so_erros(&sem).len());
        assert!(so_erros(&com).iter().all(|e| e.categoria.is_none()));
    }

    #[test]
    fn aviso_em_aux_ou_sem_arquivo_aberto_cai_no_documento_principal() {
        // aberto so o .aux (nao e texto do usuario): arquivo = principal, sem linha
        let no_aux = "(./main.aux\nLaTeX Warning: Label `x' multiply defined.\n)\n";
        let e = extrair_erros(no_aux, "tese.tex");
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].arquivo.as_deref(), Some("./tese.tex"));
        assert_eq!(e[0].linha, None);
        // nenhum arquivo aberto no log
        let solto = "Overfull \\hbox (1.0pt too wide) in paragraph at lines 3--4\n";
        let e = extrair_erros(solto, "tese.tex");
        assert_eq!(e[0].arquivo.as_deref(), Some("./tese.tex"));
        assert_eq!(e[0].linha, None);
        assert_eq!(e[0].categoria.as_deref(), Some("caixa-larga"));
    }

    #[test]
    fn parenteses_do_texto_nao_desalinham_os_arquivos() {
        let log = "(./main.tex\nTexto com (parenteses) e (outro\n(./a.tex\n)\nOverfull \\hbox (2.0pt too wide) in paragraph at lines 8--9\n)\n";
        let e = extrair_erros(log, "main.tex");
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(e[0].linha, Some(8));
    }

    #[test]
    fn linha_citada_aceita_lines_line_e_ausencia() {
        assert_eq!(primeira_linha_citada("Overfull \\hbox (1pt too wide) in paragraph at lines 12--14"), Some(12));
        assert_eq!(primeira_linha_citada("Underfull \\hbox (badness 5) in paragraph at line 7"), Some(7));
        assert_eq!(primeira_linha_citada("Overfull \\vbox (2pt too high) detected at line 99"), Some(99));
        assert_eq!(primeira_linha_citada("Overfull \\vbox (2pt too high) has occurred while \\output is active"), None);
    }

    #[test]
    fn categoria_e_opcional_ao_ler_json_antigo() {
        let json = r#"{"linha":3,"arquivo":"./a.tex","mensagem":"m","tipo":"erro"}"#;
        let e: ErroCompilacao = serde_json::from_str(json).expect("json sem categoria");
        assert_eq!(e.categoria, None);
    }

    #[test]
    fn erro_com_caminho_de_unidade_entra_na_lista() {
        let log = "C:/ProgramData/EguaTex/TinyTeX/texmf-dist/tex/latex/x/x.cls:123: LaTeX Error: File `y.sty' not found.\n./main.tex:7: Undefined control sequence.\nlinha comum: sem numero\n";
        let erros = extrair_erros(log, "main.tex");
        assert_eq!(erros.len(), 2, "{erros:?}");
        assert_eq!(erros[0].arquivo.as_deref(), Some("C:/ProgramData/EguaTex/TinyTeX/texmf-dist/tex/latex/x/x.cls"));
        assert_eq!(erros[0].linha, Some(123));
        assert_eq!(erros[1].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(erros[1].linha, Some(7));
    }
    use std::fs;
    use std::time::{Duration, Instant};

    /// Pasta temporaria que se apaga sozinha.
    struct PastaTemp(PathBuf);

    impl PastaTemp {
        fn nova(nome: &str) -> Self {
            let caminho = std::env::temp_dir().join(format!(
                "eguatex-teste-compilador-{}-{}",
                std::process::id(),
                nome
            ));
            let _ = fs::remove_dir_all(&caminho);
            fs::create_dir_all(&caminho).expect("criar pasta de teste");
            PastaTemp(caminho)
        }

        fn texto(&self) -> String {
            self.0.display().to_string()
        }
    }

    impl Drop for PastaTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tex_instalado() -> bool {
        detectar_executavel("pdflatex").is_some()
    }

    /// Documento com um comando que nao existe no meio do texto.
    const TEX_COM_ERRO: &str = r"\documentclass{article}
\begin{document}
Antes do erro.

\comandoquenaoexiste

Depois do erro.
\end{document}
";

    /// Na primeira passada cria trava.txt e termina. Da segunda em diante (trava.txt ja
    /// existe) avisa em passo2.txt e entra num laco infinito: so sai se for morto.
    const TEX_QUE_TRAVA_NA_SEGUNDA: &str = r"\documentclass{article}
\newwrite\f
\begin{document}
\IfFileExists{trava.txt}{%
  \immediate\openout\f=passo2.txt
  \immediate\write\f{segunda passada}
  \immediate\closeout\f
  \def\a{\a}\a
}{%
  \immediate\openout\f=trava.txt
  \immediate\write\f{primeira passada}
  \immediate\closeout\f
}
Texto.
\end{document}
";

    #[test]
    fn documento_em_subpasta_acha_o_sty_ao_lado() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("subpasta");
        let sub = pasta.0.join("template-latex");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("meu-estilo.sty"),
            r"\ProvidesPackage{meu-estilo}
\newcommand{\olamundo}{Ola}
",
        )
        .unwrap();
        fs::write(
            sub.join("main.tex"),
            r"\documentclass{article}
\usepackage{meu-estilo}
\begin{document}
\olamundo \erroproposital
\end{document}
",
        )
        .unwrap();
        let r = compilar_com_bibtex("template-latex/main.tex", &pasta.texto(), "pdflatex", false);
        assert!(!r.log.contains("meu-estilo.sty' not found"), "{}", r.log);
        assert!(sub.join("main.pdf").is_file(), "{}", r.log);
        let erro = r.erros.iter().find(|e| e.mensagem.contains("Undefined control sequence")).expect("sem erro");
        assert_eq!(erro.arquivo.as_deref(), Some("./template-latex/main.tex"), "{:?}", erro);
    }

    #[test]
    fn parar_no_erro_liga_e_desliga_o_halt_on_error() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("parar-no-erro");
        fs::write(pasta.0.join("main.tex"), TEX_COM_ERRO).unwrap();
        let dir = pasta.texto();

        // true: para no primeiro erro e nem chega a gerar o PDF
        let parou = compilar("main.tex", &dir, "pdflatex", true);
        assert!(!parou.sucesso, "{}", parou.log);
        assert!(parou.pdf.is_none(), "gerou PDF mesmo parando no erro");
        assert!(!pasta.0.join("main.pdf").exists());
        assert!(!parou.log.contains("Output written on"), "{}", parou.log);
        assert!(
            parou.erros.iter().any(|e| e.mensagem.contains("Undefined control sequence")),
            "{}",
            parou.log
        );

        // false: continua apesar do erro e gera o PDF mesmo assim
        let seguiu = compilar("main.tex", &dir, "pdflatex", false);
        assert!(seguiu.pdf.is_some(), "{}", seguiu.log);
        assert!(pasta.0.join("main.pdf").is_file());
        assert!(seguiu.log.contains("Output written on"), "{}", seguiu.log);
        assert!(
            seguiu.erros.iter().any(|e| e.mensagem.contains("Undefined control sequence")),
            "{}",
            seguiu.log
        );

        // a compilacao completa repassa a mesma opcao para o motor
        let _ = fs::remove_file(pasta.0.join("main.pdf"));
        let completa_parando = compilar_com_bibtex("main.tex", &dir, "pdflatex", true);
        assert!(completa_parando.pdf.is_none(), "{}", completa_parando.log);
        let completa_seguindo = compilar_com_bibtex("main.tex", &dir, "pdflatex", false);
        assert!(completa_seguindo.pdf.is_some(), "{}", completa_seguindo.log);
    }

    #[test]
    fn parar_compilacao_sem_nada_rodando_devolve_false() {
        let _vez = travar_teste();
        assert!(!cmd_parar_compilacao());
        assert!(!cmd_parar_compilacao());
        assert!(travar(&PID_ATUAL).is_none());
    }

    #[test]
    fn parar_compilacao_interrompe_a_etapa_e_cancela_as_seguintes() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("interromper");
        fs::write(pasta.0.join("main.tex"), TEX_QUE_TRAVA_NA_SEGUNDA).unwrap();
        let dir = pasta.texto();

        let dir_da_thread = dir.clone();
        let compilacao = std::thread::spawn(move || {
            compilar_com_bibtex("main.tex", &dir_da_thread, "pdflatex", true)
        });

        // espera a segunda passada (a que trava) avisar que chegou ao laco infinito
        let marca = pasta.0.join("passo2.txt");
        let limite = Instant::now() + Duration::from_secs(90);
        while !marca.is_file() && !compilacao.is_finished() && Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(50));
        }
        let interrompeu = cmd_parar_compilacao();

        // nunca deixa o teste pendurado: espera a thread acabar e insiste se precisar
        let limite = Instant::now() + Duration::from_secs(30);
        while !compilacao.is_finished() && Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(50));
        }
        if !compilacao.is_finished() {
            cmd_parar_compilacao();
        }
        let resultado = compilacao.join().expect("thread da compilação");

        assert!(marca.is_file(), "a segunda passada não chegou a rodar: {}", resultado.log);
        assert!(interrompeu, "cmd_parar_compilacao devolveu false com a segunda passada rodando");
        assert!(!resultado.sucesso);
        assert!(resultado.pdf.is_none());
        assert!(resultado.erros.is_empty());
        assert!(
            resultado.log.contains("Compilação interrompida pelo usuário."),
            "{}",
            resultado.log
        );
        assert!(resultado.log.contains("--- Passada 2 ---"), "{}", resultado.log);
        assert!(
            !resultado.log.contains("--- Passada 3 ---"),
            "a terceira passada rodou depois da interrupcao"
        );
        assert!(travar(&PID_ATUAL).is_none(), "o PID continuou guardado");
        assert!(!cmd_parar_compilacao());

        // a compilacao seguinte nao herda a interrupcao
        fs::remove_file(pasta.0.join("trava.txt")).unwrap();
        let normal = compilar("main.tex", &dir, "pdflatex", true);
        assert!(normal.sucesso, "{}", normal.log);
        assert!(!normal.log.contains("Compilação interrompida pelo usuário."));
    }

    /// Log da ultima passada (o log completo junta todas).
    fn ultima_passada(log: &str) -> &str {
        log.rsplit("--- Passada ").next().unwrap_or(log)
    }

    const BIB_KNUTH: &str =
        "@book{knuth, author={Donald Knuth}, title={The TeXbook}, year={1984}, publisher={Addison-Wesley}}\n";

    #[test]
    fn documento_com_biblatex_usa_o_biber() {
        if !tex_instalado() || detectar_executavel("biber").is_none() {
            eprintln!("pdflatex ou biber não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("biblatex");
        fs::write(pasta.0.join("ref.bib"), BIB_KNUTH).unwrap();
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\usepackage[backend=biber]{biblatex}\n\\addbibresource{ref.bib}\n\
             \\begin{document}\nTexto \\cite{knuth}.\n\\printbibliography\n\\end{document}\n",
        )
        .unwrap();

        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("--- biber ---"), "{}", r.log);
        assert!(!r.log.contains("--- bibtex ---"), "{}", r.log);
        let ultima = ultima_passada(&r.log);
        assert!(!ultima.contains("undefined"), "citação sem resolver: {}", ultima);
    }

    #[test]
    fn erro_no_texto_nao_impede_a_bibliografia_sem_parar_no_erro() {
        if !tex_instalado() || detectar_executavel("bibtex").is_none() {
            eprintln!("pdflatex ou bibtex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("erro-com-bib");
        fs::write(pasta.0.join("ref.bib"), BIB_KNUTH).unwrap();
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\nTexto \\cite{knuth}.\n\\comandoquenaoexiste\n\
             \\bibliographystyle{plain}\n\\bibliography{ref}\n\\end{document}\n",
        )
        .unwrap();

        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(!r.sucesso, "o erro do texto sumiu: {}", r.log);
        assert!(r.pdf.is_some(), "{}", r.log);
        assert!(r.log.contains("--- bibtex ---"), "{}", r.log);
        let ultima = ultima_passada(&r.log);
        assert!(!ultima.contains("Citation `knuth' undefined"), "citação sem resolver: {}", ultima);
    }

    #[test]
    fn documento_sem_mudanca_compila_uma_vez_so() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("estavel");
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\n\\tableofcontents\n\\section{Um}\\label{s}\n\
             Veja a secao \\ref{s}.\n\\end{document}\n",
        )
        .unwrap();
        let dir = pasta.texto();

        // primeira vez: o sumario e a referencia pedem uma segunda passada
        let primeira = compilar_com_bibtex("main.tex", &dir, "pdflatex", false);
        assert!(primeira.sucesso, "{}", primeira.log);
        assert!(primeira.log.contains("--- Passada 2 ---"), "{}", primeira.log);
        assert!(!primeira.log.contains("--- bibtex ---"), "rodou bibtex sem bibliografia");
        let ultima = ultima_passada(&primeira.log);
        assert!(!ultima.contains("Rerun"), "{}", ultima);

        // sem mudar nada, uma passada basta
        let segunda = compilar_com_bibtex("main.tex", &dir, "pdflatex", false);
        assert!(segunda.sucesso, "{}", segunda.log);
        assert!(!segunda.log.contains("--- Passada 2 ---"), "{}", segunda.log);
    }
}

#[cfg(test)]
#[path = "compilador_avisos_borda.rs"]
mod compilador_avisos_borda;
