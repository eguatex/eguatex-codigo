//! Corretor ortografico offline em portugues do Brasil e em ingles dos EUA.
//! Dicionarios: VERO 3.2 (LibreOffice), LGPLv3 ou MPL, em dicionarios/pt_BR.*; en_US do SCOWL
//! (licenca permissiva, dicionarios/LICENCA-EN-US.txt), em dicionarios/en_US.*.
//! Leitura do formato Hunspell pelo crate spellbook (MPL-2.0).

use spellbook::Dictionary;
use std::sync::OnceLock;

const AFF: &str = include_str!("../dicionarios/pt_BR.aff");
const DIC: &str = include_str!("../dicionarios/pt_BR.dic");
const AFF_EN: &str = include_str!("../dicionarios/en_US.aff");
const DIC_EN: &str = include_str!("../dicionarios/en_US.dic");

/// Maximo de sugestoes mostradas no menu do clique direito.
const MAX_SUGESTOES: usize = 6;

static DICIONARIO: OnceLock<Option<Dictionary>> = OnceLock::new();
static DICIONARIO_EN: OnceLock<Option<Dictionary>> = OnceLock::new();

/// Idioma pedido pela interface: "en" (ou en_US, en-GB...) usa o ingles; o resto, o portugues.
fn eh_ingles(idioma: Option<&str>) -> bool {
    idioma.is_some_and(|i| i.to_ascii_lowercase().starts_with("en"))
}

fn dicionario_de(idioma: Option<&str>) -> Result<&'static Dictionary, String> {
    if eh_ingles(idioma) {
        DICIONARIO_EN
            .get_or_init(|| Dictionary::new(AFF_EN, DIC_EN).ok())
            .as_ref()
            .ok_or_else(|| "Não foi possível carregar o dicionário en-US.".to_string())
    } else {
        DICIONARIO
            .get_or_init(|| Dictionary::new(AFF, DIC).ok())
            .as_ref()
            .ok_or_else(|| "Não foi possível carregar o dicionário pt-BR.".to_string())
    }
}

/// Uma palavra esta certa se o dicionario a aceita como esta ou com a primeira letra
/// minuscula (palavra no comeco da frase).
fn correta(dic: &Dictionary, palavra: &str) -> bool {
    if dic.check(palavra) {
        return true;
    }
    let mut letras = palavra.chars();
    match letras.next() {
        Some(primeira) if primeira.is_uppercase() => {
            let minuscula: String = primeira.to_lowercase().chain(letras).collect();
            dic.check(&minuscula)
        }
        _ => false,
    }
}

/// Devolve as palavras da lista que o dicionario nao conhece.
pub fn verificar(palavras: &[String], idioma: Option<&str>) -> Result<Vec<String>, String> {
    let dic = dicionario_de(idioma)?;
    Ok(palavras
        .iter()
        .filter(|p| !correta(dic, p))
        .cloned()
        .collect())
}

/// Sugestoes de grafia para uma palavra.
pub fn sugerir(palavra: &str, idioma: Option<&str>) -> Result<Vec<String>, String> {
    let dic = dicionario_de(idioma)?;
    let mut saida = Vec::new();
    dic.suggest(palavra, &mut saida);
    saida.truncate(MAX_SUGESTOES);
    Ok(saida)
}

#[tauri::command]
pub async fn cmd_ortografia_verificar(palavras: Vec<String>, idioma: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || verificar(&palavras, idioma.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn cmd_ortografia_sugerir(palavra: String, idioma: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || sugerir(&palavra, idioma.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod testes {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn aceita_portugues_e_recusa_erros() {
        let erradas = verificar(&s(&[
            "ação", "coração", "física", "espectroscopia", "Universidade", "vibração", "excessão",
            "fisica", "nescessário",
        ]), None)
        .unwrap();
        assert_eq!(erradas, s(&["excessão", "fisica", "nescessário"]));
    }

    #[test]
    fn sugere_a_grafia_certa() {
        let sugestoes = sugerir("fisica", None).unwrap();
        assert!(sugestoes.iter().any(|p| p == "física"), "{sugestoes:?}");
    }

    #[test]
    fn ingles_aceita_texto_cientifico_e_recusa_erros() {
        let ruins = verificar(&s(&["The", "results", "show", "that", "spectroscopy", "measurements", "recieve", "teh"]), Some("en_US")).unwrap();
        assert_eq!(ruins, s(&["recieve", "teh"]));
        // portugues continua no portugues
        assert_eq!(verificar(&s(&["resultados", "medidas"]), None).unwrap(), Vec::<String>::new());
        assert!(sugerir("recieve", Some("en")).unwrap().iter().any(|p| p == "receive"));
    }
}
