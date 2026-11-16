//! Versoes anteriores (historico local) dos arquivos de texto do projeto.
//! Ficam nos dados do app, fora da pasta do projeto, para nao irem ao Drive nem ao .zip.
//! Layout: <dados>/eguatex/versoes/<hash do projeto>/<hash do arquivo>/<milissegundos>.ver

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::projetos;

/// Versoes guardadas por arquivo.
const MAX_VERSOES_ARQUIVO: usize = 20;
/// Tamanho total por projeto (50 MB). Passou disso, as versoes mais antigas saem.
const MAX_BYTES_PROJETO: u64 = 50 * 1024 * 1024;
/// Arquivos maiores que isso (10 MB) nao entram no historico.
const MAX_BYTES_ARQUIVO: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Versao {
    /// Milissegundos desde 1970; tambem e o nome do arquivo da versao.
    pub id: String,
    pub data_ms: u64,
    pub tamanho: u64,
    /// Caminho relativo do arquivo a que a versao pertence.
    #[serde(default)]
    pub arquivo: String,
    /// "criado" na versao mais antiga do arquivo, "alterado" nas seguintes.
    #[serde(default)]
    pub estado: String,
    /// Bytes a mais (ou a menos) em relacao a versao anterior.
    #[serde(default)]
    pub delta: i64,
}

pub(crate) fn pasta_dados() -> PathBuf {
    // nos testes sem EGUATEX_DADOS, o historico vai para o temp e nao para os dados reais da pessoa
    #[cfg(test)]
    if std::env::var_os("EGUATEX_DADOS").is_none() {
        return std::env::temp_dir().join("eguatex-dados-teste").join("versoes");
    }
    std::env::var_os("EGUATEX_DADOS")
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("eguatex")
        .join("versoes")
}

fn hash_curto(texto: &str) -> String {
    let h = Sha256::digest(texto.as_bytes());
    h.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Nome da pasta do projeto no historico: o hash da string do caminho, sem barra no fim.
pub(crate) fn hash_do_projeto(caminho_projeto: &str) -> String {
    hash_curto(caminho_projeto.trim_end_matches(['/', '\\']))
}

fn pasta_projeto(raiz: &Path, caminho_projeto: &str) -> PathBuf {
    raiz.join(hash_do_projeto(caminho_projeto))
}

fn pasta_arquivo(raiz: &Path, caminho_projeto: &str, caminho_relativo: &str) -> PathBuf {
    let rel = caminho_relativo.trim_start_matches(['/', '\\']).replace('\\', "/");
    pasta_projeto(raiz, caminho_projeto).join(hash_curto(&rel))
}

fn agora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn listar_pasta(pasta: &Path) -> Vec<Versao> {
    let mut v: Vec<Versao> = match fs::read_dir(pasta) {
        Ok(e) => e
            .filter_map(|r| r.ok())
            .filter_map(|e| {
                let nome = e.file_name().to_string_lossy().into_owned();
                let id = nome.strip_suffix(".ver")?.to_string();
                let data_ms = id.parse::<u64>().ok()?;
                let tamanho = e.metadata().ok()?.len();
                Some(Versao { id, data_ms, tamanho, arquivo: String::new(), estado: String::new(), delta: 0 })
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort_by_key(|a| std::cmp::Reverse(a.data_ms));
    anotar_estados(&mut v);
    v
}

/// Preenche `estado` e `delta` numa lista ordenada da mais nova para a mais antiga.
pub(crate) fn anotar_estados(v: &mut [Versao]) {
    let n = v.len();
    for i in 0..n {
        let anterior = if i + 1 < n { Some(v[i + 1].tamanho) } else { None };
        match anterior {
            Some(t) => {
                v[i].estado = "alterado".to_string();
                v[i].delta = v[i].tamanho as i64 - t as i64;
            }
            None => {
                v[i].estado = "criado".to_string();
                v[i].delta = v[i].tamanho as i64;
            }
        }
    }
}

fn com_arquivo(mut v: Vec<Versao>, caminho_relativo: &str) -> Vec<Versao> {
    let rel = caminho_relativo.trim_start_matches(['/', '\\']).replace('\\', "/");
    for x in &mut v {
        x.arquivo = rel.clone();
    }
    v
}

/// Guarda `conteudo` como versao nova, salvo se igual a mais recente ou se a mais recente
/// e mais nova que `intervalo_min_ms`. Devolve se gravou.
fn gravar_em(
    raiz: &Path,
    caminho_projeto: &str,
    caminho_relativo: &str,
    conteudo: &str,
    intervalo_min_ms: u64,
    max_versoes: usize,
    max_bytes: u64,
) -> Result<bool, String> {
    if conteudo.len() as u64 > MAX_BYTES_ARQUIVO {
        return Ok(false);
    }
    let pasta = pasta_arquivo(raiz, caminho_projeto, caminho_relativo);
    let existentes = listar_pasta(&pasta);
    let agora = agora_ms();
    if let Some(ultima) = existentes.first() {
        if agora.saturating_sub(ultima.data_ms) < intervalo_min_ms {
            return Ok(false);
        }
        let igual = fs::read(pasta.join(format!("{}.ver", ultima.id)))
            .map(|b| b == conteudo.as_bytes())
            .unwrap_or(false);
        if igual {
            return Ok(false);
        }
    }
    fs::create_dir_all(&pasta).map_err(|e| format!("Erro ao criar pasta de versões: {e}"))?;
    // ids sempre crescentes, mesmo com duas gravacoes no mesmo milissegundo
    let id = existentes.first().map_or(agora, |u| agora.max(u.data_ms + 1));
    fs::write(pasta.join(format!("{id}.ver")), conteudo.as_bytes())
        .map_err(|e| format!("Erro ao guardar versão: {e}"))?;
    podar(raiz, caminho_projeto, &pasta, max_versoes, max_bytes);
    Ok(true)
}

/// Mantem as ultimas `max_versoes` do arquivo e o tamanho total do projeto dentro do limite.
fn podar(raiz: &Path, caminho_projeto: &str, pasta_do_arquivo: &Path, max_versoes: usize, max_bytes: u64) {
    for v in listar_pasta(pasta_do_arquivo).into_iter().skip(max_versoes) {
        let _ = fs::remove_file(pasta_do_arquivo.join(format!("{}.ver", v.id)));
    }
    // limite do projeto: remove as mais antigas de qualquer arquivo, deixando ao menos
    // a versao mais recente de cada um
    let mut todas: Vec<(u64, u64, PathBuf)> = Vec::new();
    let mut total = 0u64;
    if let Ok(arquivos) = fs::read_dir(pasta_projeto(raiz, caminho_projeto)) {
        for pasta in arquivos.filter_map(|r| r.ok()).map(|e| e.path()).filter(|p| p.is_dir()) {
            for (i, v) in listar_pasta(&pasta).into_iter().enumerate() {
                total += v.tamanho;
                if i > 0 {
                    todas.push((v.data_ms, v.tamanho, pasta.join(format!("{}.ver", v.id))));
                }
            }
        }
    }
    todas.sort();
    for (_, tamanho, caminho) in todas {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(&caminho).is_ok() {
            total = total.saturating_sub(tamanho);
        }
    }
}

fn id_valido(id: &str) -> Result<(), String> {
    if !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err("Versão inválida.".to_string())
    }
}

fn ler_de(raiz: &Path, caminho_projeto: &str, caminho_relativo: &str, id: &str) -> Result<String, String> {
    id_valido(id)?;
    let arq = pasta_arquivo(raiz, caminho_projeto, caminho_relativo).join(format!("{id}.ver"));
    let bytes = fs::read(&arq).map_err(|_| "Essa versão não existe mais.".to_string())?;
    String::from_utf8(bytes).map_err(|_| "A versão não é texto.".to_string())
}

/// Guarda a versao atual (a que esta em disco) do arquivo. Arquivo que nao abre como texto
/// ou que ainda nao existe e ignorado.
fn gravar_atual(raiz: &Path, caminho_projeto: &str, caminho_relativo: &str, intervalo_s: u64) -> Result<bool, String> {
    let Ok(texto) = projetos::ler_arquivo(caminho_projeto, caminho_relativo) else {
        return Ok(false);
    };
    gravar_em(
        raiz,
        caminho_projeto,
        caminho_relativo,
        &texto,
        intervalo_s.saturating_mul(1000),
        MAX_VERSOES_ARQUIVO,
        MAX_BYTES_PROJETO,
    )
}

/// Arquivos que o historico aceita mesmo em Latin-1 ou Windows-1252 (o editor os abre assim).
const EXTENSOES_DE_TEXTO: &[&str] = &["tex", "bib", "sty", "cls", "bst", "txt", "md"];

/// Texto de verdade: sem byte nulo e em UTF-8, ou um arquivo de LaTeX em codificacao antiga.
/// JPEG e outros binarios sem byte nulo ficam de fora.
fn parece_texto(rel: &str, bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return false;
    }
    let sem_bom = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if std::str::from_utf8(sem_bom).is_ok() {
        return true;
    }
    let ext = rel.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    EXTENSOES_DE_TEXTO.contains(&ext.as_str())
}

/// Guarda o texto que esta em disco antes de uma rotina do app (a sincronizacao do Drive) trocar ou
/// apagar o arquivo, mesmo dentro do intervalo de 5 min do salvamento. Arquivo que nao existe, que
/// nao e texto (figura, PDF) ou que passa de 10 MB fica fora. Devolve se guardou uma versao nova.
/// `projeto` e a pasta como a interface a conhece: o hash do historico usa essa string.
pub(crate) fn guardar_antes_de_mudar(raiz: &Path, projeto: &Path, rel: &str) -> bool {
    let arquivo = rel
        .split('/')
        .filter(|parte| !parte.is_empty())
        .fold(projeto.to_path_buf(), |p, parte| p.join(parte));
    let Ok(meta) = fs::metadata(&arquivo) else {
        return false;
    };
    if !meta.is_file() || meta.len() > MAX_BYTES_ARQUIVO {
        return false;
    }
    let Ok(bytes) = fs::read(&arquivo) else {
        return false;
    };
    if !parece_texto(rel, &bytes) {
        return false;
    }
    gravar_atual(raiz, &projeto.to_string_lossy(), rel, 0).unwrap_or(false)
}

/// Textos das versoes guardadas, da mais nova para a mais antiga (so para os testes).
#[cfg(test)]
pub(crate) fn textos_guardados(raiz: &Path, caminho_projeto: &str, caminho_relativo: &str) -> Vec<String> {
    listar_pasta(&pasta_arquivo(raiz, caminho_projeto, caminho_relativo))
        .iter()
        .filter_map(|v| ler_de(raiz, caminho_projeto, caminho_relativo, &v.id).ok())
        .collect()
}

/// Restaura: guarda antes a versao atual e depois grava a escolhida. Devolve o texto restaurado.
fn restaurar_em(raiz: &Path, caminho_projeto: &str, caminho_relativo: &str, id: &str) -> Result<String, String> {
    let texto = ler_de(raiz, caminho_projeto, caminho_relativo, id)?;
    gravar_atual(raiz, caminho_projeto, caminho_relativo, 0)?;
    projetos::salvar_arquivo(caminho_projeto, caminho_relativo, &texto)?;
    Ok(texto)
}

/// Guarda o estado em disco do arquivo como versao, se passou `intervalo_s` desde a ultima.
/// Com 0 guarda sempre que o texto mudou desde a ultima versao.
#[tauri::command]
pub fn cmd_versoes_gravar(caminho_projeto: String, caminho_relativo: String, intervalo_s: u64) -> Result<bool, String> {
    gravar_atual(&pasta_dados(), &caminho_projeto, &caminho_relativo, intervalo_s)
}

#[tauri::command]
pub fn cmd_versoes_listar(caminho_projeto: String, caminho_relativo: String) -> Vec<Versao> {
    com_arquivo(
        listar_pasta(&pasta_arquivo(&pasta_dados(), &caminho_projeto, &caminho_relativo)),
        &caminho_relativo,
    )
}

#[tauri::command]
pub fn cmd_versoes_ler(caminho_projeto: String, caminho_relativo: String, id: String) -> Result<String, String> {
    ler_de(&pasta_dados(), &caminho_projeto, &caminho_relativo, &id)
}

#[tauri::command]
pub fn cmd_versoes_restaurar(caminho_projeto: String, caminho_relativo: String, id: String) -> Result<String, String> {
    restaurar_em(&pasta_dados(), &caminho_projeto, &caminho_relativo, &id)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn versao(data_ms: u64, tamanho: u64) -> Versao {
        Versao { id: data_ms.to_string(), data_ms, tamanho, arquivo: String::new(), estado: String::new(), delta: 0 }
    }

    #[test]
    fn anotar_estados_marca_criado_na_mais_antiga_e_delta_nas_outras() {
        let mut v = vec![versao(3000, 150), versao(2000, 100), versao(1000, 120)];
        anotar_estados(&mut v);
        assert_eq!(v[2].estado, "criado");
        assert_eq!(v[2].delta, 120);
        assert_eq!(v[1].estado, "alterado");
        assert_eq!(v[1].delta, -20);
        assert_eq!(v[0].estado, "alterado");
        assert_eq!(v[0].delta, 50);
    }

    #[test]
    fn anotar_estados_lista_vazia_e_unica() {
        let mut vazia: Vec<Versao> = Vec::new();
        anotar_estados(&mut vazia);
        assert!(vazia.is_empty());
        let mut uma = vec![versao(1, 7)];
        anotar_estados(&mut uma);
        assert_eq!(uma[0].estado, "criado");
        assert_eq!(uma[0].delta, 7);
    }

    #[test]
    fn com_arquivo_normaliza_o_caminho() {
        let v = com_arquivo(vec![versao(1, 1)], "\\cap\\intro.tex");
        assert_eq!(v[0].arquivo, "cap/intro.tex");
    }

    fn pasta_temp(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("eguatex-versoes-{nome}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn grava_e_lista_sem_repetir_igual() {
        let raiz = pasta_temp("lista");
        assert!(gravar_em(&raiz, "/p", "/a.tex", "um", 0, 20, u64::MAX).unwrap());
        assert!(!gravar_em(&raiz, "/p", "/a.tex", "um", 0, 20, u64::MAX).unwrap());
        assert!(gravar_em(&raiz, "/p", "/a.tex", "dois", 0, 20, u64::MAX).unwrap());
        let v = listar_pasta(&pasta_arquivo(&raiz, "/p", "/a.tex"));
        assert_eq!(v.len(), 2);
        assert!(v[0].data_ms > v[1].data_ms);
        assert_eq!(ler_de(&raiz, "/p", "/a.tex", &v[0].id).unwrap(), "dois");
        assert_eq!(v[1].tamanho, 2);
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn respeita_o_intervalo_minimo() {
        let raiz = pasta_temp("intervalo");
        assert!(gravar_em(&raiz, "/p", "/a.tex", "um", 300_000, 20, u64::MAX).unwrap());
        assert!(!gravar_em(&raiz, "/p", "/a.tex", "dois", 300_000, 20, u64::MAX).unwrap());
        assert_eq!(listar_pasta(&pasta_arquivo(&raiz, "/p", "/a.tex")).len(), 1);
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn poda_por_quantidade() {
        let raiz = pasta_temp("poda");
        for i in 0..25 {
            gravar_em(&raiz, "/p", "/a.tex", &format!("v{i}"), 0, 20, u64::MAX).unwrap();
        }
        let v = listar_pasta(&pasta_arquivo(&raiz, "/p", "/a.tex"));
        assert_eq!(v.len(), 20);
        assert_eq!(ler_de(&raiz, "/p", "/a.tex", &v[0].id).unwrap(), "v24");
        assert_eq!(ler_de(&raiz, "/p", "/a.tex", &v[19].id).unwrap(), "v5");
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn poda_por_tamanho_do_projeto() {
        let raiz = pasta_temp("tamanho");
        for i in 0..6 {
            gravar_em(&raiz, "/p", "/a.tex", &format!("{i}{}", "x".repeat(99)), 0, 20, 350).unwrap();
        }
        let v = listar_pasta(&pasta_arquivo(&raiz, "/p", "/a.tex"));
        assert_eq!(v.len(), 3);
        assert!(v[0].tamanho == 100);
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn restaura_guardando_a_versao_atual() {
        let raiz = pasta_temp("restaura");
        let proj = pasta_temp("restaura-proj");
        let proj_s = proj.to_string_lossy().into_owned();
        projetos::salvar_arquivo(&proj_s, "/a.tex", "texto bom").unwrap();
        assert!(gravar_atual(&raiz, &proj_s, "/a.tex", 0).unwrap());
        projetos::salvar_arquivo(&proj_s, "/a.tex", "colagem errada").unwrap();
        let antiga = listar_pasta(&pasta_arquivo(&raiz, &proj_s, "/a.tex")).remove(0);
        let devolvido = restaurar_em(&raiz, &proj_s, "/a.tex", &antiga.id).unwrap();
        assert_eq!(devolvido, "texto bom");
        assert_eq!(projetos::ler_arquivo(&proj_s, "/a.tex").unwrap(), "texto bom");
        let v = listar_pasta(&pasta_arquivo(&raiz, &proj_s, "/a.tex"));
        assert_eq!(v.len(), 2);
        assert_eq!(ler_de(&raiz, &proj_s, "/a.tex", &v[0].id).unwrap(), "colagem errada");
        let _ = fs::remove_dir_all(&raiz);
        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn recusa_id_com_caminho() {
        let raiz = pasta_temp("id");
        assert!(ler_de(&raiz, "/p", "/a.tex", "../../x").is_err());
        let _ = fs::remove_dir_all(&raiz);
    }
}
