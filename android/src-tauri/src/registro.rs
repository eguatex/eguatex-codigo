//! Log em arquivo do fluxo do Google Drive (Picker, download e extracao), para diagnosticar
//! falhas sem depender de quem testa. Grava em `<dados locais>/ÉguaTex/logs/drive.log`
//! (no Windows, `%LOCALAPPDATA%\ÉguaTex\logs`). Nunca recebe token nem segredo.
//! Passa de 1 MB: o arquivo antigo vira `drive.log.1` e um novo comeca.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LIMITE: u64 = 1 << 20;

#[cfg(not(test))]
fn pasta() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("ÉguaTex").join("logs")
}

/// Nos testes o log vai para a pasta temporaria, para o `cargo test` nao poluir o `drive.log` real.
#[cfg(test)]
fn pasta() -> PathBuf {
    std::env::temp_dir().join("eguatex-testes").join("logs")
}

/// Caminho do log do Drive.
pub fn arquivo() -> PathBuf {
    pasta().join("drive.log")
}

/// Acrescenta uma linha `segundos-desde-1970 [etapa] texto`. Falhas de escrita sao ignoradas:
/// o log nunca derruba o fluxo.
pub fn drive(etapa: &str, texto: &str) {
    let caminho = arquivo();
    if let Some(p) = caminho.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if std::fs::metadata(&caminho).map(|m| m.len() > LIMITE).unwrap_or(false) {
        let _ = std::fs::rename(&caminho, caminho.with_extension("log.1"));
    }
    let agora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let linha = format!("{agora} [{etapa}] {}\n", texto.replace(['\r', '\n'], " "));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&caminho) {
        let _ = f.write_all(linha.as_bytes());
    }
}

#[cfg(test)]
mod testes {
    #[test]
    fn grava_linha_sem_quebra() {
        super::drive("teste", "linha um\nlinha dois");
        let texto = std::fs::read_to_string(super::arquivo()).unwrap();
        assert!(texto.contains("[teste] linha um linha dois"));
    }

    #[test]
    fn nos_testes_o_log_fica_na_pasta_temporaria() {
        assert!(super::arquivo().starts_with(std::env::temp_dir()));
    }
}
