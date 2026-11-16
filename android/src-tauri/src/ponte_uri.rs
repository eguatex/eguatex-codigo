//! Ponte entre os dialogos do Android e os comandos de arquivo da versao Windows.
//!
//! No Android, as janelas de abrir e salvar devolvem enderecos `content://`, que
//! `std::fs` nao abre. Estes comandos tem o mesmo nome dos originais: quando o
//! caminho e um endereco, copiam por uma pasta temporaria e chamam a funcao original.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tauri::{AppHandle, Runtime};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

use crate::arquivos;
use crate::limites_zip::{self, Limites};

fn eh_endereco(caminho: &str) -> bool {
    caminho.starts_with("content://") || caminho.starts_with("file://")
}

fn pasta_temporaria() -> Result<PathBuf, String> {
    let base = std::env::var_os("EGUATEX_DADOS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let pasta = base.join("temporario").join(format!(
        "{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    ));
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    Ok(pasta)
}

fn caminho_fs(endereco: &str) -> Result<FilePath, String> {
    FilePath::from_str(endereco).map_err(|e| format!("Endereço inválido: {e}"))
}

/// Nome com que o arquivo aparece para a pessoa (DISPLAY_NAME do Android).
fn nome_do_endereco(endereco: &str) -> String {
    #[cfg(target_os = "android")]
    if let Some(nome) = android::nome_exibido(endereco) {
        return nome;
    }
    let ultimo = endereco.rsplit('/').next().unwrap_or("arquivo");
    let ultimo = ultimo.rsplit("%2F").next().unwrap_or(ultimo);
    let ultimo = ultimo.rsplit("%3A").next().unwrap_or(ultimo);
    if ultimo.is_empty() { "arquivo".into() } else { ultimo.into() }
}

/// Copia `origem` para `destino` em blocos, sem carregar tudo na memoria, e para ao passar de
/// `limite` bytes (le no maximo `limite + 1`). Passou do limite: apaga o que gravou e devolve erro.
fn copiar_com_limite<R: Read>(origem: R, destino: &Path, limite: u64) -> Result<u64, String> {
    let mut saida = std::fs::File::create(destino).map_err(|e| e.to_string())?;
    let mut limitado = origem.take(limite.saturating_add(1));
    let copiados = match std::io::copy(&mut limitado, &mut saida) {
        Ok(n) => n,
        Err(e) => {
            drop(saida);
            let _ = std::fs::remove_file(destino);
            return Err(format!("Não foi possível ler o arquivo: {e}"));
        }
    };
    if copiados > limite {
        drop(saida);
        let _ = std::fs::remove_file(destino);
        return Err(format!(
            "O arquivo passa do limite de {}. Escolha um arquivo menor.",
            limites_zip::texto_tamanho(limite)
        ));
    }
    Ok(copiados)
}

/// Copia o conteudo do endereco para um arquivo temporario com o nome original, lendo em
/// streaming. `limite` e o maior tamanho aceito, em bytes.
fn trazer<R: Runtime>(app: &AppHandle<R>, endereco: &str, limite: u64) -> Result<(PathBuf, PathBuf), String> {
    let mut leitura = OpenOptions::new();
    leitura.read(true);
    let origem = app
        .fs()
        .open(caminho_fs(endereco)?, leitura)
        .map_err(|e| format!("Não foi possível ler o arquivo: {e}"))?;
    let pasta = pasta_temporaria()?;
    let nome: String = nome_do_endereco(endereco)
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let arquivo = pasta.join(nome);
    if let Err(e) = copiar_com_limite(origem, &arquivo, limite) {
        let _ = std::fs::remove_dir_all(&pasta);
        return Err(e);
    }
    Ok((pasta, arquivo))
}

/// Grava o arquivo local no endereco escolhido na janela de salvar, em streaming.
fn levar<R: Runtime>(app: &AppHandle<R>, local: &PathBuf, endereco: &str) -> Result<(), String> {
    let mut origem = std::fs::File::open(local).map_err(|e| e.to_string())?;
    let mut opcoes = OpenOptions::new();
    opcoes.write(true).create(true).truncate(true);
    let mut arquivo = app
        .fs()
        .open(caminho_fs(endereco)?, opcoes)
        .map_err(|e| format!("Não foi possível gravar no destino: {e}"))?;
    std::io::copy(&mut origem, &mut arquivo).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn cmd_importar_arquivo<R: Runtime>(
    app: AppHandle<R>,
    caminho_projeto: String,
    origem: String,
    pasta_relativa: String,
) -> Result<String, String> {
    if !eh_endereco(&origem) {
        return arquivos::cmd_importar_arquivo(caminho_projeto, origem, pasta_relativa);
    }
    let (pasta, arquivo) = trazer(&app, &origem, Limites::PADRAO.arquivo)?;
    let r = arquivos::cmd_importar_arquivo(caminho_projeto, arquivo.display().to_string(), pasta_relativa);
    let _ = std::fs::remove_dir_all(pasta);
    r
}

/// Figura escolhida no seletor do aparelho (endereco content://): vai para `figuras/` do projeto.
#[tauri::command(async)]
pub fn cmd_importar_figura<R: Runtime>(
    app: AppHandle<R>,
    caminho_projeto: String,
    origem: String,
) -> Result<String, String> {
    if !eh_endereco(&origem) {
        return arquivos::cmd_importar_figura(caminho_projeto, origem);
    }
    let (pasta, arquivo) = trazer(&app, &origem, Limites::PADRAO.arquivo)?;
    let r = arquivos::cmd_importar_figura(caminho_projeto, arquivo.display().to_string());
    let _ = std::fs::remove_dir_all(pasta);
    r
}

#[tauri::command(async)]
pub fn cmd_importar_zip<R: Runtime>(
    app: AppHandle<R>,
    arquivo_zip: String,
    pasta_pai: String,
) -> Result<arquivos::ResultadoImportacao, String> {
    if !eh_endereco(&arquivo_zip) {
        return arquivos::cmd_importar_zip(arquivo_zip, pasta_pai);
    }
    // o zip em si (compactado) pode ter quase o tamanho do que ele guarda: teto do total + 1 MB
    let (pasta, arquivo) = trazer(&app, &arquivo_zip, Limites::PADRAO.total + limites_zip::MIB)?;
    let r = arquivos::cmd_importar_zip(arquivo.display().to_string(), pasta_pai);
    let _ = std::fs::remove_dir_all(pasta);
    r
}

#[tauri::command(async)]
pub fn cmd_copiar_para<R: Runtime>(
    app: AppHandle<R>,
    caminho_projeto: String,
    caminho_relativo: String,
    destino: String,
) -> Result<(), String> {
    if !eh_endereco(&destino) {
        return arquivos::cmd_copiar_para(caminho_projeto, caminho_relativo, destino);
    }
    let pasta = pasta_temporaria()?;
    let local = pasta.join("copia");
    let r = arquivos::cmd_copiar_para(caminho_projeto, caminho_relativo, local.display().to_string())
        .and_then(|_| levar(&app, &local, &destino));
    let _ = std::fs::remove_dir_all(pasta);
    r
}

#[tauri::command(async)]
pub fn cmd_exportar_zip<R: Runtime>(app: AppHandle<R>, caminho_projeto: String, destino: String) -> Result<(), String> {
    if !eh_endereco(&destino) {
        return arquivos::cmd_exportar_zip(caminho_projeto, destino);
    }
    let pasta = pasta_temporaria()?;
    let local = pasta.join("projeto.zip");
    let r = arquivos::cmd_exportar_zip(caminho_projeto, local.display().to_string())
        .and_then(|_| levar(&app, &local, &destino));
    let _ = std::fs::remove_dir_all(pasta);
    r
}

/// Grava um arquivo gerado na tela (o .docx do conversor de PDF para Word, em
/// src/lib/pdf-para-word.ts) no destino do "Salvar como", que no Android e um endereco
/// content://. `dados` vem em base64.
#[tauri::command(async)]
pub fn cmd_gravar_bytes<R: Runtime>(app: AppHandle<R>, destino: String, dados: String) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(dados.as_bytes())
        .map_err(|e| format!("Dados inválidos: {e}"))?;
    if !eh_endereco(&destino) {
        let caminho = PathBuf::from(&destino);
        return crate::projetos::gravar_atomico(&caminho, &bytes)
            .map_err(|e| format!("Não foi possível gravar {destino}: {e}"));
    }
    let pasta = pasta_temporaria()?;
    let local = pasta.join("arquivo");
    let r = std::fs::write(&local, &bytes)
        .map_err(|e| e.to_string())
        .and_then(|_| levar(&app, &local, &destino));
    let _ = std::fs::remove_dir_all(pasta);
    r
}

/// Maior arquivo de atalhos que o aparelho aceita importar: 1 MB (um arquivo exportado tem poucos KB).
const LIMITE_TEXTO_IMPORTADO: u64 = limites_zip::MIB;

/// Le um arquivo local como texto UTF-8, parando em `limite` bytes.
fn ler_texto_limitado(caminho: &Path, limite: u64) -> Result<String, String> {
    let arquivo = std::fs::File::open(caminho).map_err(|e| format!("Não foi possível ler o arquivo: {e}"))?;
    let mut bytes = Vec::new();
    arquivo
        .take(limite.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Não foi possível ler o arquivo: {e}"))?;
    if bytes.len() as u64 > limite {
        return Err(format!(
            "O arquivo passa do limite de {}. Escolha um arquivo menor.",
            limites_zip::texto_tamanho(limite)
        ));
    }
    String::from_utf8(bytes).map_err(|_| "O arquivo não é um texto UTF-8.".to_string())
}

/// So o seletor de arquivos devolve a origem: no Android, um endereco content://. `file://` e
/// caminho solto seriam leitura de qualquer arquivo do app (o front pode ser induzido a mandar
/// qualquer texto), entao so passam no computador de testes, e so se o caminho for absoluto e existir.
fn origem_importada_valida(origem: &str) -> Result<(), String> {
    if origem.starts_with("content://") {
        return Ok(());
    }
    if cfg!(target_os = "android") || origem.starts_with("file://") {
        return Err("Escolha o arquivo pelo seletor de arquivos do aparelho.".into());
    }
    let caminho = Path::new(origem);
    if caminho.is_absolute() && caminho.is_file() {
        Ok(())
    } else {
        Err("Escolha o arquivo pelo seletor de arquivos do aparelho.".into())
    }
}

/// Texto de um arquivo escolhido no seletor do aparelho (endereco content:// no Android), usado pela
/// importacao de atalhos. Passa por uma pasta temporaria, com limite de 1 MB.
#[tauri::command(async)]
pub fn cmd_ler_texto_importado<R: Runtime>(app: AppHandle<R>, origem: String) -> Result<String, String> {
    origem_importada_valida(&origem)?;
    if !eh_endereco(&origem) {
        return ler_texto_limitado(Path::new(&origem), LIMITE_TEXTO_IMPORTADO);
    }
    let (pasta, arquivo) = trazer(&app, &origem, LIMITE_TEXTO_IMPORTADO)?;
    let r = ler_texto_limitado(&arquivo, LIMITE_TEXTO_IMPORTADO);
    let _ = std::fs::remove_dir_all(pasta);
    r
}

/// "Abrir pasta existente" no Android: abre o seletor de pastas do sistema e copia a pasta escolhida
/// para `pasta_pai` (a pasta de projetos do app). Devolve o caminho da copia, ou None se a pessoa cancelar.
#[tauri::command]
pub async fn cmd_escolher_pasta_android(pasta_pai: String) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        tauri::async_runtime::spawn_blocking(move || android::escolher_pasta(&pasta_pai))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = pasta_pai;
        Err("Só no Android.".into())
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{JObject, JValue};

    /// Chama um metodo estatico de br.ufpa.eguatex.EscolherPasta pelo class loader do app.
    fn chamar<'a>(
        env: &mut jni::JNIEnv<'a>,
        contexto: &JObject,
        metodo: &str,
        assinatura: &str,
        args: &[JValue],
    ) -> jni::errors::Result<jni::objects::JValueOwned<'a>> {
        let carregador = env.call_method(contexto, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
        let nome = env.new_string("br.ufpa.eguatex.EscolherPasta")?;
        let classe = env
            .call_method(&carregador, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&nome)])?
            .l()?;
        let classe = jni::objects::JClass::from(classe);
        env.call_static_method(&classe, metodo, assinatura, args)
    }

    pub fn escolher_pasta(pasta_pai: &str) -> Result<Option<String>, String> {
        let ctx = ndk_context::android_context();
        // SAFETY: o tao inicializa o contexto Android antes de abrir a janela.
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let contexto = unsafe { JObject::from_raw(ctx.context().cast()) };
        let pai = env.new_string(pasta_pai).map_err(|e| e.to_string())?;
        let abriu = chamar(&mut env, &contexto, "escolher", "(Ljava/lang/String;)Z", &[JValue::Object(&pai)])
            .and_then(|v| v.z())
            .map_err(|e| format!("Não foi possível abrir o seletor de pastas: {e}"))?;
        if !abriu {
            return Err("Não foi possível abrir o seletor de pastas.".into());
        }
        // a pessoa escolhe a pasta e o Kotlin copia; ate 15 minutos
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(900);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let valor = chamar(&mut env, &contexto, "resultado", "()Ljava/lang/String;", &[])
                .and_then(|v| v.l())
                .map_err(|e| e.to_string())?;
            if !valor.is_null() {
                let texto: String = env.get_string(&valor.into()).map_err(|e| e.to_string())?.into();
                if texto.is_empty() {
                    return Ok(None);
                }
                if let Some(erro) = texto.strip_prefix("erro:") {
                    return Err(format!("Não foi possível copiar a pasta: {erro}"));
                }
                return Ok(Some(texto));
            }
            if std::time::Instant::now() > limite {
                return Err("O seletor de pastas não respondeu.".into());
            }
        }
    }

    /// Pergunta ao ContentResolver o nome do arquivo (coluna `_display_name`).
    pub fn nome_exibido(endereco: &str) -> Option<String> {
        let ctx = ndk_context::android_context();
        // SAFETY: o tao inicializa o contexto Android antes de abrir a janela.
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let contexto = unsafe { JObject::from_raw(ctx.context().cast()) };
        let resultado = (|| -> jni::errors::Result<Option<String>> {
            let texto = env.new_string(endereco)?;
            let uri = env
                .call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&texto)])?
                .l()?;
            let resolver = env
                .call_method(&contexto, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?
                .l()?;
            let nula = JObject::null();
            let cursor = env
                .call_method(
                    &resolver,
                    "query",
                    "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                    &[JValue::Object(&uri), JValue::Object(&nula), JValue::Object(&nula), JValue::Object(&nula), JValue::Object(&nula)],
                )?
                .l()?;
            if cursor.is_null() {
                return Ok(None);
            }
            let mut nome = None;
            if env.call_method(&cursor, "moveToFirst", "()Z", &[])?.z()? {
                let coluna = env.new_string("_display_name")?;
                let indice = env
                    .call_method(&cursor, "getColumnIndex", "(Ljava/lang/String;)I", &[JValue::Object(&coluna)])?
                    .i()?;
                if indice >= 0 {
                    let valor = env.call_method(&cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(indice)])?.l()?;
                    if !valor.is_null() {
                        nome = Some(env.get_string(&valor.into())?.into());
                    }
                }
            }
            env.call_method(&cursor, "close", "()V", &[])?;
            Ok(nome)
        })();
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        resultado.ok().flatten()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("eguatex-teste-ponte-{}-{}", std::process::id(), nome));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn importar_texto_recusa_file_e_caminho_solto() {
        assert!(origem_importada_valida("content://com.android.providers/doc/1").is_ok());
        assert!(origem_importada_valida("file:///data/data/app/estado.json").is_err());
        assert!(origem_importada_valida("relativo/atalhos.json").is_err());
        assert!(origem_importada_valida("/nao/existe/atalhos.json").is_err());
        if !cfg!(target_os = "android") {
            let p = pasta("importar-texto");
            let arq = p.join("atalhos.json");
            std::fs::write(&arq, "{}").unwrap();
            assert!(origem_importada_valida(&arq.display().to_string()).is_ok());
            let _ = std::fs::remove_dir_all(p);
        }
    }

    #[test]
    fn ponte_leitura_para_no_limite() {
        let p = pasta("limite");
        let destino = p.join("grande.zip");
        // 400 MB de origem, teto de 1 MB: so 1 MB + 1 byte sao puxados e nada fica no disco
        let mut origem = io::repeat(1).take(400 * limites_zip::MIB);
        let erro = copiar_com_limite(&mut origem, &destino, limites_zip::MIB).unwrap_err();
        assert!(erro.contains("1 MB"), "a mensagem deve dizer o limite: {erro}");
        assert_eq!(origem.limit(), 400 * limites_zip::MIB - (limites_zip::MIB + 1));
        assert!(!destino.exists(), "sobrou arquivo parcial");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn ler_texto_limitado_aceita_texto_e_recusa_grande_e_binario() {
        let p = pasta("texto");
        let ok = p.join("atalhos.json");
        std::fs::write(&ok, "{\"formato\":\"eguatex-atalhos\"}").unwrap();
        assert!(ler_texto_limitado(&ok, 1000).unwrap().contains("eguatex-atalhos"));
        assert!(ler_texto_limitado(&ok, 5).unwrap_err().contains("limite"));
        let bin = p.join("bin.json");
        std::fs::write(&bin, [0xff, 0xfe, 0xfd]).unwrap();
        assert!(ler_texto_limitado(&bin, 1000).unwrap_err().contains("UTF-8"));
        assert!(ler_texto_limitado(&p.join("nao-existe"), 1000).is_err());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn ponte_leitura_copia_o_que_cabe_em_blocos() {
        let p = pasta("cabe");
        let destino = p.join("ok.bin");
        let n = copiar_com_limite(io::repeat(9).take(3 * limites_zip::MIB), &destino, 4 * limites_zip::MIB).unwrap();
        assert_eq!(n, 3 * limites_zip::MIB);
        assert_eq!(std::fs::metadata(&destino).unwrap().len(), 3 * limites_zip::MIB);
        // exatamente no teto ainda passa
        assert!(copiar_com_limite(io::repeat(9).take(1000), &p.join("exato"), 1000).is_ok());
        let _ = std::fs::remove_dir_all(p);
    }
}
