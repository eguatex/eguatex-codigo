use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod chave;
pub mod comum;
pub mod google;
#[cfg(test)]
mod testes_testador;

/// Modelos do seletor do chat. Vazio e o Automatico.
pub const MODELOS: &[&str] = &["", "gemini-flash-latest", "gemini-flash-lite-latest"];

/// O unico provedor de IA e o Gemini. Config antiga com outro nome carrega como "gemini".
pub const MSG_PROVEDOR: &str = "Provedor de IA desconhecido. O ÉguaTeX usa só o Gemini.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigIA {
    pub ativa: bool,
    /// Sempre "gemini". A chave nunca fica aqui: so no cofre.
    pub provedor: String,
    pub modelo: String,
    /// Acesso do agente ao projeto: "conversa", "leitura" ou "sugerir".
    #[serde(default = "nivel_padrao")]
    pub nivel: String,
}

fn nivel_padrao() -> String {
    "sugerir".into()
}

impl Default for ConfigIA {
    fn default() -> Self {
        Self {
            ativa: false,
            provedor: "gemini".into(),
            modelo: String::new(),
            nivel: nivel_padrao(),
        }
    }
}

fn caminho_config() -> PathBuf {
    // EGUATEX_DADOS: so nos testes, para nunca gravar na configuracao real da pessoa
    let base = std::env::var_os("EGUATEX_DADOS")
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("eguatex").join("ia-config.json")
}

pub fn carregar_config() -> ConfigIA {
    let caminho = caminho_config();
    if caminho.exists() {
        match std::fs::read_to_string(&caminho) {
            Ok(texto) => normalizar(serde_json::from_str(&texto).unwrap_or_default()),
            Err(_) => ConfigIA::default(),
        }
    } else {
        ConfigIA::default()
    }
}

/// Config antiga (qualquer provedor que nao seja o Gemini, ou vazio) volta para o Gemini, com o
/// modelo automatico. Modelo fora do seletor (`MODELOS`) tambem vira o Automatico.
fn normalizar(mut config: ConfigIA) -> ConfigIA {
    if config.provedor != chave::GEMINI {
        config.provedor = "gemini".into();
        config.modelo = String::new();
    }
    if !MODELOS.contains(&config.modelo.as_str()) {
        config.modelo = String::new();
    }
    config
}

pub fn salvar_config(config: &ConfigIA) -> Result<(), String> {
    if config.provedor != chave::GEMINI {
        return Err(MSG_PROVEDOR.into());
    }
    let caminho = caminho_config();
    if let Some(pai) = caminho.parent() {
        std::fs::create_dir_all(pai)
            .map_err(|e| format!("Erro ao criar diretório: {}", e))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Erro ao serializar config: {}", e))?;
    std::fs::write(&caminho, json)
        .map_err(|e| format!("Erro ao salvar config: {}", e))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn config_antiga_carrega_e_provedor_desconhecido_vira_gemini() {
        let _vez = chave::cofre_de_teste();
        let caminho = caminho_config();
        std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
        // v41: so ativa, provedor e modelo, sem o campo nivel
        std::fs::write(&caminho, r#"{"ativa":true,"provedor":"gemini","modelo":"gemini-flash-latest","ollama_url":"http://localhost:11434"}"#).unwrap();
        let c = carregar_config();
        assert!(c.ativa);
        assert_eq!(c.provedor, "gemini");
        assert_eq!(c.nivel, "sugerir");
        // o campo ollama_url da config antiga e ignorado e nao volta a ser gravado
        salvar_config(&c).unwrap();
        assert!(!std::fs::read_to_string(&caminho).unwrap().contains("ollama"));
        std::fs::write(&caminho, r#"{"ativa":false,"provedor":"compativel-antigo","modelo":"modelo-antigo","ollama_url":""}"#).unwrap();
        let c = carregar_config();
        assert_eq!(c.provedor, "gemini");
        assert_eq!(c.modelo, "");
        // config de versao anterior com outro provedor carrega como Gemini
        for antigo in ["outro-provedor", "ollama"] {
            std::fs::write(&caminho, format!(r#"{{"ativa":true,"provedor":"{antigo}","modelo":"modelo-antigo","nivel":"leitura"}}"#)).unwrap();
            let c = carregar_config();
            assert_eq!(c.provedor, "gemini", "{antigo}");
            assert_eq!(c.modelo, "", "{antigo}");
            assert_eq!(c.nivel, "leitura");
        }
        for outro in ["google-oauth", "outro-provedor", ""] {
            assert!(salvar_config(&ConfigIA { provedor: outro.into(), ..ConfigIA::default() }).is_err(), "{outro}");
        }
        // modelo fora do seletor vira o Automatico; os do seletor ficam
        std::fs::write(&caminho, r#"{"ativa":true,"provedor":"gemini","modelo":"gemini-pro-latest"}"#).unwrap();
        assert_eq!(carregar_config().modelo, "");
        for m in MODELOS {
            std::fs::write(&caminho, format!(r#"{{"ativa":true,"provedor":"gemini","modelo":"{m}"}}"#)).unwrap();
            assert_eq!(carregar_config().modelo, *m);
        }
        salvar_config(&ConfigIA::default()).unwrap();
        assert!(!carregar_config().ativa, "IA desligada por padrao");
    }
}
