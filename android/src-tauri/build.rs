use std::path::Path;

fn main() {
    embutir_cliente_google();
    tauri_build::build()
}

/// Le o JSON do cliente OAuth "Desktop app" baixado do Google Cloud
/// (src-tauri/google-oauth.json, fora do git) e embute client_id e client_secret
/// no executavel. Variaveis de ambiente com o mesmo nome tem prioridade.
///
/// O Google Picker precisa de mais dois dados do mesmo projeto do Google Cloud, lidos dos campos
/// `picker_api_key` e `project_number` do mesmo arquivo, ou das variaveis
/// EGUATEX_PICKER_API_KEY e EGUATEX_GOOGLE_PROJECT_NUMBER.
fn embutir_cliente_google() {
    let arquivo = Path::new("google-oauth.json");
    println!("cargo:rerun-if-changed=google-oauth.json");
    println!("cargo:rerun-if-env-changed=EGUATEX_GOOGLE_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=EGUATEX_GOOGLE_CLIENT_SECRET");
    println!("cargo:rerun-if-env-changed=EGUATEX_PICKER_API_KEY");
    println!("cargo:rerun-if-env-changed=EGUATEX_GOOGLE_PROJECT_NUMBER");

    if !arquivo.exists() {
        return;
    }
    let Ok(texto) = std::fs::read_to_string(arquivo) else { return };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&texto) else {
        println!("cargo:warning=google-oauth.json invalido");
        return;
    };
    let dados = &json["installed"];
    let emitir = |variavel: &str, valor: Option<&str>| {
        if std::env::var(variavel).is_err() {
            if let Some(v) = valor {
                println!("cargo:rustc-env={variavel}={v}");
            }
        }
    };
    emitir("EGUATEX_GOOGLE_CLIENT_ID", dados["client_id"].as_str());
    emitir("EGUATEX_GOOGLE_CLIENT_SECRET", dados["client_secret"].as_str());
    emitir("EGUATEX_PICKER_API_KEY", json["picker_api_key"].as_str().or(dados["picker_api_key"].as_str()));
    emitir("EGUATEX_GOOGLE_PROJECT_NUMBER", json["project_number"].as_str().or(dados["project_number"].as_str()));
}
