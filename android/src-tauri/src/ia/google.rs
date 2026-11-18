//! Login com Google (OAuth 2.0 para apps instalados, PKCE e loopback) para o Google Drive, o
//! compartilhamento e a abertura de arquivos do Google, e chamadas ao Gemini pela chave de API
//! da propria pessoa (cabecalho `x-goog-api-key`; a chave mora em `super::chave`). Os tokens do
//! login ficam so no cofre do sistema (arquivo privado do app no Android, pelo crate
//! `keyring-arquivo`) e nunca chegam ao front-end. O login nao da acesso ao Gemini.

use std::io::{BufRead, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::Emitter;

use super::chave::ChaveApi;
use super::comum::Codigo;
// `Emissor` fica sem `use`: o AppHandle tem tambem o `emit` do tauri::Emitter e os dois juntos
// no escopo seriam ambiguos. Em `&dyn Emissor` o metodo vale sem importar o trait.
pub(crate) use super::comum::MSG_SEM_REDE;

const CLIENT_ID: Option<&str> = option_env!("EGUATEX_GOOGLE_CLIENT_ID");
const CLIENT_SECRET: Option<&str> = option_env!("EGUATEX_GOOGLE_CLIENT_SECRET");
const SERVICO_COFRE: &str = "eguatex";
const USUARIO_COFRE: &str = "google-oauth";
/// O login pede so o basico e o Google Drive, sempre: o escopo `drive.file` da acesso so aos
/// arquivos que o proprio ÉguaTeX criou ou que a pessoa escolheu no Google Picker. Nenhum desses
/// escopos e confidencial, entao a tela "app nao verificado" nao aparece. O Gemini usa a chave de
/// API da pessoa, sem escopo do Google Cloud.
const ESCOPOS_BASE: &str = "openid email profile https://www.googleapis.com/auth/drive.file";
/// Endereco da Gemini API. Os testes trocam pelo servidor falso.
pub(crate) const URL_GEMINI: &str = "https://generativelanguage.googleapis.com";
const MODELOS_RESERVA: &[&str] = &["gemini-flash-latest", "gemini-flash-lite-latest"];

/// Segundos de espera antes de cada nova rodada quando o Gemini responde 5xx (ocupado).
pub(crate) const ESPERAS_OCUPADO: &[u64] = &[4, 10];

/// Modelos que deram "lotado" ou demoraram demais, e ate quando ficam de lado.
static LOTADOS: std::sync::Mutex<Vec<(String, Instant)>> = std::sync::Mutex::new(Vec::new());

fn esta_lotado(modelo: &str) -> bool {
    let mut lista = LOTADOS.lock().unwrap_or_else(|e| e.into_inner());
    lista.retain(|(_, ate)| *ate > Instant::now());
    lista.iter().any(|(m, _)| m == modelo)
}

fn marcar_lotado(modelo: &str) {
    let mut lista = LOTADOS.lock().unwrap_or_else(|e| e.into_inner());
    lista.retain(|(m, _)| m != modelo);
    lista.push((modelo.to_string(), Instant::now() + Duration::from_secs(600)));
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContaGoogle {
    pub access_token: String,
    pub refresh_token: String,
    pub expira_em: i64,
    pub email: String,
    pub nome: String,
    pub foto: String,
    /// Projeto do Google Cloud da versao 41 (Gemini pelo login). Nao e mais usado; o campo fica
    /// para ler as contas gravadas por ela.
    #[serde(default)]
    pub projeto: String,
    /// Permissoes que a pessoa marcou. Contas antigas, sem `drive.file` ou com o Drive inteiro,
    /// pedem um novo login.
    #[serde(default)]
    pub escopos: String,
}

/// O que o front-end recebe: nada de token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoConta {
    pub conectado: bool,
    pub configurado: bool,
    pub email: String,
    pub nome: String,
    pub foto: String,
    /// Texto para a interface quando a conta antiga foi desfeita e a pessoa precisa entrar de novo.
    #[serde(default)]
    pub aviso: String,
}

pub fn configurado() -> bool {
    CLIENT_ID.is_some_and(|c| !c.is_empty())
}

fn credenciais() -> Result<(&'static str, &'static str), String> {
    match CLIENT_ID {
        Some(id) if !id.is_empty() => Ok((id, CLIENT_SECRET.unwrap_or(""))),
        _ => Err("Login com Google ainda não configurado nesta versão do ÉguaTeX.".into()),
    }
}

pub(crate) fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn texto_aleatorio(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// Junta a mensagem do erro com as causas internas (rede, TLS, DNS).
pub(crate) fn cadeia(e: &dyn std::error::Error) -> String {
    let mut texto = e.to_string();
    let mut causa = e.source();
    while let Some(c) = causa {
        texto.push_str(&format!(": {c}"));
        causa = c.source();
    }
    texto
}

pub(crate) fn cliente() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_default()
}

/// So em testes: conta (e-mail, access token) que substitui o cofre, para exercitar o Drive real.
/// Liga o LoginGoogleServico (Kotlin) enquanto o navegador esta na frente (login e seletor do
/// Drive) e o desliga quando a guarda sai de escopo. Fora do Android nao faz nada.
pub(crate) mod manter_acordado {
    use std::sync::Mutex;

    /// Guardas vivas. O comando que abre o seletor segura uma ate o download acabar e o seletor
    /// segura outra por dentro: o servico liga na primeira e so desliga quando a ultima sai. A
    /// contagem e a chamada ao Kotlin ficam sob a mesma trava: sem ela, um "parar" atrasado podia
    /// chegar depois do "iniciar" de outra guarda e deixar o servico desligado com guarda viva.
    static ATIVAS: Mutex<usize> = Mutex::new(0);

    pub struct Guarda;

    impl Guarda {
        pub fn nova() -> Self {
            let mut ativas = ATIVAS.lock().unwrap_or_else(|e| e.into_inner());
            if *ativas == 0 {
                #[cfg(target_os = "android")]
                if let Err(e) = chamar("iniciar") {
                    eprintln!("LoginGoogleServico.iniciar falhou: {e}");
                }
            }
            *ativas += 1;
            Guarda
        }
    }

    impl Drop for Guarda {
        fn drop(&mut self) {
            let mut ativas = ATIVAS.lock().unwrap_or_else(|e| e.into_inner());
            *ativas = ativas.saturating_sub(1);
            if *ativas == 0 {
                #[cfg(target_os = "android")]
                let _ = chamar("parar");
            }
        }
    }

    /// Chama LoginGoogleServico.iniciar(Context) ou parar(Context). A classe vem do class loader do
    /// app: numa thread do Rust o FindClass do JNI so enxerga as classes do sistema.
    #[cfg(target_os = "android")]
    fn chamar(metodo: &str) -> jni::errors::Result<()> {
        use jni::objects::{JClass, JObject, JValue};
        let ctx = ndk_context::android_context();
        // SAFETY: o tao inicializa o contexto Android antes de abrir a janela.
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }?;
        let mut env = vm.attach_current_thread()?;
        let contexto = unsafe { JObject::from_raw(ctx.context().cast()) };
        let carregador = env.call_method(&contexto, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
        let nome = env.new_string("br.ufpa.eguatex.LoginGoogleServico")?;
        let classe = env
            .call_method(&carregador, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&nome)])?
            .l()?;
        let classe = JClass::from(classe);
        let r = env.call_static_method(&classe, metodo, "(Landroid/content/Context;)V", &[JValue::Object(&contexto)]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        r.map(|_| ())
    }
}

#[cfg(test)]
pub(crate) static CONTA_TESTE: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

// ---------- cofre ----------

fn entrada_cofre() -> Result<keyring::Entry, String> {
    cofre(USUARIO_COFRE)
}

/// Entrada do cofre de credenciais (um usuario por tipo de login): Gerenciador de Credenciais
/// no Windows, arquivo na pasta privada do app no Android.
pub(crate) fn cofre(usuario: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICO_COFRE, usuario)
        .map_err(|e| format!("Erro ao acessar o cofre de credenciais: {e}"))
}

fn ler_conta() -> Option<ContaGoogle> {
    let senha = entrada_cofre().ok()?.get_password().ok()?;
    serde_json::from_str::<ContaGoogle>(&senha)
        .ok()
        .filter(|c| tem_escopos(&c.escopos, EXIGIDOS_BASE))
}

fn gravar_conta(conta: &ContaGoogle) -> Result<(), String> {
    let texto = serde_json::to_string(conta).map_err(|e| e.to_string())?;
    entrada_cofre()?
        .set_password(&texto)
        .map_err(|e| format!("Erro ao gravar no cofre de credenciais: {e}"))
}

// ---------- login ----------

/// Permissoes do login basico (conta e Google Drive). Quem as tem esta conectado.
const EXIGIDOS_BASE: &[&str] = &["email", "profile", "drive.file"];
const AVISO_BASE: &str = "inclusive a do Google Drive";
const MSG_PERMISSAO_BASE: &str = "Faltou uma permissão na tela do Google. Clique em Entrar com Google de novo e marque todas as caixas, inclusive a do Google Drive (arquivos criados ou escolhidos por você).";
/// Marca no cofre de que uma conta antiga (Drive inteiro) foi desfeita; some no proximo login.
const COFRE_MIGRACAO: &str = "google-migracao";
pub const AVISO_MIGRACAO: &str = "Entre de novo: o ÉguaTeX agora pede menos acesso ao seu Drive.";

/// Confere se todas as permissoes exigidas foram marcadas na tela do Google.
/// "drive.file" exige exatamente `.../auth/drive.file`; o escopo inteiro (`.../auth/drive`)
/// nao vale, porque o app nao o pede mais.
pub(crate) fn tem_escopos(escopos: &str, exigidos: &[&str]) -> bool {
    exigidos.iter().all(|e| match *e {
        "drive.file" => escopos.split_whitespace().any(|x| x.ends_with("/auth/drive.file")),
        _ => escopos.contains(e),
    })
}

/// Verdadeiro se os escopos incluem o Drive inteiro (`.../auth/drive`), de versoes antigas.
fn tem_drive_inteiro(escopos: &str) -> bool {
    escopos.split_whitespace().any(|x| x.ends_with("/auth/drive"))
}

/// Se o texto gravado no cofre e de uma conta com o Drive inteiro, devolve o token a revogar
/// (vazio quando nao ha nenhum). `None` quando a conta nao precisa de migracao.
fn token_de_conta_antiga(texto: &str) -> Option<String> {
    let v: Value = serde_json::from_str(texto).ok()?;
    if !tem_drive_inteiro(v["escopos"].as_str().unwrap_or("")) {
        return None;
    }
    let refresh = v["refresh_token"].as_str().unwrap_or("");
    let token = if refresh.is_empty() { v["access_token"].as_str().unwrap_or("") } else { refresh };
    Some(token.to_string())
}

/// Token do login separado do Drive de versoes anteriores (entrada "google-drive"). Vazio se
/// o texto nao tiver token; a entrada some do mesmo jeito.
fn token_do_login_drive_antigo(texto: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(texto) else { return String::new() };
    let refresh = v["refresh_token"].as_str().unwrap_or("");
    let token = if refresh.is_empty() { v["access_token"].as_str().unwrap_or("") } else { refresh };
    token.to_string()
}

fn texto_do_cofre(usuario: &str) -> Option<String> {
    cofre(usuario).ok()?.get_password().ok()
}

/// Entradas do cofre com o Drive inteiro que precisam ser revogadas e apagadas: a conta
/// gravada com `/auth/drive` e o login separado do Drive de versoes anteriores. Cada item e
/// (entrada do cofre, token); token vazio so apaga. Funcao pura: define o que entra no plano.
fn plano_de_revogacao(conta: Option<&str>, login_drive: Option<&str>) -> Vec<(&'static str, String)> {
    let mut plano = Vec::new();
    if let Some(token) = conta.and_then(token_de_conta_antiga) {
        plano.push((USUARIO_COFRE, token));
    }
    if let Some(texto) = login_drive {
        plano.push(("google-drive", token_do_login_drive_antigo(texto)));
    }
    plano
}

/// Revoga e so entao apaga cada entrada do plano. Sem internet, devolve erro e nao apaga nada
/// do que falhou. Devolve `true` se desfez alguma conta (e deixa a marca que faz a interface
/// mostrar `AVISO_MIGRACAO`).
async fn cancelar_acesso_antigo() -> Result<bool, String> {
    // uma so vez por vez: a migracao da abertura e o login esperam um pelo outro
    let _vez = TRAVA_COFRE.lock().await;
    let plano = plano_de_revogacao(
        texto_do_cofre(USUARIO_COFRE).as_deref(),
        texto_do_cofre("google-drive").as_deref(),
    );
    let mut desfez = false;
    for (usuario, token) in plano {
        if !token.is_empty() && !revogar_token_ok(&token).await {
            return Err(MSG_SEM_INTERNET_ANTIGO.into());
        }
        // Relê a entrada: só apaga se ainda for a mesma conta antiga. Uma conta gravada
        // enquanto a revogação corria nunca é apagada.
        if ainda_e_a_antiga(usuario, texto_do_cofre(usuario).as_deref(), &token) {
            apagar_entrada(usuario);
            desfez = true;
        }
    }
    if desfez {
        if let Ok(e) = cofre(COFRE_MIGRACAO) {
            let _ = e.set_password("1");
        }
    }
    Ok(desfez)
}

/// Trava da migracao do cofre: `migrar_conta_antiga` (abertura) e `entrar_conta` (login).
static TRAVA_COFRE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A entrada do cofre continua sendo a conta antiga cujo `token` foi revogado? Funcao pura.
fn ainda_e_a_antiga(usuario: &str, texto_atual: Option<&str>, token: &str) -> bool {
    let Some(texto) = texto_atual else { return false };
    if usuario == USUARIO_COFRE {
        token_de_conta_antiga(texto).as_deref() == Some(token)
    } else {
        token_do_login_drive_antigo(texto) == token
    }
}

const MSG_SEM_INTERNET_ANTIGO: &str = "Sem internet para cancelar o acesso antigo; tente de novo";

/// Na abertura do app: desfaz a conta antiga. Sem internet, tenta de novo na proxima abertura.
pub async fn migrar_conta_antiga() -> bool {
    cancelar_acesso_antigo().await.unwrap_or(false)
}

/// Aviso de "entre de novo" enquanto houver uma conta antiga ou a marca da migracao.
fn aviso_de_migracao() -> String {
    let antiga = texto_do_cofre(USUARIO_COFRE).is_some_and(|t| token_de_conta_antiga(&t).is_some());
    if antiga || texto_do_cofre("google-drive").is_some() || texto_do_cofre(COFRE_MIGRACAO).is_some() {
        AVISO_MIGRACAO.to_string()
    } else {
        String::new()
    }
}

/// Espera a volta do navegador no servidor local e devolve o `code`.
/// Se a pessoa desmarcou alguma caixa, a pagina pede para marcar todas e
/// o login continua esperando uma nova tentativa pelo link `url_login`.
fn esperar_retorno(
    listener: TcpListener,
    state: String,
    url_login: String,
    exigidos: &[&str],
    aviso: &str,
) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let limite = Instant::now() + Duration::from_secs(300);

    loop {
        if Instant::now() > limite {
            return Err("Tempo esgotado esperando o login no navegador.".into());
        }
        let (mut stream, _) = match listener.accept() {
            Ok(par) => par,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        stream.set_nonblocking(false).ok();
        stream.set_read_timeout(Some(Duration::from_secs(10))).ok();

        // So a primeira linha importa: "GET /?code=...&state=... HTTP/1.1"
        let mut linha = String::new();
        crate::http_local::leitor_limitado(&stream).read_line(&mut linha).ok();
        if !crate::http_local::linha_completa(&linha) {
            // linha de 8 KB sem fim: nao e o navegador. Fecha e continua esperando o login de verdade.
            let _ = stream.write_all(crate::http_local::RESPOSTA_431);
            continue;
        }
        let caminho = linha.split_whitespace().nth(1).unwrap_or("/").to_string();

        // Pedidos como /favicon.ico nao trazem resposta do Google: ignora e continua esperando.
        if !caminho.contains("code=") && !caminho.contains("error=") {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }

        let url = Url::parse(&format!("http://127.0.0.1{caminho}")).map_err(|e| e.to_string())?;
        let mut code = String::new();
        let mut erro = String::new();
        let mut state_volta = String::new();
        let mut escopos = String::new();
        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "code" => code = v.into_owned(),
                "scope" => escopos = v.into_owned(),
                "error" => erro = v.into_owned(),
                "state" => state_volta = v.into_owned(),
                _ => {}
            }
        }

        let ok = erro.is_empty() && state_volta == state && !code.is_empty();
        let faltou = ok && !tem_escopos(&escopos, exigidos);
        let pagina_faltou = format!(
            "<h2>Faltou marcar permissões.</h2>             <p>O ÉguaTeX precisa de todas as caixas da tela do Google, {}.</p>             <p><a href=\"{}\" style=\"display:inline-block;padding:10px 20px;background:#2E7D32;color:#fff;             border-radius:6px;text-decoration:none\">Tentar de novo e marcar todas</a></p>",
            aviso,
            url_login.replace('&', "&amp;").replace('"', "&quot;")
        );
        let pagina = if faltou {
            pagina_faltou.as_str()
        } else if ok {
            if cfg!(target_os = "android") {
                // O Android 15 corta a internet do app enquanto o navegador esta na frente.
                // O botao traz o EguaTex de volta para o login terminar.
                "<h2>Pode voltar ao ÉguaTeX.</h2>                 <p><a href=\"eguatex://login\"                  style=\"display:inline-block;padding:14px 28px;background:#5f1a5c;color:#fff;border-radius:8px;text-decoration:none;font-size:18px\">Voltar ao ÉguaTeX</a></p>"
            } else {
                "<h2>Pode voltar ao ÉguaTeX.</h2><p>Esta aba já pode ser fechada.</p>"
            }
        } else {
            "<h2>O login não foi concluído.</h2><p>Volte ao ÉguaTeX e tente de novo.</p>"
        };
        let corpo = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>ÉguaTeX</title></head>\
             <body style=\"font-family:sans-serif;text-align:center;margin-top:15vh\">{pagina}</body></html>"
        );
        let resposta = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
            corpo.len()
        );
        let _ = stream.write_all(resposta.as_bytes());

        if faltou {
            continue;
        }
        if !erro.is_empty() {
            return Err(if erro == "access_denied" {
                "Login cancelado.".into()
            } else {
                format!("O Google recusou o login: {erro}")
            });
        }
        if state_volta != state {
            return Err("Resposta de login inválida (state diferente).".into());
        }
        return Ok(code);
    }
}

/// Login no navegador (PKCE e loopback). Devolve os tokens e o perfil da pessoa.
/// Recusa o login se faltar alguma das permissoes `exigidas`.
pub(crate) async fn autorizar(
    app: &tauri::AppHandle,
    escopos_pedidos: &'static str,
    exigidos: &'static [&'static str],
    aviso: &'static str,
    msg_falta: &'static str,
) -> Result<(Value, Value), String> {
    let (client_id, client_secret) = credenciais()?;

    let verifier = texto_aleatorio(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = texto_aleatorio(24);

    // O mesmo listener fica aberto do inicio ao fim: a porta do redirect e a porta que escuta.
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("Erro ao abrir porta local: {e}"))?;
    let porta = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{porta}");

    let url = Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("client_id", client_id),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", escopos_pedidos),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .map_err(|e| e.to_string())?;

    // Android: servico em primeiro plano enquanto o navegador esta na frente. Sem ele o sistema
    // congela o app e a pagina do Google fica carregando sem fim no 127.0.0.1 (visto num celular
    // real em 12/11/2026; o emulador nao congela). A guarda para o servico em qualquer saida.
    let _acordado = manter_acordado::Guarda::nova();

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| format!("Erro ao abrir o navegador: {e}"))?;
    let _ = app.emit("ia-google-progresso", "Aguardando o login no navegador");

    let url_login = url.to_string();
    let code = tokio::task::spawn_blocking(move || esperar_retorno(listener, state, url_login, exigidos, aviso))
        .await
        .map_err(|e| e.to_string())??;

    let _ = app.emit("ia-google-progresso", "Confirmando o login");
    // No Android 15 o app fica sem internet enquanto o navegador esta na frente.
    // Falha de conexao repete a cada 2 s por ate 5 min, ate a pessoa voltar ao app.
    let limite = Instant::now() + Duration::from_secs(300);
    let resposta = loop {
        let tentativa = cliente()
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code.as_str()),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("redirect_uri", redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier.as_str()),
            ])
            .send()
            .await;
        match tentativa {
            Ok(r) => break r,
            Err(e) if (e.is_connect() || e.is_timeout()) && Instant::now() < limite => {
                let _ = app.emit("ia-google-progresso", "Volte ao ÉguaTeX para concluir o login");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(format!("Erro de conexão com o Google: {}", cadeia(&e))),
        }
    };
    let tokens: Value = resposta
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // Na tela do Google a pessoa pode desmarcar alguma permissao.
    let escopos = tokens["scope"].as_str().unwrap_or("");
    if let Some(t) = tokens["access_token"].as_str() {
        if !tem_escopos(escopos, exigidos) {
            let _ = cliente()
                .post("https://oauth2.googleapis.com/revoke")
                .form(&[("token", t)])
                .send()
                .await;
            return Err(msg_falta.into());
        }
    }

    let access_token = tokens["access_token"]
        .as_str()
        .ok_or_else(|| format!("O Google não devolveu o acesso: {}", tokens["error_description"].as_str().unwrap_or("sem detalhe")))?
        .to_string();

    let perfil: Value = cliente()
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok((tokens, perfil))
}

/// Login basico no navegador (conta e Google Drive); grava a conta no cofre.
pub(crate) async fn entrar_conta(app: &tauri::AppHandle) -> Result<(), String> {
    // O /revoke do Google cancela o grant inteiro do par usuario e cliente OAuth. Por isso o
    // acesso antigo (Drive inteiro) e revogado ANTES do login novo; depois de gravar o token
    // novo nada que seja da mesma conta pode ser revogado. Contas da versao 41 com Google Cloud e
    // Gemini nao sao revogadas: os escopos a mais so ficam sem uso.
    cancelar_acesso_antigo().await?;
    let (tokens, perfil) = autorizar(app, ESCOPOS_BASE, EXIGIDOS_BASE, AVISO_BASE, MSG_PERMISSAO_BASE).await?;
    let escopos = tokens["scope"].as_str().unwrap_or("").to_string();
    let access_token = tokens["access_token"].as_str().unwrap_or("").to_string();

    let email = perfil["email"].as_str().unwrap_or("").to_string();

    let conta = ContaGoogle {
        access_token,
        refresh_token: tokens["refresh_token"].as_str().unwrap_or("").to_string(),
        expira_em: agora() + tokens["expires_in"].as_i64().unwrap_or(3600),
        email,
        nome: perfil["name"].as_str().unwrap_or("").to_string(),
        foto: perfil["picture"].as_str().unwrap_or("").to_string(),
        projeto: String::new(),
        escopos,
    };
    // Se outra pessoa estava no lugar desta, o token dela e revogado antes de gravar. Grant de
    // outro usuario: nao afeta o token novo. Da mesma conta, nada e revogado aqui.
    // Entrada sem e-mail nao e tratada como "outra conta": quem decide por ela e o plano de
    // revogacao de antes do login, pelo escopo.
    if let Some((email, token)) = conta_bruta(USUARIO_COFRE) {
        if !email.is_empty() && email != conta.email {
            revogar_token(&token).await;
        }
    }
    let _vez = TRAVA_COFRE.lock().await;
    gravar_conta(&conta)?;
    apagar_entrada(COFRE_MIGRACAO);
    Ok(())
}

/// E-mail e token de uma entrada do cofre, sem conferir as permissoes (contas antigas tambem).
fn conta_bruta(usuario: &str) -> Option<(String, String)> {
    let texto = cofre(usuario).ok()?.get_password().ok()?;
    let v: Value = serde_json::from_str(&texto).ok()?;
    let refresh = v["refresh_token"].as_str().unwrap_or("");
    let token = if refresh.is_empty() { v["access_token"].as_str().unwrap_or("") } else { refresh };
    (!token.is_empty()).then(|| (v["email"].as_str().unwrap_or("").to_string(), token.to_string()))
}

async fn revogar_token(token: &str) {
    let _ = revogar_token_ok(token).await;
}

/// Revoga o token e diz se o Google confirmou. Token que o Google ja nao conhece (HTTP 400)
/// conta como revogado; falha de rede, nao.
async fn revogar_token_ok(token: &str) -> bool {
    match cliente()
        .post("https://oauth2.googleapis.com/revoke")
        .form(&[("token", token)])
        .send()
        .await
    {
        Ok(r) => r.status().is_success() || r.status().as_u16() == 400,
        Err(_) => false,
    }
}

fn apagar_entrada(usuario: &str) {
    if let Ok(e) = cofre(usuario) {
        let _ = e.delete_credential();
    }
}

/// Token de acesso valido para as APIs do Google (Drive, Docs).
pub(crate) async fn token_acesso() -> Result<String, String> {
    #[cfg(test)]
    if let Some((_, token)) = CONTA_TESTE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(token);
    }
    Ok(token_valido().await?.access_token)
}

/// Devolve a conta com um access_token valido, renovando se faltar menos de 60 s.
async fn token_valido() -> Result<ContaGoogle, String> {
    let mut conta = ler_conta().ok_or("Entre com sua conta Google no ícone de conta, na barra lateral.")?;
    if agora() < conta.expira_em - 60 {
        return Ok(conta);
    }
    let (client_id, client_secret) = credenciais()?;
    let resp: Value = cliente()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", conta.refresh_token.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| format!("Erro de conexão com o Google: {}", cadeia(&e)))?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let novo = resp["access_token"]
        .as_str()
        .ok_or("A sessão do Google expirou. Entre de novo com sua conta.")?;
    conta.access_token = novo.to_string();
    conta.expira_em = agora() + resp["expires_in"].as_i64().unwrap_or(3600);
    if let Some(r) = resp["refresh_token"].as_str() {
        conta.refresh_token = r.to_string();
    }
    gravar_conta(&conta)?;
    Ok(conta)
}

fn mensagem_erro(json: &Value) -> String {
    json["error"]["message"].as_str().unwrap_or("erro desconhecido").to_string()
}

// ---------- agente com streaming ----------

const MSG_TEMPO: &str = "O Gemini demorou demais para responder. Tente de novo em alguns minutos.";
pub(crate) const MSG_INTERROMPIDO: &str = "Interrompido.";
use super::comum::{AVISO_INTERROMPIDA, MSG_SEM_RESPOSTA};

/// A resposta tem algo que a tela aproveita: texto que nao seja pensamento, ou chamada de
/// ferramenta. So pensamento (ou partes vazias) nao serve: vira erro em vez de bolha vazia.
fn tem_resposta_util(partes: &[Value]) -> bool {
    partes.iter().any(|p| {
        p.get("functionCall").is_some() || (!p["thought"].as_bool().unwrap_or(false) && p["text"].as_str().is_some_and(|t| !t.trim().is_empty()))
    })
}

/// Codigo para a resposta HTTP do Gemini. `texto` e o corpo (ou a mensagem) do erro.
pub fn classificar_gemini(status: u16, texto: &str) -> Codigo {
    let chave_ruim = texto.contains("API_KEY_INVALID") || texto.contains("API key not valid");
    match status {
        200..=299 => Codigo::Ok,
        400 if chave_ruim => Codigo::Invalida,
        401 => Codigo::Invalida,
        403 => Codigo::Bloqueada,
        429 => Codigo::Limite,
        _ if texto.contains("RESOURCE_EXHAUSTED") => Codigo::Limite,
        404 => Codigo::Indisponivel,
        500..=599 => Codigo::Ocupado,
        _ => Codigo::Erro,
    }
}

/// Texto em portugues para o erro HTTP do Gemini. `msg` e a mensagem original da API. O limite
/// tem texto proprio (cota gratuita); o resto sai de `comum::mensagem`.
fn mensagem_http(status: u16, msg: &str) -> String {
    match classificar_gemini(status, msg) {
        Codigo::Limite => "Limite gratuito do Gemini atingido. Tente de novo mais tarde.".into(),
        Codigo::Invalida => super::comum::mensagem(Codigo::Invalida, status, ""),
        Codigo::Bloqueada => super::comum::mensagem(Codigo::Bloqueada, status, ""),
        codigo => super::comum::mensagem(codigo, status, msg),
    }
}

/// Teste da chave: um generateContent de 5 tokens no Flash Lite (no Flash se o Lite sumir).
pub(crate) async fn testar_chave(chave: &ChaveApi, base: &str, limite: Duration) -> Codigo {
    let Some(cabecalho) = chave.cabecalho("") else { return Codigo::Invalida };
    let corpo = json!({"contents": [{"role": "user", "parts": [{"text": "Oi"}]}], "generationConfig": {"maxOutputTokens": 5}});
    let mut resultado = Codigo::Erro;
    for modelo in ["gemini-flash-lite-latest", "gemini-flash-latest"] {
        let envio = super::comum::cliente_com_limite(limite)
            .post(format!("{base}/v1beta/models/{modelo}:generateContent"))
            .header("x-goog-api-key", cabecalho.clone())
            .json(&corpo)
            .send()
            .await;
        let resp = match envio {
            Ok(r) => r,
            Err(e) => return super::comum::codigo_de_envio(&e),
        };
        let status = resp.status().as_u16();
        let texto = match resp.text().await {
            Ok(t) => t,
            Err(e) => return super::comum::codigo_de_envio(&e),
        };
        resultado = classificar_gemini(status, &texto);
        if status != 404 {
            break;
        }
    }
    resultado
}

/// Raciocinio do modelo: baixo no Flash (mais rapido ate o primeiro texto), padrao no Pro e no
/// Flash Lite. Gemini 3 em diante usa `thinkingLevel`; o 2.5 usa `thinkingBudget`.
fn config_pensamento(modelo: &str) -> Option<Value> {
    let m = modelo.to_lowercase();
    if !m.contains("flash") || m.contains("lite") {
        return None;
    }
    if m.contains("2.5") || m.contains("2.0") {
        return Some(json!({"thinkingBudget": 512}));
    }
    Some(json!({"thinkingLevel": "low"}))
}

fn com_pensamento(corpo: &Value, modelo: &str, ligado: bool) -> Value {
    let mut c = corpo.clone();
    if !ligado {
        return c;
    }
    if let Some(cfg) = config_pensamento(modelo) {
        if !c["generationConfig"].is_object() {
            c["generationConfig"] = json!({});
        }
        // se o front-end ja mandou um thinkingConfig, ele vale
        if c["generationConfig"].get("thinkingConfig").is_none() {
            c["generationConfig"]["thinkingConfig"] = cfg;
        }
    }
    c
}

/// Conversas em andamento. O botao Parar avisa pelo canal da conversa.
static CANCELAMENTOS: std::sync::Mutex<Vec<(String, tokio::sync::watch::Sender<bool>)>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn registrar_conversa(id: &str) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let mut lista = CANCELAMENTOS.lock().unwrap_or_else(|e| e.into_inner());
    lista.retain(|(i, _)| i != id);
    lista.push((id.to_string(), tx));
    rx
}

pub(crate) fn liberar_conversa(id: &str) {
    CANCELAMENTOS.lock().unwrap_or_else(|e| e.into_inner()).retain(|(i, _)| i != id);
}

/// Cancela o stream da conversa `id`, se estiver rodando.
pub fn cancelar_agente(id: &str) {
    for (i, tx) in CANCELAMENTOS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        if i == id {
            let _ = tx.send(true);
        }
    }
}

pub(crate) async fn cancelado(rx: &mut tokio::sync::watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// Le o corpo SSE aos pedacos e devolve o `data:` de cada evento completo.
#[derive(Default)]
pub(crate) struct LeitorSse {
    buf: Vec<u8>,
}

impl LeitorSse {
    pub(crate) fn alimentar(&mut self, dados: &[u8]) -> Vec<String> {
        self.buf.extend(dados.iter().filter(|b| **b != b'\r'));
        let mut eventos = Vec::new();
        while let Some(pos) = self.buf.windows(2).position(|w| w == b"\n\n") {
            let bloco: Vec<u8> = self.buf.drain(..pos + 2).collect();
            let texto = String::from_utf8_lossy(&bloco);
            let linhas: Vec<&str> = texto
                .lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .map(|l| l.strip_prefix(' ').unwrap_or(l))
                .collect();
            if !linhas.is_empty() {
                eventos.push(linhas.join("\n"));
            }
        }
        eventos
    }

    /// O stream acabou sem linha em branco no fim: processa o que sobrou.
    pub(crate) fn terminar(&mut self) -> Vec<String> {
        self.alimentar(b"\n\n")
    }
}

/// Junta os eventos do stream numa unica resposta (`content` com `parts`).
#[derive(Default)]
struct Acumulador {
    partes: Vec<Value>,
    bloqueio: Option<String>,
    /// `finishReason` do ultimo candidato. Sem ele o stream foi cortado.
    fim: Option<String>,
}

impl Acumulador {
    /// Soma um evento e devolve o texto novo (sem os pensamentos) para a tela.
    fn juntar(&mut self, evento: &Value) -> String {
        if let Some(b) = evento["promptFeedback"]["blockReason"].as_str() {
            self.bloqueio = Some(b.to_string());
        }
        if let Some(f) = evento["candidates"][0]["finishReason"].as_str() {
            self.fim = Some(f.to_string());
        }
        let mut novo = String::new();
        for parte in evento["candidates"][0]["content"]["parts"].as_array().into_iter().flatten() {
            let pensamento = parte["thought"].as_bool().unwrap_or(false);
            if let Some(t) = parte["text"].as_str() {
                if !pensamento {
                    novo.push_str(t);
                }
                if let Some(ultima) = self.partes.last_mut() {
                    let mesmo_tipo = ultima.get("text").is_some()
                        && ultima.get("functionCall").is_none()
                        && ultima["thought"].as_bool().unwrap_or(false) == pensamento;
                    if mesmo_tipo {
                        let junto = format!("{}{}", ultima["text"].as_str().unwrap_or(""), t);
                        ultima["text"] = Value::String(junto);
                        if let Some(assinatura) = parte.get("thoughtSignature") {
                            ultima["thoughtSignature"] = assinatura.clone();
                        }
                        continue;
                    }
                }
            }
            self.partes.push(parte.clone());
        }
        novo
    }
}

/// Stream que acabou sem `finishReason`: tira as chamadas de ferramenta (podem estar pela metade)
/// e poe o aviso de resposta interrompida no fim do ultimo texto visivel.
fn sem_fim_vira_aviso(mut acc: Acumulador) -> Acumulador {
    if acc.fim.is_some() {
        return acc;
    }
    acc.partes.retain(|p| p.get("functionCall").is_none());
    let visivel = acc.partes.iter_mut().rev().find(|p| !p["thought"].as_bool().unwrap_or(false) && p["text"].as_str().is_some_and(|t| !t.trim().is_empty()));
    if let Some(p) = visivel {
        p["text"] = json!(format!("{}{AVISO_INTERROMPIDA}", p["text"].as_str().unwrap_or("")));
    }
    acc
}

/// Resultado de uma leitura de stream que falhou: a mensagem e se ja saiu texto para a tela.
struct FalhaStream {
    erro: String,
    emitiu: bool,
    /// Erro do proprio pedido, mandado dentro do stream (chave, pedido ruim): outro modelo daria o
    /// mesmo erro, entao nao vale tentar o proximo.
    definitivo: bool,
}

async fn ler_stream(
    mut resp: reqwest::Response,
    rx: &mut tokio::sync::watch::Receiver<bool>,
    app: &dyn super::comum::Emissor,
    id: &str,
) -> Result<(Acumulador, bool), FalhaStream> {
    let mut leitor = LeitorSse::default();
    let mut acc = Acumulador::default();
    let mut emitiu = false;
    loop {
        let pedaco = tokio::select! {
            r = tokio::time::timeout(Duration::from_secs(60), resp.chunk()) => r,
            _ = cancelado(rx) => return Err(FalhaStream { erro: MSG_INTERROMPIDO.into(), emitiu, definitivo: false }),
        };
        let (eventos, fim) = match pedaco {
            Ok(Ok(Some(bytes))) => (leitor.alimentar(&bytes), false),
            Ok(Ok(None)) => (leitor.terminar(), true),
            Ok(Err(e)) => {
                let erro = if e.is_timeout() { MSG_TEMPO.to_string() } else { "A conexão com o Gemini caiu no meio da resposta. Tente de novo.".to_string() };
                return Err(FalhaStream { erro, emitiu, definitivo: false });
            }
            Err(_) => return Err(FalhaStream { erro: MSG_TEMPO.into(), emitiu, definitivo: false }),
        };
        for dados in eventos {
            let Ok(evento) = serde_json::from_str::<Value>(&dados) else { continue };
            if evento.get("error").is_some() {
                let codigo = evento["error"]["code"].as_u64().unwrap_or(0) as u16;
                let erro = match codigo {
                    0 => super::comum::msg_erro_no_stream(&mensagem_erro(&evento)),
                    _ => mensagem_http(codigo, &mensagem_erro(&evento)),
                };
                // lotado, aposentado ou ocupado: o proximo modelo pode responder
                let definitivo = !matches!(codigo, 404 | 429 | 500..=599);
                return Err(FalhaStream { erro, emitiu, definitivo });
            }
            let novo = acc.juntar(&evento);
            if !novo.is_empty() {
                emitiu = true;
                let _ = app.emit("ia-pedaco", json!({"id": id, "texto": novo}));
            }
        }
        if fim {
            return Ok((acc, emitiu));
        }
    }
}

/// Agente com o Gemini: recebe o corpo completo do pedido (contents, tools, systemInstruction)
/// montado no front-end, le a resposta em stream e emite cada pedaco de texto no evento
/// `ia-pedaco` (`{id, texto}`). Devolve o `content` inteiro, com texto e chamadas de ferramenta.
/// A chave fica so aqui no Rust, no cabecalho `x-goog-api-key`.
pub(crate) async fn chamar_gemini_stream(
    app: &dyn super::comum::Emissor,
    id: &str,
    modelo: &str,
    corpo: &Value,
    rx: &mut tokio::sync::watch::Receiver<bool>,
    chave: &ChaveApi,
    base: &str,
) -> Result<Value, String> {
    let Some(cabecalho) = chave.cabecalho("") else {
        return Err(super::comum::mensagem(Codigo::Invalida, 401, ""));
    };

    let mut candidatos: Vec<&str> = Vec::new();
    if !modelo.trim().is_empty() {
        candidatos.push(modelo.trim());
    }
    for m in MODELOS_RESERVA {
        if !candidatos.contains(m) {
            candidatos.push(m);
        }
    }
    candidatos.sort_by_key(|m| esta_lotado(m));

    let mut ultimo_erro = String::from("O Gemini não respondeu.");
    let total = candidatos.len();
    // Pico de uso do Gemini (HTTP 5xx em todos os modelos): espera e repete a lista inteira.
    let mut sem_tempo: Vec<&str> = Vec::new();
    for rodada in 0..=ESPERAS_OCUPADO.len() {
        if rodada > 0 {
            let _ = app.emit("ia-ocupado", json!({"id": id, "tentativa": rodada + 1, "total": ESPERAS_OCUPADO.len() + 1}));
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(ESPERAS_OCUPADO[rodada - 1])) => {}
                _ = cancelado(rx) => return Err(MSG_INTERROMPIDO.into()),
            }
        }
        let mut ocupado = false;
        for (i, m) in candidatos.iter().copied().enumerate() {
            if sem_tempo.contains(&m) {
                continue;
            }
            // tempo ate o Gemini comecar a responder; depois disso vale o limite de silencio do stream
            let limite = if i + 1 == total || m == modelo.trim() { 90 } else { 25 };
            let mut com_raciocinio = true;
            'tentativa: loop {
                let corpo_m = com_pensamento(corpo, m, com_raciocinio);
                let envio = super::comum::cliente_stream()
                    .post(format!("{base}/v1beta/models/{m}:streamGenerateContent?alt=sse"))
                    .header("x-goog-api-key", cabecalho.clone())
                    .json(&corpo_m)
                    .send();
                let resp = tokio::select! {
                    r = tokio::time::timeout(Duration::from_secs(limite), envio) => r,
                    _ = cancelado(rx) => return Err(MSG_INTERROMPIDO.into()),
                };
                let resp = match resp {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) if e.is_connect() => return Err(MSG_SEM_REDE.into()),
                    Ok(Err(e)) if e.is_timeout() => {
                        marcar_lotado(m);
                        sem_tempo.push(m);
                        ultimo_erro = MSG_TEMPO.into();
                        break 'tentativa;
                    }
                    Ok(Err(_)) => return Err("Erro de conexão com o Gemini. Tente de novo.".into()),
                    Err(_) => {
                        marcar_lotado(m);
                        sem_tempo.push(m);
                        ultimo_erro = MSG_TEMPO.into();
                        break 'tentativa;
                    }
                };

                let status = resp.status().as_u16();
                if (200..300).contains(&status) {
                    match ler_stream(resp, rx, app, id).await.map(|(acc, emitiu)| (sem_fim_vira_aviso(acc), emitiu)) {
                        Ok((acc, _)) if tem_resposta_util(&acc.partes) => {
                            return Ok(json!({"role": "model", "parts": acc.partes, "modeloUsado": m}));
                        }
                        Ok((acc, _)) => {
                            ultimo_erro = match acc.bloqueio {
                                Some(motivo) => format!("O Gemini recusou o pedido ({motivo})."),
                                None => MSG_SEM_RESPOSTA.into(),
                            };
                            break 'tentativa;
                        }
                        Err(f) if f.erro == MSG_INTERROMPIDO => return Err(f.erro),
                        // ja saiu texto para a tela: trocar de modelo duplicaria a resposta
                        Err(f) if f.emitiu || f.definitivo => return Err(f.erro),
                        Err(f) => {
                            if f.erro == MSG_TEMPO {
                                marcar_lotado(m);
                                sem_tempo.push(m);
                            }
                            ultimo_erro = f.erro;
                            break 'tentativa;
                        }
                    }
                }

                let texto = resp.text().await.unwrap_or_default();
                let json: Value = serde_json::from_str::<Value>(&texto)
                    .map(|v| if v.is_array() { v[0].clone() } else { v })
                    .unwrap_or(Value::Null);
                let msg = mensagem_erro(&json);
                // chave recusada: o motivo API_KEY_INVALID vem no detalhe do erro, fora da mensagem
                let msg = if texto.contains("API_KEY_INVALID") && !msg.contains("API_KEY_INVALID") { format!("{msg} API_KEY_INVALID") } else { msg };
                // modelo que nao aceita o campo de raciocinio: repete uma vez sem ele
                if status == 400 && com_raciocinio && msg.to_lowercase().contains("thinking") {
                    com_raciocinio = false;
                    continue 'tentativa;
                }
                let aposentado = status == 404 || msg.contains("no longer available") || msg.contains("not found");
                ultimo_erro = mensagem_http(status, &msg);
                if status == 503 || status == 502 || status == 504 || status == 500 || aposentado {
                    marcar_lotado(m);
                }
                if status == 503 || status == 502 || status == 504 || status == 500 {
                    ocupado = true;
                }
                if !(status == 429 || status == 503 || status == 502 || status == 504 || status == 500 || aposentado) {
                    return Err(ultimo_erro);
                }
                break 'tentativa;
            }
        }
        if !ocupado {
            break;
        }
    }
    Err(ultimo_erro)
}

pub const MSG_DESLIGADA: &str = "A IA está desligada. Abra o assistente de IA e ative com a sua chave.";

/// `cmd_ia_agente`: uma rodada do agente com o Gemini. Nada sai do aparelho com a IA desligada
/// ou sem chave. O texto de erro nunca traz a chave.
pub async fn chamar_agente(app: &dyn super::comum::Emissor, id: &str, modelo: &str, corpo: Value) -> Result<Value, String> {
    if !super::carregar_config().ativa {
        return Err(MSG_DESLIGADA.into());
    }
    let chave = super::chave::ler().ok_or(super::comum::MSG_SEM_CHAVE)?;
    let mut rx = registrar_conversa(id);
    let r = chamar_gemini_stream(app, id, modelo, &corpo, &mut rx, &chave, URL_GEMINI).await;
    liberar_conversa(id);
    r.map_err(|e| chave.limpar(&e))
}

// ---------- estado e saida ----------

pub fn info() -> InfoConta {
    #[cfg(test)]
    if let Some((email, _)) = CONTA_TESTE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return InfoConta { conectado: true, configurado: true, email, nome: String::new(), foto: String::new(), aviso: String::new() };
    }
    let conta = ler_conta();
    InfoConta {
        conectado: conta.is_some(),
        configurado: configurado(),
        email: conta.as_ref().map(|c| c.email.clone()).unwrap_or_default(),
        nome: conta.as_ref().map(|c| c.nome.clone()).unwrap_or_default(),
        foto: conta.as_ref().map(|c| c.foto.clone()).unwrap_or_default(),
        aviso: if conta.is_some() { String::new() } else { aviso_de_migracao() },
    }
}

/// Revoga o acesso na Google e apaga o registro do cofre.
pub async fn sair() -> Result<(), String> {
    // revoga tambem contas antigas, sem todas as permissoes, e o login separado do Drive
    let mut revogados = Vec::new();
    for usuario in [USUARIO_COFRE, "google-drive"] {
        if let Some((_, token)) = conta_bruta(usuario) {
            if !revogados.contains(&token) {
                revogar_token(&token).await;
                revogados.push(token);
            }
        }
    }
    apagar_entrada("google-drive");
    apagar_entrada(COFRE_MIGRACAO);
    match entrada_cofre()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Erro ao apagar a conta do cofre: {e}")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Read;
    use std::net::TcpStream;

    fn pedir(porta: u16, caminho: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        write!(s, "GET {caminho} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).ok();
        r
    }

    #[test]
    fn retorno_do_navegador_com_code_e_state() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || esperar_retorno(l, "abc".into(), "https://login".into(), EXIGIDOS_BASE, AVISO_BASE));
        assert!(pedir(porta, "/favicon.ico").contains("404"));
        let tudo = "openid%20email%20profile%20https://www.googleapis.com/auth/drive.file";
        let parcial = pedir(porta, "/?state=abc&code=PARCIAL&scope=email%20profile");
        assert!(parcial.contains("Faltou marcar") && parcial.contains("https://login"));
        assert!(pedir(porta, &format!("/?state=abc&code=4%2F0XYZ&scope={tudo}")).contains("Pode voltar ao ÉguaTeX"));
        assert_eq!(h.join().unwrap().unwrap(), "4/0XYZ");
    }

    /// Escreve ate 400 MB sem quebra de linha. Devolve quantos MB o servidor aceitou antes de largar a conexao.
    fn despejar_sem_quebra_de_linha(s: &mut TcpStream) -> usize {
        use std::io::Write as _;
        s.set_write_timeout(Some(Duration::from_secs(20))).ok();
        let mb = vec![b'a'; 1 << 20];
        let mut aceitos = 0;
        while aceitos < 400 {
            if s.write_all(&mb).is_err() {
                break;
            }
            aceitos += 1;
        }
        aceitos
    }

    #[test]
    fn oauth_linha_400mb_recusada() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || esperar_retorno(l, "abc".into(), "https://login".into(), EXIGIDOS_BASE, AVISO_BASE));
        let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        let aceitos = despejar_sem_quebra_de_linha(&mut s);
        assert!(aceitos < 50, "o servidor aceitou {aceitos} MB de uma linha so");
        drop(s);
        // o servidor continua de pe: o retorno certo do navegador ainda conclui o login
        let tudo = "openid%20email%20profile%20https://www.googleapis.com/auth/drive.file";
        assert!(pedir(porta, &format!("/?state=abc&code=OK&scope={tudo}")).contains("Pode voltar"));
        assert_eq!(h.join().unwrap().unwrap(), "OK");
    }

    #[test]
    fn retorno_com_state_falso_e_recusado() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || esperar_retorno(l, "abc".into(), "https://login".into(), EXIGIDOS_BASE, AVISO_BASE));
        pedir(porta, "/?state=ERRADO&code=x");
        assert!(h.join().unwrap().is_err());
    }

    #[test]
    fn login_cancelado() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || esperar_retorno(l, "abc".into(), "https://login".into(), EXIGIDOS_BASE, AVISO_BASE));
        pedir(porta, "/?error=access_denied&state=abc");
        assert_eq!(h.join().unwrap().unwrap_err(), "Login cancelado.");
    }

    #[test]
    fn pkce_challenge_e_base64url_sem_padding() {
        let v = texto_aleatorio(64);
        let c = URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes()));
        assert_eq!(c.len(), 43);
        assert!(!c.contains('=') && !c.contains('+') && !c.contains('/'));
    }

    #[test]
    fn drive_exige_o_escopo_drive_file() {
        let so_arquivo = "email https://www.googleapis.com/auth/drive.file";
        let inteiro = "email https://www.googleapis.com/auth/drive";
        assert!(tem_escopos(so_arquivo, &["email", "drive.file"]));
        assert!(!tem_escopos(inteiro, &["email", "drive.file"]));
        assert!(tem_drive_inteiro(inteiro) && !tem_drive_inteiro(so_arquivo));
    }

    #[test]
    fn so_apaga_se_a_entrada_ainda_for_a_conta_antiga() {
        let antiga = r#"{"refresh_token":"r1","escopos":"email https://www.googleapis.com/auth/drive"}"#;
        let nova = r#"{"refresh_token":"r2","escopos":"email https://www.googleapis.com/auth/drive.file"}"#;
        assert!(ainda_e_a_antiga(USUARIO_COFRE, Some(antiga), "r1"));
        // conta nova gravada durante a revogacao: nao apaga
        assert!(!ainda_e_a_antiga(USUARIO_COFRE, Some(nova), "r1"));
        // outra conta antiga com outro token: nao apaga
        assert!(!ainda_e_a_antiga(USUARIO_COFRE, Some(antiga), "outro"));
        assert!(!ainda_e_a_antiga(USUARIO_COFRE, None, "r1"));
        assert!(ainda_e_a_antiga("google-drive", Some(r#"{"refresh_token":"rd"}"#), "rd"));
        assert!(!ainda_e_a_antiga("google-drive", Some(r#"{"refresh_token":"novo"}"#), "rd"));
    }

    #[test]
    fn plano_revoga_o_drive_inteiro_antes_do_login_novo() {
        let antiga = r#"{"refresh_token":"r1","escopos":"email https://www.googleapis.com/auth/drive"}"#;
        let nova = r#"{"refresh_token":"r2","escopos":"email https://www.googleapis.com/auth/drive.file"}"#;
        let drive = r#"{"refresh_token":"rd"}"#;
        assert_eq!(
            plano_de_revogacao(Some(antiga), Some(drive)),
            vec![(USUARIO_COFRE, "r1".to_string()), ("google-drive", "rd".to_string())]
        );
        // conta nova (drive.file) nao e revogada: revogar derrubaria o grant do login atual
        assert_eq!(plano_de_revogacao(Some(nova), None), vec![]);
        assert_eq!(plano_de_revogacao(None, None), vec![]);
        assert!(MSG_SEM_INTERNET_ANTIGO.starts_with("Sem internet para cancelar o acesso antigo"));
    }

    #[test]
    fn login_separado_do_drive_tambem_e_revogado_na_migracao() {
        let antigo = r#"{"access_token":"a","refresh_token":"rd","expira_em":1,"email":"x@y.com"}"#;
        assert_eq!(token_do_login_drive_antigo(antigo), "rd");
        assert_eq!(token_do_login_drive_antigo(r#"{"access_token":"so-acesso","refresh_token":""}"#), "so-acesso");
        assert_eq!(token_do_login_drive_antigo("lixo"), "");
    }

    #[test]
    fn conta_com_drive_inteiro_pede_migracao_e_devolve_o_token() {
        let antiga = r#"{"access_token":"a","refresh_token":"r1","expira_em":1,"email":"x@y.com","escopos":"email profile https://www.googleapis.com/auth/drive x/cloud-platform"}"#;
        assert_eq!(token_de_conta_antiga(antiga).as_deref(), Some("r1"));
        let sem_refresh = r#"{"access_token":"a","refresh_token":"","escopos":"https://www.googleapis.com/auth/drive"}"#;
        assert_eq!(token_de_conta_antiga(sem_refresh).as_deref(), Some("a"));
        let nova = r#"{"access_token":"a","refresh_token":"r1","escopos":"email https://www.googleapis.com/auth/drive.file"}"#;
        assert_eq!(token_de_conta_antiga(nova), None);
        let sem_campo = r#"{"access_token":"a","refresh_token":"r1","email":"x@y.com"}"#;
        assert_eq!(token_de_conta_antiga(sem_campo), None);
        assert_eq!(token_de_conta_antiga("nao e json"), None);
        assert!(AVISO_MIGRACAO.starts_with("Entre de novo"));
    }

    #[test]
    fn modelo_lotado_vai_para_o_fim_da_fila() {
        marcar_lotado("gemini-flash-latest");
        let mut fila = vec!["gemini-flash-latest", "gemini-flash-lite-latest"];
        fila.sort_by_key(|m| esta_lotado(m));
        assert_eq!(fila, vec!["gemini-flash-lite-latest", "gemini-flash-latest"]);
    }

    #[test]
    fn login_pede_so_openid_email_profile_e_drive_file() {
        assert!(tem_escopos(ESCOPOS_BASE, EXIGIDOS_BASE));
        let pedidos: Vec<&str> = ESCOPOS_BASE.split_whitespace().collect();
        assert_eq!(pedidos, vec!["openid", "email", "profile", "https://www.googleapis.com/auth/drive.file"]);
        assert!(!ESCOPOS_BASE.contains("cloud-platform"));
        assert!(!ESCOPOS_BASE.contains("generative-language"));
    }

    #[test]
    fn conta_so_com_login_basico_continua_conectada() {
        let c = ContaGoogle { email: "x@y.com".into(), escopos: ESCOPOS_BASE.into(), ..Default::default() };
        assert!(tem_escopos(&c.escopos, EXIGIDOS_BASE));
        assert!(!tem_escopos("openid email profile", EXIGIDOS_BASE));
        assert!(!tem_escopos("email profile https://www.googleapis.com/auth/drive", EXIGIDOS_BASE));
    }

    /// Escopos de uma conta gravada pela versao 41, que pedia tambem Google Cloud e Gemini.
    const ESCOPOS_V41: &str = "openid email profile https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language.retriever https://www.googleapis.com/auth/drive.file";

    #[test]
    fn conta_da_v41_com_escopos_a_mais_continua_conectada_e_nao_e_revogada() {
        let texto = format!(r#"{{"access_token":"a","refresh_token":"r41","expira_em":1,"email":"x@y.com","nome":"X","foto":"","projeto":"eguatex-abc","escopos":"{ESCOPOS_V41}"}}"#);
        let c: ContaGoogle = serde_json::from_str(&texto).unwrap();
        assert_eq!(c.projeto, "eguatex-abc");
        assert!(tem_escopos(&c.escopos, EXIGIDOS_BASE));
        // so o Drive inteiro entra no plano de revogacao; os escopos a mais da v41 ficam sem uso
        assert_eq!(token_de_conta_antiga(&texto), None);
        assert_eq!(plano_de_revogacao(Some(&texto), None), vec![]);
    }

    #[test]
    fn conta_sem_o_campo_projeto_tambem_le() {
        let sem_projeto = r#"{"access_token":"a","refresh_token":"r","expira_em":1,"email":"x@y.com","nome":"X","foto":"","escopos":"email profile https://www.googleapis.com/auth/drive.file"}"#;
        let c: ContaGoogle = serde_json::from_str(sem_projeto).unwrap();
        assert_eq!(c.projeto, "");
        assert!(tem_escopos(&c.escopos, EXIGIDOS_BASE));
    }

    #[test]
    fn escopos_do_login_cobrem_todos_os_exigidos() {
        assert!(tem_escopos(ESCOPOS_BASE, EXIGIDOS_BASE));
        assert!(ESCOPOS_BASE.split_whitespace().any(|x| x == "https://www.googleapis.com/auth/drive.file"));
        assert!(!ESCOPOS_BASE.split_whitespace().any(|x| x == "https://www.googleapis.com/auth/drive"));
    }

    #[test]
    fn escopos_vazios_ou_com_o_drive_inteiro_nao_bastam() {
        assert!(!tem_escopos("", EXIGIDOS_BASE));
        assert!(!tem_escopos("   ", EXIGIDOS_BASE));
        assert!(!tem_escopos("email profile x/cloud-platform x/generative-language.retriever https://www.googleapis.com/auth/drive", EXIGIDOS_BASE));
        // lista vazia de exigidos sempre passa
        assert!(tem_escopos("", &[]));
    }

    #[test]
    fn cada_escopo_exigido_faltando_reprova() {
        let todos = ["email", "profile", "https://www.googleapis.com/auth/drive.file"];
        for i in 0..todos.len() {
            let sem: Vec<&str> = todos.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, e)| *e).collect();
            assert!(!tem_escopos(&sem.join(" "), EXIGIDOS_BASE), "faltando {} deveria reprovar", todos[i]);
        }
        assert!(tem_escopos(&todos.join(" "), EXIGIDOS_BASE));
    }

    #[test]
    fn conta_antiga_sem_campo_escopos_le_com_vazio_e_e_filtrada() {
        let antigo = r#"{"access_token":"a","refresh_token":"r","expira_em":1,"email":"x@y.com","nome":"X","foto":"","projeto":"p"}"#;
        let c: ContaGoogle = serde_json::from_str(antigo).unwrap();
        assert_eq!(c.escopos, "");
        assert!(!tem_escopos(&c.escopos, EXIGIDOS_BASE));
    }

    #[test]
    fn conta_com_escopos_completos_faz_ida_e_volta_no_json() {
        let c = ContaGoogle { email: "x@y.com".into(), escopos: ESCOPOS_BASE.into(), ..Default::default() };
        let volta: ContaGoogle = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(volta.escopos, ESCOPOS_BASE);
        assert!(tem_escopos(&volta.escopos, EXIGIDOS_BASE));
    }

    #[test]
    fn json_invalido_nao_vira_conta() {
        assert!(serde_json::from_str::<ContaGoogle>("isto nao e json").is_err());
        assert!(serde_json::from_str::<ContaGoogle>("{}").is_err());
    }
}

#[cfg(test)]
mod testes_migracao_extra {
    use super::*;

    #[test]
    fn conta_nova_so_com_drive_file_nao_e_revogada() {
        let nova = format!(r#"{{"access_token":"a","refresh_token":"r1","escopos":"{}"}}"#, ESCOPOS_BASE);
        assert_eq!(token_de_conta_antiga(&nova), None);
        let so_file = r#"{"access_token":"a","refresh_token":"r1","escopos":"https://www.googleapis.com/auth/drive.file"}"#;
        assert_eq!(token_de_conta_antiga(so_file), None);
        let readonly = r#"{"refresh_token":"r1","escopos":"https://www.googleapis.com/auth/drive.readonly https://www.googleapis.com/auth/drive.metadata"}"#;
        assert_eq!(token_de_conta_antiga(readonly), None);
        assert_eq!(token_de_conta_antiga(r#"{"refresh_token":"r1","escopos":""}"#), None);
        assert!(!tem_drive_inteiro("https://www.googleapis.com/auth/drive.file"));
    }

    #[test]
    fn conta_antiga_devolve_refresh_e_cai_para_access_ou_vazio() {
        let antiga = r#"{"access_token":"a","refresh_token":"r9","escopos":"https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/drive"}"#;
        assert_eq!(token_de_conta_antiga(antiga).as_deref(), Some("r9"));
        let sem_nada = r#"{"escopos":"https://www.googleapis.com/auth/drive"}"#;
        assert_eq!(token_de_conta_antiga(sem_nada).as_deref(), Some(""));
        assert!(tem_drive_inteiro("email https://www.googleapis.com/auth/drive"));
    }

    #[test]
    fn plano_so_inclui_drive_inteiro_independente_do_email() {
        let nova_outro = r#"{"email":"outra@y.com","refresh_token":"r2","escopos":"email https://www.googleapis.com/auth/drive.file"}"#;
        let antiga_outro = r#"{"email":"outra@y.com","refresh_token":"r3","escopos":"https://www.googleapis.com/auth/drive"}"#;
        assert_eq!(plano_de_revogacao(Some(nova_outro), None), vec![]);
        assert_eq!(plano_de_revogacao(Some("lixo"), None), vec![]);
        assert_eq!(plano_de_revogacao(Some(antiga_outro), None), vec![(USUARIO_COFRE, "r3".to_string())]);
        // conta nova mais login antigo do Drive: so o login antigo entra
        assert_eq!(plano_de_revogacao(Some(nova_outro), Some(r#"{"refresh_token":"rd"}"#)), vec![("google-drive", "rd".to_string())]);
    }
}

#[cfg(test)]
mod testes_stream {
    use super::*;

    #[test]
    fn leitor_sse_junta_pedacos_cortados_no_meio() {
        let mut l = LeitorSse::default();
        assert!(l.alimentar(b"data: {\"a\":").is_empty());
        let ev = l.alimentar(b"1}\r\n\r\ndata: {\"b\":2}\n\n");
        assert_eq!(ev, vec!["{\"a\":1}".to_string(), "{\"b\":2}".to_string()]);
        assert!(l.alimentar(b"data: {\"c\":3}").is_empty());
        assert_eq!(l.terminar(), vec!["{\"c\":3}".to_string()]);
    }

    #[test]
    fn leitor_sse_nao_quebra_letra_acentuada_no_meio_dos_bytes() {
        let bytes = "data: {\"t\":\"ação\"}\n\n".as_bytes().to_vec();
        let corte = bytes.iter().position(|b| *b == 0xC3).unwrap() + 1;
        let mut l = LeitorSse::default();
        assert!(l.alimentar(&bytes[..corte]).is_empty());
        assert_eq!(l.alimentar(&bytes[corte..]), vec!["{\"t\":\"ação\"}".to_string()]);
    }

    #[test]
    fn acumulador_junta_texto_e_guarda_chamada_de_ferramenta() {
        let mut a = Acumulador::default();
        let e1 = json!({"candidates":[{"content":{"parts":[{"text":"Olá, "}]}}]});
        let e2 = json!({"candidates":[{"content":{"parts":[{"text":"mundo","thoughtSignature":"sig"}]}}]});
        let e3 = json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"ler_arquivo","args":{"caminho":"a.tex"}}}]}}]});
        assert_eq!(a.juntar(&e1), "Olá, ");
        assert_eq!(a.juntar(&e2), "mundo");
        assert_eq!(a.juntar(&e3), "");
        assert_eq!(a.partes.len(), 2);
        assert_eq!(a.partes[0]["text"], "Olá, mundo");
        assert_eq!(a.partes[0]["thoughtSignature"], "sig");
        assert_eq!(a.partes[1]["functionCall"]["name"], "ler_arquivo");
    }

    #[test]
    fn acumulador_nao_mostra_pensamento_e_nao_mistura_com_resposta() {
        let mut a = Acumulador::default();
        let p = json!({"candidates":[{"content":{"parts":[{"text":"pensando","thought":true}]}}]});
        let t = json!({"candidates":[{"content":{"parts":[{"text":"resposta"}]}}]});
        assert_eq!(a.juntar(&p), "");
        assert_eq!(a.juntar(&t), "resposta");
        assert_eq!(a.partes.len(), 2);
    }

    #[test]
    fn resposta_so_com_pensamento_ou_vazia_nao_conta_como_resposta() {
        assert!(!tem_resposta_util(&[]));
        assert!(!tem_resposta_util(&[json!({"text": "pensando", "thought": true})]));
        assert!(!tem_resposta_util(&[json!({"text": "  "}), json!({"thoughtSignature": "x"})]));
        assert!(tem_resposta_util(&[json!({"text": "pensando", "thought": true}), json!({"text": "oi"})]));
        assert!(tem_resposta_util(&[json!({"functionCall": {"name": "ler_arquivo", "args": {}}})]));
        assert_eq!(MSG_SEM_RESPOSTA, "A IA não respondeu; tente de novo.");
    }

    #[test]
    fn acumulador_guarda_o_motivo_do_bloqueio() {
        let mut a = Acumulador::default();
        a.juntar(&json!({"promptFeedback":{"blockReason":"SAFETY"}}));
        assert_eq!(a.bloqueio.as_deref(), Some("SAFETY"));
        assert!(a.partes.is_empty());
    }

    #[test]
    fn raciocinio_baixo_no_flash_e_padrao_no_pro() {
        assert_eq!(config_pensamento("gemini-flash-latest"), Some(json!({"thinkingLevel": "low"})));
        assert_eq!(config_pensamento("gemini-2.5-flash"), Some(json!({"thinkingBudget": 512})));
        assert_eq!(config_pensamento("gemini-pro-latest"), None);
        assert_eq!(config_pensamento("gemini-flash-lite-latest"), None);
        let corpo = json!({"generationConfig": {"temperature": 0.4}});
        let c = com_pensamento(&corpo, "gemini-flash-latest", true);
        assert_eq!(c["generationConfig"]["thinkingConfig"]["thinkingLevel"], "low");
        assert_eq!(c["generationConfig"]["temperature"], 0.4);
        assert!(com_pensamento(&corpo, "gemini-flash-latest", false)["generationConfig"].get("thinkingConfig").is_none());
        assert!(com_pensamento(&corpo, "gemini-pro-latest", true)["generationConfig"].get("thinkingConfig").is_none());
    }

    #[test]
    fn mensagens_de_erro_em_portugues() {
        assert!(mensagem_http(400, "x").contains("erro 400"));
        // sem login no Gemini: 401 e chave invalida, 403 e chave bloqueada
        assert!(mensagem_http(401, "x").contains("chave do Gemini foi recusada"));
        assert!(mensagem_http(400, "API key not valid. Please pass a valid API key.").contains("chave do Gemini foi recusada"));
        assert!(mensagem_http(403, "x").contains("não tem permissão"));
        assert!(mensagem_http(429, "x").contains("Limite gratuito"));
        assert!(mensagem_http(503, "x").contains("está sobrecarregado"));
        assert!(mensagem_http(502, "<html>").contains("está sobrecarregado"));
        assert!(mensagem_http(504, "<html>").contains("está sobrecarregado"));
        // 500, 402 e 529 tambem saem em portugues, sem o texto cru em ingles sozinho
        assert!(mensagem_http(500, "falha").contains("está sobrecarregado"));
        assert!(mensagem_http(529, "overloaded").contains("está sobrecarregado"));
        assert!(mensagem_http(402, "Payment required").starts_with("O Gemini devolveu o erro 402"));
        assert!(mensagem_http(404, "not found").contains("não está disponível"));
        assert!(MSG_SEM_REDE.starts_with("Sem conexão"));
        assert!(MSG_TEMPO.contains("demorou demais"));
    }

    #[test]
    fn cancelar_avisa_so_a_conversa_certa() {
        let mut a = registrar_conversa("c1");
        let b = registrar_conversa("c2");
        cancelar_agente("c1");
        assert!(*a.borrow_and_update());
        assert!(!*b.borrow());
        liberar_conversa("c1");
        liberar_conversa("c2");
    }
}

#[cfg(test)]
mod testes_chave_gemini {
    use super::*;
    use crate::ia::chave::chave_de_teste;
    use crate::ia::comum::teste::{Gravador, json as resp_json, porta_fechada, sem_cancelar, servidor, servidor_mudo, sse};

    const CHAVE: &str = "AIzaSyTESTE-SEGREDO";

    #[tokio::test]
    async fn teste_da_chave_vai_so_no_cabecalho_e_404_repete_no_flash() {
        let (base, pedidos) = servidor(vec![resp_json(404, r#"{"error":{"code":404,"message":"not found"}}"#), resp_json(200, r#"{"candidates":[]}"#)]);
        assert_eq!(testar_chave(&chave_de_teste(CHAVE), &base, Duration::from_secs(5)).await, Codigo::Ok);
        let p1 = pedidos.recv().unwrap();
        let p2 = pedidos.recv().unwrap();
        assert!(p1.linha.starts_with("POST /v1beta/models/gemini-flash-lite-latest:generateContent "));
        assert!(p2.linha.starts_with("POST /v1beta/models/gemini-flash-latest:generateContent "));
        for p in [&p1, &p2] {
            assert_eq!(p.cabecalho("x-goog-api-key"), Some(CHAVE));
            assert!(p.cabecalho("authorization").is_none() && p.cabecalho("x-goog-user-project").is_none());
            assert!(!p.linha.contains(CHAVE) && !p.linha.contains("key=") && !p.corpo.contains(CHAVE));
            assert_eq!(p.json()["generationConfig"]["maxOutputTokens"], 5);
        }
    }

    #[tokio::test]
    async fn teste_da_chave_classifica_cada_erro() {
        let invalida = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"reason":"API_KEY_INVALID"}]}}"#;
        let casos = [
            (400, invalida.to_string(), Codigo::Invalida),
            (400, r#"{"error":{"code":400,"message":"x","details":[{"reason":"API_KEY_INVALID"}]}}"#.to_string(), Codigo::Invalida),
            (401, "{}".to_string(), Codigo::Invalida),
            (403, r#"{"error":{"code":403,"message":"PERMISSION_DENIED"}}"#.to_string(), Codigo::Bloqueada),
            (429, r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED"}}"#.to_string(), Codigo::Limite),
            (400, r#"{"error":{"status":"RESOURCE_EXHAUSTED"}}"#.to_string(), Codigo::Limite),
            (503, "{}".to_string(), Codigo::Ocupado),
            (500, "{}".to_string(), Codigo::Ocupado),
            (400, r#"{"error":{"message":"outro"}}"#.to_string(), Codigo::Erro),
        ];
        for (status, corpo, esperado) in casos {
            let (base, _p) = servidor(vec![resp_json(status, &corpo)]);
            assert_eq!(testar_chave(&chave_de_teste(CHAVE), &base, Duration::from_secs(5)).await, esperado, "{status} {corpo}");
        }
        assert_eq!(testar_chave(&chave_de_teste(CHAVE), &porta_fechada(), Duration::from_secs(5)).await, Codigo::SemInternet);
        assert_eq!(testar_chave(&chave_de_teste(CHAVE), &servidor_mudo(), Duration::from_secs(1)).await, Codigo::Tempo);
    }

    #[tokio::test]
    async fn chat_em_stream_com_a_chave_so_no_cabecalho() {
        let stream = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Oi\"}]}}]}\n\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"!\"}]},\"finishReason\":\"STOP\"}]}\n\n";
        let (base, pedidos) = servidor(vec![sse(stream)]);
        let tela = Gravador::default();
        let mut rx = sem_cancelar();
        let corpo = json!({"contents": [{"role": "user", "parts": [{"text": "oi"}]}]});
        let r = chamar_gemini_stream(&tela, "g1", "gemini-flash-lite-latest", &corpo, &mut rx, &chave_de_teste(CHAVE), &base).await.unwrap();
        assert_eq!(r["parts"][0]["text"], "Oi!");
        assert_eq!(tela.texto(), "Oi!");
        let p = pedidos.recv().unwrap();
        assert!(p.linha.starts_with("POST /v1beta/models/gemini-flash-lite-latest:streamGenerateContent?alt=sse "), "{}", p.linha);
        assert_eq!(p.cabecalho("x-goog-api-key"), Some(CHAVE));
        assert!(p.cabecalho("x-goog-user-project").is_none() && p.cabecalho("authorization").is_none());
        assert!(!p.linha.contains(CHAVE) && !p.corpo.contains(CHAVE));
    }

    #[tokio::test]
    async fn stream_sem_finish_reason_sai_com_aviso_e_nao_executa_ferramenta() {
        let stream = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Metade\"}]}}]}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"buscar\",\"args\":{}}}]}}]}\n\n"
        );
        let (base, _p) = servidor(vec![sse(stream)]);
        let tela = Gravador::default();
        let mut rx = sem_cancelar();
        let corpo = json!({"contents": [{"role": "user", "parts": [{"text": "oi"}]}]});
        let r = chamar_gemini_stream(&tela, "g-int", "gemini-flash-lite-latest", &corpo, &mut rx, &chave_de_teste(CHAVE), &base).await.unwrap();
        assert_eq!(tela.texto(), "Metade");
        let p = r["parts"].as_array().unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0]["text"], format!("Metade{AVISO_INTERROMPIDA}"));
        assert!(!r.to_string().contains("functionCall"));
        // so pensamento e o aviso nao vai nele
        let mut acc = Acumulador::default();
        acc.juntar(&json!({"candidates":[{"content":{"parts":[{"text":"pensando","thought":true}]}}]}));
        assert!(!sem_fim_vira_aviso(acc).partes[0]["text"].as_str().unwrap().contains("interrompida"));
    }

    #[tokio::test]
    async fn chat_com_chave_recusada_nao_tira_a_pessoa_do_drive() {
        let invalida = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","details":[{"reason":"API_KEY_INVALID"}]}}"#;
        let (base, _p) = servidor(vec![resp_json(400, invalida)]);
        let mut rx = sem_cancelar();
        let corpo = json!({"contents": [{"role": "user", "parts": [{"text": "oi"}]}]});
        let erro = chamar_gemini_stream(&Gravador::default(), "g2", "gemini-flash-lite-latest", &corpo, &mut rx, &chave_de_teste(CHAVE), &base).await.unwrap_err();
        assert!(erro.contains("chave do Gemini foi recusada"), "{erro}");
        let (base, _p) = servidor(vec![resp_json(403, r#"{"error":{"code":403,"message":"denied"}}"#)]);
        let erro = chamar_gemini_stream(&Gravador::default(), "g3", "gemini-flash-lite-latest", &corpo, &mut rx, &chave_de_teste(CHAVE), &base).await.unwrap_err();
        assert!(erro.contains("não tem permissão"), "{erro}");
        // o corpo de chamar_gemini_stream nao chama sair(): o login do Drive fica
        let fonte = include_str!("google.rs").replace("\r\n", "\n");
        let i = fonte.find("pub(crate) async fn chamar_gemini_stream(").unwrap();
        let fim = fonte[i..].find("\n}\n").unwrap();
        assert!(!fonte[i..i + fim].contains("sair()"));
    }
}

#[cfg(test)]
mod testes_fonte {
    //! Leitura do codigo-fonte: o que a versao 42_API retirou nao volta.

    fn sem_cr(t: &str) -> String {
        t.replace("\r\n", "\n")
    }

    /// Codigo do arquivo fora dos modulos de teste (corta no primeiro modulo `#[cfg(test)]`).
    fn producao(fonte: &str) -> String {
        match fonte.find("#[cfg(test)]\nmod ") {
            Some(i) => fonte[..i].to_string(),
            None => fonte.to_string(),
        }
    }

    const ARQUIVOS: &[(&str, &str)] = &[
        ("ia.rs", include_str!("../ia.rs")),
        ("lib.rs", include_str!("../lib.rs")),
        ("drive.rs", include_str!("../drive.rs")),
        ("drive_picker.rs", include_str!("../drive_picker.rs")),
        ("ia/google.rs", include_str!("google.rs")),
        ("ia/chave.rs", include_str!("chave.rs")),
        ("ia/comum.rs", include_str!("comum.rs")),
    ];

    fn codigo(nome: &str) -> String {
        producao(&sem_cr(ARQUIVOS.iter().find(|(n, _)| *n == nome).unwrap().1))
    }

    #[test]
    fn sem_escopos_do_gemini_chave_em_url_nem_projeto_do_google_cloud() {
        let proibidos = ["cloud-platform", "generative-language.retriever", "?key=", "&key=", "x-goog-user-project", "preparar_projeto", "URL_TERMOS_GEMINI", "TERMOS:", "EXIGIDOS_CONTA"];
        for (nome, _) in ARQUIVOS {
            let c = codigo(nome);
            for proibido in proibidos {
                assert!(!c.contains(proibido), "{nome} ainda tem {proibido}");
            }
        }
    }

    #[test]
    fn so_o_gemini_responde() {
        // o modulo de IA tem so a chave, as pecas comuns e o Gemini
        let ia = sem_cr(include_str!("../ia.rs"));
        let modulos: Vec<&str> = ia.lines().filter(|l| l.starts_with("pub mod ")).collect();
        assert_eq!(modulos, vec!["pub mod chave;", "pub mod comum;", "pub mod google;"]);
        assert!(sem_cr(include_str!("chave.rs")).contains(r#"pub const GEMINI: &str = "gemini";"#));
    }

    #[test]
    fn nenhum_print_de_chave() {
        for (nome, _) in ARQUIVOS {
            for linha in codigo(nome).lines() {
                let l = linha.to_lowercase();
                if l.contains("println!") || l.contains("eprintln!") || l.contains("dbg!") {
                    assert!(!l.contains("chave") && !l.contains("key") && !l.contains("token"), "{nome}: {linha}");
                }
            }
        }
        // os modulos novos nao imprimem nada
        for nome in ["ia/chave.rs", "ia/comum.rs"] {
            let c = codigo(nome);
            assert!(!c.contains("println!") && !c.contains("dbg!") && !c.contains("log::"), "{nome}");
        }
    }

    #[test]
    fn chave_api_sem_debug_display_clone_ou_serialize_derivados() {
        let fonte = sem_cr(include_str!("chave.rs"));
        let i = fonte.find("pub struct ChaveApi(String);").unwrap();
        let antes = &fonte[fonte[..i].rfind("\n\n").unwrap()..i];
        assert!(!antes.contains("derive"), "ChaveApi nao pode ter derive: {antes}");
        assert!(!fonte.contains("impl std::fmt::Display for ChaveApi"));
        assert!(fonte.contains(r#"f.write_str("ChaveApi(***)")"#));
        assert!(fonte.contains("valor.set_sensitive(true);"));
    }

    #[test]
    fn comandos_genericos_e_do_gemini_pelo_login_sairam_do_handler() {
        let lib = sem_cr(include_str!("../lib.rs"));
        let i = lib.find("tauri::generate_handler![").unwrap();
        let lista = &lib[i..i + lib[i..].find("])").unwrap()];
        for retirado in ["cmd_ia_salvar_chave", "cmd_ia_verificar_chave", "cmd_ia_remover_chave", "cmd_ia_google_preparar", "cmd_ia_google_entrar"] {
            assert!(!lista.contains(&format!("{retirado},")), "{retirado} ainda no handler");
            assert!(!producao(&lib).contains(&format!("fn {retirado}(")), "{retirado} ainda existe");
        }
        for retirado in ["cmd_ia_modelos"] {
            assert!(!lista.contains(&format!("{retirado},")), "{retirado} ainda no handler");
        }
        for novo in ["cmd_ia_chave_ativar", "cmd_ia_chave_status", "cmd_ia_chave_remover", "cmd_conta_google_entrar", "cmd_ia_google_info", "cmd_ia_google_sair"] {
            assert!(lista.contains(&format!("{novo},")), "{novo} falta no handler");
        }
        assert!(lib.contains(r#"pub const NUMERO_COMPILACAO: &str = "#));
    }

    #[test]
    fn ia_config_nao_tem_campo_de_chave() {
        let ia = sem_cr(include_str!("../ia.rs"));
        let i = ia.find("pub struct ConfigIA {").unwrap();
        let estrutura = &ia[i..i + ia[i..].find('}').unwrap()];
        let campos: Vec<&str> = estrutura.lines().filter(|l| l.trim_start().starts_with("pub ")).collect();
        for c in campos {
            assert!(!c.contains("chave") && !c.contains("key") && !c.contains("token"), "{c}");
        }
    }
}
