//! Traducao de trechos pelo Google Tradutor (o mesmo servico do menu "Traduzir" do
//! Android e do Chrome). Nao precisa de conta nem de chave. So roda quando a pessoa
//! pede, e o trecho selecionado e o unico dado enviado.

use serde::Serialize;

const ENDERECO: &str = "https://translate.googleapis.com/translate_a/single";
/// O servico recusa pedidos muito grandes; acima disso o trecho vai em partes.
const LIMITE_PARTE: usize = 4500;
/// Sem User-Agent de navegador o servico devolve 429 com frequencia.
const AGENTE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
const TENTATIVAS: u32 = 4;

fn deve_repetir(status: u16) -> bool {
    status == 429 || (500..=504).contains(&status)
}

/// 1 s, 2 s, 4 s entre tentativas.
fn espera(tentativa: u32) -> std::time::Duration {
    std::time::Duration::from_secs(1 << tentativa)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Traducao {
    pub traducao: String,
    /// Codigo do idioma de origem (o detectado, quando a origem e "auto").
    pub origem: String,
}

/// Junta as frases traduzidas da resposta: [[["trad","orig",...],...], null, "en", ...].
fn ler_resposta(json: &serde_json::Value) -> Option<Traducao> {
    let frases = json.get(0)?.as_array()?;
    let traducao: String = frases
        .iter()
        .filter_map(|f| f.get(0).and_then(|t| t.as_str()))
        .collect();
    let origem = json.get(2).and_then(|o| o.as_str()).unwrap_or("").to_string();
    Some(Traducao { traducao, origem })
}

/// Divide em partes de ate LIMITE_PARTE caracteres, cortando em quebras de linha.
fn dividir(texto: &str) -> Vec<String> {
    let mut partes = Vec::new();
    let mut atual = String::new();
    for linha in texto.split_inclusive('\n') {
        if !atual.is_empty() && atual.chars().count() + linha.chars().count() > LIMITE_PARTE {
            partes.push(std::mem::take(&mut atual));
        }
        atual.push_str(linha);
    }
    if !atual.is_empty() {
        partes.push(atual);
    }
    partes
}

#[tauri::command]
pub async fn cmd_traduzir(texto: String, origem: String, destino: String) -> Result<Traducao, String> {
    if texto.trim().is_empty() {
        return Err("Nada para traduzir".to_string());
    }
    let cliente = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let origem_pedida = if origem.is_empty() { "auto".to_string() } else { origem };
    let mut total = Traducao { traducao: String::new(), origem: String::new() };
    for parte in dividir(&texto) {
        let mut resposta = None;
        for tentativa in 0..TENTATIVAS {
            let r = cliente
                .post(ENDERECO)
                .header(reqwest::header::USER_AGENT, AGENTE)
                .query(&[("client", "gtx"), ("sl", origem_pedida.as_str()), ("tl", destino.as_str()), ("dt", "t")])
                .form(&[("q", parte.as_str())])
                .send()
                .await
                .map_err(|_| "Sem conexão com o Google Tradutor. A tradução precisa de internet.".to_string())?;
            let status = r.status().as_u16();
            if r.status().is_success() {
                resposta = Some(r);
                break;
            }
            if !deve_repetir(status) || tentativa + 1 == TENTATIVAS {
                return Err(format!("O Google Tradutor recusou o pedido (código {status}). Tente de novo em instantes."));
            }
            tokio::time::sleep(espera(tentativa)).await;
        }
        let resposta = resposta.ok_or_else(|| "Sem resposta do Google Tradutor".to_string())?;
        let json: serde_json::Value = resposta.json().await.map_err(|e| format!("Resposta inesperada: {}", e))?;
        let pedaco = ler_resposta(&json).ok_or_else(|| "Resposta inesperada do Google Tradutor".to_string())?;
        total.traducao.push_str(&pedaco.traducao);
        if total.origem.is_empty() {
            total.origem = pedaco.origem;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_frases_e_idioma_detectado() {
        let json = serde_json::json!([[["Olá. ", "Hello. ", null], ["Tudo bem?", "How are you?", null]], null, "en"]);
        assert_eq!(
            ler_resposta(&json),
            Some(Traducao { traducao: "Olá. Tudo bem?".into(), origem: "en".into() })
        );
    }

    #[test]
    fn repete_so_em_429_e_5xx_com_espera_crescente() {
        assert!(deve_repetir(429));
        assert!(deve_repetir(503));
        assert!(!deve_repetir(400));
        assert!(!deve_repetir(404));
        assert_eq!(espera(0).as_secs(), 1);
        assert_eq!(espera(1).as_secs(), 2);
        assert_eq!(espera(2).as_secs(), 4);
    }

    #[test]
    fn divide_textos_grandes_em_quebras_de_linha() {
        let linha = format!("{}\n", "a".repeat(1000));
        let texto = linha.repeat(10);
        let partes = dividir(&texto);
        assert!(partes.len() > 1);
        assert!(partes.iter().all(|p| p.chars().count() <= LIMITE_PARTE));
        assert_eq!(partes.concat(), texto);
    }
}
