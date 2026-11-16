//! Ponte entre o Rust e o motor LaTeX em WebAssembly (busytex) do EguaTex Android.
//!
//! O Android nao roda o TinyTeX do Windows. O pdflatex, xelatex, lualatex e bibtex8
//! compilados para WebAssembly rodam num Web Worker dentro do app, sem internet.
//! Este modulo:
//! - serve os arquivos do motor (que viajam dentro do APK, na pasta de assets
//!   `busytex/`) pelo protocolo `busytex://`;
//! - entrega ao Worker os arquivos do projeto;
//! - grava no disco o PDF que o Worker devolve;
//! - extrai os erros do log com o mesmo leitor da versao Windows.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::compilador::ErroCompilacao;

/// Pastas que nunca entram na compilacao.
const IGNORADAS: &[&str] = &[".git", ".eguatex", "node_modules", "__MACOSX"];

/// Extensoes que o TeX le como texto. O resto vai em base64.
const TEXTO: &[&str] = &[
    "tex", "sty", "cls", "bib", "bst", "bbx", "cbx", "lbx", "def", "cfg", "clo", "fd", "ltx", "dtx", "ins", "txt",
    "csv", "dat", "tikz", "pgf", "aux", "bbl", "toc", "lof", "lot", "idx", "ind", "md",
];

/// Arquivos gerados pela compilacao anterior: o motor refaz todos.
const AUXILIARES: &[&str] = &["log", "aux", "toc", "lof", "lot", "out", "bbl", "blg", "fls", "synctex", "gz", "xdv"];

#[derive(Serialize)]
pub struct ArquivoMotor {
    /// Caminho relativo a raiz do projeto, com barras normais.
    pub caminho: String,
    pub texto: Option<String>,
    pub base64: Option<String>,
}

fn extensao(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn juntar(base: &Path, pasta: &Path, saida: &mut Vec<ArquivoMotor>, total: &mut u64) -> Result<(), String> {
    let entradas = std::fs::read_dir(pasta).map_err(|e| format!("Erro ao ler {}: {e}", pasta.display()))?;
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        let nome = entrada.file_name().to_string_lossy().to_string();
        let tipo = entrada.file_type().map_err(|e| format!("Erro ao ler {}: {e}", caminho.display()))?;
        // atalho (symlink) nao e seguido: aponta para fora do projeto ou faz laco
        if tipo.is_symlink() {
            continue;
        }
        if tipo.is_dir() {
            if !IGNORADAS.contains(&nome.as_str()) {
                juntar(base, &caminho, saida, total)?;
            }
            continue;
        }
        let ext = extensao(&caminho);
        if AUXILIARES.contains(&ext.as_str()) || ext == "pdf" && pasta == base && saida_e_pdf_principal(&nome) {
            continue;
        }
        let relativo = caminho
            .strip_prefix(base)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(&caminho).map_err(|e| format!("Erro ao ler {relativo}: {e}"))?;
        *total += bytes.len() as u64;
        if *total > 300 * 1024 * 1024 {
            return Err("O projeto passa de 300 MB. Tire arquivos grandes que o documento não usa.".into());
        }
        if TEXTO.contains(&ext.as_str()) {
            match String::from_utf8(bytes) {
                Ok(t) => saida.push(ArquivoMotor { caminho: relativo, texto: Some(t), base64: None }),
                Err(e) => saida.push(ArquivoMotor {
                    caminho: relativo,
                    texto: None,
                    base64: Some(base64::engine::general_purpose::STANDARD.encode(e.into_bytes())),
                }),
            }
        } else {
            saida.push(ArquivoMotor {
                caminho: relativo,
                texto: None,
                base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
            });
        }
    }
    Ok(())
}

/// PDFs na raiz com o mesmo nome de um .tex sao saida da compilacao anterior. Como a
/// lista ainda nao foi toda lida aqui, a regra fica so para o nome mais comum.
fn saida_e_pdf_principal(nome: &str) -> bool {
    nome.eq_ignore_ascii_case("main.pdf")
}

/// Todos os arquivos do projeto, prontos para o sistema de arquivos em memoria do motor.
#[tauri::command(async)]
pub fn cmd_motor_arquivos(diretorio: String) -> Result<Vec<ArquivoMotor>, String> {
    let base = PathBuf::from(&diretorio);
    let mut saida = Vec::new();
    let mut total = 0;
    juntar(&base, &base, &mut saida, &mut total)?;
    Ok(saida)
}

// Cache do motor em disco: os auxiliares (.aux, .bbl, .toc...) da ultima compilacao que deu
// certo e a lista de pacotes de dados que ela usou. Fica em `.eguatex/cache-motor/` dentro
// do projeto, pasta que o zip exportado, o Drive e a leitura do projeto (IGNORADAS) pulam.

const PASTA_CACHE: &str = ".eguatex/cache-motor";
const PASTA_CACHE_AUX: &str = "aux";
const META_CACHE: &str = "meta.json";
const ARQUIVO_PACOTES: &str = "pacotes.json";

/// Extensoes que o cache aceita: as mesmas de `EXT_AUX` em `busytex_pipeline.js`.
const EXT_CACHE: &[&str] = &["aux", "toc", "lof", "lot", "out", "nav", "snm", "bbl", "idx", "ind", "nlo", "nls", "loa", "thm"];
/// Limite de cada auxiliar e do cache inteiro. Um auxiliar normal tem poucos KB.
const MAX_ARQUIVO_CACHE: usize = 8 * 1024 * 1024;
const MAX_TOTAL_CACHE: usize = 32 * 1024 * 1024;

fn extensao_do_cache(caminho: &Path) -> bool {
    EXT_CACHE.contains(&extensao(caminho).as_str())
}

#[derive(Serialize, Deserialize, Default)]
struct MetaCache {
    principal: String,
    assinatura: Option<String>,
}

/// Auxiliar vindo do frontend: caminho relativo ao projeto e conteudo em base64.
#[derive(Deserialize)]
pub struct ArquivoCache {
    pub caminho: String,
    pub base64: String,
}

#[derive(Serialize)]
pub struct CacheMotor {
    /// Assinatura das citacoes e do .bib da compilacao que gerou o cache.
    pub assinatura: Option<String>,
    pub arquivos: Vec<ArquivoMotor>,
}

/// Pacotes que a ultima compilacao boa do projeto usou.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct MemoriaPacotes {
    #[serde(default)]
    pub principal: String,
    /// Pacotes de dados do busytex (nomes de arquivo, como `ubuntu-texlive-latex-extra.js`).
    #[serde(default)]
    pub dados: Vec<String>,
    /// Pacotes extras (extras/pacotes) que o projeto precisou.
    #[serde(default)]
    pub extras: Vec<String>,
    #[serde(default)]
    pub pgf: bool,
}

#[derive(Serialize)]
pub struct ResumoProjeto {
    /// Texto do documento principal, se existir.
    pub texto: Option<String>,
    /// O projeto tem .cls ou .sty proprio (o motor carrega todos os pacotes).
    pub tem_cls_sty: bool,
    pub pacotes: Option<MemoriaPacotes>,
}

fn pasta_cache(base: &Path) -> PathBuf {
    base.join(PASTA_CACHE)
}

/// Caminho relativo seguro: sem `..`, sem raiz, sem letra de unidade, sem barra invertida.
fn caminho_relativo_seguro(rel: &str) -> Option<PathBuf> {
    let mut saida = PathBuf::new();
    for parte in rel.split('/') {
        if parte.is_empty() || parte == "." || parte == ".." || parte.contains(':') || parte.contains('\\') {
            return None;
        }
        saida.push(parte);
    }
    if saida.as_os_str().is_empty() { None } else { Some(saida) }
}

/// Apaga so os auxiliares e o meta.json (a gravacao de um cache novo comeca por aqui).
fn apagar_auxiliares(base: &Path) {
    let _ = std::fs::remove_dir_all(pasta_cache(base).join(PASTA_CACHE_AUX));
    let _ = std::fs::remove_file(pasta_cache(base).join(META_CACHE));
}

/// Apaga o cache inteiro do projeto: auxiliares, meta.json e a lista de pacotes lembrada.
fn apagar_cache(base: &Path) {
    apagar_auxiliares(base);
    let _ = std::fs::remove_file(pasta_cache(base).join(ARQUIVO_PACOTES));
}

fn gravar_cache(base: &Path, principal: &str, assinatura: Option<String>, arquivos: &[ArquivoCache]) -> Result<(), String> {
    apagar_auxiliares(base);
    let aux = pasta_cache(base).join(PASTA_CACHE_AUX);
    std::fs::create_dir_all(&aux).map_err(|e| format!("Erro ao criar o cache do motor: {e}"))?;
    let mut total = 0usize;
    for arquivo in arquivos {
        let Some(rel) = caminho_relativo_seguro(&arquivo.caminho) else { continue };
        // base64 tem 4 letras para cada 3 bytes: descarta antes de decodificar o que ja passa do limite
        if !extensao_do_cache(&rel) || arquivo.base64.len() / 4 * 3 > MAX_ARQUIVO_CACHE + 3 {
            continue;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(arquivo.base64.as_bytes())
            .map_err(|e| format!("Auxiliar corrompido ({}): {e}", arquivo.caminho))?;
        if bytes.len() > MAX_ARQUIVO_CACHE || total + bytes.len() > MAX_TOTAL_CACHE {
            continue;
        }
        total += bytes.len();
        let destino = aux.join(rel);
        if let Some(pai) = destino.parent() {
            std::fs::create_dir_all(pai).map_err(|e| e.to_string())?;
        }
        std::fs::write(&destino, bytes).map_err(|e| format!("Erro ao gravar o cache do motor: {e}"))?;
    }
    let meta = serde_json::to_string(&MetaCache { principal: principal.to_string(), assinatura }).map_err(|e| e.to_string())?;
    std::fs::write(pasta_cache(base).join(META_CACHE), meta).map_err(|e| format!("Erro ao gravar o cache do motor: {e}"))
}

fn ler_arquivos_cache(aux: &Path, pasta: &Path, saida: &mut Vec<ArquivoMotor>, total: &mut usize) {
    let Ok(entradas) = std::fs::read_dir(pasta) else { return };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        let Ok(tipo) = entrada.file_type() else { continue };
        if tipo.is_symlink() {
            continue;
        }
        if tipo.is_dir() {
            ler_arquivos_cache(aux, &caminho, saida, total);
            continue;
        }
        // arquivo de outra extensao ou grande demais (cache adulterado) nao vai ao motor
        let tamanho = entrada.metadata().map(|m| m.len() as usize).unwrap_or(usize::MAX);
        if !extensao_do_cache(&caminho) || tamanho > MAX_ARQUIVO_CACHE || *total + tamanho > MAX_TOTAL_CACHE {
            continue;
        }
        if let (Ok(rel), Ok(bytes)) = (caminho.strip_prefix(aux), std::fs::read(&caminho)) {
            *total += bytes.len();
            saida.push(ArquivoMotor {
                caminho: rel.to_string_lossy().replace('\\', "/"),
                texto: None,
                base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
            });
        }
    }
}

/// Le o cache. Se ele foi gravado para outro documento principal, apaga e devolve vazio.
fn ler_cache(base: &Path, principal: &str) -> CacheMotor {
    let vazio = CacheMotor { assinatura: None, arquivos: Vec::new() };
    let meta: Option<MetaCache> = std::fs::read_to_string(pasta_cache(base).join(META_CACHE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let Some(meta) = meta else { return vazio };
    if meta.principal != principal {
        apagar_cache(base);
        return vazio;
    }
    let aux = pasta_cache(base).join(PASTA_CACHE_AUX);
    let mut arquivos = Vec::new();
    ler_arquivos_cache(&aux, &aux, &mut arquivos, &mut 0);
    arquivos.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    CacheMotor { assinatura: meta.assinatura, arquivos }
}

fn gravar_pacotes(base: &Path, memoria: &MemoriaPacotes) -> Result<(), String> {
    std::fs::create_dir_all(pasta_cache(base)).map_err(|e| e.to_string())?;
    let texto = serde_json::to_string(memoria).map_err(|e| e.to_string())?;
    std::fs::write(pasta_cache(base).join(ARQUIVO_PACOTES), texto).map_err(|e| format!("Erro ao gravar a lista de pacotes: {e}"))
}

/// Lista de pacotes do projeto; so vale para o mesmo documento principal.
fn ler_pacotes(base: &Path, principal: &str) -> Option<MemoriaPacotes> {
    let texto = std::fs::read_to_string(pasta_cache(base).join(ARQUIVO_PACOTES)).ok()?;
    let memoria: MemoriaPacotes = serde_json::from_str(&texto).ok()?;
    (memoria.principal == principal).then_some(memoria)
}

fn tem_classe_ou_estilo(pasta: &Path) -> bool {
    let Ok(entradas) = std::fs::read_dir(pasta) else { return false };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        let Ok(tipo) = entrada.file_type() else { continue };
        if tipo.is_symlink() {
            continue;
        }
        if tipo.is_dir() {
            let nome = entrada.file_name().to_string_lossy().to_string();
            if !IGNORADAS.contains(&nome.as_str()) && tem_classe_ou_estilo(&caminho) {
                return true;
            }
        } else if matches!(extensao(&caminho).as_str(), "cls" | "sty") {
            return true;
        }
    }
    false
}

/// Cache dos auxiliares em disco, para a primeira compilacao depois de abrir o app.
#[tauri::command(async)]
pub fn cmd_motor_cache_ler(diretorio: String, principal: String) -> CacheMotor {
    ler_cache(Path::new(&diretorio), &principal)
}

#[tauri::command(async)]
pub fn cmd_motor_cache_gravar(
    diretorio: String,
    principal: String,
    assinatura: Option<String>,
    arquivos: Vec<ArquivoCache>,
) -> Result<(), String> {
    gravar_cache(Path::new(&diretorio), &principal, assinatura, &arquivos)
}

/// Apaga o cache em disco. Erro de LaTeX do usuario apaga so os auxiliares (`pacotes` ausente ou
/// falso): a lista de pacotes continua valendo. Falha ou tempo esgotado do Worker, Parar e Limpar
/// auxiliares passam `pacotes: true` e apagam tambem o `pacotes.json`.
#[tauri::command(async)]
pub fn cmd_motor_cache_apagar(diretorio: String, pacotes: Option<bool>) {
    let base = Path::new(&diretorio);
    if pacotes == Some(true) {
        apagar_cache(base);
    } else {
        apagar_auxiliares(base);
    }
}

#[tauri::command(async)]
pub fn cmd_motor_pacotes_gravar(diretorio: String, principal: String, dados: Vec<String>, extras: Vec<String>, pgf: bool) -> Result<(), String> {
    gravar_pacotes(Path::new(&diretorio), &MemoriaPacotes { principal, dados, extras, pgf })
}

/// O que o preaquecimento precisa: texto do principal, se ha .cls ou .sty proprio e os pacotes lembrados.
#[tauri::command(async)]
pub fn cmd_motor_resumo(diretorio: String, principal: String) -> ResumoProjeto {
    let base = Path::new(&diretorio);
    let principal = principal.trim_start_matches('/').to_string();
    let texto = caminho_relativo_seguro(&principal).and_then(|rel| std::fs::read_to_string(base.join(rel)).ok());
    ResumoProjeto { texto, tem_cls_sty: tem_classe_ou_estilo(base), pacotes: ler_pacotes(base, &principal) }
}

/// Grava bytes vindos do frontend (o PDF gerado). O caminho vem no cabecalho
/// `caminho`, codificado com encodeURIComponent; o corpo e binario.
#[tauri::command(async)]
pub fn cmd_motor_gravar(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let caminho = request
        .headers()
        .get("caminho")
        .and_then(|v| v.to_str().ok())
        .ok_or("Faltou o caminho do arquivo")?;
    let caminho = percent_decode(caminho);
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("O conteúdo do arquivo não veio em binário".into());
    };
    if let Some(pai) = Path::new(&caminho).parent() {
        std::fs::create_dir_all(pai).map_err(|e| e.to_string())?;
    }
    std::fs::write(&caminho, bytes).map_err(|e| format!("Erro ao gravar {caminho}: {e}"))
}

/// Grava o PDF vindo do frontend em base64. No Android o corpo binario puro nao chega
/// ao Rust pelo IPC; o texto base64 chega sempre.
#[tauri::command(async)]
pub fn cmd_motor_gravar_base64(caminho: String, base64: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64.as_bytes())
        .map_err(|e| format!("PDF corrompido no caminho: {e}"))?;
    if let Some(pai) = Path::new(&caminho).parent() {
        std::fs::create_dir_all(pai).map_err(|e| e.to_string())?;
    }
    std::fs::write(&caminho, bytes).map_err(|e| format!("Erro ao gravar {caminho}: {e}"))
}

/// Grava um arquivo de texto do motor (o .log da compilacao), ao lado do .tex.
#[tauri::command(async)]
pub fn cmd_motor_gravar_texto(caminho: String, conteudo: String) -> Result<(), String> {
    std::fs::write(&caminho, conteudo).map_err(|e| format!("Erro ao gravar {caminho}: {e}"))
}

/// Erros e avisos do log, com o mesmo leitor usado na versao Windows. Quando o documento
/// principal esta numa subpasta ("t/a.tex"), os arquivos dos erros ganham a subpasta
/// ("./t/a.tex"), como no Windows, para o clique no erro abrir o arquivo certo.
#[tauri::command]
pub fn cmd_motor_erros(log: String, principal: String) -> Vec<ErroCompilacao> {
    let sub = Path::new(principal.trim_start_matches('/'))
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    // o aviso sem arquivo .tex aberto no log cai no documento principal
    let nome_principal = Path::new(principal.trim_start_matches('/'))
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let resultado = crate::compilador::ResultadoCompilacao {
        sucesso: false,
        pdf: None,
        log: String::new(),
        erros: crate::compilador::extrair_erros(&log, &nome_principal),
        motor_usado: String::new(),
        tempo_ms: 0,
    };
    crate::compilador::com_subpasta(resultado, &sub).erros
}

fn percent_decode(texto: &str) -> String {
    let b = texto.as_bytes();
    let mut saida = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&texto[i + 1..i + 3], 16) {
                saida.push(v);
                i += 3;
                continue;
            }
        }
        saida.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

fn tipo_mime(nome: &str) -> &'static str {
    if nome.ends_with(".wasm") {
        "application/wasm"
    } else if nome.ends_with(".js") {
        "text/javascript"
    } else if nome.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

/// Resposta do protocolo `busytex://`: le o arquivo dos assets do APK.
pub fn responder(caminho_url: &str) -> tauri::http::Response<Vec<u8>> {
    let nome = caminho_url.trim_start_matches('/');
    let valido = !nome.is_empty() && !nome.contains("..") && nome.chars().all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c));
    let resultado = if valido { ler_asset(&format!("busytex/{nome}")) } else { Err("nome inválido".into()) };
    let construtor = tauri::http::Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store");
    match resultado {
        Ok(bytes) => construtor
            .status(200)
            .header("Content-Type", tipo_mime(nome))
            .body(bytes)
            .unwrap(),
        Err(e) => construtor.status(404).header("Content-Type", "text/plain").body(e.into_bytes()).unwrap(),
    }
}

#[cfg(target_os = "android")]
fn ler_asset(caminho: &str) -> Result<Vec<u8>, String> {
    android::ler(caminho)
}

/// Fora do Android (testes no PC), le da pasta indicada em EGUATEX_BUSYTEX.
#[cfg(not(target_os = "android"))]
fn ler_asset(caminho: &str) -> Result<Vec<u8>, String> {
    let base = std::env::var_os("EGUATEX_BUSYTEX").ok_or("EGUATEX_BUSYTEX não definida")?;
    let nome = caminho.trim_start_matches("busytex/");
    std::fs::read(PathBuf::from(base).join(nome)).map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{GlobalRef, JObject};
    use std::ffi::CString;
    use std::sync::OnceLock;

    struct Gerente {
        ptr: *mut ndk_sys::AAssetManager,
        _java: GlobalRef,
    }
    // SAFETY: o AAssetManager do NDK pode ser usado de qualquer thread; a referencia
    // global ao objeto Java mantem o ponteiro valido enquanto o app roda.
    unsafe impl Send for Gerente {}
    unsafe impl Sync for Gerente {}

    static GERENTE: OnceLock<Result<Gerente, String>> = OnceLock::new();

    fn obter() -> Result<Gerente, String> {
        let ctx = ndk_context::android_context();
        // SAFETY: o tao inicializa o contexto Android antes de abrir a janela.
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let contexto = unsafe { JObject::from_raw(ctx.context().cast()) };
        let gerente = env
            .call_method(&contexto, "getAssets", "()Landroid/content/res/AssetManager;", &[])
            .and_then(|v| v.l())
            .map_err(|e| format!("getAssets falhou: {e}"))?;
        let global = env.new_global_ref(&gerente).map_err(|e| e.to_string())?;
        // SAFETY: env e gerente sao validos nesta thread.
        let ptr = unsafe { ndk_sys::AAssetManager_fromJava(env.get_raw().cast(), global.as_obj().as_raw().cast()) };
        if ptr.is_null() {
            return Err("AAssetManager nulo".into());
        }
        Ok(Gerente { ptr, _java: global })
    }

    pub fn ler(caminho: &str) -> Result<Vec<u8>, String> {
        let gerente = GERENTE.get_or_init(obter).as_ref().map_err(|e| e.clone())?;
        let nome = CString::new(caminho).map_err(|e| e.to_string())?;
        // SAFETY: ponteiros validos; o asset e fechado no fim.
        unsafe {
            let asset = ndk_sys::AAssetManager_open(gerente.ptr, nome.as_ptr(), ndk_sys::AASSET_MODE_STREAMING as i32);
            if asset.is_null() {
                return Err(format!("{caminho} não está no APK"));
            }
            let tamanho = ndk_sys::AAsset_getLength64(asset).max(0) as usize;
            let mut bytes = Vec::with_capacity(tamanho);
            let mut bloco = vec![0u8; 1 << 20];
            loop {
                let lidos = ndk_sys::AAsset_read(asset, bloco.as_mut_ptr().cast(), bloco.len());
                if lidos <= 0 {
                    break;
                }
                bytes.extend_from_slice(&bloco[..lidos as usize]);
            }
            ndk_sys::AAsset_close(asset);
            Ok(bytes)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn decodifica_caminho() {
        assert_eq!(percent_decode("%2Fdata%2Fprojeto%20um%2Fmain.pdf"), "/data/projeto um/main.pdf");
        assert_eq!(percent_decode("a%C3%A7a%C3%AD"), "açaí");
    }

    #[test]
    fn erros_do_motor_ganham_a_subpasta_do_principal() {
        let log = "./a.tex:3: Undefined control sequence.\n! Undefined control sequence.\n";
        let na_subpasta = cmd_motor_erros(log.into(), "t/a.tex".into());
        assert_eq!(na_subpasta[0].arquivo.as_deref(), Some("./t/a.tex"));
        assert_eq!(na_subpasta[1].arquivo, None);
        let aninhada = cmd_motor_erros(log.into(), "/x/y/a.tex".into());
        assert_eq!(aninhada[0].arquivo.as_deref(), Some("./x/y/a.tex"));
        // principal na raiz: o caminho do log fica como veio
        let na_raiz = cmd_motor_erros(log.into(), "a.tex".into());
        assert_eq!(na_raiz[0].arquivo.as_deref(), Some("./a.tex"));
    }

    fn pasta_de_teste(nome: &str) -> PathBuf {
        let base = std::env::temp_dir().join(nome);
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn cache(caminho: &str, texto: &str) -> ArquivoCache {
        ArquivoCache { caminho: caminho.into(), base64: base64::engine::general_purpose::STANDARD.encode(texto) }
    }

    #[test]
    fn cache_grava_e_le_os_auxiliares_com_subpasta() {
        let base = pasta_de_teste("eguatex-cache-ida-volta");
        let arquivos = [cache("main.aux", "\\citation{x}"), cache("cap/c1.aux", "\\relax"), cache("main.bbl", "bbl")];
        gravar_cache(&base, "main.tex", Some("assin".into()), &arquivos).unwrap();
        let lido = ler_cache(&base, "main.tex");
        assert_eq!(lido.assinatura.as_deref(), Some("assin"));
        let nomes: Vec<&str> = lido.arquivos.iter().map(|a| a.caminho.as_str()).collect();
        assert_eq!(nomes, ["cap/c1.aux", "main.aux", "main.bbl"]);
        let main_aux = lido.arquivos.iter().find(|a| a.caminho == "main.aux").unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(main_aux.base64.as_ref().unwrap()).unwrap();
        assert_eq!(bytes, b"\\citation{x}");
        // o cache fica dentro de .eguatex e a leitura do projeto nao o entrega ao motor
        assert!(base.join(".eguatex/cache-motor/meta.json").exists());
        std::fs::write(base.join("main.tex"), "x").unwrap();
        let projeto = cmd_motor_arquivos(base.display().to_string()).unwrap();
        assert_eq!(projeto.len(), 1);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cache_de_outro_principal_e_apagado() {
        let base = pasta_de_teste("eguatex-cache-principal");
        gravar_cache(&base, "main.tex", None, &[cache("main.aux", "a")]).unwrap();
        let outro = ler_cache(&base, "tese.tex");
        assert!(outro.arquivos.is_empty());
        // apagado de verdade: voltar ao nome antigo nao traz o cache de volta
        assert!(ler_cache(&base, "main.tex").arquivos.is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cache_so_aceita_as_extensoes_de_auxiliares() {
        let base = pasta_de_teste("eguatex-cache-extensoes");
        let arquivos = [
            cache("main.aux", "a"),
            cache("main.bbl", "b"),
            cache("main.blg", "blg"),
            cache("script.sh", "x"),
            cache("main.tex", "x"),
            cache("semextensao", "x"),
        ];
        gravar_cache(&base, "main.tex", None, &arquivos).unwrap();
        let aux = base.join(".eguatex/cache-motor/aux");
        assert!(aux.join("main.aux").exists() && aux.join("main.bbl").exists());
        for fora in ["main.blg", "script.sh", "main.tex", "semextensao"] {
            assert!(!aux.join(fora).exists(), "{fora} nao devia entrar no cache");
        }
        // cache adulterado no disco: arquivo de outra extensao nao vai ao motor
        std::fs::write(aux.join("intruso.exe"), "x").unwrap();
        let nomes: Vec<String> = ler_cache(&base, "main.tex").arquivos.into_iter().map(|a| a.caminho).collect();
        assert_eq!(nomes, ["main.aux", "main.bbl"]);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cache_limita_o_tamanho_de_cada_arquivo_e_do_total() {
        let base = pasta_de_teste("eguatex-cache-limites");
        let grande = "a".repeat(MAX_ARQUIVO_CACHE + 1);
        let no_limite = "b".repeat(MAX_ARQUIVO_CACHE);
        gravar_cache(&base, "main.tex", None, &[cache("grande.aux", &grande), cache("limite.aux", &no_limite)]).unwrap();
        let aux = base.join(".eguatex/cache-motor/aux");
        assert!(!aux.join("grande.aux").exists());
        assert_eq!(std::fs::metadata(aux.join("limite.aux")).unwrap().len() as usize, MAX_ARQUIVO_CACHE);
        // 5 arquivos de 8 MB: so 4 cabem em 32 MB
        let cinco: Vec<ArquivoCache> = (0..5).map(|i| cache(&format!("c{i}.aux"), &no_limite)).collect();
        gravar_cache(&base, "main.tex", None, &cinco).unwrap();
        let gravados = (0..5).filter(|i| aux.join(format!("c{i}.aux")).exists()).count();
        assert_eq!(gravados, MAX_TOTAL_CACHE / MAX_ARQUIVO_CACHE);
        // na leitura, um arquivo inflado depois da gravacao fica de fora
        std::fs::write(aux.join("c0.aux"), grande).unwrap();
        let lidos = ler_cache(&base, "main.tex").arquivos;
        assert!(lidos.iter().all(|a| a.caminho != "c0.aux"));
        assert_eq!(lidos.len(), gravados - 1);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn erro_de_latex_apaga_so_os_auxiliares_e_a_falha_do_worker_apaga_tambem_os_pacotes() {
        let base = pasta_de_teste("eguatex-cache-apagar-pacotes");
        let memoria = MemoriaPacotes { principal: "main.tex".into(), dados: vec!["texlive-basic.js".into()], extras: vec![], pgf: false };
        let diretorio = base.display().to_string();
        for (pacotes, some_pacotes) in [(None, true), (Some(false), true), (Some(true), false)] {
            gravar_cache(&base, "main.tex", None, &[cache("main.aux", "a")]).unwrap();
            gravar_pacotes(&base, &memoria).unwrap();
            cmd_motor_cache_apagar(diretorio.clone(), pacotes);
            assert!(ler_cache(&base, "main.tex").arquivos.is_empty(), "auxiliares sempre saem ({pacotes:?})");
            assert_eq!(ler_pacotes(&base, "main.tex").is_some(), some_pacotes, "pacotes.json com {pacotes:?}");
        }
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn caminho_seguro_recusa_o_que_sai_da_pasta() {
        for ruim in ["", ".", "..", "../a.aux", "a/../b.aux", "/a.aux", "a//b.aux", "C:/a.aux", "a:b.aux", "a\\b.aux"] {
            assert!(caminho_relativo_seguro(ruim).is_none(), "{ruim:?} devia ser recusado");
        }
        assert_eq!(caminho_relativo_seguro("cap/c1.aux"), Some(PathBuf::from("cap").join("c1.aux")));
    }

    #[test]
    fn meta_e_pacotes_invalidos_viram_vazio_sem_erro() {
        let base = pasta_de_teste("eguatex-cache-invalido");
        gravar_cache(&base, "main.tex", None, &[cache("main.aux", "a")]).unwrap();
        let pasta = pasta_cache(&base);
        std::fs::write(pasta.join(META_CACHE), "{ isto nao e json").unwrap();
        assert!(ler_cache(&base, "main.tex").arquivos.is_empty());
        std::fs::write(pasta.join(ARQUIVO_PACOTES), "lixo").unwrap();
        assert_eq!(ler_pacotes(&base, "main.tex"), None);
        // campos faltando: serde(default) preenche, e o principal precisa bater
        std::fs::write(pasta.join(ARQUIVO_PACOTES), r#"{"principal":"main.tex"}"#).unwrap();
        let lida = ler_pacotes(&base, "main.tex").unwrap();
        assert!(lida.dados.is_empty() && lida.extras.is_empty() && !lida.pgf);
        std::fs::write(pasta.join(ARQUIVO_PACOTES), "{}").unwrap();
        assert_eq!(ler_pacotes(&base, "main.tex"), None);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cache_fica_fora_do_zip_e_do_drive() {
        let base = pasta_de_teste("eguatex-cache-zip");
        std::fs::write(base.join("main.tex"), "x").unwrap();
        gravar_cache(&base, "main.tex", Some("a".into()), &[cache("main.aux", "a"), cache("cap/c1.aux", "b")]).unwrap();
        gravar_pacotes(&base, &MemoriaPacotes { principal: "main.tex".into(), ..Default::default() }).unwrap();
        // a mesma coleta serve ao zip exportado e a lista de arquivos do Drive (`arquivos_locais`)
        let mut itens = Vec::new();
        crate::arquivos::coletar_itens_zip(&base, "", &mut itens, None).unwrap();
        let nomes: Vec<String> = itens
            .into_iter()
            .map(|i| match i {
                crate::arquivos::ItemZip::Arquivo { nome, .. } => nome,
                crate::arquivos::ItemZip::Pasta { nome } => nome,
            })
            .collect();
        assert_eq!(nomes, ["main.tex"]);
        let _ = std::fs::remove_dir_all(base);
    }

    /// Atalho (symlink) para fora do projeto: nao e seguido. Sem permissao para criar atalho (Windows
    /// sem modo de desenvolvedor), o teste nao tem o que verificar e termina.
    #[test]
    fn atalhos_nao_sao_seguidos() {
        let base = pasta_de_teste("eguatex-cache-atalho");
        let fora = pasta_de_teste("eguatex-cache-atalho-fora");
        std::fs::write(fora.join("segredo.aux"), "x").unwrap();
        std::fs::write(fora.join("segredo.tex"), "x").unwrap();
        std::fs::write(base.join("main.tex"), "x").unwrap();
        #[cfg(unix)]
        let feito = std::os::unix::fs::symlink(&fora, base.join("link")).is_ok();
        #[cfg(windows)]
        let feito = std::os::windows::fs::symlink_dir(&fora, base.join("link")).is_ok();
        #[cfg(not(any(unix, windows)))]
        let feito = false;
        if feito {
            let lista = cmd_motor_arquivos(base.display().to_string()).unwrap();
            assert_eq!(lista.len(), 1);
            assert_eq!(lista[0].caminho, "main.tex");
            assert!(!tem_classe_ou_estilo(&base));
            let aux = pasta_cache(&base).join(PASTA_CACHE_AUX);
            std::fs::create_dir_all(&aux).unwrap();
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink(&fora, aux.join("dentro"));
            #[cfg(windows)]
            let _ = std::os::windows::fs::symlink_dir(&fora, aux.join("dentro"));
            std::fs::write(pasta_cache(&base).join(META_CACHE), r#"{"principal":"main.tex","assinatura":null}"#).unwrap();
            assert!(ler_cache(&base, "main.tex").arquivos.is_empty());
        }
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&fora);
    }

    #[test]
    fn cache_apagar_remove_os_auxiliares() {
        let base = pasta_de_teste("eguatex-cache-apagar");
        gravar_cache(&base, "main.tex", None, &[cache("main.aux", "a")]).unwrap();
        apagar_cache(&base);
        assert!(ler_cache(&base, "main.tex").arquivos.is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cache_nunca_grava_fora_de_eguatex() {
        let base = pasta_de_teste("eguatex-cache-seguro");
        let arquivos = [
            cache("../fora.aux", "x"),
            cache("/abs.aux", "x"),
            cache("C:/x.aux", "x"),
            cache("a\\b.aux", "x"),
            cache("a/../../fora2.aux", "x"),
            cache("ok.aux", "x"),
        ];
        gravar_cache(&base, "main.tex", None, &arquivos).unwrap();
        let lido = ler_cache(&base, "main.tex");
        assert_eq!(lido.arquivos.len(), 1);
        assert_eq!(lido.arquivos[0].caminho, "ok.aux");
        assert!(!base.join("fora.aux").exists());
        assert!(!base.parent().unwrap().join("fora.aux").exists());
        assert!(!base.parent().unwrap().join("fora2.aux").exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn pacotes_lembrados_valem_so_para_o_mesmo_principal() {
        let base = pasta_de_teste("eguatex-cache-pacotes");
        let memoria = MemoriaPacotes {
            principal: "main.tex".into(),
            dados: vec!["texlive-basic.js".into(), "ubuntu-texlive-latex-extra.js".into()],
            extras: vec!["abntex2".into()],
            pgf: true,
        };
        gravar_pacotes(&base, &memoria).unwrap();
        assert_eq!(ler_pacotes(&base, "main.tex"), Some(memoria));
        assert_eq!(ler_pacotes(&base, "outro.tex"), None);
        // gravar um cache novo de auxiliares nao mexe nos pacotes; apagar o cache inteiro (falha, Limpar auxiliares) tira os dois
        gravar_cache(&base, "main.tex", None, &[cache("main.aux", "a")]).unwrap();
        assert!(ler_pacotes(&base, "main.tex").is_some());
        apagar_cache(&base);
        assert!(ler_pacotes(&base, "main.tex").is_none());
        assert!(!pasta_cache(&base).join("pacotes.json").exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn resumo_acha_estilo_proprio_e_le_o_principal() {
        let base = pasta_de_teste("eguatex-cache-resumo");
        std::fs::write(base.join("main.tex"), "\\documentclass{abntex2}").unwrap();
        let sem = cmd_motor_resumo(base.display().to_string(), "/main.tex".into());
        assert_eq!(sem.texto.as_deref(), Some("\\documentclass{abntex2}"));
        assert!(!sem.tem_cls_sty);
        std::fs::create_dir_all(base.join("estilos")).unwrap();
        std::fs::write(base.join("estilos/meu.sty"), "").unwrap();
        assert!(cmd_motor_resumo(base.display().to_string(), "main.tex".into()).tem_cls_sty);
        assert!(cmd_motor_resumo(base.display().to_string(), "../main.tex".into()).texto.is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn junta_arquivos_do_projeto() {
        let base = std::env::temp_dir().join("eguatex-motor-teste");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("figuras")).unwrap();
        std::fs::write(base.join("main.tex"), "\\documentclass{article}").unwrap();
        std::fs::write(base.join("main.aux"), "velho").unwrap();
        std::fs::write(base.join("figuras/a.png"), [0x89u8, 0x50, 0x4e, 0x47]).unwrap();
        let mut lista = cmd_motor_arquivos(base.display().to_string()).unwrap();
        lista.sort_by(|a, b| a.caminho.cmp(&b.caminho));
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].caminho, "figuras/a.png");
        assert!(lista[0].base64.is_some());
        assert_eq!(lista[1].texto.as_deref(), Some("\\documentclass{article}"));
        let _ = std::fs::remove_dir_all(base);
    }
}
