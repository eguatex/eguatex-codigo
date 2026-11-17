//! "Procurar atualização": só roda quando a pessoa clica no menu, nunca sozinho.
//! Faz uma consulta (GET) à API de releases do GitHub e compara a versão publicada com a
//! instalada. Nada é enviado além do nome e da versão do app no cabeçalho User-Agent,
//! que o GitHub exige em toda chamada. Sem release, sem internet ou com erro, devolve o
//! estado e a tela mostra uma mensagem; nada trava.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::Duration;

/// Repositório do código e das releases (conta e nome no GitHub).
const REPOSITORIO: &str = "eguatex/eguatex-codigo";

/// Extensão do arquivo de instalação que o botão de baixar deve apontar. `None`: a página da
/// release. No Android o link da versão nova aponta direto para o .apk.
const EXTENSAO_DO_INSTALADOR: Option<&str> = Some(".apk");

/// Resposta para a tela. `estado` é um destes: `nova`, `atualizado`, `sem-release`,
/// `sem-internet`, `erro`. Os textos em português ficam em `dados/i18n/pt-BR.json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultadoAtualizacao {
    pub estado: String,
    pub versao_atual: String,
    pub versao_nova: Option<String>,
    pub link: Option<String>,
}

#[derive(Deserialize)]
struct ReleaseGithub {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<AssetGithub>,
}

#[derive(Deserialize)]
struct AssetGithub {
    name: String,
    browser_download_url: String,
}

/// "v0.2.0", "0.2" e "1.4.0-beta.1" viram os números `[0, 2, 0]`, `[0, 2]` e `[1, 4, 0]`.
/// Sufixo de pré-lançamento (depois de `-` ou `+`) é ignorado. Texto sem número dá `None`.
fn numeros_da_versao(texto: &str) -> Option<Vec<u64>> {
    let limpo = texto.trim().trim_start_matches(['v', 'V']);
    let nucleo = limpo.split(['-', '+']).next()?;
    let partes: Option<Vec<u64>> = nucleo.split('.').map(|p| p.parse().ok()).collect();
    partes.filter(|p| !p.is_empty())
}

/// Compara duas versões número a número, completando com zeros: `0.2` e `0.2.0` são iguais.
/// `0.10.0` é maior que `0.9.0` (a conta é numérica, não de texto). Versão ilegível dá `None`.
pub fn comparar_versoes(a: &str, b: &str) -> Option<Ordering> {
    let (va, vb) = (numeros_da_versao(a)?, numeros_da_versao(b)?);
    for i in 0..va.len().max(vb.len()) {
        let (x, y) = (va.get(i).copied().unwrap_or(0), vb.get(i).copied().unwrap_or(0));
        if x != y {
            return Some(x.cmp(&y));
        }
    }
    Some(Ordering::Equal)
}

fn resultado(estado: &str, atual: &str, nova: Option<String>, link: Option<String>) -> ResultadoAtualizacao {
    ResultadoAtualizacao { estado: estado.to_string(), versao_atual: atual.to_string(), versao_nova: nova, link }
}

/// Lê o JSON de uma release do GitHub e diz se há versão mais nova que `atual`.
/// Separada da rede para testar com um JSON de exemplo.
fn interpretar_release(json: &str, atual: &str, extensao: Option<&str>) -> ResultadoAtualizacao {
    let Ok(release) = serde_json::from_str::<ReleaseGithub>(json) else {
        return resultado("erro", atual, None, None);
    };
    match comparar_versoes(atual, &release.tag_name) {
        Some(Ordering::Less) => {
            let instalador = extensao.and_then(|ext| {
                release.assets.iter().find(|a| a.name.to_lowercase().ends_with(ext)).map(|a| a.browser_download_url.clone())
            });
            let versao = release.tag_name.trim_start_matches(['v', 'V']).to_string();
            resultado("nova", atual, Some(versao), Some(instalador.unwrap_or(release.html_url)))
        }
        Some(_) => resultado("atualizado", atual, None, None),
        None => resultado("erro", atual, None, None),
    }
}

/// Consulta a última release publicada. Nunca falha: o estado vai no resultado.
/// Arquivo que o instalador do Windows grava ao lado do programa quando a caixa
/// "Avisar quando houver versao nova" foi marcada.
const MARCA_DO_INSTALADOR: &str = "avisar-versao-nova.txt";

/// Le e apaga a marca do instalador. Sem a marca (ou no Android), devolve false.
fn ler_marca_do_instalador(pasta: &std::path::Path) -> bool {
    let marca = pasta.join(MARCA_DO_INSTALADOR);
    if !marca.is_file() {
        return false;
    }
    let _ = std::fs::remove_file(&marca);
    true
}

#[tauri::command]
pub fn cmd_aviso_do_instalador() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(ler_marca_do_instalador))
        .unwrap_or(false)
}

#[tauri::command]
pub async fn cmd_procurar_atualizacao() -> ResultadoAtualizacao {
    let atual = env!("CARGO_PKG_VERSION");
    let cliente = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(format!("EguaTex/{atual}"))
        .build()
    {
        Ok(c) => c,
        Err(_) => return resultado("erro", atual, None, None),
    };
    let url = format!("https://api.github.com/repos/{REPOSITORIO}/releases/latest");
    let resposta = match cliente.get(url).header("Accept", "application/vnd.github+json").send().await {
        Ok(r) => r,
        Err(e) if e.is_connect() || e.is_timeout() => return resultado("sem-internet", atual, None, None),
        Err(_) => return resultado("erro", atual, None, None),
    };
    // 404: o repositório ainda não publicou nenhuma release
    if resposta.status() == reqwest::StatusCode::NOT_FOUND {
        return resultado("sem-release", atual, None, None);
    }
    if !resposta.status().is_success() {
        return resultado("erro", atual, None, None);
    }
    match resposta.text().await {
        Ok(json) => interpretar_release(&json, atual, EXTENSAO_DO_INSTALADOR),
        Err(_) => resultado("erro", atual, None, None),
    }
}

#[cfg(test)]
mod testes {
    #[test]
    fn marca_do_instalador_e_lida_uma_vez() {
        let pasta = std::env::temp_dir().join(format!("eguatex-marca-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        assert!(!super::ler_marca_do_instalador(&pasta));
        std::fs::write(pasta.join(super::MARCA_DO_INSTALADOR), "1").unwrap();
        assert!(super::ler_marca_do_instalador(&pasta));
        assert!(!super::ler_marca_do_instalador(&pasta));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    use super::*;

    #[test]
    fn versao_menor_e_maior() {
        assert_eq!(comparar_versoes("0.1.0", "0.2.0"), Some(Ordering::Less));
        assert_eq!(comparar_versoes("0.2.0", "0.1.0"), Some(Ordering::Greater));
        assert_eq!(comparar_versoes("1.0.0", "0.9.9"), Some(Ordering::Greater));
    }

    #[test]
    fn versoes_iguais() {
        assert_eq!(comparar_versoes("0.1.0", "0.1.0"), Some(Ordering::Equal));
        assert_eq!(comparar_versoes("0.2", "0.2.0"), Some(Ordering::Equal));
    }

    #[test]
    fn formato_com_v_na_frente() {
        assert_eq!(comparar_versoes("0.1.0", "v0.2.0"), Some(Ordering::Less));
        assert_eq!(comparar_versoes("v0.2.0", "0.2.0"), Some(Ordering::Equal));
        assert_eq!(comparar_versoes("V1.0.0", "v1.0.1"), Some(Ordering::Less));
    }

    #[test]
    fn conta_e_numerica_nao_de_texto() {
        assert_eq!(comparar_versoes("0.9.0", "0.10.0"), Some(Ordering::Less));
    }

    #[test]
    fn sufixo_de_pre_lancamento_e_ignorado() {
        assert_eq!(comparar_versoes("0.1.0", "0.2.0-beta.1"), Some(Ordering::Less));
    }

    #[test]
    fn texto_sem_numero_nao_compara() {
        assert_eq!(comparar_versoes("0.1.0", "ultima"), None);
        assert_eq!(comparar_versoes("", "0.1.0"), None);
        assert_eq!(comparar_versoes("0.1.x", "0.1.0"), None);
    }

    const RELEASE: &str = r#"{
        "tag_name": "v0.2.0",
        "html_url": "https://github.com/jessicamelo0532/egua-tex/releases/tag/v0.2.0",
        "assets": [
            {"name": "EguaTex_0.2.0_x64-setup.exe", "browser_download_url": "https://exemplo/EguaTex_0.2.0_x64-setup.exe"},
            {"name": "EguaTex_android.apk", "browser_download_url": "https://exemplo/EguaTex_android.apk"}
        ]
    }"#;

    #[test]
    fn release_mais_nova_aponta_para_a_pagina() {
        let r = interpretar_release(RELEASE, "0.1.0", None);
        assert_eq!(r.estado, "nova");
        assert_eq!(r.versao_nova.as_deref(), Some("0.2.0"));
        assert_eq!(r.link.as_deref(), Some("https://github.com/jessicamelo0532/egua-tex/releases/tag/v0.2.0"));
    }

    #[test]
    fn release_mais_nova_aponta_para_o_apk_quando_pedido() {
        let r = interpretar_release(RELEASE, "0.1.0", Some(".apk"));
        assert_eq!(r.link.as_deref(), Some("https://exemplo/EguaTex_android.apk"));
        // sem arquivo com a extensao, cai na pagina da release
        let r = interpretar_release(RELEASE, "0.1.0", Some(".dmg"));
        assert_eq!(r.link.as_deref(), Some("https://github.com/jessicamelo0532/egua-tex/releases/tag/v0.2.0"));
    }

    #[test]
    fn mesma_versao_ou_mais_antiga_na_release_e_atualizado() {
        assert_eq!(interpretar_release(RELEASE, "0.2.0", None).estado, "atualizado");
        assert_eq!(interpretar_release(RELEASE, "0.3.0", None).estado, "atualizado");
        assert_eq!(interpretar_release(RELEASE, "0.2.0", None).link, None);
    }

    #[test]
    fn json_estranho_ou_tag_ilegivel_vira_erro() {
        assert_eq!(interpretar_release("nao e json", "0.1.0", None).estado, "erro");
        assert_eq!(interpretar_release(r#"{"tag_name":"ultima","html_url":"x"}"#, "0.1.0", None).estado, "erro");
    }

    /// Consulta de verdade ao GitHub (precisa de internet):
    ///   cargo test --lib consulta_real_do_github -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn consulta_real_do_github() {
        let r = cmd_procurar_atualizacao().await;
        println!("{r:?}");
        assert!(["nova", "atualizado", "sem-release", "sem-internet", "erro"].contains(&r.estado.as_str()));
    }
}
