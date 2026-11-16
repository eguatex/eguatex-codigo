//! Compartilhamento de projetos pelo Google Drive, como o "Share" do Overleaf, sem
//! limite de colaboradores.
//!
//! O projeto continua sendo uma pasta comum no disco. No Drive ele vira um unico arquivo
//! `<nome>.eguatex.zip`, dentro da pasta "ÉguaTex" que o app cria. O app usa so o escopo
//! `drive.file`: toca apenas arquivos que ele criou ou que a pessoa escolheu no Google Picker
//! (drive_picker.rs). O Picker libera so o item escolhido, por isso o projeto compartilhado
//! e um arquivo e nao uma pasta.
//!
//! Sincronizar = baixar o zip, mesclar por arquivo com o disco e reenviar o zip no mesmo id.
//! Quando os dois lados mudam o mesmo arquivo, a versao do Drive fica no lugar e a local vira
//! "nome (conflito de fulano).ext", sem perder nada. Projetos antigos, salvos como pasta com
//! arquivos, viram zip na primeira sincronizacao do dono.
//!
//! O login e a conta Google unica do ÉguaTex (ia/google.rs).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::arquivos::{ItemZip, coletar_itens_zip};
use crate::drive_picker::{self, Modo, SUFIXO_PROJETO};
use crate::ia::google;

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";
const MIME_PASTA: &str = "application/vnd.google-apps.folder";
const MIME_ZIP: &str = "application/zip";
/// Marca os projetos criados pelo ÉguaTex (zip e pastas antigas).
const MARCA: (&str, &str) = ("eguatex", "1");
/// Marca a pasta "ÉguaTex" onde ficam os zips dos projetos.
const MARCA_PASTA_APP: (&str, &str) = ("eguatex", "app");
const NOME_PASTA_APP: &str = "ÉguaTex";
/// Comeco do erro quando o arquivo some ou a pessoa perde o acesso (HTTP 404). A interface
/// tira o prefixo e oferece "Abrir de novo pelo seletor do Google".
pub const ACESSO_PERDIDO: &str = "ACESSO_PERDIDO:";
/// Teto do conteudo de um zip aberto (o mesmo do "Abrir .zip"), contra arquivos feitos para
/// estourar a memoria.
const LIMITE_DESCOMPACTADO: u64 = crate::limites_zip::Limites::PADRAO.total;

/// Tempo maximo de um download ou upload de arquivo. O cliente padrao corta em 120 s, pouco para um
/// .zip de tese com imagens numa conexao lenta (e o erro virava "Sem conexao").
const TEMPO_TRANSFERENCIA: Duration = Duration::from_secs(30 * 60);

static SINCRONIZANDO: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Conteudo dos arquivos do projeto pelo caminho relativo ("cap/intro.tex").
type Conteudo = BTreeMap<String, Vec<u8>>;

// ---------------------------------------------------------------------------
// Conta
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct InfoDrive {
    pub conectado: bool,
    pub configurado: bool,
    pub email: String,
    pub nome: String,
    pub foto: String,
    /// "Entre de novo..." quando a conta antiga (Drive inteiro) foi desfeita.
    pub aviso: String,
    /// O seletor de arquivos do Google (Picker) esta configurado nesta versao.
    pub seletor_pronto: bool,
}

fn info() -> InfoDrive {
    let c = google::info();
    InfoDrive {
        conectado: c.conectado,
        configurado: c.configurado,
        email: c.email,
        nome: c.nome,
        foto: c.foto,
        aviso: c.aviso,
        seletor_pronto: drive_picker::configurado(),
    }
}

#[tauri::command]
pub fn cmd_drive_info() -> InfoDrive {
    info()
}

/// Entra com a conta Google unica. O Gemini so e preparado quando a pessoa liga o assistente.
#[tauri::command]
pub async fn cmd_drive_entrar(app: tauri::AppHandle) -> Result<InfoDrive, String> {
    google::entrar_conta(&app).await?;
    Ok(info())
}

#[tauri::command]
pub async fn cmd_drive_sair() -> Result<(), String> {
    google::sair().await
}

async fn token() -> Result<String, String> {
    google::token_acesso().await
}

// ---------------------------------------------------------------------------
// Chamadas a API do Drive
// ---------------------------------------------------------------------------

fn erro_da_api(status: u16, corpo: &str) -> String {
    let msg = serde_json::from_str::<Value>(corpo)
        .ok()
        .and_then(|j| j["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| corpo.chars().take(200).collect());
    match status {
        401 => "A sessão do Google Drive expirou. Entre de novo com a conta Google.".into(),
        404 => format!("{ACESSO_PERDIDO}O arquivo não está mais no seu Google Drive, ou o ÉguaTex perdeu o acesso a ele."),
        _ => format!("Google Drive recusou o pedido ({status}): {msg}"),
    }
}

fn acesso_perdido(erro: &str) -> bool {
    erro.starts_with(ACESSO_PERDIDO)
}

async fn enviar(req: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    let token = token().await?;
    // Conexao que nao abre (o pedido nem saiu) ganha novas tentativas. No Android o sistema corta
    // a rede do app enquanto o navegador do seletor esta na frente; ela volta quando a pessoa
    // retorna ao app, entao a espera la e de ate 1 minuto.
    let (tentativas, pausa) = if cfg!(target_os = "android") { (20, 3) } else { (3, 1) };
    let mut pedido = Some(req);
    let mut feitas = 0;
    let resp = loop {
        feitas += 1;
        let Some(atual) = pedido.take() else { return Err("Sem conexão com o Google Drive.".into()) };
        let reserva = atual.try_clone();
        match atual.bearer_auth(&token).send().await {
            Ok(r) => break r,
            Err(e) if e.is_connect() && feitas < tentativas && reserva.is_some() => {
                crate::registro::drive("rede", &format!("conexao falhou (tentativa {feitas} de {tentativas}): {e}"));
                pedido = reserva;
                tokio::time::sleep(Duration::from_secs(pausa)).await;
            }
            Err(e) => {
                crate::registro::drive("rede", &format!("pedido falhou: {e}"));
                return Err(if e.is_timeout() { "O Google Drive demorou demais para responder. Tente de novo." } else { "Sem conexão com o Google Drive." }.to_string());
            }
        }
    };
    if resp.status().is_success() {
        Ok(resp)
    } else {
        let status = resp.status().as_u16();
        let corpo = resp.text().await.unwrap_or_default();
        Err(erro_da_api(status, &corpo))
    }
}

async fn api(metodo: Method, url: &str, corpo: Option<Value>) -> Result<Value, String> {
    let mut req = google::cliente().request(metodo, url);
    if let Some(c) = corpo {
        req = req.json(&c);
    }
    let resp = enviar(req).await?;
    let texto = resp.text().await.map_err(|e| e.to_string())?;
    if texto.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&texto).map_err(|e| e.to_string())
}

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Dados de um arquivo do Drive que o app usa para detectar mudanca.
#[derive(Debug, Clone, Default)]
struct MetaRemota {
    id: String,
    nome: String,
    mime: String,
    md5: String,
    /// `headRevisionId`, ou `modifiedTime` quando o Drive nao devolve a revisao.
    versao: String,
    dono_sou_eu: bool,
}

fn ler_meta(j: &Value) -> MetaRemota {
    MetaRemota {
        id: j["id"].as_str().unwrap_or("").into(),
        nome: j["name"].as_str().unwrap_or("").into(),
        mime: j["mimeType"].as_str().unwrap_or("").into(),
        md5: j["md5Checksum"].as_str().unwrap_or("").into(),
        versao: j["headRevisionId"].as_str().or(j["modifiedTime"].as_str()).unwrap_or("").into(),
        dono_sou_eu: j["ownedByMe"].as_bool().unwrap_or(false),
    }
}

const CAMPOS_META: &str = "id,name,mimeType,md5Checksum,modifiedTime,headRevisionId,ownedByMe,trashed";

/// Metadados do arquivo. Arquivo na lixeira conta como perdido.
async fn meta(id: &str) -> Result<MetaRemota, String> {
    if !id_valido(id) {
        return Err("Identificador inválido.".into());
    }
    let j = api(Method::GET, &format!("{API}/files/{id}?fields={CAMPOS_META}"), None).await?;
    if j["trashed"].as_bool() == Some(true) {
        return Err(erro_da_api(404, ""));
    }
    Ok(ler_meta(&j))
}

#[derive(Debug, Clone)]
struct Remoto {
    id: String,
}

/// Arquivos de uma pasta antiga do Drive (formato anterior ao zip), pelo caminho relativo.
async fn listar_remoto(raiz: &str) -> Result<BTreeMap<String, Remoto>, String> {
    let mut arquivos = BTreeMap::new();
    let mut vistas = BTreeSet::new();
    let mut fila = VecDeque::from([(raiz.to_string(), String::new())]);
    while let Some((id, prefixo)) = fila.pop_front() {
        let mut pagina: Option<String> = None;
        loop {
            let mut url = reqwest::Url::parse(&format!("{API}/files")).map_err(|e| e.to_string())?;
            url.query_pairs_mut()
                .append_pair("q", &format!("'{id}' in parents and trashed=false"))
                .append_pair("fields", "nextPageToken,files(id,name,mimeType)")
                .append_pair("pageSize", "1000");
            if let Some(p) = &pagina {
                url.query_pairs_mut().append_pair("pageToken", p);
            }
            let j = api(Method::GET, url.as_str(), None).await?;
            for f in j["files"].as_array().into_iter().flatten() {
                let nome = f["name"].as_str().unwrap_or("");
                // "C:x" faria o join sair do projeto; CON, "a?b" e nomes com ponto no fim
                // nao existem no Windows: o item e pulado e o resto sincroniza
                if crate::arquivos::validar_nome(nome).is_err() {
                    continue;
                }
                let rel = if prefixo.is_empty() { nome.to_string() } else { format!("{prefixo}/{nome}") };
                let fid = f["id"].as_str().unwrap_or("").to_string();
                let mime = f["mimeType"].as_str().unwrap_or("");
                if mime == MIME_PASTA {
                    if !nome.starts_with('.') && vistas.insert(rel.clone()) {
                        fila.push_back((fid, rel));
                    }
                } else if !mime.starts_with("application/vnd.google-apps.") {
                    // Documentos Google nativos (Docs, Planilhas) nao tem conteudo para baixar
                    arquivos.entry(rel).or_insert(Remoto { id: fid });
                }
            }
            pagina = j["nextPageToken"].as_str().map(str::to_string);
            if pagina.is_none() {
                break;
            }
        }
    }
    Ok(arquivos)
}

/// Janela do Tauri guardada na primeira sincronizacao, para `baixar` avisar o andamento.
static APP_PROGRESSO: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// Canal do evento `tarefa-progresso` do download do Drive (o frontend liga ao rotulo da tarefa).
const CANAL_DOWNLOAD: &str = "drive-baixar";

/// No maximo 10 eventos por segundo.
const INTERVALO_PROGRESSO_MS: u128 = 100;

/// Decide se o andamento vai para a tela: a primeira vez, depois de 100 ms do ultimo, ou no fim.
pub(crate) fn deve_emitir_progresso(ultimo_ms: Option<u128>, agora_ms: u128, terminou: bool) -> bool {
    terminou || ultimo_ms.is_none_or(|u| agora_ms.saturating_sub(u) >= INTERVALO_PROGRESSO_MS)
}

/// Par (feito, total) do evento. Sem Content-Length (`total` ausente ou zero) nao ha porcentagem,
/// entao nao emite e a barra fica sem numero.
fn contagem_do_download(feito: u64, total: Option<u64>) -> Option<(u64, u64)> {
    total.filter(|t| *t > 0).map(|t| (feito.min(t), t))
}

/// Guarda a janela do Tauri para os eventos de progresso (chamado pelo setup e pela sincronizacao).
pub(crate) fn registrar_app_progresso(app: &tauri::AppHandle) {
    let _ = APP_PROGRESSO.set(app.clone());
}

/// Emite `tarefa-progresso` `{id: canal, feito, total}`. Sem janela registrada ou sem total, nao emite.
pub(crate) fn emitir_progresso_canal(canal: &str, feito: u64, total: Option<u64>) {
    use tauri::Emitter;
    if let (Some(app), Some((feito, total))) = (APP_PROGRESSO.get(), contagem_do_download(feito, total)) {
        let _ = app.emit("tarefa-progresso", json!({ "id": canal, "feito": feito, "total": total }));
    }
}

fn emitir_progresso(feito: u64, total: Option<u64>) {
    emitir_progresso_canal(CANAL_DOWNLOAD, feito, total);
}

async fn baixar(id: &str) -> Result<Vec<u8>, String> {
    let req = google::cliente().get(format!("{API}/files/{id}?alt=media")).timeout(TEMPO_TRANSFERENCIA);
    let mut resp = enviar(req).await?;
    let total = resp.content_length();
    let inicio = std::time::Instant::now();
    let mut ultimo: Option<u128> = None;
    let mut bytes: Vec<u8> = Vec::with_capacity(total.unwrap_or(0).min(64 * 1024 * 1024) as usize);
    while let Some(pedaco) = resp.chunk().await.map_err(|e| e.to_string())? {
        bytes.extend_from_slice(&pedaco);
        let agora = inicio.elapsed().as_millis();
        if deve_emitir_progresso(ultimo, agora, false) {
            ultimo = Some(agora);
            emitir_progresso(bytes.len() as u64, total);
        }
    }
    emitir_progresso(bytes.len() as u64, total);
    Ok(bytes)
}

#[cfg(test)]
mod progresso_testes {
    use super::*;

    #[test]
    fn emite_no_maximo_dez_por_segundo() {
        assert!(deve_emitir_progresso(None, 0, false));
        assert!(!deve_emitir_progresso(Some(0), 99, false));
        assert!(deve_emitir_progresso(Some(0), 100, false));
        assert!(!deve_emitir_progresso(Some(500), 450, false));
        // simula 1 s de pedacos a cada 10 ms: 10 eventos
        let (mut ultimo, mut n) = (None, 0);
        for ms in (0..1000u128).step_by(10) {
            if deve_emitir_progresso(ultimo, ms, false) {
                ultimo = Some(ms);
                n += 1;
            }
        }
        assert_eq!(n, 10);
    }

    #[test]
    fn o_fim_sempre_emite() {
        assert!(deve_emitir_progresso(Some(1000), 1001, true));
    }

    #[test]
    fn contagem_sem_content_length_nao_emite() {
        assert_eq!(contagem_do_download(10, None), None);
        assert_eq!(contagem_do_download(10, Some(0)), None);
        assert_eq!(contagem_do_download(50, Some(200)), Some((50, 200)));
        assert_eq!(contagem_do_download(300, Some(200)), Some((200, 200)));
    }
}

async fn para_lixeira(id: &str) -> Result<(), String> {
    api(Method::PATCH, &format!("{API}/files/{id}"), Some(json!({ "trashed": true }))).await.map(|_| ())
}

/// Pasta "ÉguaTex" no Meu Drive, criada pelo app na primeira vez.
async fn pasta_do_app() -> Result<String, String> {
    let mut url = reqwest::Url::parse(&format!("{API}/files")).map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair(
            "q",
            &format!(
                "mimeType='{MIME_PASTA}' and trashed=false and 'me' in owners and appProperties has {{ key='{}' and value='{}' }}",
                MARCA_PASTA_APP.0, MARCA_PASTA_APP.1
            ),
        )
        .append_pair("fields", "files(id)")
        .append_pair("pageSize", "1");
    let j = api(Method::GET, url.as_str(), None).await?;
    if let Some(id) = j["files"][0]["id"].as_str() {
        return Ok(id.to_string());
    }
    let corpo = json!({
        "name": NOME_PASTA_APP,
        "mimeType": MIME_PASTA,
        "appProperties": { MARCA_PASTA_APP.0: MARCA_PASTA_APP.1 },
    });
    let j = api(Method::POST, &format!("{API}/files?fields=id"), Some(corpo)).await?;
    j["id"].as_str().map(str::to_string).ok_or("O Drive não devolveu a pasta criada.".into())
}

/// Cria o zip do projeto no Drive, dentro da pasta do app.
async fn subir_novo(nome: &str, pai: &str, bytes: Vec<u8>) -> Result<MetaRemota, String> {
    let fronteira = format!("eguatex{}", google::agora());
    let meta = json!({
        "name": nome,
        "parents": [pai],
        "mimeType": MIME_ZIP,
        "appProperties": { MARCA.0: MARCA.1 },
    })
    .to_string();
    let mut corpo = Vec::with_capacity(bytes.len() + meta.len() + 200);
    corpo.extend_from_slice(format!("--{fronteira}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n").as_bytes());
    corpo.extend_from_slice(format!("--{fronteira}\r\nContent-Type: {MIME_ZIP}\r\n\r\n").as_bytes());
    corpo.extend_from_slice(&bytes);
    corpo.extend_from_slice(format!("\r\n--{fronteira}--").as_bytes());
    let req = google::cliente()
        .post(format!("{UPLOAD}/files?uploadType=multipart&fields={CAMPOS_META}"))
        .timeout(TEMPO_TRANSFERENCIA)
        .header("Content-Type", format!("multipart/related; boundary={fronteira}"))
        .body(corpo);
    let j: Value = enviar(req).await?.json().await.map_err(|e| e.to_string())?;
    Ok(ler_meta(&j))
}

/// Troca o conteudo do zip (PATCH no mesmo id).
async fn subir_conteudo(id: &str, bytes: Vec<u8>) -> Result<MetaRemota, String> {
    let req = google::cliente()
        .patch(format!("{UPLOAD}/files/{id}?uploadType=media&fields={CAMPOS_META}"))
        .timeout(TEMPO_TRANSFERENCIA)
        .header("Content-Type", MIME_ZIP)
        .body(bytes);
    let j: Value = enviar(req).await?.json().await.map_err(|e| e.to_string())?;
    Ok(ler_meta(&j))
}

/// Uma revisao do zip no Drive (so o que a checagem de gravacao alheia precisa).
#[derive(Debug, Clone, PartialEq)]
struct Revisao {
    id: String,
    email: String,
    sou_eu: bool,
}

/// Revisoes do zip, da mais antiga para a mais nova.
async fn listar_revisoes(id: &str) -> Result<Vec<Revisao>, String> {
    let j = api(
        Method::GET,
        &format!("{API}/files/{id}/revisions?pageSize=200&fields=revisions(id,modifiedTime,lastModifyingUser(emailAddress,me))"),
        None,
    )
    .await?;
    Ok(j["revisions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| Revisao {
            id: r["id"].as_str().unwrap_or("").into(),
            email: r["lastModifyingUser"]["emailAddress"].as_str().unwrap_or("").to_lowercase(),
            sou_eu: r["lastModifyingUser"]["me"].as_bool().unwrap_or(false),
        })
        .collect())
}

/// A revisao mais recente de outra pessoa entre a revisao conhecida antes do PATCH (`anterior`)
/// e a que o PATCH criou (`nova`). Se uma das duas nao esta na lista, nao ha como afirmar: `None`.
fn revisao_alheia_a_baixar<'a>(revisoes: &'a [Revisao], anterior: &str, nova: &str, eu: &str) -> Option<&'a Revisao> {
    let (Some(a), Some(n)) = (
        revisoes.iter().position(|r| r.id == anterior),
        revisoes.iter().position(|r| r.id == nova),
    ) else {
        return None;
    };
    if n <= a + 1 {
        return None;
    }
    let eu = eu.to_lowercase();
    revisoes[a + 1..n].iter().rev().find(|r| !r.sou_eu && (eu.is_empty() || r.email != eu))
}

/// Ha uma revisao de outra pessoa entre a anterior e a nova?
#[cfg(test)]
fn tem_revisao_alheia(revisoes: &[Revisao], anterior: &str, nova: &str, eu: &str) -> bool {
    revisao_alheia_a_baixar(revisoes, anterior, nova, eu).is_some()
}

/// Conteudo de uma revisao antiga do arquivo (files/{id}/revisions/{rev}?alt=media).
async fn baixar_revisao(id: &str, revisao: &str) -> Result<Vec<u8>, String> {
    if !id_valido(id) || revisao.is_empty() || !revisao.chars().all(|c| c.is_ascii_alphanumeric() || "-_=.~".contains(c)) {
        return Err("Identificador de revisão inválido.".into());
    }
    let req = google::cliente().get(format!("{API}/files/{id}/revisions/{revisao}?alt=media")).timeout(TEMPO_TRANSFERENCIA);
    let resp = enviar(req).await?;
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

/// Junta o que este computador enviou (`enviado`) com o que outra pessoa gravou no meio (`dela`),
/// tendo o zip de antes (`anterior`) como base. So um lado mudou: vale a mudanca. Os dois
/// mudaram o mesmo arquivo: a versao dela fica e a de agora vira "(conflito de ...)". Nada se perde.
fn juntar_com_revisao_alheia(anterior: &Conteudo, enviado: &Conteudo, dela: &Conteudo, eu: &str) -> Mescla {
    let base: BTreeMap<String, String> = anterior.iter().map(|(k, v)| (k.clone(), sha(v))).collect();
    mesclar(&base, enviado, dela, &BTreeSet::new(), eu)
}

// ---------------------------------------------------------------------------
// Zip do projeto
// ---------------------------------------------------------------------------

/// Monta o zip com os arquivos do projeto. Sem data nos arquivos: o mesmo conteudo gera
/// sempre os mesmos bytes.
fn empacotar(arquivos: &Conteudo) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (nome, bytes) in arquivos {
        zip.start_file(nome.as_str(), opcoes.large_file(bytes.len() as u64 >= u64::from(u32::MAX)))
            .map_err(|e| format!("Erro ao gravar {nome} no zip: {e}"))?;
        zip.write_all(bytes).map_err(|e| format!("Erro ao gravar {nome} no zip: {e}"))?;
    }
    zip.finish().map(|c| c.into_inner()).map_err(|e| format!("Erro ao finalizar o zip: {e}"))
}

/// Le o zip do projeto. Pula caminhos que sairiam da pasta, nomes que nao existem no
/// Windows, pastas ocultas (`.eguatex/`, `.git/`) e lixo do macOS.
fn desempacotar_tudo(bytes: &[u8]) -> Result<(Conteudo, Conteudo), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("O projeto no Drive não é um zip válido: {e}"))?;
    // entradas, profundidade, tamanho por arquivo e razao de compressao, antes de ler qualquer entrada
    crate::limites_zip::conferir(&mut zip, &crate::limites_zip::Limites::PADRAO)?;
    let mut saida = Conteudo::new();
    let mut preservados = Conteudo::new();
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut item = zip.by_index(i).map_err(|e| format!("O zip do projeto está corrompido: {e}"))?;
        if item.is_dir() {
            continue;
        }
        let rel = item.name().replace('\\', "/");
        let partes: Vec<&str> = rel.split('/').collect();
        // caminho absoluto ou com ".." sairia da pasta do projeto: descartado. Nome com ":"
        // ou reservado do Windows (a:b.tex, CON) nao e perigoso aqui, so e preservado abaixo.
        if rel.starts_with('/') || partes.contains(&"..") || rel.contains('\0') {
            continue;
        }
        let ultimo = partes.len() - 1;
        let oculta = partes.iter().enumerate().any(|(n, p)| n < ultimo && p.starts_with('.')) || partes.contains(&"__MACOSX");
        if oculta {
            continue;
        }
        // nome que o Windows nao aceita, executavel (.exe, .lnk, .bat...) ou com "-" no inicio:
        // nao vai para o disco, mas a entrada continua no zip reenviado, para nao sumir do
        // Drive no proximo envio (limites_zip::motivo_de_bloqueio)
        let invalido = partes.iter().any(|p| crate::arquivos::validar_nome(p).is_err())
            || crate::limites_zip::motivo_de_bloqueio(&rel).is_some();
        // o tamanho declarado no zip e conferido antes de ler a entrada
        if total.saturating_add(item.size()) > LIMITE_DESCOMPACTADO {
            return Err("O projeto no Drive é grande demais para abrir (mais de 500 MB descompactado).".into());
        }
        let mut buf = Vec::new();
        (&mut item).take(LIMITE_DESCOMPACTADO + 1).read_to_end(&mut buf).map_err(|e| format!("{rel}: {e}"))?;
        total += buf.len() as u64;
        if total > LIMITE_DESCOMPACTADO {
            return Err("O projeto no Drive é grande demais para abrir (mais de 500 MB descompactado).".into());
        }
        if invalido {
            preservados.insert(rel, buf);
        } else {
            saida.insert(rel, buf);
        }
    }
    Ok((saida, preservados))
}

/// Entre os preservados, os que ficaram de fora por seguranca (executavel ou "-" no inicio),
/// em ordem, para o aviso na tela. Nomes so invalidos no Windows nao entram aqui.
fn nomes_ignorados_por_seguranca(preservados: &Conteudo) -> Vec<String> {
    preservados
        .keys()
        .filter(|nome| crate::limites_zip::motivo_de_bloqueio(nome).is_some())
        .cloned()
        .collect()
}

/// Tira de `locais` e de `base` os caminhos que `motivo_de_bloqueio` recusa e os devolve. Eles
/// nao fazem parte da mescla: o Drive nunca os entrega ao disco, entao a ausencia deles em
/// `remotos` nao significa "apagado la". Voltam ao zip por `conteudo_do_zip`.
fn separar_bloqueados_locais(locais: &mut Conteudo, base: &mut BTreeMap<String, String>) -> Conteudo {
    let nomes: Vec<String> =
        locais.keys().filter(|n| crate::limites_zip::motivo_de_bloqueio(n).is_some()).cloned().collect();
    base.retain(|n, _| crate::limites_zip::motivo_de_bloqueio(n).is_none());
    nomes.into_iter().filter_map(|n| locais.remove(&n).map(|b| (n, b))).collect()
}

/// O que vai no zip reenviado: o resultado da mescla, os preservados do Drive e os bloqueados
/// que existem no disco (a versao do disco vence a preservada).
fn conteudo_do_zip(final_: &Conteudo, preservados: &Conteudo, bloqueados_locais: &Conteudo) -> Conteudo {
    let mut zip = final_.clone();
    zip.extend(preservados.clone());
    zip.extend(bloqueados_locais.clone());
    zip
}

/// So os arquivos que viram arquivos do projeto (sem os preservados).
#[cfg(test)]
fn desempacotar(bytes: &[u8]) -> Result<Conteudo, String> {
    desempacotar_tudo(bytes).map(|(arquivos, _)| arquivos)
}

/// Base da mescla de uma rodada. Na conversao da pasta antiga a base e vazia: a mescla so une.
/// Sem base, um arquivo que existe so no disco (por exemplo, criado por um colaborador e
/// invisivel para o dono com `drive.file`) nunca e tratado como "apagado la".
fn base_da_rodada(estado: &Estado) -> BTreeMap<String, String> {
    if estado.e_formato_antigo() { BTreeMap::new() } else { estado.sha_base() }
}

/// Troca o arquivo do Drive de um projeto. A base some, para a primeira mescla com o novo
/// arquivo so unir e nunca apagar arquivos locais que o novo zip nao tem.
fn trocar_arquivo(estado: &mut Estado, novo_id: String) {
    estado.arquivo_id = novo_id;
    estado.pasta_id.clear();
    estado.md5_remoto.clear();
    estado.versao.clear();
    estado.arquivos.clear();
}

/// Permissoes da pasta antiga que valem copiar para o zip: pessoas, grupos e "qualquer pessoa
/// com o link", sem o dono.
fn permissoes_a_copiar(j: &Value) -> Vec<Value> {
    j["permissions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let papel = p["role"].as_str()?;
            if !matches!(papel, "writer" | "reader" | "commenter") {
                return None;
            }
            match p["type"].as_str()? {
                "anyone" => Some(json!({ "type": "anyone", "role": papel })),
                tipo @ ("user" | "group") => {
                    let email = p["emailAddress"].as_str()?;
                    Some(json!({ "type": tipo, "role": papel, "emailAddress": email }))
                }
                _ => None,
            }
        })
        .collect()
}

/// Copia quem tinha acesso a pasta antiga para o zip novo (sem mandar e-mail).
async fn copiar_permissoes(pasta: &str, zip: &str) -> Result<(), String> {
    let j = api(Method::GET, &format!("{API}/files/{pasta}?fields=permissions(id,type,role,emailAddress)"), None).await?;
    let mut falhas = Vec::new();
    for p in permissoes_a_copiar(&j) {
        let url = format!("{API}/files/{zip}/permissions?sendNotificationEmail=false");
        if let Err(e) = api(Method::POST, &url, Some(p.clone())).await {
            falhas.push(format!("{}: {e}", p["emailAddress"].as_str().unwrap_or("link aberto")));
        }
    }
    if falhas.is_empty() { Ok(()) } else { Err(falhas.join("; ")) }
}

// ---------------------------------------------------------------------------
// Estado local (.eguatex/drive.json)
// ---------------------------------------------------------------------------

/// Ultimo conteudo sincronizado de um arquivo. Projetos antigos tambem guardam `id` e
/// `md5_remoto` (um por arquivo no Drive); o formato novo usa so `sha_local`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Registro {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    md5_remoto: String,
    sha_local: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Estado {
    /// Pasta do projeto no formato antigo. Some depois de convertida para zip.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pasta_id: String,
    /// Id do `.eguatex.zip` no Drive.
    #[serde(default)]
    arquivo_id: String,
    /// md5 do zip na ultima sincronizacao; igual ao atual = ninguem mexeu la.
    #[serde(default)]
    md5_remoto: String,
    /// Revisao (ou data) do zip na ultima sincronizacao.
    #[serde(default)]
    versao: String,
    #[serde(default)]
    arquivos: BTreeMap<String, Registro>,
}

impl Estado {
    fn e_formato_antigo(&self) -> bool {
        self.arquivo_id.is_empty() && !self.pasta_id.is_empty()
    }

    fn sha_base(&self) -> BTreeMap<String, String> {
        self.arquivos.iter().map(|(k, r)| (k.clone(), r.sha_local.clone())).collect()
    }

    /// Id do que representa o projeto no Drive (o zip, ou a pasta antiga ate converter).
    fn id_remoto(&self) -> Option<&str> {
        if !self.arquivo_id.is_empty() {
            Some(&self.arquivo_id)
        } else if !self.pasta_id.is_empty() {
            Some(&self.pasta_id)
        } else {
            None
        }
    }
}

fn caminho_estado(projeto: &Path) -> PathBuf {
    projeto.join(".eguatex").join("drive.json")
}

fn ler_estado(projeto: &Path) -> Option<Estado> {
    let texto = fs::read_to_string(caminho_estado(projeto)).ok()?;
    serde_json::from_str(&texto).ok()
}

fn gravar_estado(projeto: &Path, estado: &Estado) -> Result<(), String> {
    let p = caminho_estado(projeto);
    if let Some(pai) = p.parent() {
        fs::create_dir_all(pai).map_err(|e| e.to_string())?;
    }
    let texto = serde_json::to_string_pretty(estado).map_err(|e| e.to_string())?;
    crate::projetos::gravar_atomico(&p, texto.as_bytes()).map_err(|e| format!("Não foi possível gravar o estado do Drive: {e}"))
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn local(projeto: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(projeto.to_path_buf(), |p, parte| p.join(parte))
}

/// Arquivos do projeto que entram na sincronizacao (os mesmos do .zip de exportacao: sem
/// auxiliares do LaTeX, sem o PDF gerado e sem pastas ocultas como `.eguatex/`).
fn arquivos_locais(projeto: &Path) -> Result<BTreeSet<String>, String> {
    let mut itens = Vec::new();
    coletar_itens_zip(projeto, "", &mut itens, None).map_err(|e| e.to_string())?;
    Ok(itens
        .into_iter()
        .filter_map(|i| match i {
            ItemZip::Arquivo { nome, .. } => Some(nome),
            ItemZip::Pasta { .. } => None,
        })
        .collect())
}

/// Le os arquivos do projeto. Os que outro programa travou voltam a parte: ficam como estao
/// nos dois lados ate o proximo ciclo.
fn ler_locais(projeto: &Path) -> Result<(Conteudo, BTreeSet<String>), String> {
    let mut locais = Conteudo::new();
    let mut travados = BTreeSet::new();
    for rel in arquivos_locais(projeto)? {
        match fs::read(local(projeto, &rel)) {
            Ok(b) => {
                locais.insert(rel, b);
            }
            Err(_) => {
                travados.insert(rel);
            }
        }
    }
    Ok((locais, travados))
}

/// "cap/intro.tex" vira "cap/intro (conflito de nome).tex", com numero se ja existir.
fn nome_conflito_com(existe: &dyn Fn(&str) -> bool, rel: &str, quem: &str) -> String {
    let (pasta, arquivo) = match rel.rsplit_once('/') {
        Some((p, a)) => (format!("{p}/"), a.to_string()),
        None => (String::new(), rel.to_string()),
    };
    let (base, ext) = match arquivo.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (arquivo.clone(), String::new()),
    };
    let quem = quem.split('@').next().unwrap_or("outra pessoa");
    let quem = if quem.is_empty() { "outra pessoa" } else { quem };
    for n in 1..1000 {
        let marca = if n == 1 { format!("conflito de {quem}") } else { format!("conflito de {quem} {n}") };
        let candidato = format!("{pasta}{base} ({marca}){ext}");
        if !existe(&candidato) {
            return candidato;
        }
    }
    format!("{pasta}{base} (conflito){ext}")
}

#[cfg(test)]
fn nome_conflito(projeto: &Path, rel: &str, quem: &str) -> String {
    nome_conflito_com(&|n| local(projeto, n).exists(), rel, quem)
}

// ---------------------------------------------------------------------------
// Mescla (funcao pura: nao toca disco nem rede)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
struct Mescla {
    /// Como o projeto deve ficar no disco e no zip.
    final_: Conteudo,
    /// Arquivos do disco que precisam ser gravados (novos ou diferentes).
    escrever: Vec<String>,
    /// Arquivos do disco que precisam ser apagados.
    apagar: Vec<String>,
    /// Arquivos que o zip do Drive ainda nao tem igual.
    enviados: u32,
    baixados: u32,
    apagados: u32,
    conflitos: Vec<String>,
}

impl Mescla {
    /// O zip no Drive precisa ser reenviado?
    fn zip_mudou(&self, remotos: &Conteudo) -> bool {
        self.final_ != *remotos
    }
}

/// Mescla tres versoes de cada arquivo: a do disco, a do Drive e a ultima sincronizada
/// (`base`, so o sha). Quem mexeu sozinho vence; se os dois mexeram, o Drive fica no lugar e
/// a versao local vira "(conflito de fulano)".
fn mesclar(
    base: &BTreeMap<String, String>,
    locais: &Conteudo,
    remotos: &Conteudo,
    travados: &BTreeSet<String>,
    eu: &str,
) -> Mescla {
    let mut m = Mescla::default();
    let nomes: BTreeSet<&String> = locais.keys().chain(remotos.keys()).collect();
    for rel in nomes {
        if travados.contains(rel) {
            continue;
        }
        let l = locais.get(rel);
        let r = remotos.get(rel);
        let b = base.get(rel);
        match (l, r) {
            (None, None) => {}
            // so aqui
            (Some(l), None) => match b {
                Some(b) if sha(l) == *b => {} // apagado la, sem edicao aqui: some tambem aqui
                _ => {
                    m.final_.insert(rel.clone(), l.clone());
                }
            },
            // so la
            (None, Some(r)) => match b {
                Some(b) if sha(r) == *b => {} // apagado aqui, sem edicao la: some do Drive
                _ => {
                    m.final_.insert(rel.clone(), r.clone());
                }
            },
            // nos dois lados
            (Some(l), Some(r)) => {
                if l == r {
                    m.final_.insert(rel.clone(), r.clone());
                } else if b.is_some_and(|b| sha(l) == *b) {
                    m.final_.insert(rel.clone(), r.clone()); // so o Drive mudou
                } else if b.is_some_and(|b| sha(r) == *b) {
                    m.final_.insert(rel.clone(), l.clone()); // so o disco mudou
                } else {
                    // os dois mudaram: a versao do Drive fica e a local vira copia de conflito
                    let copia = nome_conflito_com(
                        &|n| locais.contains_key(n) || remotos.contains_key(n) || m.final_.contains_key(n),
                        rel,
                        eu,
                    );
                    m.final_.insert(rel.clone(), r.clone());
                    m.final_.insert(copia.clone(), l.clone());
                    m.conflitos.push(copia);
                }
            }
        }
    }
    // arquivos travados ficam como o Drive os tem, para nao sumirem do zip
    for rel in travados {
        if let Some(r) = remotos.get(rel) {
            m.final_.insert(rel.clone(), r.clone());
        }
    }
    for (rel, bytes) in &m.final_ {
        if travados.contains(rel) {
            continue;
        }
        if locais.get(rel) != Some(bytes) {
            m.escrever.push(rel.clone());
            if remotos.get(rel) == Some(bytes) {
                m.baixados += 1;
            }
        }
        if remotos.get(rel) != Some(bytes) {
            m.enviados += 1;
        }
    }
    for rel in locais.keys() {
        if !m.final_.contains_key(rel) && !travados.contains(rel) {
            m.apagar.push(rel.clone());
            m.apagados += 1;
        }
    }
    for rel in remotos.keys() {
        if !m.final_.contains_key(rel) && !locais.contains_key(rel) {
            m.apagados += 1;
        }
    }
    m
}

/// Algum arquivo do disco difere da ultima sincronizacao (novo, editado ou apagado)?
fn mudou_local(base: &BTreeMap<String, String>, locais: &Conteudo, travados: &BTreeSet<String>) -> bool {
    let nomes_locais: BTreeSet<&String> = locais.keys().chain(travados.iter()).collect();
    if nomes_locais != base.keys().collect::<BTreeSet<_>>() {
        return true;
    }
    locais.iter().any(|(rel, b)| base.get(rel).is_none_or(|s| *s != sha(b)))
}

// ---------------------------------------------------------------------------
// Sincronizacao
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct ResultadoSync {
    pub enviados: u32,
    pub baixados: u32,
    pub apagados: u32,
    /// Copias criadas quando os dois lados mudaram o mesmo arquivo.
    pub conflitos: Vec<String>,
    /// Arquivos abertos do Drive que mudaram dos dois lados: a versao do disco ficou como esta e a
    /// do Drive foi salva ao lado, com esses nomes ("nome (conflito de fulano).ext").
    pub copias_do_drive: Vec<String>,
    /// Arquivos que mudaram no disco (o editor recarrega estes).
    pub alterados: Vec<String>,
    /// O projeto estava como pasta no Drive e agora e um arquivo .eguatex.zip.
    pub convertido: bool,
    /// Avisos para a pessoa (pasta antiga mantida, nomes que o Windows nao aceita).
    pub avisos: Vec<String>,
    /// Arquivos que nao sincronizaram (sem acesso, envio ou gravacao que falhou), um texto por
    /// arquivo. Tambem entram em `avisos`; a interface pode mostrar so as falhas na barra.
    pub falhas: Vec<String>,
}

/// Anota uma falha por arquivo em `falhas` e em `avisos`.
fn registrar_falha(r: &mut ResultadoSync, texto: String) {
    r.falhas.push(texto.clone());
    r.avisos.push(texto);
}

fn gravar_local(projeto: &Path, rel: &str, bytes: &[u8]) -> Result<(), String> {
    gravar_local_em(&crate::versoes::pasta_dados(), projeto, rel, bytes)
}

/// Grava o arquivo no disco. Se ja existia, era texto e vai mudar, o texto de antes fica em
/// "Versões anteriores" (o historico fica em `raiz`).
fn gravar_local_em(raiz: &Path, projeto: &Path, rel: &str, bytes: &[u8]) -> Result<(), String> {
    if rel.split('/').any(|parte| crate::arquivos::validar_nome(parte).is_err()) {
        return Err(format!("{rel}: nome inválido no Windows"));
    }
    let destino = local(projeto, rel);
    if let Some(pai) = destino.parent() {
        fs::create_dir_all(pai).map_err(|e| e.to_string())?;
    }
    if destino.is_file() && fs::read(&destino).is_ok_and(|atual| atual != bytes) {
        crate::versoes::guardar_antes_de_mudar(raiz, projeto, rel);
    }
    fs::write(&destino, bytes).map_err(|e| format!("{rel}: {e}"))
}

/// Apaga o arquivo do disco, guardando antes o texto dele em "Versões anteriores".
fn apagar_local(projeto: &Path, rel: &str) -> std::io::Result<()> {
    apagar_local_em(&crate::versoes::pasta_dados(), projeto, rel)
}

fn apagar_local_em(raiz: &Path, projeto: &Path, rel: &str) -> std::io::Result<()> {
    crate::versoes::guardar_antes_de_mudar(raiz, projeto, rel);
    fs::remove_file(local(projeto, rel))
}

/// Nome do zip no Drive para o projeto: "Tese" vira "Tese.eguatex.zip".
fn nome_do_zip(nome_projeto: &str) -> String {
    let limpo = nome_seguro(nome_projeto);
    format!("{}{SUFIXO_PROJETO}", if limpo.is_empty() { "Projeto" } else { &limpo })
}

/// Nome da pasta local de um projeto baixado: tira o ".eguatex.zip" do nome do arquivo.
fn nome_local_do_zip(nome_arquivo: &str) -> String {
    let corte = nome_arquivo.len().saturating_sub(SUFIXO_PROJETO.len());
    let sem = if nome_arquivo.is_char_boundary(corte) && nome_arquivo[corte..].eq_ignore_ascii_case(SUFIXO_PROJETO) {
        &nome_arquivo[..corte]
    } else {
        nome_arquivo
    };
    let limpo = nome_seguro(sem);
    if limpo.is_empty() { "Projeto".to_string() } else { limpo }
}

fn nome_seguro(nome: &str) -> String {
    nome.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect::<String>().trim().trim_end_matches('.').to_string()
}

/// Uma rodada: baixa o que ha no Drive, mescla, reenvia e grava no disco. `Ok(None)` se alguem
/// mexeu no zip enquanto a rodada corria (a rodada recomeca, sem ter gravado nada).
async fn ciclo(projeto: &Path, estado: &mut Estado, eu: &str) -> Result<Option<ResultadoSync>, String> {
    let (mut locais, travados) = ler_locais(projeto)?;
    let mut base = base_da_rodada(estado);
    // executaveis e nomes com "-" que ja estao no disco ficam fora da mescla (senao "nao veio do
    // Drive" viraria "apagado la") e voltam ao zip na hora de empacotar
    let bloqueados_locais = separar_bloqueados_locais(&mut locais, &mut base);

    // 1. o que ha no Drive
    let mut remotos = Conteudo::new();
    let mut preservados = Conteudo::new();
    let mut meta_antes: Option<MetaRemota> = None;
    let mut pasta_antiga: Option<MetaRemota> = None;
    if estado.e_formato_antigo() {
        let pasta = meta(&estado.pasta_id).await?;
        if !pasta.dono_sou_eu {
            return Err("Este projeto está no formato antigo do Drive e só o dono pode convertê-lo. Peça para ele abrir o projeto no ÉguaTex atualizado e sincronizar uma vez.".into());
        }
        for (rel, r) in listar_remoto(&estado.pasta_id).await? {
            remotos.insert(rel, baixar(&r.id).await?);
        }
        pasta_antiga = Some(pasta);
    } else if !estado.arquivo_id.is_empty() {
        let m = meta(&estado.arquivo_id).await?;
        if !m.md5.is_empty() && m.md5 == estado.md5_remoto && bloqueados_locais.is_empty() && !mudou_local(&base, &locais, &travados) {
            return Ok(Some(ResultadoSync::default()));
        }
        (remotos, preservados) = desempacotar_tudo(&baixar(&estado.arquivo_id).await?)?;
        meta_antes = Some(m);
    }

    // 2. mescla
    let mescla = mesclar(&base, &locais, &remotos, &travados, eu);
    let bloqueado_mudou = bloqueados_locais.iter().any(|(rel, b)| preservados.get(rel) != Some(b));
    let precisa_subir = pasta_antiga.is_some() || estado.arquivo_id.is_empty() || mescla.zip_mudou(&remotos) || bloqueado_mudou;
    // o que o disco tem vale no zip; so o que nao existe no disco continua so preservado
    for rel in bloqueados_locais.keys() {
        preservados.remove(rel);
    }

    // 3. reenvia o zip (antes de mexer no disco)
    let mut meta_depois = meta_antes.clone().unwrap_or_default();
    if precisa_subir {
        let bytes = empacotar(&conteudo_do_zip(&mescla.final_, &preservados, &bloqueados_locais))?;
        if estado.arquivo_id.is_empty() {
            let nome_pasta = match &pasta_antiga {
                Some(p) => p.nome.clone(),
                None => projeto.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            };
            let pai = pasta_do_app().await?;
            meta_depois = subir_novo(&nome_do_zip(&nome_pasta), &pai, bytes).await?;
        } else {
            // Se alguem trocou o zip desde o download, recomeca em vez de sobrescrever. A
            // checagem compara md5 e revisao logo antes do PATCH. Resta uma janela de alguns
            // milissegundos entre a checagem e o PATCH: o upload do Drive v3 nao tem
            // precondicao (If-Match) documentada, entao uma gravacao nessa janela ainda ganharia.
            let agora = meta(&estado.arquivo_id).await?;
            let antes = meta_antes.clone().unwrap_or_default();
            if agora.md5 != antes.md5 || agora.versao != antes.versao {
                return Ok(None);
            }
            meta_depois = subir_conteudo(&estado.arquivo_id, bytes).await?;
            // alguem gravou entre o download e o PATCH (a janela que a checagem acima nao cobre):
            // o PATCH ja passou por cima da versao dela. Baixa essa revisao, junta com o que foi
            // enviado (conflito vira copia) e reenvia; a rodada seguinte traz o resultado para o
            // disco. Se a lista de revisoes falhar, segue como antes.
            if !antes.versao.is_empty() && !meta_depois.versao.is_empty()
                && let Ok(revs) = listar_revisoes(&estado.arquivo_id).await
                    && let Some(alheia) = revisao_alheia_a_baixar(&revs, &antes.versao, &meta_depois.versao, eu) {
                        let bytes = baixar_revisao(&estado.arquivo_id, &alheia.id).await.map_err(|e| {
                            format!("Outra pessoa gravou no projeto durante a sincronização e a versão dela não pôde ser baixada ({e}). Ela continua no histórico de versões do arquivo no Google Drive.")
                        })?;
                        let (dela, dela_preservados) = desempacotar_tudo(&bytes)?;
                        let juntado = juntar_com_revisao_alheia(&remotos, &mescla.final_, &dela, eu);
                        let mut preservados_total = preservados.clone();
                        preservados_total.extend(dela_preservados);
                        let no_zip = conteudo_do_zip(&juntado.final_, &preservados_total, &bloqueados_locais);
                        subir_conteudo(&estado.arquivo_id, empacotar(&no_zip)?).await?;
                        return Ok(None);
                    }
        }
    }

    // 4. disco
    let mut r = ResultadoSync {
        enviados: mescla.enviados,
        baixados: mescla.baixados,
        apagados: mescla.apagados,
        conflitos: mescla.conflitos.clone(),
        convertido: pasta_antiga.is_some(),
        ..Default::default()
    };
    let ignorados = nomes_ignorados_por_seguranca(&preservados);
    if !ignorados.is_empty() {
        r.avisos.push(crate::limites_zip::aviso_de_ignorados(&ignorados));
    }
    // os bloqueados por seguranca ja tem o aviso acima: aqui ficam so os nomes que o Windows nao aceita
    let so_windows: Vec<String> = preservados
        .keys()
        .filter(|n| crate::limites_zip::motivo_de_bloqueio(n).is_none())
        .map(|n| crate::limites_zip::nome_para_aviso(n))
        .collect();
    if !so_windows.is_empty() {
        let nomes = &so_windows;
        r.avisos.push(format!(
            "O projeto no Drive tem arquivos com nomes que o Windows não aceita. Eles continuam no Drive, mas não aparecem neste computador: {}",
            nomes.join(", ")
        ));
    }
    let mut falhas: BTreeMap<String, String> = BTreeMap::new();
    for rel in &mescla.escrever {
        match gravar_local(projeto, rel, &mescla.final_[rel]) {
            Ok(()) => r.alterados.push(rel.clone()),
            Err(e) => {
                falhas.insert(rel.clone(), e);
            }
        }
    }
    for rel in &mescla.apagar {
        match apagar_local(projeto, rel) {
            Ok(()) => r.alterados.push(rel.clone()),
            Err(e) => {
                falhas.insert(rel.clone(), format!("{rel}: {e}"));
            }
        }
    }

    // 5. estado: o que nao foi gravado no disco mantem a base antiga e tenta no proximo ciclo
    let mut novos = BTreeMap::new();
    for (rel, bytes) in &mescla.final_ {
        let sha_novo = if falhas.contains_key(rel) {
            match estado.arquivos.get(rel) {
                Some(antigo) => antigo.sha_local.clone(),
                None => continue,
            }
        } else {
            sha(bytes)
        };
        novos.insert(rel.clone(), Registro { sha_local: sha_novo, ..Default::default() });
    }
    for rel in &travados {
        if let Some(antigo) = estado.arquivos.get(rel) {
            novos.insert(rel.clone(), antigo.clone());
        }
    }
    let pasta_a_apagar = pasta_antiga.as_ref().map(|p| p.id.clone());
    if !meta_depois.id.is_empty() {
        estado.arquivo_id = meta_depois.id.clone();
    }
    estado.md5_remoto = meta_depois.md5.clone();
    estado.versao = meta_depois.versao.clone();
    estado.pasta_id.clear();
    estado.arquivos = novos;
    gravar_estado(projeto, estado)?;

    // 6. a pasta antiga so vai para a lixeira depois de o zip estar no Drive e o estado gravado
    // e so depois de copiar quem tinha acesso a ela; se a copia falhar, a pasta fica e a pessoa e avisada
    if let Some(id) = pasta_a_apagar {
        match copiar_permissoes(&id, &estado.arquivo_id).await {
            Ok(()) => {
                let _ = para_lixeira(&id).await;
            }
            Err(e) => r.avisos.push(format!(
                "A pasta antiga continua no seu Drive porque não foi possível copiar quem tinha acesso a ela para o novo arquivo ({e}). Convide essas pessoas de novo em Compartilhar."
            )),
        }
    }
    if let Some((_, e)) = falhas.iter().next() {
        return Err(e.clone());
    }
    Ok(Some(r))
}

/// Roda as rodadas com a trava `SINCRONIZANDO` ja em maos.
async fn sincronizar_travado(projeto: &Path, mut estado: Estado) -> Result<ResultadoSync, String> {
    let eu = google::info().email;
    for _ in 0..3 {
        if let Some(r) = ciclo(projeto, &mut estado, &eu).await? {
            return Ok(r);
        }
    }
    Err("O projeto mudou várias vezes no Drive durante a sincronização. Tente de novo em instantes.".into())
}

async fn sincronizar(projeto: &Path) -> Result<ResultadoSync, String> {
    // o estado so e lido depois de pegar a trava: outra chamada pode ter acabado de gravar
    let _vez = SINCRONIZANDO.lock().await;
    let Some(estado) = ler_estado(projeto) else {
        // sem o zip do ÉguaTex: so os arquivos abertos do Drive (vinculos) voltam para la
        let vinculos = ler_vinculos(projeto);
        if vinculos.is_empty() {
            return Err("Este projeto não está no Google Drive.".into());
        }
        return sincronizar_vinculos(projeto, vinculos).await;
    };
    sincronizar_travado(projeto, estado).await
}

fn pasta_projeto(caminho: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(caminho);
    if p.is_dir() { Ok(p) } else { Err(format!("Pasta do projeto não encontrada: {caminho}")) }
}

/// Id do projeto no Drive (o zip, ou a pasta antiga), se o projeto estiver no Drive.
#[tauri::command]
pub fn cmd_drive_projeto(caminho: String) -> Option<String> {
    let projeto = Path::new(&caminho);
    ler_estado(projeto)
        .and_then(|e| e.id_remoto().map(str::to_string))
        // projeto de arquivos soltos do Drive: o id do primeiro arquivo vinculado basta para a interface
        .or_else(|| ler_vinculos(projeto).values().next().map(|v| v.id.clone()))
}

#[tauri::command]
pub async fn cmd_drive_sincronizar(caminho: String) -> Result<ResultadoSync, String> {
    sincronizar(&pasta_projeto(&caminho)?).await
}

/// Decide sob a trava: se ja ha zip, devolve o id; senao cria. `ler` e `criar` entram como
/// parametros para o teste simular duas chamadas simultaneas sem o Drive.
async fn publicar_sob_trava<L, C, F>(trava: &tokio::sync::Mutex<()>, ler: L, criar: C) -> Result<String, String>
where
    L: Fn() -> Option<Estado>,
    C: Fn(Estado) -> F,
    F: std::future::Future<Output = Result<(), String>>,
{
    // o estado so e lido depois de pegar a trava, senao duas chamadas veriam "sem zip" e criariam dois
    let _vez = trava.lock().await;
    let estado = match ler() {
        Some(e) if !e.arquivo_id.is_empty() => return Ok(e.arquivo_id),
        Some(e) => e,
        None => Estado::default(),
    };
    criar(estado).await?;
    ler()
        .map(|e| e.arquivo_id)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "O Drive não devolveu o arquivo do projeto.".to_string())
}

/// Envia o projeto para o Drive (primeira vez) e devolve o id do zip. Projetos no formato
/// antigo (pasta) viram zip aqui.
async fn publicar(projeto: &Path) -> Result<String, String> {
    publicar_sob_trava(&SINCRONIZANDO, || ler_estado(projeto), |e| async move { sincronizar_travado(projeto, e).await.map(|_| ()) }).await
}

/// Quando o projeto baixado de outra pessoa deu 404, a pessoa escolhe o arquivo de novo no
/// seletor do Google. O app passa a usar o id escolhido; a sincronizacao seguinte mescla com o
/// disco, sem apagar nada local. `false` se a pessoa cancelou.
#[tauri::command]
pub async fn cmd_drive_reabrir_seletor(app: tauri::AppHandle, caminho: String) -> Result<bool, String> {
    let projeto = pasta_projeto(&caminho)?;
    let mut estado = ler_estado(&projeto).ok_or("Este projeto não está no Google Drive.")?;
    // Android: o app fica acordado e com rede ate o fim do comando, nao so durante o seletor
    let _acordado = google::manter_acordado::Guarda::nova();
    let Some(escolhido) = drive_picker::escolher(&app, Modo::Projeto).await? else {
        return Ok(false);
    };
    // so vale se o arquivo escolhido for mesmo um projeto do ÉguaTex
    conferir_projeto(&meta(&escolhido.id).await?)?;
    trocar_arquivo(&mut estado, escolhido.id);
    gravar_estado(&projeto, &estado)?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Pessoas e link (agem sobre o id do zip)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Pessoa {
    pub id: String,
    pub email: String,
    pub nome: String,
    /// "owner" (dono), "writer" (pode editar) ou "reader" (so ver)
    pub papel: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Compartilhamento {
    pub publicado: bool,
    pub link: String,
    pub link_aberto: bool,
    pub pessoas: Vec<Pessoa>,
    /// Nome do arquivo no Drive, que a pessoa convidada escolhe no seletor.
    pub arquivo: String,
}

async fn ler_compartilhamento(arquivo: &str) -> Result<Compartilhamento, String> {
    let j = api(
        Method::GET,
        &format!("{API}/files/{arquivo}?fields=name,webViewLink,permissions(id,type,role,emailAddress,displayName)"),
        None,
    )
    .await?;
    let mut pessoas = Vec::new();
    let mut link_aberto = false;
    for p in j["permissions"].as_array().into_iter().flatten() {
        match p["type"].as_str() {
            Some("anyone") => link_aberto = true,
            Some("user") | Some("group") => pessoas.push(Pessoa {
                id: p["id"].as_str().unwrap_or("").into(),
                email: p["emailAddress"].as_str().unwrap_or("").into(),
                nome: p["displayName"].as_str().unwrap_or("").into(),
                papel: p["role"].as_str().unwrap_or("").into(),
            }),
            _ => {}
        }
    }
    pessoas.sort_by_key(|p| (p.papel != "owner", p.email.clone()));
    Ok(Compartilhamento {
        publicado: true,
        link: j["webViewLink"].as_str().unwrap_or("").into(),
        link_aberto,
        pessoas,
        arquivo: j["name"].as_str().unwrap_or("").into(),
    })
}

#[tauri::command]
pub async fn cmd_drive_compartilhamento(caminho: String) -> Result<Compartilhamento, String> {
    match ler_estado(Path::new(&caminho)) {
        // pasta antiga (sem zip ainda): conta como nao publicado ate a conversao
        Some(e) if !e.arquivo_id.is_empty() => ler_compartilhamento(&e.arquivo_id).await,
        _ => Ok(Compartilhamento { publicado: false, link: String::new(), link_aberto: false, pessoas: vec![], arquivo: String::new() }),
    }
}

fn email_valido(e: &str) -> bool {
    let e = e.trim();
    match e.split_once('@') {
        Some((a, b)) => !a.is_empty() && b.contains('.') && !b.starts_with('.') && !b.ends_with('.') && !e.contains(char::is_whitespace),
        None => false,
    }
}

/// Texto do e-mail de convite: explica como a pessoa abre o projeto no ÉguaTex.
fn texto_do_convite(mensagem: &str, arquivo: &str) -> String {
    let instrucao = format!(
        "Para editar: instale o ÉguaTex, abra Projetos, Compartilhados comigo, clique em Escolher no Google Drive e selecione o arquivo \"{arquivo}\" em Compartilhados comigo."
    );
    if mensagem.trim().is_empty() {
        format!("Convite para editar um projeto no ÉguaTex. {instrucao}")
    } else {
        format!("{}\n\n{instrucao}", mensagem.trim())
    }
}

/// Convida pessoas por e-mail (sem limite). Publica o projeto antes, se preciso.
#[tauri::command]
pub async fn cmd_drive_convidar(
    caminho: String,
    emails: Vec<String>,
    papel: String,
    mensagem: String,
) -> Result<Compartilhamento, String> {
    let emails: Vec<String> = emails.iter().map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty()).collect();
    if let Some(ruim) = emails.iter().find(|e| !email_valido(e)) {
        return Err(format!("E-mail inválido: {ruim}"));
    }
    if emails.is_empty() {
        return Err("Digite pelo menos um e-mail.".into());
    }
    let papel = if papel == "reader" { "reader" } else { "writer" };
    let arquivo = publicar(&pasta_projeto(&caminho)?).await?;
    let nome = meta(&arquivo).await?.nome;
    let texto = texto_do_convite(&mensagem, &nome);
    for email in &emails {
        let mut url = reqwest::Url::parse(&format!("{API}/files/{arquivo}/permissions")).map_err(|e| e.to_string())?;
        url.query_pairs_mut()
            .append_pair("sendNotificationEmail", "true")
            .append_pair("emailMessage", &texto);
        api(Method::POST, url.as_str(), Some(json!({ "type": "user", "role": papel, "emailAddress": email })))
            .await
            .map_err(|e| format!("{email}: {e}"))?;
    }
    ler_compartilhamento(&arquivo).await
}

#[tauri::command]
pub async fn cmd_drive_remover_pessoa(caminho: String, permissao: String) -> Result<Compartilhamento, String> {
    let e = ler_estado(Path::new(&caminho)).ok_or("Este projeto não está no Google Drive.")?;
    let id = e.id_remoto().ok_or("Este projeto não está no Google Drive.")?;
    if !id_valido(&permissao) {
        return Err("Identificador de permissão inválido.".into());
    }
    api(Method::DELETE, &format!("{API}/files/{id}/permissions/{permissao}"), None).await?;
    ler_compartilhamento(id).await
}

/// Liga ou desliga "qualquer pessoa com o link pode editar". Publica antes, se preciso.
#[tauri::command]
pub async fn cmd_drive_link(caminho: String, ligado: bool) -> Result<Compartilhamento, String> {
    let arquivo = publicar(&pasta_projeto(&caminho)?).await?;
    let atual = api(Method::GET, &format!("{API}/files/{arquivo}?fields=permissions(id,type)"), None).await?;
    let anyone: Vec<String> = atual["permissions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["type"] == "anyone")
        .filter_map(|p| p["id"].as_str().map(str::to_string))
        .collect();
    if ligado && anyone.is_empty() {
        api(Method::POST, &format!("{API}/files/{arquivo}/permissions"), Some(json!({ "type": "anyone", "role": "writer" }))).await?;
    } else if !ligado {
        for id in anyone {
            api(Method::DELETE, &format!("{API}/files/{arquivo}/permissions/{id}"), None).await?;
        }
    }
    ler_compartilhamento(&arquivo).await
}

// ---------------------------------------------------------------------------
// Projetos do Drive
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ProjetoRemoto {
    pub id: String,
    pub nome: String,
    pub dono: String,
    pub modificado: String,
}

/// Projetos do EguaTex que a propria pessoa tem no Drive (zips com a marca do app e pastas
/// antigas), para baixar em outro computador. Projetos de outras pessoas so aparecem pelo
/// seletor do Google.
#[tauri::command]
pub async fn cmd_drive_meus_projetos() -> Result<Vec<ProjetoRemoto>, String> {
    let mut url = reqwest::Url::parse(&format!("{API}/files")).map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair(
            "q",
            &format!(
                "'me' in owners and (mimeType='{MIME_ZIP}' or mimeType='{MIME_PASTA}') and trashed=false and appProperties has {{ key='{}' and value='{}' }}",
                MARCA.0, MARCA.1
            ),
        )
        .append_pair("fields", "files(id,name,modifiedTime)")
        .append_pair("orderBy", "modifiedTime desc")
        .append_pair("pageSize", "200");
    let j = api(Method::GET, url.as_str(), None).await?;
    Ok(j["files"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| ProjetoRemoto {
            id: f["id"].as_str().unwrap_or("").into(),
            nome: nome_local_do_zip(f["name"].as_str().unwrap_or("")),
            dono: String::new(),
            modificado: f["modifiedTime"].as_str().unwrap_or("").into(),
        })
        .collect())
}

/// Salva um projeto do computador no Drive (cria o zip e envia os arquivos), sem
/// compartilhar com ninguem. Devolve o id do zip no Drive.
#[tauri::command]
pub async fn cmd_drive_enviar_projeto(caminho: String) -> Result<String, String> {
    let projeto = Path::new(&caminho);
    if !projeto.is_dir() {
        return Err(format!("A pasta {caminho} não existe."));
    }
    let id = publicar(projeto).await?;
    sincronizar(projeto).await?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Planilhas e Documentos Google (escolhidos no Picker)
// ---------------------------------------------------------------------------

const MIME_DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

#[derive(Serialize)]
pub struct ArquivoGoogle {
    pub id: String,
    pub nome: String,
    pub modificado: String,
}

/// Abre o seletor do Google para a pessoa escolher uma planilha ("planilha") ou um documento
/// ("documento"). O app so le o arquivo escolhido. `None` se a pessoa cancelou.
#[tauri::command]
pub async fn cmd_drive_escolher(app: tauri::AppHandle, tipo: String) -> Result<Option<ArquivoGoogle>, String> {
    let modo = match Modo::de_texto(&tipo) {
        Some(m @ (Modo::Planilha | Modo::Documento)) => m,
        _ => return Err(format!("Tipo desconhecido: {tipo}")),
    };
    Ok(drive_picker::escolher(&app, modo).await?.map(|e| ArquivoGoogle { id: e.id, nome: e.nome, modificado: String::new() }))
}

/// Exporta um arquivo Google (planilha ou documento) no formato `mime`.
async fn exportar_google(id: &str, mime: &str) -> Result<Vec<u8>, String> {
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Identificador inválido.".into());
    }
    let mut url = reqwest::Url::parse(&format!("{API}/files/{id}/export")).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("mimeType", mime);
    let resp = enviar(google::cliente().get(url)).await?;
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

/// Primeira aba de uma planilha Google, em CSV (o assistente de tabela le as celulas).
#[tauri::command]
pub async fn cmd_drive_planilha_csv(id: String) -> Result<String, String> {
    let bytes = exportar_google(&id, "text/csv").await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Converte um documento Google em LaTeX com o Pandoc. As imagens vao para a pasta
/// `figuras-<nome>` dentro do projeto. Devolve o texto LaTeX (so o corpo).
#[tauri::command]
pub async fn cmd_drive_documento_latex(id: String, nome: String, caminho_projeto: String) -> Result<String, String> {
    let pandoc = crate::exportar::pandoc().ok_or("O Pandoc não foi encontrado.")?;
    let bytes = exportar_google(&id, MIME_DOCX).await?;
    let projeto = PathBuf::from(&caminho_projeto);
    if !projeto.is_dir() {
        return Err("Abra um projeto antes de importar.".into());
    }
    let limpo: String = nome
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(40)
        .collect();
    let pasta_figuras = format!("figuras-{}", if limpo.is_empty() { "google-docs".into() } else { limpo });
    let temporario = std::env::temp_dir().join(format!("eguatex-docs-{}-{id}.docx", std::process::id()));
    fs::write(&temporario, &bytes).map_err(|e| e.to_string())?;
    let mut comando = std::process::Command::new(pandoc);
    comando
        .current_dir(&projeto)
        .arg("--from=docx")
        .arg("--to=latex")
        .arg("--wrap=preserve")
        .arg(format!("--extract-media={pasta_figuras}"))
        .arg(&temporario);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        comando.creation_flags(0x0800_0000);
    }
    let saida = tauri::async_runtime::spawn_blocking(move || comando.output())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Falha ao executar o Pandoc: {e}"))?;
    let _ = fs::remove_file(&temporario);
    if !saida.status.success() {
        return Err(format!("O Pandoc não converteu o documento: {}", String::from_utf8_lossy(&saida.stderr)));
    }
    // no Windows o pandoc pode escrever "figuras-x\media\image1.png"; no LaTeX a barra e "/"
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    let mut saida_latex = String::with_capacity(texto.len());
    let mut resto = texto.as_str();
    while let Some(i) = resto.find("\\includegraphics") {
        let (antes, depois) = resto.split_at(i);
        saida_latex.push_str(antes);
        let fim = depois.find('}').map(|f| f + 1).unwrap_or(depois.len());
        let (comando, sobra) = depois.split_at(fim);
        let (nome_cmd, args) = comando.split_at("\\includegraphics".len());
        saida_latex.push_str(nome_cmd);
        saida_latex.push_str(&args.replace('\\', "/"));
        resto = sobra;
    }
    saida_latex.push_str(resto);
    Ok(saida_latex)
}

/// Tira o id de um link do Drive (".../file/d/ID/view", ".../folders/ID?usp=sharing" ou
/// "...?id=ID") ou aceita o id puro.
fn id_do_link(link: &str) -> Option<String> {
    let link = link.trim();
    let candidato = if let Some(resto) = link.split("/folders/").nth(1) {
        resto.split(['?', '/', '#']).next().unwrap_or("")
    } else if let Some(resto) = link.split("/d/").nth(1) {
        resto.split(['?', '/', '#']).next().unwrap_or("")
    } else if let Some(resto) = link.split("id=").nth(1) {
        resto.split(['&', '#']).next().unwrap_or("")
    } else {
        link
    };
    (candidato.len() >= 10 && candidato.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .then(|| candidato.to_string())
}

/// O que o id aponta: um zip de projeto ou uma pasta antiga do app.
fn conferir_projeto(m: &MetaRemota) -> Result<(), String> {
    let e_zip = m.nome.to_lowercase().ends_with(SUFIXO_PROJETO) || m.mime == MIME_ZIP;
    if e_zip || m.mime == MIME_PASTA {
        Ok(())
    } else {
        Err("Esse arquivo não é um projeto do ÉguaTex. Escolha um arquivo que termina em .eguatex.zip.".into())
    }
}

/// Baixa um projeto para uma pasta nova dentro de `pasta_pai`. Aceita o id ou o link copiado do
/// Drive. Se o id der 404 (a pessoa nao e dona e ainda nao escolheu o arquivo no Picker), ou se
/// nada for informado, abre o seletor do Google. Devolve o caminho local, ou `None` se a
/// pessoa cancelou o seletor.
#[tauri::command]
pub async fn cmd_drive_baixar_projeto(app: tauri::AppHandle, id_ou_link: String, pasta_pai: String) -> Result<Option<String>, String> {
    let informado = id_ou_link.trim();
    let id = if informado.is_empty() {
        None
    } else {
        Some(id_do_link(informado).ok_or("Link do Google Drive inválido. Copie o link do arquivo .eguatex.zip.")?)
    };
    let achado = match id {
        Some(id) => match meta(&id).await {
            Ok(m) => Some(m),
            Err(e) if acesso_perdido(&e) => None,
            Err(e) => return Err(e),
        },
        None => None,
    };
    // Android: o app fica acordado e com rede ate o fim do comando, nao so durante o seletor
    let _acordado = google::manter_acordado::Guarda::nova();
    let m = match achado {
        Some(m) => m,
        None => {
            let Some(e) = drive_picker::escolher(&app, Modo::Projeto).await? else { return Ok(None) };
            meta(&e.id).await?
        }
    };
    baixar_projeto_de(m, &pasta_pai).await
}

/// O que vem depois de achar o arquivo: so o .eguatex.zip e sincronizado como projeto
/// compartilhado; zip comum e outros arquivos abrem como arquivos num projeto novo (mesmo
/// caminho do "Abrir do Google Drive").
async fn baixar_projeto_de(m: MetaRemota, pasta_pai: &str) -> Result<Option<String>, String> {
    let decisao = decidir_escolha_projeto(&m.nome, &m.mime);
    match decisao {
        EscolhaProjeto::Inaceitavel => {
            return Err(format!("{}: o ÉguaTex não abre esse tipo de arquivo. Use um .zip, .tex, .bib, figura ou PDF.", m.nome));
        }
        EscolhaProjeto::ZipComum | EscolhaProjeto::ArquivoSolto | EscolhaProjeto::TalvezZip => {
            let bytes = baixar(&m.id).await?;
            let zip = e_zip_de_fato(&m.nome, &m.mime, &bytes);
            if decisao == EscolhaProjeto::TalvezZip && !zip {
                return Err(format!("{}: o ÉguaTex não abre esse tipo de arquivo. Use um .zip, .tex, .bib, figura ou PDF.", m.nome));
            }
            // zip de um projeto do ÉguaTex (com .eguatex/drive.json) segue como projeto compartilhado
            if !(zip && zip_e_projeto_eguatex(&bytes)) {
                return abrir_como_arquivos(Path::new(pasta_pai), m, bytes).await.map(Some);
            }
        }
        EscolhaProjeto::ProjetoCompartilhado => {}
    }
    conferir_projeto(&m)?;

    let nome = nome_local_do_zip(&m.nome);
    let pai = PathBuf::from(pasta_pai);
    fs::create_dir_all(&pai).map_err(|e| e.to_string())?;
    let destino = (1..10000)
        .map(|n| if n == 1 { pai.join(&nome) } else { pai.join(format!("{nome} ({n})")) })
        .find(|p| !p.exists())
        .ok_or("Não foi possível achar um nome livre para a pasta.")?;
    fs::create_dir_all(&destino).map_err(|e| e.to_string())?;
    let estado = if m.mime == MIME_PASTA {
        Estado { pasta_id: m.id.clone(), ..Default::default() }
    } else {
        Estado { arquivo_id: m.id.clone(), ..Default::default() }
    };
    gravar_estado(&destino, &estado)?;
    if let Err(e) = sincronizar(&destino).await {
        let _ = fs::remove_dir_all(&destino);
        return Err(e);
    }
    Ok(Some(destino.display().to_string()))
}

// ---------------------------------------------------------------------------
// Arquivos soltos do Drive ("Abrir do Google Drive")
// ---------------------------------------------------------------------------
//
// A pessoa escolhe arquivos no Picker (um por um, nao a pasta: o escopo `drive.file` so libera o
// que foi escolhido). Eles viram arquivos comuns no projeto, e `.eguatex/drive-vinculos.json`
// lembra de qual arquivo do Drive cada um veio. Ao sincronizar um projeto sem o zip do ÉguaTex,
// so os arquivos vinculados voltam ao Drive; arquivos novos criados no projeto ficam so no disco.

const ARQUIVO_VINCULOS: &str = "drive-vinculos.json";

/// De qual arquivo do Drive um arquivo do projeto veio, e como ele estava na ultima sincronizacao.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Vinculo {
    id: String,
    /// md5 do arquivo no Drive na ultima sincronizacao (vazio para tipos sem md5).
    #[serde(default)]
    md5: String,
    /// Revisao (ou data) do arquivo no Drive na ultima sincronizacao.
    #[serde(default)]
    versao: String,
    /// sha do arquivo no disco quando ele bateu com o Drive pela ultima vez.
    #[serde(default)]
    sha_local: String,
    /// Versao do Drive que ja gerou uma copia "(conflito de ...)". Enquanto o Drive nao mudar de
    /// novo, nao cria outra copia nem sobrescreve nada.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    conflito_de: String,
    /// sha do arquivo no disco no momento do conflito: se mudar depois, a pessoa resolveu.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    sha_conflito: String,
}

/// Caminho local relativo ("fig 1.png") para o vinculo.
type Vinculos = BTreeMap<String, Vinculo>;

fn caminho_vinculos(projeto: &Path) -> PathBuf {
    projeto.join(".eguatex").join(ARQUIVO_VINCULOS)
}

/// Le o mapa de vinculos. Arquivo ausente ou corrompido vira mapa vazio (nada some do disco).
fn ler_vinculos(projeto: &Path) -> Vinculos {
    fs::read_to_string(caminho_vinculos(projeto))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn gravar_vinculos(projeto: &Path, vinculos: &Vinculos) -> Result<(), String> {
    let p = caminho_vinculos(projeto);
    if let Some(pai) = p.parent() {
        fs::create_dir_all(pai).map_err(|e| e.to_string())?;
    }
    let texto = serde_json::to_string_pretty(vinculos).map_err(|e| e.to_string())?;
    crate::projetos::gravar_atomico(&p, texto.as_bytes()).map_err(|e| format!("Não foi possível gravar os vínculos do Drive: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcaoVinculo {
    /// Nada mudou, ou o arquivo local sumiu (nunca apaga no Drive).
    Nada,
    /// So o disco mudou: envia (PATCH).
    Enviar,
    /// So o Drive mudou: traz para o disco.
    Baixar,
    /// Os dois mudaram: baixa o do Drive como "(conflito de ...)" e nao sobrescreve nada.
    Conflito,
    /// Ja ha copia de conflito desta versao do Drive e a pessoa ainda nao mexeu no arquivo local.
    EmConflito,
}

/// Identidade da versao no Drive: a revisao, ou o md5 se o Drive nao a devolveu.
fn identidade_remota<'a>(md5: &'a str, versao: &'a str) -> &'a str {
    if versao.is_empty() { md5 } else { versao }
}

fn remoto_mudou(v: &Vinculo, md5: &str, versao: &str) -> bool {
    if !md5.is_empty() && !v.md5.is_empty() {
        md5 != v.md5
    } else {
        versao != v.versao
    }
}

/// Decide o que fazer com um arquivo vinculado (funcao pura). `sha_local` e `None` se o arquivo
/// sumiu do disco ou esta travado por outro programa.
fn decidir_vinculo(v: &Vinculo, sha_local: Option<&str>, md5_remoto: &str, versao_remota: &str) -> AcaoVinculo {
    let Some(sha_atual) = sha_local else { return AcaoVinculo::Nada };
    if !v.conflito_de.is_empty() && v.conflito_de == identidade_remota(md5_remoto, versao_remota) {
        // a copia do Drive ja esta no disco; so segue quando a pessoa mexeu no arquivo depois
        return if sha_atual != v.sha_conflito { AcaoVinculo::Enviar } else { AcaoVinculo::EmConflito };
    }
    let local = sha_atual != v.sha_local;
    let remoto = remoto_mudou(v, md5_remoto, versao_remota);
    match (local, remoto) {
        (false, false) => AcaoVinculo::Nada,
        (true, false) => AcaoVinculo::Enviar,
        (false, true) => AcaoVinculo::Baixar,
        (true, true) => AcaoVinculo::Conflito,
    }
}

/// Depois de reler o arquivo no Drive logo antes do envio: se ele mudou (md5 ou revisao) desde a
/// leitura anterior, o envio vira conflito. Outras acoes nao mudam. Funcao pura.
fn acao_apos_releitura(acao: AcaoVinculo, antes: (&str, &str), agora: (&str, &str)) -> AcaoVinculo {
    let mudou = if !antes.0.is_empty() && !agora.0.is_empty() { antes.0 != agora.0 } else { antes.1 != agora.1 };
    if acao == AcaoVinculo::Enviar && mudou { AcaoVinculo::Conflito } else { acao }
}

/// "fig.png" vira "fig (2).png" (e "(3)"...) se o nome ja existir. `existe` recebe o nome relativo.
fn nome_livre(existe: &dyn Fn(&str) -> bool, rel: &str) -> String {
    if !existe(rel) {
        return rel.to_string();
    }
    let (pasta, arquivo) = match rel.rsplit_once('/') {
        Some((p, a)) => (format!("{p}/"), a.to_string()),
        None => (String::new(), rel.to_string()),
    };
    let (base, ext) = match arquivo.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (arquivo.clone(), String::new()),
    };
    for n in 2..10000 {
        let candidato = format!("{pasta}{base} ({n}){ext}");
        if !existe(&candidato) {
            return candidato;
        }
    }
    format!("{pasta}{base} (copia){ext}")
}

/// Grava o arquivo no projeto sem sobrescrever nada: nome repetido vira "nome (2).ext".
/// Devolve o caminho relativo gravado.
///
/// O arquivo e criado com `create_new` (falha se alguem criou o mesmo nome no meio tempo, e ai
/// tenta o proximo numero) e a pasta de destino precisa continuar dentro do projeto, mesmo com
/// link simbolico ou juncao no caminho.
fn colocar_arquivo(projeto: &Path, rel: &str, bytes: &[u8]) -> Result<String, String> {
    let mut candidato = nome_livre(&|n| local(projeto, n).exists(), rel);
    for _ in 0..100 {
        if candidato.split('/').any(|parte| crate::arquivos::validar_nome(parte).is_err()) || candidato.split('/').any(|p| p == "..") {
            return Err(format!("{candidato}: nome inválido"));
        }
        let destino = local(projeto, &candidato);
        let pai = destino.parent().unwrap_or(projeto).to_path_buf();
        crate::arquivos::garantir_dentro(projeto, &pai)?;
        fs::create_dir_all(&pai).map_err(|e| e.to_string())?;
        match fs::OpenOptions::new().write(true).create_new(true).open(&destino) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(bytes) {
                    drop(f);
                    let _ = fs::remove_file(&destino);
                    return Err(format!("{candidato}: {e}"));
                }
                return Ok(candidato);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let ocupado = candidato.clone();
                candidato = nome_livre(&|n| n == ocupado || local(projeto, n).exists(), rel);
            }
            Err(e) => return Err(format!("{candidato}: {e}")),
        }
    }
    Err(format!("{rel}: não foi possível achar um nome livre"))
}

/// Um arquivo do Drive ja baixado, com a subpasta do projeto em que deve entrar ("" = raiz).
struct Baixado {
    meta: MetaRemota,
    bytes: Vec<u8>,
    pasta: String,
}

impl Baixado {
    fn solto(meta: MetaRemota, bytes: Vec<u8>) -> Self {
        Baixado { meta, bytes, pasta: String::new() }
    }
}

/// Junta duas partes de caminho relativo ("" some): ("Tese", "cap") vira "Tese/cap".
fn juntar_rel(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}/{b}"),
    }
}

/// O que a pessoa marcou dentro de uma pasta do Drive: cada arquivo com a subpasta em que fica
/// (relativa a pasta escolhida; "" = a propria pasta) e as subpastas marcadas.
#[derive(Debug, Default, PartialEq)]
struct EstruturaDaPasta {
    arquivos: Vec<(drive_picker::ArquivoEscolhido, String)>,
    pastas: Vec<String>,
    /// Arquivos cujo pai nao e a pasta nem uma subpasta marcada: ficam na raiz da pasta.
    sem_pasta: Vec<String>,
}

/// Funcao pura. O Picker so devolve o id do pai de cada item; os nomes das subpastas so se
/// conhecem quando a pessoa marcou a subpasta (ai ela vem na lista, com nome e pai).
fn resolver_pastas(raiz_id: &str, itens: &[drive_picker::ArquivoEscolhido]) -> EstruturaDaPasta {
    let mut caminhos: BTreeMap<String, String> = BTreeMap::new();
    caminhos.insert(raiz_id.to_string(), String::new());
    let subpastas: Vec<&drive_picker::ArquivoEscolhido> = itens.iter().filter(|i| i.mime_type == MIME_PASTA).collect();
    // resolve de cima para baixo, quantas voltas forem precisas
    for _ in 0..=subpastas.len() {
        let mut andou = false;
        for s in &subpastas {
            if caminhos.contains_key(&s.id) {
                continue;
            }
            if let Some(pai) = s.parents.first().and_then(|p| caminhos.get(p)).cloned() {
                caminhos.insert(s.id.clone(), juntar_rel(&pai, &nome_seguro(&s.nome)));
                andou = true;
            }
        }
        if !andou {
            break;
        }
    }
    let mut est = EstruturaDaPasta::default();
    for s in &subpastas {
        // subpasta de pai desconhecido: entra como filha direta da pasta escolhida
        let rel = caminhos.entry(s.id.clone()).or_insert_with(|| nome_seguro(&s.nome)).clone();
        est.pastas.push(rel);
    }
    for i in itens.iter().filter(|i| i.mime_type != MIME_PASTA) {
        match i.parents.first().and_then(|p| caminhos.get(p)) {
            Some(dir) => est.arquivos.push((i.clone(), dir.clone())),
            None => {
                est.sem_pasta.push(i.nome.clone());
                est.arquivos.push((i.clone(), String::new()));
            }
        }
    }
    est
}

/// Tipos MIME que o Drive costuma dar a um .zip (o Windows e o Overleaf variam).
fn mime_de_zip(mime: &str) -> bool {
    matches!(mime, "application/zip" | "application/x-zip-compressed" | "application/x-zip" | "application/x-compressed" | "multipart/x-zip")
}

/// "PK" no comeco: assinatura de um zip (arquivo completo ou vazio).
fn bytes_sao_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

/// Este arquivo do Drive e um zip? Pela extensao, pelo tipo MIME ou, se o Drive deu um tipo
/// generico (octet-stream ou vazio), pelo conteudo.
fn e_zip_de_fato(nome: &str, mime: &str, bytes: &[u8]) -> bool {
    e_zip(nome) || mime_de_zip(mime) || ((mime == "application/octet-stream" || mime.is_empty()) && bytes_sao_zip(bytes))
}

/// Figura vinda do Drive: o que o LaTeX le direto, o que da para converter em png, ou recusada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatoFigura {
    Direto,
    ConverterParaPng,
    Recusado,
}

const MSG_FIGURA_RECUSADA: &str = "Use .png, .jpg, .pdf ou .eps. O .svg não entra no pdflatex: exporte como PDF ou PNG";

fn formato_da_figura(nome: &str) -> FormatoFigura {
    match nome.rsplit_once('.').map(|(_, e)| e.to_lowercase()).as_deref() {
        Some("png" | "jpg" | "jpeg" | "pdf" | "eps") => FormatoFigura::Direto,
        Some("gif" | "webp" | "bmp" | "tif" | "tiff") => FormatoFigura::ConverterParaPng,
        _ => FormatoFigura::Recusado,
    }
}

/// Converte gif, webp, bmp ou tiff em png (so o primeiro quadro, se for animado).
fn converter_para_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("não foi possível ler a imagem ({e})"))?;
    let mut saida = Cursor::new(Vec::new());
    img.write_to(&mut saida, image::ImageFormat::Png).map_err(|e| format!("não foi possível converter para png ({e})"))?;
    Ok(saida.into_inner())
}

/// "foto.gif" vira "foto.png".
fn nome_como_png(nome: &str) -> String {
    format!("{}.png", nome.rsplit_once('.').map(|(b, _)| b).filter(|b| !b.is_empty()).unwrap_or(nome))
}

fn extensao_aceita(nome: &str) -> bool {
    nome.rsplit_once('.').is_some_and(|(_, e)| drive_picker::EXTENSOES_ARQUIVOS.contains(&e.to_lowercase().as_str()))
}

fn e_zip(nome: &str) -> bool {
    nome.to_lowercase().ends_with(".zip")
}

/// O zip tem `.eguatex/drive.json`: veio de um projeto do ÉguaTex ligado ao Drive.
fn zip_e_projeto_eguatex(bytes: &[u8]) -> bool {
    zip::ZipArchive::new(Cursor::new(bytes))
        .map(|z| z.file_names().any(|n| n.replace('\\', "/").trim_start_matches("./") == ".eguatex/drive.json"))
        .unwrap_or(false)
}

/// Descompacta o zip dentro do projeto, com a mesma protecao contra caminhos perigosos da
/// sincronizacao. Nao sobrescreve: nome repetido vira "nome (2).ext". Devolve os caminhos gravados.
///
/// Tira o "embrulho" do zip (as pastas que so contem o projeto, como `modelos/Template_SBC/`):
/// o que sobra entra em `prefixo` ("" = raiz do projeto).
#[cfg(test)]
fn extrair_zip(projeto: &Path, bytes: &[u8], prefixo: &str) -> Result<Vec<String>, String> {
    extrair_zip_com_ignorados(projeto, bytes, prefixo).map(|(gravados, _)| gravados)
}

/// Como `extrair_zip`, e devolve tambem os nomes que ficaram de fora por seguranca (executavel
/// ou "-" no inicio), para o aviso.
fn extrair_zip_com_ignorados(projeto: &Path, bytes: &[u8], prefixo: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let (arquivos, preservados) = desempacotar_tudo(bytes)?;
    let ignorados = nomes_ignorados_por_seguranca(&preservados);
    let nomes: Vec<&str> = arquivos.keys().map(String::as_str).collect();
    let embrulho = crate::arquivos::prefixo_embrulho(&nomes);
    let mut gravados = Vec::new();
    for (rel, conteudo) in &arquivos {
        let sem = if embrulho.is_empty() { rel.as_str() } else { rel.strip_prefix(&format!("{embrulho}/")).unwrap_or(rel) };
        gravados.push(colocar_arquivo(projeto, &juntar_rel(prefixo, sem), conteudo)?);
    }
    Ok((gravados, ignorados))
}

/// "124.zip" vira "124": nome da subpasta do projeto aberto que recebe o conteudo do zip.
fn nome_da_subpasta_do_zip(nome_arquivo: &str) -> String {
    let sem = match nome_arquivo.len().checked_sub(4).filter(|&i| nome_arquivo.is_char_boundary(i)) {
        Some(i) if nome_arquivo[i..].eq_ignore_ascii_case(".zip") => &nome_arquivo[..i],
        _ => nome_arquivo,
    };
    let limpo = nome_seguro(sem);
    if limpo.is_empty() { "Arquivos do Drive".to_string() } else { limpo }
}

/// Quem mexeu por ultimo no arquivo do Drive (para o nome "(conflito de fulano)").
async fn ultimo_editor(id: &str) -> String {
    api(Method::GET, &format!("{API}/files/{id}?fields=lastModifyingUser(emailAddress,displayName)"), None)
        .await
        .ok()
        .and_then(|j| {
            j["lastModifyingUser"]["emailAddress"]
                .as_str()
                .or(j["lastModifyingUser"]["displayName"].as_str())
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// Troca o conteudo de um arquivo qualquer do Drive (PATCH no mesmo id), no tipo que ele ja tem.
async fn subir_conteudo_tipo(id: &str, bytes: Vec<u8>, mime: &str) -> Result<MetaRemota, String> {
    let mime = if mime.is_empty() { "application/octet-stream" } else { mime };
    let req = google::cliente()
        .patch(format!("{UPLOAD}/files/{id}?uploadType=media&fields={CAMPOS_META}"))
        .timeout(TEMPO_TRANSFERENCIA)
        .header("Content-Type", mime)
        .body(bytes);
    let j: Value = enviar(req).await?.json().await.map_err(|e| e.to_string())?;
    Ok(ler_meta(&j))
}

fn sem_prefixo_de_acesso(erro: &str) -> &str {
    erro.strip_prefix(ACESSO_PERDIDO).unwrap_or(erro)
}

/// Sincroniza os arquivos vinculados de um projeto que nao tem o zip do ÉguaTex. Chamar com a
/// trava `SINCRONIZANDO` em maos.
async fn sincronizar_vinculos(projeto: &Path, mut vinculos: Vinculos) -> Result<ResultadoSync, String> {
    let mut r = ResultadoSync::default();
    let nomes: Vec<String> = vinculos.keys().cloned().collect();
    for rel in nomes {
        let v = vinculos[&rel].clone();
        let bytes_local = fs::read(local(projeto, &rel)).ok();
        let sha_local = bytes_local.as_deref().map(sha);
        let mut m = match meta(&v.id).await {
            Ok(m) => m,
            Err(e) => {
                registrar_falha(&mut r, format!("{rel}: {}", sem_prefixo_de_acesso(&e)));
                continue;
            }
        };
        let mut acao = decidir_vinculo(&v, sha_local.as_deref(), &m.md5, &m.versao);
        if acao == AcaoVinculo::Enviar {
            // o PATCH nao tem precondicao: relê o arquivo logo antes e, se o Drive mudou nesse
            // meio tempo, trata como conflito (baixa como copia, nao sobrescreve)
            match meta(&v.id).await {
                Ok(agora) => {
                    let nova = acao_apos_releitura(acao, (&m.md5, &m.versao), (&agora.md5, &agora.versao));
                    if nova != acao {
                        acao = nova;
                        m = agora;
                    }
                }
                Err(e) => {
                    registrar_falha(&mut r, format!("{rel}: {}", sem_prefixo_de_acesso(&e)));
                    continue;
                }
            }
        }
        let atual = vinculos.get_mut(&rel).expect("vinculo existe");
        match acao {
            AcaoVinculo::Nada => {}
            // a copia ja foi avisada quando foi criada; nao repete o aviso a cada sincronizacao
            AcaoVinculo::EmConflito => {}
            AcaoVinculo::Enviar => {
                let bytes = bytes_local.expect("enviar so com arquivo local");
                match subir_conteudo_tipo(&v.id, bytes, &m.mime).await {
                    Ok(nova) => {
                        *atual = Vinculo { id: v.id.clone(), md5: nova.md5, versao: nova.versao, sha_local: sha_local.unwrap_or_default(), ..Default::default() };
                        r.enviados += 1;
                    }
                    Err(e) => registrar_falha(&mut r, format!("{rel}: não foi possível enviar ({})", sem_prefixo_de_acesso(&e))),
                }
            }
            AcaoVinculo::Baixar => match baixar(&v.id).await {
                Ok(bytes) => match gravar_local(projeto, &rel, &bytes) {
                    Ok(()) => {
                        *atual = Vinculo { id: v.id.clone(), md5: m.md5, versao: m.versao, sha_local: sha(&bytes), ..Default::default() };
                        r.baixados += 1;
                        r.alterados.push(rel.clone());
                    }
                    Err(e) => registrar_falha(&mut r, e),
                },
                Err(e) => registrar_falha(&mut r, format!("{rel}: {}", sem_prefixo_de_acesso(&e))),
            },
            AcaoVinculo::Conflito => match baixar(&v.id).await {
                Ok(bytes) => {
                    let quem = ultimo_editor(&v.id).await;
                    let copia = nome_conflito_com(&|n| local(projeto, n).exists(), &rel, &quem);
                    match gravar_local(projeto, &copia, &bytes) {
                        Ok(()) => {
                            atual.conflito_de = identidade_remota(&m.md5, &m.versao).to_string();
                            atual.sha_conflito = sha_local.unwrap_or_default();
                            r.copias_do_drive.push(copia.clone());
                            r.alterados.push(copia);
                        }
                        Err(e) => registrar_falha(&mut r, e),
                    }
                }
                Err(e) => registrar_falha(&mut r, format!("{rel}: {}", sem_prefixo_de_acesso(&e))),
            },
        }
    }
    gravar_vinculos(projeto, &vinculos)?;
    Ok(r)
}

#[derive(Debug, Default, Serialize)]
pub struct ResultadoAbrir {
    /// Pasta do projeto (a nova, ou a que ja estava aberta).
    pub caminho: String,
    pub novo: bool,
    /// Caminhos gravados no projeto, relativos.
    pub adicionados: Vec<String>,
    /// Quantos ficaram ligados ao Drive (voltam ao Drive quando mudam).
    pub vinculados: u32,
    /// Arquivos ignorados e outros avisos para a pessoa.
    pub avisos: Vec<String>,
    /// Abriu como projeto do ÉguaTex (zip .eguatex.zip), sincronizado pelo fluxo normal.
    pub projeto_do_drive: bool,
    /// `.tex` com `\documentclass` que a tela deve abrir e usar como principal (caminho relativo).
    /// Vem dos arquivos trazidos: num projeto aberto, so dos zips; num projeto novo, de qualquer `.tex`.
    pub principal_sugerido: Option<String>,
}

/// Pasta nova para o projeto dentro de `pasta_pai`, sem repetir nome.
fn pasta_nova(pasta_pai: &Path, nome: &str) -> Result<PathBuf, String> {
    let limpo = nome_seguro(nome);
    let base = if limpo.is_empty() { "Projeto do Drive".to_string() } else { limpo };
    fs::create_dir_all(pasta_pai).map_err(|e| e.to_string())?;
    (1..10000)
        .map(|n| if n == 1 { pasta_pai.join(&base) } else { pasta_pai.join(format!("{base} ({n})")) })
        .find(|p| !p.exists())
        .ok_or_else(|| "Não foi possível achar um nome livre para a pasta.".to_string())
}

/// Abre o seletor do Google com varios arquivos e traz os escolhidos para um projeto.
/// `destino` e "novo" (cria uma pasta em `pasta_pai`) ou o caminho do projeto aberto.
/// Documentos do Google (Docs, Planilhas) e tipos que o ÉguaTex nao usa ficam de fora, com aviso.
/// `None` se a pessoa cancelou o seletor.
#[tauri::command]
pub async fn cmd_drive_abrir_arquivos(
    app: tauri::AppHandle,
    destino: String,
    pasta_pai: String,
    nome_projeto: Option<String>,
) -> Result<Option<ResultadoAbrir>, String> {
    let existente = if destino == "novo" { None } else { Some(pasta_projeto(&destino)?) };
    // Android: o app fica acordado e com rede ate o fim do comando, nao so durante o seletor
    let _acordado = google::manter_acordado::Guarda::nova();
    let Some(escolhidos) = drive_picker::escolher_arquivos(&app).await? else { return Ok(None) };
    crate::registro::drive("abrir", &format!("destino={destino} itens={}", escolhidos.len()));

    // Pasta: o Picker so libera a propria pasta, nao o que tem dentro. O seletor abre de novo
    // dentro dela para a pessoa marcar os arquivos; as subpastas marcadas viram subpastas aqui.
    let (pastas, avulsos): (Vec<_>, Vec<_>) = escolhidos.into_iter().partition(|e| e.mime_type == MIME_PASTA);
    let mut dentro_das_pastas = Vec::new();
    for p in pastas {
        if let Some(itens) = drive_picker::escolher_dentro_da_pasta(&app, &p.id).await? {
            dentro_das_pastas.push((p, itens));
        }
    }
    let r = abrir_selecao(existente, &pasta_pai, nome_projeto, avulsos, dentro_das_pastas).await;
    match &r {
        Ok(Some(x)) => crate::registro::drive("abrir", &format!("ok: {} (novo={}, {} arquivo(s), avisos: {})", x.caminho, x.novo, x.adicionados.len(), x.avisos.join(" | "))),
        Ok(None) => crate::registro::drive("abrir", "nada a abrir (lista vazia)"),
        Err(e) => crate::registro::drive("abrir", &format!("erro: {e}")),
    }
    r
}

/// O que vem depois do seletor: baixa o que foi escolhido (arquivos avulsos, mais o que foi marcado
/// dentro de cada pasta) e grava no projeto. Sem Picker nem janela: da para testar com o Drive real.
async fn abrir_selecao(
    existente: Option<PathBuf>,
    pasta_pai: &str,
    nome_projeto: Option<String>,
    avulsos: Vec<drive_picker::ArquivoEscolhido>,
    pastas: Vec<(drive_picker::ArquivoEscolhido, Vec<drive_picker::ArquivoEscolhido>)>,
) -> Result<Option<ResultadoAbrir>, String> {
    let mut r = ResultadoAbrir::default();
    let so_uma_pasta = pastas.len() == 1 && avulsos.is_empty();
    let mut itens: Vec<(drive_picker::ArquivoEscolhido, String)> = avulsos.into_iter().map(|a| (a, String::new())).collect();
    let mut pastas_a_criar: Vec<String> = Vec::new();
    let mut nome_da_pasta_unica = String::new();
    for (p, dentro) in &pastas {
        let est = resolver_pastas(&p.id, dentro);
        // uma pasta so, num projeto novo: o projeto leva o nome dela e os arquivos vao para a raiz
        let base = if so_uma_pasta && existente.is_none() { String::new() } else { nome_seguro(&p.nome) };
        if so_uma_pasta {
            nome_da_pasta_unica = p.nome.clone();
        }
        if !base.is_empty() {
            pastas_a_criar.push(base.clone());
        }
        pastas_a_criar.extend(est.pastas.iter().map(|s| juntar_rel(&base, s)));
        if !est.sem_pasta.is_empty() {
            r.avisos.push(format!("{}: arquivos de subpastas que você não marcou ficaram na pasta principal: {}", p.nome, est.sem_pasta.join(", ")));
        }
        itens.extend(est.arquivos.into_iter().map(|(a, sub)| (a, juntar_rel(&base, &sub))));
    }
    if itens.is_empty() && pastas_a_criar.is_empty() {
        return Ok(None);
    }

    // baixa tudo antes de criar a pasta nova: se nada servir, nao sobra pasta vazia
    let mut baixados: Vec<Baixado> = Vec::new();
    for (e, pasta) in &itens {
        let m = match meta(&e.id).await {
            Ok(m) => m,
            Err(err) => {
                crate::registro::drive("baixar", &format!("meta {} falhou: {err}", e.nome));
                r.avisos.push(format!("{}: {}", e.nome, sem_prefixo_de_acesso(&err)));
                continue;
            }
        };
        if m.mime.starts_with("application/vnd.google-apps.") {
            r.avisos.push(format!("{}: documentos e planilhas do Google não entram. Use \"Importar do Google Documentos\" no menu Arquivo.", m.nome));
            continue;
        }
        match baixar(&m.id).await {
            Ok(b) => {
                crate::registro::drive("baixar", &format!("{} ({}): {} bytes", m.nome, m.mime, b.len()));
                baixados.push(Baixado { meta: m, bytes: b, pasta: pasta.clone() });
            }
            Err(err) => {
                crate::registro::drive("baixar", &format!("{} falhou: {err}", m.nome));
                r.avisos.push(format!("{}: {}", m.nome, sem_prefixo_de_acesso(&err)))
            }
        }
    }
    if baixados.is_empty() && pastas_a_criar.is_empty() {
        return Err(if r.avisos.is_empty() { "Nenhum arquivo foi escolhido.".into() } else { r.avisos.join(" ") });
    }

    // um .eguatex.zip (ou zip com .eguatex/drive.json) escolhido numa pasta nova vira o projeto
    let projeto_zip = if existente.is_none() {
        baixados.iter().position(|b| b.pasta.is_empty() && (b.meta.nome.to_lowercase().ends_with(SUFIXO_PROJETO) || (e_zip_de_fato(&b.meta.nome, &b.meta.mime, &b.bytes) && zip_e_projeto_eguatex(&b.bytes))))
    } else {
        None
    };

    let (dir, novo) = match &existente {
        Some(p) => (p.clone(), false),
        None => {
            let nome = match (&nome_projeto, projeto_zip) {
                (Some(n), _) if !n.trim().is_empty() => n.trim().to_string(),
                (_, Some(i)) => nome_local_do_zip(&baixados[i].meta.nome),
                _ if !nome_da_pasta_unica.is_empty() => nome_da_pasta_unica.clone(),
                _ if baixados.len() == 1 => nome_do_projeto_aberto(&baixados[0].meta.nome),
                _ => "Projeto do Drive".to_string(),
            };
            (pasta_nova(Path::new(pasta_pai), &nome)?, true)
        }
    };
    if novo {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    r.caminho = dir.display().to_string();
    r.novo = novo;
    // as pastas que a pessoa marcou existem no projeto mesmo vazias
    for rel in &pastas_a_criar {
        let alvo = local(&dir, rel);
        if crate::arquivos::garantir_dentro(&dir, &alvo).is_ok() {
            let _ = fs::create_dir_all(alvo);
        }
    }

    let resultado = trazer_para_o_projeto(&dir, baixados, projeto_zip, &mut r).await;
    if let Err(e) = resultado {
        if novo {
            let _ = fs::remove_dir_all(&dir);
        }
        return Err(e);
    }
    Ok(Some(r))
}

async fn trazer_para_o_projeto(
    dir: &Path,
    baixados: Vec<Baixado>,
    projeto_zip: Option<usize>,
    r: &mut ResultadoAbrir,
) -> Result<(), String> {
    // com a trava: uma sincronizacao em andamento nao pode regravar os vinculos por cima
    let _vez = SINCRONIZANDO.lock().await;
    let mut vinculos = ler_vinculos(dir);
    let tem_zip_do_projeto = ler_estado(dir).is_some_and(|e| e.id_remoto().is_some());
    // candidatos a principal: (caminho no projeto, conteudo)
    let mut candidatos: Vec<(String, Vec<u8>)> = Vec::new();
    for (i, b) in baixados.iter().enumerate() {
        let (m, bytes) = (&b.meta, &b.bytes);
        if Some(i) == projeto_zip {
            continue;
        }
        if e_zip_de_fato(&m.nome, &m.mime, bytes) {
            if m.nome.to_lowercase().ends_with(SUFIXO_PROJETO) || zip_e_projeto_eguatex(bytes) {
                r.avisos.push(format!("{}: os arquivos foram copiados, mas sem ligação com o projeto compartilhado. Para editar em conjunto, abra-o pela tela inicial.", m.nome));
            }
            // projeto aberto: o zip entra na subpasta com o nome dele; projeto novo: direto na raiz
            let destino = if !b.pasta.is_empty() || r.novo { b.pasta.clone() } else { nome_da_subpasta_do_zip(&m.nome) };
            match extrair_zip_com_ignorados(dir, bytes, &destino) {
                Ok((lista, ignorados)) => {
                    if !ignorados.is_empty() {
                        r.avisos.push(format!("{}: {}", m.nome, crate::limites_zip::aviso_de_ignorados(&ignorados)));
                    }
                    for rel in lista.iter().filter(|x| x.to_lowercase().ends_with(".tex")) {
                        if let Ok(conteudo) = fs::read(local(dir, rel)) {
                            candidatos.push((rel.clone(), conteudo));
                        }
                    }
                    r.adicionados.extend(lista)
                }
                Err(e) => r.avisos.push(format!("{}: {e}", m.nome)),
            }
            continue;
        }
        let nome = nome_seguro(&m.nome);
        if crate::arquivos::validar_nome(&nome).is_err() {
            r.avisos.push(format!("{}: nome que o Windows não aceita.", m.nome));
            continue;
        }
        let rel = colocar_arquivo(dir, &juntar_rel(&b.pasta, &nome), bytes)?;
        // o mesmo arquivo do Drive trazido de novo (copia "(2)") nao e ligado duas vezes
        let ja_ligado = vinculos.values().any(|x| x.id == m.id);
        if !tem_zip_do_projeto && projeto_zip.is_none() && !ja_ligado {
            vinculos.insert(rel.clone(), Vinculo { id: m.id.clone(), md5: m.md5.clone(), versao: m.versao.clone(), sha_local: sha(bytes), ..Default::default() });
            r.vinculados += 1;
        }
        if r.novo && rel.to_lowercase().ends_with(".tex") {
            candidatos.push((rel.clone(), bytes.clone()));
        }
        r.adicionados.push(rel);
    }
    if r.vinculados > 0 {
        gravar_vinculos(dir, &vinculos)?;
    }
    if projeto_zip.is_none() {
        let nome_pasta = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        r.principal_sugerido = crate::arquivos::principal_entre(&candidatos, &nome_pasta);
    }
    if let Some(i) = projeto_zip {
        // o projeto do ÉguaTex entra pelo fluxo normal: o estado aponta para o zip e a primeira
        // sincronizacao baixa tudo (e leva junto os arquivos soltos que ja estao na pasta)
        gravar_estado(dir, &Estado { arquivo_id: baixados[i].meta.id.clone(), ..Default::default() })?;
        let res = sincronizar_travado(dir, ler_estado(dir).unwrap_or_default()).await?;
        r.projeto_do_drive = true;
        r.adicionados.extend(res.alterados);
        r.avisos.extend(res.avisos);
    }
    Ok(())
}

/// Caminho relativo de uma figura vinda do Drive: sempre dentro de `figuras/`.
fn caminho_da_figura(nome: &str) -> String {
    format!("{}/{}", crate::arquivos::PASTA_FIGURAS, nome_seguro(nome))
}

/// Grava a figura baixada em `figuras/` (nome repetido vira "nome (2).ext") e, se `ligar` e o
/// projeto nao usa o zip do ÉguaTex, liga o arquivo ao do Drive como os outros arquivos abertos
/// de la. Figura convertida (gif, webp...) nao se liga: o arquivo do Drive tem outro formato.
async fn trazer_figura(projeto: &Path, m: &MetaRemota, bytes: &[u8], ligar: bool) -> Result<String, String> {
    // com a trava: uma sincronizacao em andamento nao pode regravar os vinculos por cima
    let _vez = SINCRONIZANDO.lock().await;
    let rel = colocar_arquivo(projeto, &caminho_da_figura(&m.nome), bytes)?;
    let tem_zip_do_projeto = ler_estado(projeto).is_some_and(|e| e.id_remoto().is_some());
    let mut vinculos = ler_vinculos(projeto);
    if ligar && !tem_zip_do_projeto && !vinculos.values().any(|x| x.id == m.id) {
        vinculos.insert(rel.clone(), Vinculo { id: m.id.clone(), md5: m.md5.clone(), versao: m.versao.clone(), sha_local: sha(bytes), ..Default::default() });
        gravar_vinculos(projeto, &vinculos)?;
    }
    Ok(rel)
}

/// Baixa a figura `id` do Drive para `figuras/` do projeto e devolve o caminho relativo.
/// png, jpg, jpeg, pdf e eps entram como estao; gif, webp, bmp e tiff viram png; o resto (inclusive
/// svg, que o pdflatex nao inclui) e recusado. Documentos do Google nao sao figuras.
async fn figura_do_drive(projeto: &Path, id: &str) -> Result<String, String> {
    let mut m = meta(id).await?;
    if m.mime.starts_with("application/vnd.google-apps.") {
        return Err(format!("{}: documentos e planilhas do Google não são figuras. {MSG_FIGURA_RECUSADA}.", m.nome));
    }
    let formato = formato_da_figura(&m.nome);
    if formato == FormatoFigura::Recusado {
        return Err(format!("{}: {MSG_FIGURA_RECUSADA}.", m.nome));
    }
    if crate::arquivos::validar_nome(&nome_seguro(&m.nome)).is_err() {
        return Err(format!("{}: nome que o Windows não aceita.", m.nome));
    }
    let mut bytes = baixar(&m.id).await?;
    let mut ligar = true;
    if formato == FormatoFigura::ConverterParaPng {
        bytes = converter_para_png(&bytes).map_err(|e| format!("{}: {e}. {MSG_FIGURA_RECUSADA}.", m.nome))?;
        m.nome = nome_como_png(&m.nome);
        ligar = false;
    }
    trazer_figura(projeto, &m, &bytes, ligar).await
}

/// Inserir > Figura > Do Google Drive: abre o seletor do Google (uma figura so), baixa para
/// `figuras/` do projeto e devolve o caminho relativo, pronto para o \includegraphics.
/// `None` se a pessoa cancelou.
#[tauri::command]
pub async fn cmd_drive_abrir_figura(app: tauri::AppHandle, caminho: String) -> Result<Option<String>, String> {
    let projeto = pasta_projeto(&caminho)?;
    // Android: o app fica acordado e com rede ate o fim do comando, nao so durante o seletor
    let _acordado = google::manter_acordado::Guarda::nova();
    let Some(e) = drive_picker::escolher_imagem(&app).await? else { return Ok(None) };
    figura_do_drive(&projeto, &e.id).await.map(Some)
}

/// O que fazer com o que a pessoa escolheu no seletor do modo projeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscolhaProjeto {
    /// `.eguatex.zip` (ou pasta do formato antigo): projeto compartilhado e sincronizado.
    ProjetoCompartilhado,
    /// Qualquer outro `.zip` (ex.: exportado do Overleaf): descompacta num projeto novo.
    ZipComum,
    /// Arquivo que nao e zip (.tex, .bib, figura...): abre como arquivo, num projeto novo.
    ArquivoSolto,
    /// Tipo generico (octet-stream) e nome sem extensao conhecida: so o conteudo diz se e zip.
    TalvezZip,
    /// Documento do Google ou tipo que o ÉguaTex nao usa.
    Inaceitavel,
}

fn decidir_escolha_projeto(nome: &str, mime: &str) -> EscolhaProjeto {
    if nome.to_lowercase().ends_with(SUFIXO_PROJETO) || mime == MIME_PASTA {
        EscolhaProjeto::ProjetoCompartilhado
    } else if mime.starts_with("application/vnd.google-apps.") {
        EscolhaProjeto::Inaceitavel
    } else if e_zip(nome) || mime_de_zip(mime) {
        EscolhaProjeto::ZipComum
    } else if extensao_aceita(nome) {
        EscolhaProjeto::ArquivoSolto
    } else if mime == "application/octet-stream" || mime.is_empty() {
        // tipo generico e nome sem extensao conhecida: so o conteudo diz se e um zip
        EscolhaProjeto::TalvezZip
    } else {
        EscolhaProjeto::Inaceitavel
    }
}

/// "124.zip" vira "124"; "main.tex" vira "main". Nome da pasta do projeto novo.
fn nome_do_projeto_aberto(nome_arquivo: &str) -> String {
    let sem = nome_arquivo.rsplit_once('.').map(|(b, _)| b).filter(|b| !b.is_empty()).unwrap_or(nome_arquivo);
    let limpo = nome_seguro(sem);
    if limpo.is_empty() { "Projeto do Drive".to_string() } else { limpo }
}

/// Zip comum ou arquivo solto escolhido no seletor do modo projeto: vira uma pasta de projeto
/// nova em `pasta_pai`, pelo mesmo caminho do "Abrir do Google Drive" (zip descompactado com a
/// protecao contra caminhos perigosos; arquivo solto ligado ao do Drive em drive-vinculos.json).
/// Devolve o caminho da pasta nova.
async fn abrir_como_arquivos(pasta_pai: &Path, m: MetaRemota, bytes: Vec<u8>) -> Result<String, String> {
    let dir = pasta_nova(pasta_pai, &nome_do_projeto_aberto(&m.nome))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut r = ResultadoAbrir { novo: true, ..Default::default() };
    let nome = m.nome.clone();
    let res = trazer_para_o_projeto(&dir, vec![Baixado::solto(m, bytes)], None, &mut r).await;
    if let Err(e) = res {
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }
    if r.adicionados.is_empty() {
        let _ = fs::remove_dir_all(&dir);
        let motivo = if r.avisos.is_empty() { "não tem arquivos que o ÉguaTex use".to_string() } else { r.avisos.join(" ") };
        return Err(format!("{nome}: {motivo}"));
    }
    Ok(dir.display().to_string())
}

/// Deixa de sincronizar este projeto (o arquivo no Drive continua la).
#[tauri::command]
pub fn cmd_drive_desligar(caminho: String) -> Result<(), String> {
    for p in [caminho_estado(Path::new(&caminho)), caminho_vinculos(Path::new(&caminho))] {
        if p.exists() {
            fs::remove_file(p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}


#[cfg(test)]
mod testes {
    use super::*;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eguatex-{rotulo}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn c(itens: &[(&str, &str)]) -> Conteudo {
        itens.iter().map(|(k, v)| (k.to_string(), v.as_bytes().to_vec())).collect()
    }

    fn b(itens: &[(&str, &str)]) -> BTreeMap<String, String> {
        itens.iter().map(|(k, v)| (k.to_string(), sha(v.as_bytes()))).collect()
    }

    fn sem_travados() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn id_de_links_do_drive() {
        let id = "1AbC-dEf_GhIjKlMnOp";
        assert_eq!(id_do_link(&format!("https://drive.google.com/drive/folders/{id}?usp=sharing")).as_deref(), Some(id));
        assert_eq!(id_do_link(&format!("https://drive.google.com/file/d/{id}/view?usp=sharing")).as_deref(), Some(id));
        assert_eq!(id_do_link(&format!("https://drive.google.com/open?id={id}&x=1")).as_deref(), Some(id));
        assert_eq!(id_do_link(id).as_deref(), Some(id));
        assert!(id_do_link("https://exemplo.com/nada aqui").is_none());
    }

    #[test]
    fn valida_emails() {
        assert!(email_valido("nome@exemplo.com"));
        assert!(!email_valido("nome"));
        assert!(!email_valido("nome@exemplo"));
        assert!(!email_valido("a na@ufpa.br"));
    }

    #[test]
    fn nome_da_copia_de_conflito() {
        let dir = dir_temp("conflito");
        fs::create_dir_all(dir.join("cap")).unwrap();
        assert_eq!(nome_conflito(&dir, "cap/intro.tex", "nome@exemplo.com"), "cap/intro (conflito de nome).tex");
        fs::write(dir.join("cap/intro (conflito de nome).tex"), "x").unwrap();
        assert_eq!(nome_conflito(&dir, "cap/intro.tex", "nome@exemplo.com"), "cap/intro (conflito de nome 2).tex");
        assert_eq!(nome_conflito(&dir, "Makefile", "outro@exemplo.com"), "Makefile (conflito de outro)");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn arquivos_locais_pula_auxiliares_e_ocultos() {
        let dir = dir_temp("locais");
        fs::create_dir_all(dir.join(".eguatex")).unwrap();
        fs::create_dir_all(dir.join("figs")).unwrap();
        for f in ["main.tex", "main.pdf", "main.aux", "figs/a.png", ".eguatex/drive.json"] {
            fs::write(dir.join(f), "x").unwrap();
        }
        let l = arquivos_locais(&dir).unwrap();
        assert_eq!(l, BTreeSet::from(["figs/a.png".to_string(), "main.tex".to_string()]));
        let _ = fs::remove_dir_all(&dir);
    }

    // ---------- zip ----------

    #[test]
    fn empacota_e_desempacota_o_projeto() {
        let origem = c(&[("main.tex", "\\documentclass{article}"), ("cap/intro.tex", "Olá, ção"), ("figs/a.bin", "\u{0}\u{1}")]);
        let zip = empacotar(&origem).unwrap();
        assert_eq!(desempacotar(&zip).unwrap(), origem);
        // sem data nos arquivos: o mesmo conteudo gera os mesmos bytes
        assert_eq!(empacotar(&origem).unwrap(), zip);
        assert_eq!(desempacotar(&empacotar(&Conteudo::new()).unwrap()).unwrap(), Conteudo::new());
    }

    #[test]
    fn desempacotar_pula_caminhos_perigosos_e_ocultos() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for nome in ["ok.tex", "../fora.tex", "/abs.tex", ".eguatex/drive.json", ".git/config", "__MACOSX/x", "a/CON", "pasta/b.tex", ".latexmkrc"] {
            zip.start_file(nome, o).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.add_directory("vazia/", o).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let achados: Vec<String> = desempacotar(&bytes).unwrap().into_keys().collect();
        assert_eq!(achados, vec![".latexmkrc", "ok.tex", "pasta/b.tex"]);
        assert!(desempacotar(b"isto nao e um zip").is_err());
    }

    #[test]
    fn conversao_da_pasta_antiga_nunca_apaga_arquivo_local() {
        // colab.tex foi criado por um colaborador na pasta, ja estava no disco do dono (base) e
        // o dono nao o enxerga mais no Drive (drive.file)
        let antigo = r#"{"pasta_id":"PASTA123456","arquivos":{"colab.tex":{"sha_local":"@S@"}}}"#.replace("@S@", &sha(b"dele"));
        let e: Estado = serde_json::from_str(&antigo).unwrap();
        assert!(e.e_formato_antigo());
        let locais = c(&[("colab.tex", "dele"), ("meu.tex", "meu")]);
        let remotos = c(&[("meu.tex", "meu")]);
        // com a base antiga o arquivo seria tratado como "apagado la" e removido do disco
        let errada = mesclar(&e.sha_base(), &locais, &remotos, &sem_travados(), "dono");
        assert_eq!(errada.apagar, vec!["colab.tex"]);
        // a base da rodada e vazia na conversao: nada local e apagado e tudo entra no zip
        let m = mesclar(&base_da_rodada(&e), &locais, &remotos, &sem_travados(), "dono");
        assert!(m.apagar.is_empty() && m.conflitos.is_empty());
        assert_eq!(m.final_, locais);
        // fora da conversao a base continua valendo
        let novo = Estado { arquivo_id: "ZIP1234567".into(), arquivos: e.arquivos.clone(), ..Default::default() };
        assert_eq!(base_da_rodada(&novo).len(), 1);
    }

    #[test]
    fn trocar_o_arquivo_do_drive_limpa_a_base() {
        let mut e = Estado {
            arquivo_id: "ANTIGO12345".into(),
            md5_remoto: "m".into(),
            versao: "v".into(),
            arquivos: BTreeMap::from([("a.tex".to_string(), Registro { sha_local: sha(b"a"), ..Default::default() })]),
            ..Default::default()
        };
        let locais = c(&[("a.tex", "a")]);
        // o novo zip nao tem a.tex: com a base antiga, a.tex seria apagado do disco
        assert_eq!(mesclar(&e.sha_base(), &locais, &c(&[("b.tex", "b")]), &sem_travados(), "x").apagar, vec!["a.tex"]);
        trocar_arquivo(&mut e, "NOVO1234567".into());
        assert_eq!((e.arquivo_id.as_str(), e.md5_remoto.as_str(), e.versao.as_str()), ("NOVO1234567", "", ""));
        assert!(e.arquivos.is_empty());
        let m = mesclar(&base_da_rodada(&e), &locais, &c(&[("b.tex", "b")]), &sem_travados(), "x");
        assert!(m.apagar.is_empty());
        assert_eq!(m.final_, c(&[("a.tex", "a"), ("b.tex", "b")]));
    }

    #[test]
    fn permissoes_copiadas_da_pasta_antiga() {
        let j = json!({"permissions": [
            {"id": "1", "type": "user", "role": "owner", "emailAddress": "dono@x.com"},
            {"id": "2", "type": "user", "role": "writer", "emailAddress": "nome@exemplo.com"},
            {"id": "3", "type": "group", "role": "reader", "emailAddress": "grupo@x.com"},
            {"id": "4", "type": "anyone", "role": "writer"},
            {"id": "5", "type": "domain", "role": "reader"},
            {"id": "6", "type": "user", "role": "writer"}
        ]});
        let p = permissoes_a_copiar(&j);
        assert_eq!(
            p,
            vec![
                json!({"type": "user", "role": "writer", "emailAddress": "nome@exemplo.com"}),
                json!({"type": "group", "role": "reader", "emailAddress": "grupo@x.com"}),
                json!({"type": "anyone", "role": "writer"}),
            ]
        );
        assert!(permissoes_a_copiar(&json!({})).is_empty());
    }

    #[test]
    fn bloqueado_que_ja_esta_no_disco_nao_e_apagado_nem_alterado() {
        for bloq in ["a.js", "x.lnk", "r.bat", "-nota.tex"] {
            let mut locais = c(&[("main.tex", "M"), (bloq, "B")]);
            let mut base: BTreeMap<String, String> = locais.iter().map(|(k, v)| (k.clone(), sha(v))).collect();
            // o zip que o Drive guarda e o que esta rodada baixa
            let (remotos, mut preservados) = desempacotar_tudo(&empacotar(&locais).unwrap()).unwrap();
            assert!(preservados.contains_key(bloq), "{bloq} deveria voltar em preservados");
            let originais = locais.clone();
            let bloqueados = separar_bloqueados_locais(&mut locais, &mut base);
            assert_eq!(bloqueados.keys().collect::<Vec<_>>(), vec![bloq]);
            let m = mesclar(&base, &locais, &remotos, &BTreeSet::new(), "x");
            assert!(m.apagar.is_empty(), "{bloq}: {:?}", m.apagar);
            assert!(m.escrever.is_empty(), "{bloq}: {:?}", m.escrever);
            assert!(!m.zip_mudou(&remotos));
            for rel in bloqueados.keys() {
                preservados.remove(rel);
            }
            assert!(preservados.is_empty());
            // o zip reenviado continua com o arquivo, igual ao do disco
            let zip = conteudo_do_zip(&m.final_, &preservados, &bloqueados);
            assert_eq!(zip, originais, "{bloq}");
            assert!(nomes_ignorados_por_seguranca(&preservados).is_empty());
        }
    }

    #[test]
    fn bloqueado_so_no_drive_nao_e_gravado_e_aparece_no_aviso() {
        let mut locais = c(&[("main.tex", "M")]);
        let mut base: BTreeMap<String, String> = locais.iter().map(|(k, v)| (k.clone(), sha(v))).collect();
        let remoto = c(&[("main.tex", "M"), ("a.js", "B")]);
        let (remotos, preservados) = desempacotar_tudo(&empacotar(&remoto).unwrap()).unwrap();
        let bloqueados = separar_bloqueados_locais(&mut locais, &mut base);
        assert!(bloqueados.is_empty());
        let m = mesclar(&base, &locais, &remotos, &BTreeSet::new(), "x");
        assert!(!m.final_.contains_key("a.js") && m.escrever.is_empty() && m.apagar.is_empty());
        assert_eq!(nomes_ignorados_por_seguranca(&preservados), vec!["a.js".to_string()]);
        assert_eq!(conteudo_do_zip(&m.final_, &preservados, &bloqueados), remoto);
    }

    #[test]
    fn bloqueado_editado_no_disco_sobe_no_zip() {
        let mut locais = c(&[("main.tex", "M"), ("a.js", "novo")]);
        let mut base = BTreeMap::new();
        let antigo = c(&[("main.tex", "M"), ("a.js", "velho")]);
        let (remotos, preservados) = desempacotar_tudo(&empacotar(&antigo).unwrap()).unwrap();
        let bloqueados = separar_bloqueados_locais(&mut locais, &mut base);
        assert!(bloqueados.iter().any(|(rel, b)| preservados.get(rel) != Some(b)));
        let m = mesclar(&base, &locais, &remotos, &BTreeSet::new(), "x");
        assert_eq!(conteudo_do_zip(&m.final_, &preservados, &bloqueados)["a.js"], b"novo");
    }

    #[test]
    fn zip_drive_bloqueia_lnk_exe() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        let nomes = [
            "main.tex", "pasta/ok.txt", "atalho.LNK", "run.exe", "lote.Bat ", "script.ps1.", "app.msi", "roda.vbs",
            "web.js", "site.url", "pasta.scf", "tela.scr", "cmd.cmd", "sub/leia.txt:escondido.exe", "-filtro.tex",
        ];
        for nome in nomes {
            zip.start_file(nome, o).unwrap();
            zip.write_all(b"x").unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let (arquivos, preservados) = desempacotar_tudo(&bytes).unwrap();
        // nada executavel vai para o disco: so main.tex e pasta/ok.txt
        assert_eq!(arquivos.keys().collect::<Vec<_>>(), vec!["main.tex", "pasta/ok.txt"]);
        // mas continua no zip reenviado, para nao sumir do Drive de quem enviou
        assert_eq!(preservados.len(), nomes.len() - 2, "{:?}", preservados.keys().collect::<Vec<_>>());
        let ignorados = nomes_ignorados_por_seguranca(&preservados);
        assert_eq!(ignorados.len(), nomes.len() - 2, "{ignorados:?}");
        let (a2, p2) = desempacotar_tudo(&{
            let mut no_zip = arquivos.clone();
            no_zip.extend(preservados.clone());
            empacotar(&no_zip).unwrap()
        })
        .unwrap();
        assert_eq!((a2, p2), (arquivos, preservados));
    }

    #[test]
    fn zip_com_5001_entradas_e_recusado() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for i in 0..5001 {
            zip.start_file(format!("a{i}.tex"), o).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let e = desempacotar_tudo(&bytes).unwrap_err();
        assert!(e.contains("5001"), "{e}");
    }

    #[test]
    fn zip_de_razao_alta_e_recusado() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("zeros.tex", o).unwrap();
        zip.write_all(&vec![0u8; 20 << 20]).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let e = desempacotar_tudo(&bytes).unwrap_err();
        assert!(e.contains("bomba de descompressão"), "{e}");
    }

    #[test]
    fn extrair_zip_com_exe_devolve_o_nome_ignorado_no_aviso() {
        let dir = dir_temp("zip-exe-aviso");
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for n in ["main.tex", "run.exe"] {
            zip.start_file(n, o).unwrap();
            zip.write_all(b"x").unwrap();
        }
        let (gravados, ignorados) = extrair_zip_com_ignorados(&dir, &zip.finish().unwrap().into_inner(), "").unwrap();
        assert_eq!(gravados, vec!["main.tex"]);
        assert!(crate::limites_zip::aviso_de_ignorados(&ignorados).contains("run.exe"));
        assert!(!dir.join("run.exe").exists());
    }

    #[test]
    fn nome_invalido_no_windows_continua_no_zip_reenviado() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (nome, corpo) in [("ok.tex", "A"), ("a:b.tex", "B"), ("pasta/CON", "C")] {
            zip.start_file(nome, o).unwrap();
            zip.write_all(corpo.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let (arquivos, preservados) = desempacotar_tudo(&bytes).unwrap();
        assert_eq!(arquivos.keys().collect::<Vec<_>>(), vec!["ok.tex"]);
        assert_eq!(preservados.keys().collect::<Vec<_>>(), vec!["a:b.tex", "pasta/CON"]);
        // o reenvio (arquivos mesclados + preservados) nao perde a entrada
        let mut no_zip = arquivos.clone();
        no_zip.extend(preservados.clone());
        let (a2, p2) = desempacotar_tudo(&empacotar(&no_zip).unwrap()).unwrap();
        assert_eq!((a2, p2), (arquivos, preservados));
    }

    #[test]
    fn nomes_do_zip_no_drive() {
        assert_eq!(nome_do_zip("Tese"), "Tese.eguatex.zip");
        assert_eq!(nome_do_zip("a/b:c"), "a-b-c.eguatex.zip");
        assert_eq!(nome_do_zip("  "), "Projeto.eguatex.zip");
        assert_eq!(nome_local_do_zip("Tese final.eguatex.zip"), "Tese final");
        assert_eq!(nome_local_do_zip("Tese.EGUATEX.ZIP"), "Tese");
        assert_eq!(nome_local_do_zip("notas"), "notas");
        assert_eq!(nome_local_do_zip("ção"), "ção");
    }

    // ---------- mescla ----------

    #[test]
    fn mescla_arquivo_novo_de_cada_lado() {
        let m = mesclar(&b(&[]), &c(&[("a.tex", "A")]), &c(&[("b.tex", "B")]), &sem_travados(), "nome@exemplo.com");
        assert_eq!(m.final_, c(&[("a.tex", "A"), ("b.tex", "B")]));
        assert_eq!(m.escrever, vec!["b.tex"]);
        assert_eq!((m.enviados, m.baixados, m.apagados), (1, 1, 0));
        assert!(m.apagar.is_empty() && m.conflitos.is_empty());
        assert!(m.zip_mudou(&c(&[("b.tex", "B")])));
    }

    #[test]
    fn mescla_quem_mexeu_sozinho_vence() {
        let base = b(&[("x.tex", "v1"), ("y.tex", "v1")]);
        let locais = c(&[("x.tex", "v2"), ("y.tex", "v1")]);
        let remotos = c(&[("x.tex", "v1"), ("y.tex", "v3")]);
        let m = mesclar(&base, &locais, &remotos, &sem_travados(), "nome");
        assert_eq!(m.final_, c(&[("x.tex", "v2"), ("y.tex", "v3")]));
        assert_eq!(m.escrever, vec!["y.tex"]);
        assert_eq!((m.enviados, m.baixados), (1, 1));
        assert!(m.conflitos.is_empty());
    }

    #[test]
    fn mescla_sem_mudanca_nao_reenvia_o_zip() {
        let base = b(&[("x.tex", "v1")]);
        let m = mesclar(&base, &c(&[("x.tex", "v1")]), &c(&[("x.tex", "v1")]), &sem_travados(), "nome");
        assert!(!m.zip_mudou(&c(&[("x.tex", "v1")])));
        assert_eq!((m.enviados, m.baixados, m.apagados), (0, 0, 0));
        assert!(m.escrever.is_empty() && m.apagar.is_empty());
    }

    #[test]
    fn mescla_com_conflito_deixa_o_drive_no_lugar_e_guarda_a_copia_local() {
        let base = b(&[("main.tex", "v1")]);
        let locais = c(&[("main.tex", "minha")]);
        let remotos = c(&[("main.tex", "dela")]);
        let m = mesclar(&base, &locais, &remotos, &sem_travados(), "nome@exemplo.com");
        assert_eq!(m.final_, c(&[("main.tex", "dela"), ("main (conflito de nome).tex", "minha")]));
        assert_eq!(m.conflitos, vec!["main (conflito de nome).tex"]);
        assert!(m.escrever.contains(&"main.tex".to_string()) && m.escrever.contains(&"main (conflito de nome).tex".to_string()));
        assert_eq!((m.enviados, m.baixados), (1, 1));
        // primeiro encontro (sem base) com conteudos diferentes tambem e conflito
        let m2 = mesclar(&b(&[]), &locais, &remotos, &sem_travados(), "nome");
        assert_eq!(m2.conflitos.len(), 1);
        // segunda copia ganha numero
        let locais2 = c(&[("main.tex", "minha"), ("main (conflito de nome).tex", "antiga")]);
        let m3 = mesclar(&base, &locais2, &remotos, &sem_travados(), "nome");
        assert_eq!(m3.conflitos, vec!["main (conflito de nome 2).tex"]);
    }

    #[test]
    fn mescla_apagados() {
        let base = b(&[("velho.tex", "v1"), ("editado.tex", "v1"), ("meu.tex", "v1"), ("dela.tex", "v1")]);
        // apagados no Drive: um sem edicao local (some daqui) e outro editado aqui (volta)
        let locais = c(&[("velho.tex", "v1"), ("editado.tex", "v2")]);
        let m = mesclar(&base, &locais, &Conteudo::new(), &sem_travados(), "nome");
        assert_eq!(m.final_, c(&[("editado.tex", "v2")]));
        assert_eq!(m.apagar, vec!["velho.tex"]);
        assert_eq!(m.apagados, 1);
        // apagados aqui: um sem edicao no Drive (sai do Drive) e outro editado la (volta)
        let remotos = c(&[("meu.tex", "v1"), ("dela.tex", "v2")]);
        let m = mesclar(&base, &Conteudo::new(), &remotos, &sem_travados(), "nome");
        assert_eq!(m.final_, c(&[("dela.tex", "v2")]));
        assert_eq!(m.escrever, vec!["dela.tex"]);
        assert_eq!(m.apagados, 1); // meu.tex sai do Drive
    }

    #[test]
    fn mescla_arquivo_travado_fica_como_esta() {
        let base = b(&[("preso.tex", "v1")]);
        let remotos = c(&[("preso.tex", "v1"), ("b.tex", "B")]);
        let travados = BTreeSet::from(["preso.tex".to_string()]);
        let m = mesclar(&base, &c(&[]), &remotos, &travados, "nome");
        assert_eq!(m.final_, c(&[("preso.tex", "v1"), ("b.tex", "B")]));
        assert!(m.escrever == vec!["b.tex"] && m.apagar.is_empty());
        assert!(!m.zip_mudou(&remotos));
    }

    #[test]
    fn detecta_mudanca_local() {
        let base = b(&[("x.tex", "v1")]);
        assert!(!mudou_local(&base, &c(&[("x.tex", "v1")]), &sem_travados()));
        assert!(mudou_local(&base, &c(&[("x.tex", "v2")]), &sem_travados()));
        assert!(mudou_local(&base, &c(&[("x.tex", "v1"), ("n.tex", "")]), &sem_travados()));
        assert!(mudou_local(&base, &c(&[]), &sem_travados()));
        assert!(!mudou_local(&base, &c(&[]), &BTreeSet::from(["x.tex".to_string()])));
    }

    // ---------- estado e conversao da pasta antiga ----------

    #[test]
    fn estado_antigo_le_e_e_reconhecido_como_pasta() {
        let antigo = r#"{"pasta_id":"PASTA123456","arquivos":{"main.tex":{"id":"F1","md5_remoto":"abc","sha_local":"def"}}}"#;
        let e: Estado = serde_json::from_str(antigo).unwrap();
        assert!(e.e_formato_antigo());
        assert_eq!(e.id_remoto(), Some("PASTA123456"));
        assert_eq!(e.sha_base().get("main.tex").map(String::as_str), Some("def"));
        // o formato novo nao grava pasta_id nem os ids por arquivo
        let novo = Estado { arquivo_id: "ZIP1234567".into(), arquivos: BTreeMap::from([("a".into(), Registro { sha_local: "s".into(), ..Default::default() })]), ..Default::default() };
        let texto = serde_json::to_string(&novo).unwrap();
        assert!(!texto.contains("pasta_id") && !texto.contains("\"id\""));
        let volta: Estado = serde_json::from_str(&texto).unwrap();
        assert!(!volta.e_formato_antigo());
        assert_eq!(volta.id_remoto(), Some("ZIP1234567"));
    }

    #[test]
    fn converte_pasta_antiga_em_zip_com_conflito_e_sem_perder_arquivo() {
        // base gravada pela versao antiga; o dono editou intro.tex aqui, outra pessoa editou
        // intro.tex e criou novo.tex na pasta do Drive
        let antigo = r#"{"pasta_id":"PASTA123456","arquivos":{
            "intro.tex":{"id":"F1","md5_remoto":"m1","sha_local":"@INTRO@"},
            "fig.png":{"id":"F2","md5_remoto":"m2","sha_local":"@FIG@"}}}"#
            .replace("@INTRO@", &sha(b"v1"))
            .replace("@FIG@", &sha(b"png"));
        let e: Estado = serde_json::from_str(&antigo).unwrap();
        let locais = c(&[("intro.tex", "minha edicao"), ("fig.png", "png"), ("so-aqui.tex", "S")]);
        let remotos = c(&[("intro.tex", "edicao dela"), ("fig.png", "png"), ("novo.tex", "N")]);
        let m = mesclar(&e.sha_base(), &locais, &remotos, &sem_travados(), "dono@x.com");
        assert_eq!(m.conflitos, vec!["intro (conflito de dono).tex"]);
        assert_eq!(m.final_.len(), 5);
        assert_eq!(m.final_["intro.tex"], b"edicao dela");
        assert_eq!(m.final_["intro (conflito de dono).tex"], b"minha edicao");
        assert_eq!(m.final_["so-aqui.tex"], b"S");
        assert_eq!(m.final_["novo.tex"], b"N");
        assert!(m.apagar.is_empty(), "nenhum arquivo local pode ser apagado na conversao");
        // o zip gerado tem tudo
        assert_eq!(desempacotar(&empacotar(&m.final_).unwrap()).unwrap(), m.final_);
    }

    #[test]
    fn convite_explica_como_abrir_o_projeto() {
        let t = texto_do_convite("", "Tese.eguatex.zip");
        assert!(t.contains("Compartilhados comigo") && t.contains("Escolher no Google Drive") && t.contains("Tese.eguatex.zip"));
        let t = texto_do_convite("Oi, vamos escrever juntos", "Tese.eguatex.zip");
        assert!(t.starts_with("Oi, vamos escrever juntos\n\n") && t.contains("Tese.eguatex.zip"));
        assert!(!t.contains('\u{2014}') && !t.contains('\u{2013}'));
    }

    #[test]
    fn erro_404_leva_o_prefixo_do_seletor() {
        assert!(acesso_perdido(&erro_da_api(404, "{}")));
        assert!(!acesso_perdido(&erro_da_api(403, "{}")));
        assert!(!erro_da_api(404, "{}").contains('\u{2014}'));
    }

    #[test]
    fn projeto_do_drive_precisa_ser_zip_ou_pasta_antiga() {
        let zip = MetaRemota { nome: "Tese.eguatex.zip".into(), mime: MIME_ZIP.into(), ..Default::default() };
        let pasta = MetaRemota { nome: "Tese".into(), mime: MIME_PASTA.into(), ..Default::default() };
        let outro = MetaRemota { nome: "foto.png".into(), mime: "image/png".into(), ..Default::default() };
        assert!(conferir_projeto(&zip).is_ok() && conferir_projeto(&pasta).is_ok());
        assert!(conferir_projeto(&outro).is_err());
    }
}

#[cfg(test)]
mod testes_extra {
    use super::*;

    const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0xFF, 0xFE, 0x0D, 0x0A, 0x00];

    fn dir_temp(rotulo: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eguatex-extra-{rotulo}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn c(itens: &[(&str, &[u8])]) -> Conteudo {
        itens.iter().map(|(k, v)| (k.to_string(), v.to_vec())).collect()
    }

    fn b(itens: &[(&str, &[u8])]) -> BTreeMap<String, String> {
        itens.iter().map(|(k, v)| (k.to_string(), sha(v))).collect()
    }

    fn nada() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn apagado_no_drive_e_editado_aqui_sobrevive_e_sobe() {
        let base = b(&[("a.tex", b"v1")]);
        let m = mesclar(&base, &c(&[("a.tex", b"v2")]), &Conteudo::new(), &nada(), "nome");
        assert_eq!(m.final_, c(&[("a.tex", b"v2")]));
        assert!(m.apagar.is_empty());
        assert_eq!(m.enviados, 1);
        assert!(m.zip_mudou(&Conteudo::new()));
    }

    #[test]
    fn apagado_aqui_e_editado_no_drive_sobrevive_e_baixa() {
        let base = b(&[("a.tex", b"v1")]);
        let m = mesclar(&base, &Conteudo::new(), &c(&[("a.tex", b"v2")]), &nada(), "nome");
        assert_eq!(m.final_, c(&[("a.tex", b"v2")]));
        assert_eq!(m.escrever, vec!["a.tex"]);
        assert_eq!(m.baixados, 1);
    }

    #[test]
    fn apagado_de_um_lado_sem_edicao_no_outro_some_dos_dois() {
        let base = b(&[("a.tex", b"v1")]);
        let m = mesclar(&base, &Conteudo::new(), &c(&[("a.tex", b"v1")]), &nada(), "nome");
        assert!(m.final_.is_empty());
        let m = mesclar(&base, &c(&[("a.tex", b"v1")]), &Conteudo::new(), &nada(), "nome");
        assert!(m.final_.is_empty());
        assert_eq!(m.apagar, vec!["a.tex"]);
    }

    #[test]
    fn dois_editaram_o_mesmo_arquivo_guarda_as_duas_versoes() {
        let base = b(&[("t.tex", b"v1")]);
        let m = mesclar(&base, &c(&[("t.tex", b"minha")]), &c(&[("t.tex", b"dela")]), &nada(), "paulo@x.com");
        assert_eq!(m.final_["t.tex"], b"dela");
        assert_eq!(m.final_["t (conflito de paulo).tex"], b"minha");
        assert_eq!(m.final_.len(), 2);
        assert_eq!(m.conflitos.len(), 1);
        assert!(m.apagar.is_empty());
    }

    #[test]
    fn png_binario_passa_byte_a_byte_pelo_zip_e_pela_mescla() {
        let origem = c(&[("figs/a.png", PNG), ("main.tex", b"x")]);
        let zip = empacotar(&origem).unwrap();
        let lido = desempacotar(&zip).unwrap();
        assert_eq!(lido["figs/a.png"], PNG);
        let m = mesclar(&BTreeMap::new(), &Conteudo::new(), &lido, &nada(), "nome");
        assert_eq!(m.final_["figs/a.png"], PNG);
        let mut novo = PNG.to_vec();
        novo[10] = 0x01;
        let m = mesclar(&b(&[("figs/a.png", PNG)]), &c(&[("figs/a.png", &novo)]), &c(&[("figs/a.png", PNG)]), &nada(), "nome");
        assert_eq!(m.final_["figs/a.png"], novo);
    }

    #[test]
    fn nomes_com_acento_e_espaco_sobrevivem_ao_zip_e_ao_disco() {
        let nomes = ["relatório final.tex", "capítulo um/ação 2.tex", "figuras/gráfico ç.png"];
        let origem: Conteudo = nomes.iter().map(|n| (n.to_string(), PNG.to_vec())).collect();
        let lido = desempacotar(&empacotar(&origem).unwrap()).unwrap();
        assert_eq!(lido, origem);
        let dir = dir_temp("acento");
        for (rel, bytes) in &lido {
            gravar_local(&dir, rel, bytes).unwrap();
        }
        let achados = arquivos_locais(&dir).unwrap();
        assert_eq!(achados, nomes.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>());
        assert_eq!(fs::read(dir.join("relatório final.tex")).unwrap(), PNG);
        assert_eq!(nome_do_zip("Tese de ação"), "Tese de ação.eguatex.zip");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn zip_corrompido_da_erro_e_nao_mexe_no_disco() {
        let dir = dir_temp("corrompido");
        fs::write(dir.join("main.tex"), "meu").unwrap();
        let bom = empacotar(&c(&[("main.tex", b"longo conteudo repetido repetido repetido")])).unwrap();
        assert!(desempacotar(&bom[..bom.len() / 2]).is_err());
        assert!(desempacotar(b"").is_err());
        assert!(desempacotar(&[0xFF; 64]).is_err());
        let mut ruim = bom.clone();
        let n = ruim.len();
        for byte in ruim[n - 22..].iter_mut() {
            *byte = 0;
        }
        assert!(desempacotar(&ruim).is_err());
        assert_eq!(fs::read_to_string(dir.join("main.tex")).unwrap(), "meu");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn zip_com_caminhos_perigosos_nunca_escreve_fora_da_pasta() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for nome in ["../fora.tex", "a/../../fora2.tex", "/abs.tex", "C:/abs.tex", "..\\fora3.tex", "ok.tex"] {
            zip.start_file(nome, o).unwrap();
            zip.write_all(b"x").unwrap();
        }
        let lido = desempacotar(&zip.finish().unwrap().into_inner()).unwrap();
        for rel in lido.keys() {
            assert!(!rel.contains(".."), "caminho com .. passou: {rel}");
            assert!(!rel.starts_with('/') && !rel.contains(':'), "caminho absoluto passou: {rel}");
        }
        assert!(lido.contains_key("ok.tex"));
        let dir = dir_temp("perigo");
        for (rel, bytes) in &lido {
            gravar_local(&dir, rel, bytes).unwrap();
        }
        for fora in ["fora.tex", "fora2.tex", "fora3.tex", "abs.tex"] {
            assert!(!dir.parent().unwrap().join(fora).exists(), "{fora} escapou da pasta");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn projeto_vazio_empacota_e_mescla_sem_nada() {
        let zip = empacotar(&Conteudo::new()).unwrap();
        assert!(desempacotar(&zip).unwrap().is_empty());
        let m = mesclar(&BTreeMap::new(), &Conteudo::new(), &Conteudo::new(), &nada(), "nome");
        assert!(m.final_.is_empty() && m.escrever.is_empty() && m.apagar.is_empty());
        assert_eq!((m.enviados, m.baixados, m.apagados), (0, 0, 0));
        let dir = dir_temp("vazio");
        assert!(arquivos_locais(&dir).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn drive_json_antigo_corrompido_e_vazio() {
        let dir = dir_temp("json");
        assert!(ler_estado(&dir).is_none());
        fs::create_dir_all(dir.join(".eguatex")).unwrap();
        fs::write(caminho_estado(&dir), "{ isto nao e json").unwrap();
        assert!(ler_estado(&dir).is_none());
        fs::write(caminho_estado(&dir), "").unwrap();
        assert!(ler_estado(&dir).is_none());
        fs::write(caminho_estado(&dir), r#"{"pasta_id":"PASTA123456","arquivos":{"a.tex":{"id":"F","md5_remoto":"m","sha_local":"s"}}}"#).unwrap();
        let e = ler_estado(&dir).unwrap();
        assert!(e.e_formato_antigo());
        assert_eq!(e.id_remoto(), Some("PASTA123456"));
        let novo = Estado {
            arquivo_id: "ZIP1234567".into(),
            md5_remoto: "m".into(),
            versao: "7".into(),
            arquivos: BTreeMap::from([("a".to_string(), Registro { sha_local: "s".into(), ..Default::default() })]),
            ..Default::default()
        };
        gravar_estado(&dir, &novo).unwrap();
        let volta = ler_estado(&dir).unwrap();
        assert_eq!((volta.arquivo_id.as_str(), volta.md5_remoto.as_str(), volta.versao.as_str()), ("ZIP1234567", "m", "7"));
        assert_eq!(volta.sha_base().get("a").map(String::as_str), Some("s"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn estado_corrompido_vira_base_vazia_e_nao_apaga_nada() {
        let m = mesclar(&BTreeMap::new(), &c(&[("a.tex", b"A"), ("c.tex", b"igual")]), &c(&[("b.tex", b"B"), ("c.tex", b"igual")]), &nada(), "nome");
        assert_eq!(m.final_, c(&[("a.tex", b"A"), ("b.tex", b"B"), ("c.tex", b"igual")]));
        assert!(m.apagar.is_empty());
        assert_eq!(m.apagados, 0);
    }

    #[test]
    fn mescla_e_idempotente() {
        let base = b(&[("x.tex", b"v1"), ("y.tex", b"v1"), ("z.tex", b"v1"), ("f.png", PNG)]);
        let locais = c(&[("x.tex", b"minha"), ("y.tex", b"v1"), ("n.tex", b"N"), ("f.png", PNG)]);
        let remotos = c(&[("x.tex", b"dela"), ("y.tex", b"v2"), ("z.tex", b"v1"), ("f.png", PNG)]);
        let m1 = mesclar(&base, &locais, &remotos, &nada(), "nome");
        let base2: BTreeMap<String, String> = m1.final_.iter().map(|(k, v)| (k.clone(), sha(v))).collect();
        let m2 = mesclar(&base2, &m1.final_, &m1.final_, &nada(), "nome");
        assert_eq!(m2.final_, m1.final_);
        assert!(m2.escrever.is_empty() && m2.apagar.is_empty() && m2.conflitos.is_empty());
        assert_eq!((m2.enviados, m2.baixados, m2.apagados), (0, 0, 0));
        assert!(!m2.zip_mudou(&m1.final_));
        assert_eq!(empacotar(&m1.final_).unwrap(), empacotar(&m2.final_).unwrap());
        let m3 = mesclar(&BTreeMap::new(), &m1.final_, &m1.final_, &nada(), "nome");
        assert_eq!(m3.final_, m1.final_);
        assert!(m3.conflitos.is_empty() && !m3.zip_mudou(&m1.final_));
    }

    #[test]
    fn conversao_com_arquivo_so_no_disco_nao_apaga_e_sobe() {
        let antigo = Estado {
            pasta_id: "PASTA123456".into(),
            arquivos: BTreeMap::from([("so-aqui.tex".to_string(), Registro { sha_local: sha(b"S"), ..Default::default() })]),
            ..Default::default()
        };
        assert!(antigo.e_formato_antigo());
        assert!(base_da_rodada(&antigo).is_empty());
        let m = mesclar(&base_da_rodada(&antigo), &c(&[("so-aqui.tex", b"S")]), &c(&[("dela.tex", b"D")]), &nada(), "dono");
        assert!(m.apagar.is_empty());
        assert_eq!(m.final_, c(&[("so-aqui.tex", b"S"), ("dela.tex", b"D")]));
        assert!(m.zip_mudou(&c(&[("dela.tex", b"D")])));
        // estado novo usa a base gravada
        let novo = Estado { arquivo_id: "ZIP1234567".into(), arquivos: antigo.arquivos.clone(), ..Default::default() };
        assert_eq!(base_da_rodada(&novo).len(), 1);
    }

    #[test]
    fn reabrir_seletor_com_base_antiga_nao_apaga_arquivo_local() {
        let mut e = Estado {
            arquivo_id: "ANTIGO12345".into(),
            md5_remoto: "m".into(),
            versao: "v".into(),
            arquivos: BTreeMap::from([("a.tex".to_string(), Registro { sha_local: sha(b"a"), ..Default::default() })]),
            ..Default::default()
        };
        trocar_arquivo(&mut e, "NOVO1234567".into());
        let m = mesclar(&base_da_rodada(&e), &c(&[("a.tex", b"a")]), &Conteudo::new(), &nada(), "x");
        assert!(m.apagar.is_empty());
        assert_eq!(m.final_, c(&[("a.tex", b"a")]));
    }

    #[test]
    fn nome_a_dois_pontos_b_e_reenviado_sem_ir_ao_disco() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        zip.start_file("a:b.tex", o).unwrap();
        zip.write_all(b"B").unwrap();
        zip.start_file("ok.tex", o).unwrap();
        zip.write_all(b"A").unwrap();
        let (arq, pres) = desempacotar_tudo(&zip.finish().unwrap().into_inner()).unwrap();
        assert!(!arq.contains_key("a:b.tex") && pres["a:b.tex"] == b"B");
        let m = mesclar(&BTreeMap::new(), &arq, &arq, &nada(), "x");
        let mut reenvio = m.final_.clone();
        reenvio.extend(pres);
        let (a2, p2) = desempacotar_tudo(&empacotar(&reenvio).unwrap()).unwrap();
        assert_eq!(p2["a:b.tex"], b"B");
        assert_eq!(a2["ok.tex"], b"A");
    }

    // ---------- revisoes alheias e publicar simultaneo ----------

    fn rev(id: &str, email: &str, eu: bool) -> Revisao {
        Revisao { id: id.into(), email: email.into(), sou_eu: eu }
    }

    #[test]
    fn revisao_alheia_entre_a_anterior_e_a_nova() {
        let lista = vec![rev("1", "eu@x.com", true), rev("2", "ana@x.com", false), rev("3", "eu@x.com", true)];
        assert!(tem_revisao_alheia(&lista, "1", "3", "eu@x.com"));
        let minhas = vec![rev("1", "eu@x.com", true), rev("2", "eu@x.com", true), rev("3", "eu@x.com", true)];
        assert!(!tem_revisao_alheia(&minhas, "1", "3", "eu@x.com"));
        // consecutivas, ou id fora da lista: nao ha o que afirmar
        assert!(!tem_revisao_alheia(&lista, "2", "3", "eu@x.com"));
        assert!(!tem_revisao_alheia(&lista, "9", "3", "eu@x.com"));
    }

    #[tokio::test]
    async fn duas_publicacoes_simultaneas_criam_um_so_zip() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let trava = Arc::new(tokio::sync::Mutex::new(()));
        let estado: Arc<std::sync::Mutex<Option<Estado>>> = Arc::new(std::sync::Mutex::new(None));
        let criacoes = Arc::new(AtomicUsize::new(0));
        let mut tarefas = Vec::new();
        for _ in 0..2 {
            let (trava, estado, criacoes) = (trava.clone(), estado.clone(), criacoes.clone());
            tarefas.push(tokio::spawn(async move {
                let para_ler = estado.clone();
                publicar_sob_trava(
                    &trava,
                    move || para_ler.lock().unwrap().clone(),
                    |_| {
                        let (estado, criacoes) = (estado.clone(), criacoes.clone());
                        async move {
                            criacoes.fetch_add(1, Ordering::SeqCst);
                            // o envio demora: a outra chamada chega enquanto isso
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            *estado.lock().unwrap() = Some(Estado { arquivo_id: "ZIP1".into(), ..Default::default() });
                            Ok(())
                        }
                    },
                )
                .await
            }));
        }
        for t in tarefas {
            assert_eq!(t.await.unwrap().unwrap(), "ZIP1");
        }
        assert_eq!(criacoes.load(Ordering::SeqCst), 1);
    }
}

#[cfg(test)]
mod testes_vinculos {
    use super::*;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eguatex-vinc-{rotulo}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn c(itens: &[(&str, &str)]) -> Conteudo {
        itens.iter().map(|(k, v)| (k.to_string(), v.as_bytes().to_vec())).collect()
    }

    fn v(md5: &str, versao: &str, sha_local: &str) -> Vinculo {
        Vinculo { id: "ID1".into(), md5: md5.into(), versao: versao.into(), sha_local: sha_local.into(), ..Default::default() }
    }

    #[test]
    fn vinculos_gravam_e_leem_de_volta() {
        let dir = dir_temp("ida-volta");
        let mut m = Vinculos::new();
        m.insert("main.tex".into(), v("abc", "rev1", "sha1"));
        m.insert("fig 1.png".into(), Vinculo { conflito_de: "rev9".into(), sha_conflito: "shaX".into(), ..v("", "rev2", "sha2") });
        gravar_vinculos(&dir, &m).unwrap();
        assert!(dir.join(".eguatex").join("drive-vinculos.json").exists());
        assert_eq!(ler_vinculos(&dir), m);
    }

    #[test]
    fn vinculos_ausentes_ou_corrompidos_viram_mapa_vazio() {
        let dir = dir_temp("corrompido");
        assert!(ler_vinculos(&dir).is_empty());
        fs::create_dir_all(dir.join(".eguatex")).unwrap();
        fs::write(dir.join(".eguatex").join("drive-vinculos.json"), "{ isto nao e json").unwrap();
        assert!(ler_vinculos(&dir).is_empty());
        fs::write(dir.join(".eguatex").join("drive-vinculos.json"), "").unwrap();
        assert!(ler_vinculos(&dir).is_empty());
        // campos novos ausentes no arquivo antigo: lidos com vazio
        fs::write(dir.join(".eguatex").join("drive-vinculos.json"), r#"{"a.tex":{"id":"X1"}}"#).unwrap();
        assert_eq!(ler_vinculos(&dir)["a.tex"].id, "X1");
    }

    #[test]
    fn nome_repetido_vira_nome_2_sem_sobrescrever() {
        let existentes = ["a.tex", "a (2).tex", "cap/b.png", "README"];
        let existe = |n: &str| existentes.contains(&n);
        assert_eq!(nome_livre(&existe, "novo.tex"), "novo.tex");
        assert_eq!(nome_livre(&existe, "a.tex"), "a (3).tex");
        assert_eq!(nome_livre(&existe, "cap/b.png"), "cap/b (2).png");
        assert_eq!(nome_livre(&existe, "README"), "README (2)");

        let dir = dir_temp("colocar");
        assert_eq!(colocar_arquivo(&dir, "main.tex", b"um").unwrap(), "main.tex");
        assert_eq!(colocar_arquivo(&dir, "main.tex", b"dois").unwrap(), "main (2).tex");
        assert_eq!(fs::read(dir.join("main.tex")).unwrap(), b"um");
        assert_eq!(fs::read(dir.join("main (2).tex")).unwrap(), b"dois");
    }

    #[test]
    fn decisao_de_envio_e_conflito() {
        let base = v("m1", "r1", "s1");
        // nada mudou
        assert_eq!(decidir_vinculo(&base, Some("s1"), "m1", "r1"), AcaoVinculo::Nada);
        // so o disco mudou: envia
        assert_eq!(decidir_vinculo(&base, Some("s2"), "m1", "r1"), AcaoVinculo::Enviar);
        // so o Drive mudou: traz
        assert_eq!(decidir_vinculo(&base, Some("s1"), "m2", "r2"), AcaoVinculo::Baixar);
        // os dois mudaram: conflito, nada e sobrescrito
        assert_eq!(decidir_vinculo(&base, Some("s2"), "m2", "r2"), AcaoVinculo::Conflito);
        // arquivo local apagado ou travado: nunca apaga nem envia
        assert_eq!(decidir_vinculo(&base, None, "m2", "r2"), AcaoVinculo::Nada);
        // sem md5 (tipo sem checksum): vale a revisao
        let sem_md5 = v("", "r1", "s1");
        assert_eq!(decidir_vinculo(&sem_md5, Some("s1"), "", "r2"), AcaoVinculo::Baixar);
        assert_eq!(decidir_vinculo(&sem_md5, Some("s2"), "", "r2"), AcaoVinculo::Conflito);
        // md5 igual com revisao nova (so tocou nos metadados): nao e mudanca
        assert_eq!(decidir_vinculo(&base, Some("s1"), "m1", "r9"), AcaoVinculo::Nada);
    }

    #[test]
    fn depois_do_conflito_nao_repete_a_copia_ate_a_pessoa_mexer() {
        let em_conflito = Vinculo { conflito_de: "r2".into(), sha_conflito: "s2".into(), ..v("m1", "r1", "s1") };
        // mesma versao do Drive, arquivo local igual ao do conflito: espera
        assert_eq!(decidir_vinculo(&em_conflito, Some("s2"), "m2", "r2"), AcaoVinculo::EmConflito);
        // a pessoa juntou as versoes e salvou: envia
        assert_eq!(decidir_vinculo(&em_conflito, Some("s3"), "m2", "r2"), AcaoVinculo::Enviar);
        // o Drive mudou de novo: volta a regra normal (conflito de novo)
        assert_eq!(decidir_vinculo(&em_conflito, Some("s2"), "m3", "r3"), AcaoVinculo::Conflito);
    }

    #[test]
    fn extensoes_aceitas_e_zip_de_projeto() {
        for ok in ["a.tex", "REFS.BIB", "x.sty", "x.cls", "x.bst", "n.txt", "l.md", "f.PNG", "f.jpg", "f.jpeg", "f.pdf", "f.eps", "f.svg", "p.zip"] {
            assert!(extensao_aceita(ok), "{ok}");
        }
        for ruim in ["a.docx", "semextensao", "a.exe"] {
            assert!(!extensao_aceita(ruim), "{ruim}");
        }
        let mut c = Conteudo::new();
        c.insert("main.tex".into(), b"x".to_vec());
        let comum = empacotar(&c).unwrap();
        assert!(!zip_e_projeto_eguatex(&comum));
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file(".eguatex/drive.json", o).unwrap();
        z.write_all(b"{}").unwrap();
        z.start_file("main.tex", o).unwrap();
        z.write_all(b"x").unwrap();
        assert!(zip_e_projeto_eguatex(&z.finish().unwrap().into_inner()));
    }

    #[test]
    fn zip_extrai_sem_sair_da_pasta_e_sem_sobrescrever() {
        let dir = dir_temp("zip");
        fs::write(dir.join("main.tex"), "meu").unwrap();
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (nome, txt) in [("main.tex", "do zip"), ("../fora.tex", "perigo"), ("cap/a.tex", "capitulo")] {
            z.start_file(nome, o).unwrap();
            z.write_all(txt.as_bytes()).unwrap();
        }
        let gravados = extrair_zip(&dir, &z.finish().unwrap().into_inner(), "").unwrap();
        assert!(gravados.contains(&"main (2).tex".to_string()) && gravados.contains(&"cap/a.tex".to_string()));
        assert_eq!(fs::read_to_string(dir.join("main.tex")).unwrap(), "meu");
        assert!(!dir.parent().unwrap().join("fora.tex").exists());
        assert_eq!(gravados.len(), 2);
    }

    #[test]
    fn pasta_nova_nao_repete_nome() {
        let pai = dir_temp("pasta-pai");
        let a = pasta_nova(&pai, "Tese: final").unwrap();
        assert!(a.ends_with("Tese- final"));
        fs::create_dir_all(&a).unwrap();
        assert!(pasta_nova(&pai, "Tese: final").unwrap().ends_with("Tese- final (2)"));
    }

    #[test]
    fn figura_do_drive_fica_em_figuras_com_nome_seguro() {
        assert_eq!(caminho_da_figura("foto.png"), "figuras/foto.png");
        assert_eq!(caminho_da_figura("a/b:c.png"), "figuras/a-b-c.png");
    }

    #[tokio::test]
    async fn figura_do_drive_repetida_vira_nome_2_e_liga_ao_arquivo_uma_vez() {
        let dir = dir_temp("figura-drive");
        let m = MetaRemota { id: "FIG1".into(), nome: "grafico.png".into(), mime: "image/png".into(), md5: "m1".into(), versao: "r1".into(), dono_sou_eu: true };
        assert_eq!(trazer_figura(&dir, &m, b"png-bytes", true).await.unwrap(), "figuras/grafico.png");
        assert_eq!(trazer_figura(&dir, &m, b"png-bytes", true).await.unwrap(), "figuras/grafico (2).png");
        assert_eq!(fs::read(dir.join("figuras").join("grafico (2).png")).unwrap(), b"png-bytes");
        // so a primeira copia fica ligada ao arquivo do Drive
        let v = ler_vinculos(&dir);
        assert_eq!(v.len(), 1);
        assert_eq!(v["figuras/grafico.png"].id, "FIG1");
        assert_eq!(v["figuras/grafico.png"].sha_local, sha(b"png-bytes"));
        // outra figura do Drive com o mesmo nome: nao sobrescreve e liga a ela
        let outra = MetaRemota { id: "FIG2".into(), ..m };
        assert_eq!(trazer_figura(&dir, &outra, b"outra", true).await.unwrap(), "figuras/grafico (3).png");
        assert_eq!(ler_vinculos(&dir)["figuras/grafico (3).png"].id, "FIG2");
        assert_eq!(fs::read(dir.join("figuras").join("grafico.png")).unwrap(), b"png-bytes");
    }

    #[tokio::test]
    async fn figura_do_drive_em_projeto_com_zip_nao_cria_vinculo() {
        let dir = dir_temp("figura-zip");
        gravar_estado(&dir, &Estado { arquivo_id: "ZIP1".into(), ..Default::default() }).unwrap();
        let m = MetaRemota { id: "FIG1".into(), nome: "g.png".into(), ..Default::default() };
        assert_eq!(trazer_figura(&dir, &m, b"png-bytes", true).await.unwrap(), "figuras/g.png");
        assert!(ler_vinculos(&dir).is_empty());
    }

    #[test]
    fn decisao_do_que_escolhido_no_modo_projeto() {
        use EscolhaProjeto::*;
        // projeto compartilhado: so o .eguatex.zip (e a pasta do formato antigo)
        assert_eq!(decidir_escolha_projeto("Tese.eguatex.zip", "application/zip"), ProjetoCompartilhado);
        assert_eq!(decidir_escolha_projeto("TESE.EGUATEX.ZIP", ""), ProjetoCompartilhado);
        assert_eq!(decidir_escolha_projeto("Tese", MIME_PASTA), ProjetoCompartilhado);
        // zip comum (o "124.zip" do Overleaf): abre como arquivos, nao recusa
        assert_eq!(decidir_escolha_projeto("124.zip", "application/zip"), ZipComum);
        assert_eq!(decidir_escolha_projeto("124.ZIP", "application/octet-stream"), ZipComum);
        assert_eq!(decidir_escolha_projeto("projeto final.zip", "application/x-zip-compressed"), ZipComum);
        // outro tipo: arquivo solto se o ÉguaTex usa, senao nao serve
        for ok in ["main.tex", "refs.bib", "abnt.cls", "fig.png", "doc.pdf", "notas.md"] {
            assert_eq!(decidir_escolha_projeto(ok, "text/plain"), ArquivoSolto, "{ok}");
        }
        for ruim in ["planilha.xlsx", "foto.gif", "programa.exe", "semextensao"] {
            assert_eq!(decidir_escolha_projeto(ruim, "application/x-msdownload"), Inaceitavel, "{ruim}");
        }
        assert_eq!(decidir_escolha_projeto("Meu doc", "application/vnd.google-apps.document"), Inaceitavel);
        assert_eq!(decidir_escolha_projeto("Planilha.zip", "application/vnd.google-apps.spreadsheet"), Inaceitavel);
    }

    // ---------- zip por extensao, por tipo MIME ou pelo conteudo ----------

    #[test]
    fn zip_vale_por_extensao_por_tipo_mime_ou_pelo_conteudo() {
        use EscolhaProjeto::*;
        for mime in ["application/zip", "application/x-zip-compressed", "application/x-zip", "application/octet-stream", ""] {
            assert_eq!(decidir_escolha_projeto("124.zip", mime), ZipComum, "{mime}");
        }
        // sem .zip no nome mas com tipo de zip
        for mime in ["application/zip", "application/x-zip-compressed", "application/x-zip"] {
            assert_eq!(decidir_escolha_projeto("projeto", mime), ZipComum, "{mime}");
        }
        // tipo generico e nome sem extensao conhecida: so o conteudo decide
        assert_eq!(decidir_escolha_projeto("sem-extensao", "application/octet-stream"), TalvezZip);
        assert!(e_zip_de_fato("sem-extensao", "application/octet-stream", b"PK\x03\x04resto"));
        assert!(!e_zip_de_fato("sem-extensao", "application/octet-stream", b"texto qualquer"));
        assert!(e_zip_de_fato("a.ZIP", "", b""));
        assert!(!e_zip_de_fato("a.tex", "text/plain", b"PK\x03\x04"));
        assert!(bytes_sao_zip(b"PK\x05\x06"));
    }

    // ---------- pasta do Drive: estrutura a partir do que a pessoa marcou ----------

    fn item(id: &str, nome: &str, mime: &str, pai: &str) -> drive_picker::ArquivoEscolhido {
        drive_picker::ArquivoEscolhido { id: id.into(), nome: nome.into(), mime_type: mime.into(), parents: if pai.is_empty() { vec![] } else { vec![pai.into()] } }
    }

    #[test]
    fn estrutura_da_pasta_mantem_as_subpastas_marcadas() {
        let itens = vec![
            item("A", "main.tex", "text/plain", "RAIZ"),
            item("B", "refs.bib", "application/octet-stream", "RAIZ"),
            item("S1", "cap", MIME_PASTA, "RAIZ"),
            item("S2", "figs", MIME_PASTA, "S1"),
            item("C", "intro.tex", "text/plain", "S1"),
            item("D", "fig1.png", "image/png", "S2"),
            item("E", "solto.tex", "text/plain", "DESCONHECIDA"),
        ];
        let est = resolver_pastas("RAIZ", &itens);
        let onde: BTreeMap<&str, &str> = est.arquivos.iter().map(|(a, d)| (a.nome.as_str(), d.as_str())).collect();
        assert_eq!(onde["main.tex"], "");
        assert_eq!(onde["refs.bib"], "");
        assert_eq!(onde["intro.tex"], "cap");
        assert_eq!(onde["fig1.png"], "cap/figs");
        // pai que nao e a pasta nem subpasta marcada: fica na raiz e avisa
        assert_eq!(onde["solto.tex"], "");
        assert_eq!(est.sem_pasta, vec!["solto.tex".to_string()]);
        let mut pastas = est.pastas.clone();
        pastas.sort();
        assert_eq!(pastas, vec!["cap".to_string(), "cap/figs".to_string()]);
    }

    #[test]
    fn estrutura_da_pasta_vazia_ou_com_subpasta_de_pai_desconhecido() {
        assert_eq!(resolver_pastas("RAIZ", &[]), EstruturaDaPasta::default());
        let est = resolver_pastas("RAIZ", &[item("S", "a:b", MIME_PASTA, "OUTRA"), item("F", "x.tex", "text/plain", "S")]);
        // subpasta de pai desconhecido vira filha direta, com nome seguro; o arquivo dentro dela acompanha
        assert_eq!(est.pastas, vec!["a-b".to_string()]);
        assert_eq!(est.arquivos[0].1, "a-b");
        assert!(est.sem_pasta.is_empty());
        assert_eq!(juntar_rel("", "x"), "x");
        assert_eq!(juntar_rel("a", ""), "a");
        assert_eq!(juntar_rel("a", "b/c"), "a/b/c");
    }

    #[tokio::test]
    async fn arquivos_de_pasta_e_zip_dentro_de_pasta_entram_nas_subpastas() {
        let dir = dir_temp("pasta-drive");
        let tex = MetaRemota { id: "T1".into(), nome: "intro.tex".into(), mime: "text/plain".into(), md5: "m".into(), versao: "r".into(), dono_sou_eu: true };
        let mut r = ResultadoAbrir::default();
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("dentro.tex", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"x").unwrap();
        let zip_meta = MetaRemota { id: "Z1".into(), nome: "pacote.zip".into(), mime: "application/x-zip".into(), ..Default::default() };
        let baixados = vec![
            Baixado { meta: tex, bytes: b"capitulo".to_vec(), pasta: "Tese/cap".into() },
            Baixado { meta: zip_meta, bytes: z.finish().unwrap().into_inner(), pasta: "Tese".into() },
        ];
        trazer_para_o_projeto(&dir, baixados, None, &mut r).await.unwrap();
        assert_eq!(fs::read(dir.join("Tese").join("cap").join("intro.tex")).unwrap(), b"capitulo");
        assert_eq!(fs::read(dir.join("Tese").join("dentro.tex")).unwrap(), b"x");
        assert_eq!(ler_vinculos(&dir)["Tese/cap/intro.tex"].id, "T1");
        assert_eq!(r.vinculados, 1);
    }

    /// Zip na estrutura do 124.zip: pastas de embrulho, o .tex numa subpasta e um pdf ao lado.
    fn zip_do_124() -> Vec<u8> {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (nome, txt) in [
            ("modelosparapublicaodeartigos/Template_SBC/template-latex/sbc-template.tex", "\\documentclass{article}\n\\usepackage{sbc-template}\n\\begin{document}\n\\end{document}"),
            ("modelosparapublicaodeartigos/Template_SBC/template-latex/sbc-template.sty", "% estilo"),
            ("modelosparapublicaodeartigos/Template_SBC/sbc_template.pdf", "pdf"),
        ] {
            z.start_file(nome, o).unwrap();
            z.write_all(txt.as_bytes()).unwrap();
        }
        z.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn trazer_para_o_projeto_tira_embrulho_e_sugere_principal() {
        let meta = MetaRemota { id: "Z124".into(), nome: "124.zip".into(), mime: "application/zip".into(), ..Default::default() };

        // projeto aberto (com um main.tex em branco): o zip vai para "124/", sem as pastas de embrulho
        let dir = dir_temp("zip-124-aberto");
        fs::write(dir.join("main.tex"), "\\documentclass{article}\n\\begin{document}\\end{document}").unwrap();
        let mut r = ResultadoAbrir::default();
        trazer_para_o_projeto(&dir, vec![Baixado::solto(meta.clone(), zip_do_124())], None, &mut r).await.unwrap();
        assert_eq!(r.principal_sugerido.as_deref(), Some("124/template-latex/sbc-template.tex"));
        assert!(dir.join("124/template-latex/sbc-template.tex").is_file());
        assert!(dir.join("124/template-latex/sbc-template.sty").is_file());
        assert!(dir.join("124/sbc_template.pdf").is_file());
        assert!(!dir.join("modelosparapublicaodeartigos").exists());
        assert!(fs::read_to_string(dir.join("main.tex")).unwrap().contains("\\begin{document}"));

        // projeto novo vindo do zip: direto na raiz, sem embrulho
        let novo = dir_temp("zip-124-novo");
        let mut r2 = ResultadoAbrir { novo: true, ..Default::default() };
        trazer_para_o_projeto(&novo, vec![Baixado::solto(meta, zip_do_124())], None, &mut r2).await.unwrap();
        assert_eq!(r2.principal_sugerido.as_deref(), Some("template-latex/sbc-template.tex"));
        assert!(novo.join("template-latex/sbc-template.sty").is_file());
        assert!(novo.join("sbc_template.pdf").is_file());

        // .tex solto: num projeto aberto nao sugere nada; num projeto novo sugere o proprio arquivo
        let solto_meta = MetaRemota { id: "T9".into(), nome: "artigo.tex".into(), mime: "text/plain".into(), ..Default::default() };
        let mut r3 = ResultadoAbrir::default();
        trazer_para_o_projeto(&dir, vec![Baixado::solto(solto_meta.clone(), b"\\documentclass{article}\n\\begin{document}\n\\end{document}".to_vec())], None, &mut r3).await.unwrap();
        assert_eq!(r3.principal_sugerido, None);
        let novo2 = dir_temp("tex-solto-novo");
        let mut r4 = ResultadoAbrir { novo: true, ..Default::default() };
        trazer_para_o_projeto(&novo2, vec![Baixado::solto(solto_meta, b"\\documentclass{article}\n\\begin{document}\n\\end{document}".to_vec())], None, &mut r4).await.unwrap();
        assert_eq!(r4.principal_sugerido.as_deref(), Some("artigo.tex"));
    }

    // ---------- figura do Drive: formatos ----------

    #[test]
    fn svg_e_recusado_na_figura() {
        assert_eq!(formato_da_figura("a.svg"), FormatoFigura::Recusado);
        assert_eq!(formato_da_figura("A.SVG"), FormatoFigura::Recusado);
        assert!(MSG_FIGURA_RECUSADA.contains("exporte como PDF ou PNG"));
    }

    // ---------- historico local antes de o Drive mudar ou apagar um arquivo ----------

    #[test]
    fn gravar_local_guarda_versao_do_texto_anterior() {
        let dir = dir_temp("ver-gravar");
        let raiz = dir_temp("ver-gravar-raiz");
        fs::write(dir.join("main.tex"), "texto do disco").unwrap();
        gravar_local_em(&raiz, &dir, "main.tex", b"texto do drive").unwrap();
        assert_eq!(fs::read(dir.join("main.tex")).unwrap(), b"texto do drive");
        // o historico usa o caminho do projeto como o frontend o manda (a string do caminho)
        let projeto = dir.display().to_string();
        assert_eq!(crate::versoes::textos_guardados(&raiz, &projeto, "/main.tex"), vec!["texto do disco"]);
        // com a barra no fim e a barra no inicio do arquivo, o mesmo historico
        assert_eq!(crate::versoes::textos_guardados(&raiz, &format!("{projeto}\\"), "main.tex"), vec!["texto do disco"]);
        // arquivo novo: nao havia o que guardar
        gravar_local_em(&raiz, &dir, "novo.tex", b"primeiro").unwrap();
        assert!(crate::versoes::textos_guardados(&raiz, &projeto, "/novo.tex").is_empty());
        // gravar o mesmo texto de novo nao cria versao repetida
        gravar_local_em(&raiz, &dir, "main.tex", b"texto do drive").unwrap();
        assert_eq!(crate::versoes::textos_guardados(&raiz, &projeto, "/main.tex").len(), 1);
        // gravar outro texto guarda o que o Drive tinha trazido
        gravar_local_em(&raiz, &dir, "main.tex", b"terceiro").unwrap();
        assert_eq!(crate::versoes::textos_guardados(&raiz, &projeto, "/main.tex"), vec!["texto do drive", "texto do disco"]);
    }

    #[test]
    fn apagar_pelo_drive_guarda_versao() {
        let dir = dir_temp("ver-apagar");
        let raiz = dir_temp("ver-apagar-raiz");
        fs::create_dir_all(dir.join("cap")).unwrap();
        fs::write(dir.join("cap").join("a.tex"), "capitulo 1").unwrap();
        apagar_local_em(&raiz, &dir, "cap/a.tex").unwrap();
        assert!(!dir.join("cap").join("a.tex").exists());
        let projeto = dir.display().to_string();
        assert_eq!(crate::versoes::textos_guardados(&raiz, &projeto, "/cap/a.tex"), vec!["capitulo 1"]);
        // apagar o que nao existe devolve o erro de sempre e nao guarda nada
        assert!(apagar_local_em(&raiz, &dir, "cap/b.tex").is_err());
        assert!(crate::versoes::textos_guardados(&raiz, &projeto, "/cap/b.tex").is_empty());
    }

    #[test]
    fn figura_binaria_nao_entra_no_historico() {
        let dir = dir_temp("ver-figura");
        let raiz = dir_temp("ver-figura-raiz");
        fs::create_dir_all(dir.join("figuras")).unwrap();
        let png: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        let jpg: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF";
        fs::write(dir.join("figuras").join("a.png"), png).unwrap();
        fs::write(dir.join("figuras").join("b.jpg"), jpg).unwrap();
        gravar_local_em(&raiz, &dir, "figuras/a.png", jpg).unwrap();
        gravar_local_em(&raiz, &dir, "figuras/b.jpg", png).unwrap();
        apagar_local_em(&raiz, &dir, "figuras/a.png").unwrap();
        let projeto = dir.display().to_string();
        assert!(crate::versoes::textos_guardados(&raiz, &projeto, "/figuras/a.png").is_empty());
        assert!(crate::versoes::textos_guardados(&raiz, &projeto, "/figuras/b.jpg").is_empty());
        assert!(!raiz.join(crate::versoes::hash_do_projeto(&projeto)).exists(), "nenhuma pasta de historico para figura");
    }

    #[test]
    fn svg_continua_arquivo_comum_do_projeto() {
        // "Adicionar arquivos do Drive" ainda traz .svg; so a insercao de figura o recusa
        assert!(extensao_aceita("logo.svg"));
        assert!(drive_picker::EXTENSOES_ARQUIVOS.contains(&"svg"));
    }

    #[test]
    fn formatos_de_figura_aceitos_convertidos_e_recusados() {
        use FormatoFigura::*;
        for ok in ["a.png", "a.JPG", "a.jpeg", "a.pdf", "a.eps"] {
            assert_eq!(formato_da_figura(ok), Direto, "{ok}");
        }
        for conv in ["a.gif", "a.webp", "a.bmp", "a.tif", "a.TIFF"] {
            assert_eq!(formato_da_figura(conv), ConverterParaPng, "{conv}");
        }
        for ruim in ["a.tex", "a.zip", "a.heic", "semextensao", "a.png.exe", "a.svg"] {
            assert_eq!(formato_da_figura(ruim), Recusado, "{ruim}");
        }
        assert_eq!(MSG_FIGURA_RECUSADA, "Use .png, .jpg, .pdf ou .eps. O .svg não entra no pdflatex: exporte como PDF ou PNG");
        assert_eq!(nome_como_png("foto final.gif"), "foto final.png");
        assert_eq!(nome_como_png("semponto"), "semponto.png");
    }

    #[test]
    fn gif_bmp_e_tiff_viram_png_de_verdade() {
        use image::{ImageFormat, Rgba, RgbaImage};
        let mut img = RgbaImage::new(3, 2);
        for p in img.pixels_mut() {
            *p = Rgba([200, 30, 30, 255]);
        }
        for formato in [ImageFormat::Gif, ImageFormat::Bmp, ImageFormat::Tiff] {
            let mut origem = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(img.clone()).write_to(&mut origem, formato).unwrap();
            let png = converter_para_png(&origem.into_inner()).unwrap_or_else(|e| panic!("{formato:?}: {e}"));
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "{formato:?} nao virou png");
            let volta = image::load_from_memory(&png).unwrap();
            assert_eq!((volta.width(), volta.height()), (3, 2));
        }
        assert!(converter_para_png(b"isto nao e imagem").is_err());
    }

    #[tokio::test]
    async fn figura_convertida_nao_e_ligada_ao_arquivo_do_drive() {
        let dir = dir_temp("figura-convertida");
        let m = MetaRemota { id: "GIF1".into(), nome: "anim.png".into(), ..Default::default() };
        assert_eq!(trazer_figura(&dir, &m, b"png", false).await.unwrap(), "figuras/anim.png");
        assert!(ler_vinculos(&dir).is_empty());
    }

    #[test]
    fn nome_da_pasta_do_projeto_aberto() {
        assert_eq!(nome_do_projeto_aberto("124.zip"), "124");
        assert_eq!(nome_do_projeto_aberto("Tese final.zip"), "Tese final");
        assert_eq!(nome_do_projeto_aberto("main.tex"), "main");
        assert_eq!(nome_do_projeto_aberto("a:b.zip"), "a-b");
        assert_eq!(nome_do_projeto_aberto(".zip"), ".zip".trim_end_matches('.'));
    }

    #[tokio::test]
    async fn zip_comum_vira_projeto_local_sem_sair_da_pasta() {
        let pai = dir_temp("zip-comum");
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (nome, txt) in [("main.tex", "\\documentclass{article}"), ("cap/a.tex", "capitulo"), ("../fora.tex", "perigo")] {
            z.start_file(nome, o).unwrap();
            z.write_all(txt.as_bytes()).unwrap();
        }
        let bytes = z.finish().unwrap().into_inner();
        let m = MetaRemota { id: "ZIP124".into(), nome: "124.zip".into(), mime: "application/zip".into(), ..Default::default() };
        let dir = abrir_como_arquivos(&pai, m.clone(), bytes.clone()).await.unwrap();
        assert!(dir.ends_with("124"));
        assert!(Path::new(&dir).join("main.tex").exists() && Path::new(&dir).join("cap").join("a.tex").exists());
        assert!(!pai.join("fora.tex").exists());
        // o zip em si nao vira vinculo (nao ha arquivo do Drive para cada item de dentro)
        assert!(ler_vinculos(Path::new(&dir)).is_empty());
        // escolher o mesmo zip de novo cria outra pasta, sem misturar
        let dir2 = abrir_como_arquivos(&pai, m, bytes).await.unwrap();
        assert!(dir2.ends_with("124 (2)"));
    }

    #[tokio::test]
    async fn arquivo_solto_vira_projeto_novo_ligado_ao_drive_e_zip_vazio_nao_deixa_pasta() {
        let pai = dir_temp("solto");
        let m = MetaRemota { id: "TEX1".into(), nome: "main.tex".into(), mime: "text/plain".into(), md5: "m".into(), versao: "r".into(), dono_sou_eu: true };
        let dir = abrir_como_arquivos(&pai, m, b"conteudo".to_vec()).await.unwrap();
        assert_eq!(fs::read(Path::new(&dir).join("main.tex")).unwrap(), b"conteudo");
        assert_eq!(ler_vinculos(Path::new(&dir))["main.tex"].id, "TEX1");

        let vazio = zip::ZipWriter::new(Cursor::new(Vec::new())).finish().unwrap().into_inner();
        let z = MetaRemota { id: "ZV".into(), nome: "vazio.zip".into(), mime: "application/zip".into(), ..Default::default() };
        assert!(abrir_como_arquivos(&pai, z, vazio).await.is_err());
        assert!(!pai.join("vazio").exists(), "nao pode sobrar pasta vazia");
    }

    // ---------- revisao alheia: a versao dela e mesclada, nada se perde ----------

    fn rv(id: &str, email: &str, eu: bool) -> Revisao {
        Revisao { id: id.into(), email: email.into(), sou_eu: eu }
    }

    #[test]
    fn escolhe_a_ultima_revisao_alheia_entre_a_anterior_e_a_nova() {
        let lista = vec![rv("1", "eu@x.com", true), rv("2", "ana@x.com", false), rv("3", "bia@x.com", false), rv("4", "eu@x.com", true)];
        assert_eq!(revisao_alheia_a_baixar(&lista, "1", "4", "eu@x.com").map(|r| r.id.as_str()), Some("3"));
        assert_eq!(revisao_alheia_a_baixar(&lista, "3", "4", "eu@x.com"), None);
        assert_eq!(revisao_alheia_a_baixar(&lista, "9", "4", "eu@x.com"), None);
    }

    #[test]
    fn mescla_com_a_revisao_alheia_junta_as_duas_sem_perder_nada() {
        let anterior = c(&[("a.tex", "base a"), ("b.tex", "base b"), ("d.tex", "base d")]);
        // o que este computador enviou por cima: editou a.tex e criou c.tex
        let enviado = c(&[("a.tex", "meu a"), ("b.tex", "base b"), ("c.tex", "meu c"), ("d.tex", "base d")]);
        // o que a outra pessoa gravou no meio: editou b.tex e apagou d.tex
        let dela = c(&[("a.tex", "base a"), ("b.tex", "b dela")]);
        let m = juntar_com_revisao_alheia(&anterior, &enviado, &dela, "eu@x.com");
        assert_eq!(m.final_, c(&[("a.tex", "meu a"), ("b.tex", "b dela"), ("c.tex", "meu c")]));
        assert!(m.conflitos.is_empty());
    }

    #[test]
    fn mesmo_arquivo_dos_dois_lados_na_revisao_alheia_vira_copia_de_conflito() {
        let anterior = c(&[("a.tex", "base")]);
        let enviado = c(&[("a.tex", "versao daqui")]);
        let dela = c(&[("a.tex", "versao dela")]);
        let m = juntar_com_revisao_alheia(&anterior, &enviado, &dela, "eu@x.com");
        // a dela fica no lugar e a de agora vira copia: as duas existem
        assert_eq!(m.final_["a.tex"], b"versao dela");
        assert_eq!(m.conflitos, vec!["a (conflito de eu).tex".to_string()]);
        assert_eq!(m.final_["a (conflito de eu).tex"], b"versao daqui");
        assert_eq!(m.final_.len(), 2);
    }

    // ---------- vinculo: releitura antes do PATCH ----------

    #[test]
    fn se_o_drive_mudou_entre_a_leitura_e_o_envio_vira_conflito() {
        use AcaoVinculo::*;
        assert_eq!(acao_apos_releitura(Enviar, ("m1", "r1"), ("m1", "r1")), Enviar);
        assert_eq!(acao_apos_releitura(Enviar, ("m1", "r1"), ("m2", "r2")), Conflito);
        assert_eq!(acao_apos_releitura(Enviar, ("", "r1"), ("", "r2")), Conflito);
        // so o md5 igual com revisao nova (metadados): continua enviando
        assert_eq!(acao_apos_releitura(Enviar, ("m1", "r1"), ("m1", "r9")), Enviar);
        // outras acoes nao mudam
        assert_eq!(acao_apos_releitura(Baixar, ("m1", "r1"), ("m2", "r2")), Baixar);
        assert_eq!(acao_apos_releitura(Nada, ("m1", "r1"), ("m2", "r2")), Nada);
    }

    #[test]
    fn falha_vai_para_falhas_e_tambem_para_avisos() {
        let mut r = ResultadoSync::default();
        registrar_falha(&mut r, "fig.png: sem acesso".into());
        assert_eq!(r.falhas, vec!["fig.png: sem acesso".to_string()]);
        assert_eq!(r.avisos, r.falhas);
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["falhas"][0], "fig.png: sem acesso");
    }

    #[test]
    fn colocar_arquivo_nao_sobrescreve_nem_sai_da_pasta() {
        let dir = dir_temp("colocar-exclusivo");
        fs::write(dir.join("a.tex"), "meu").unwrap();
        assert_eq!(colocar_arquivo(&dir, "a.tex", b"novo").unwrap(), "a (2).tex");
        assert_eq!(fs::read_to_string(dir.join("a.tex")).unwrap(), "meu");
        assert!(colocar_arquivo(&dir, "../fora.tex", b"x").is_err());
        assert!(!dir.parent().unwrap().join("fora.tex").exists());
    }

    #[test]
    fn estado_e_vinculos_gravam_por_arquivo_temporario_sem_deixar_lixo() {
        let dir = dir_temp("atomico");
        gravar_estado(&dir, &Estado { arquivo_id: "Z1".into(), ..Default::default() }).unwrap();
        let mut m = Vinculos::new();
        m.insert("a.tex".into(), Vinculo { id: "I1".into(), ..Default::default() });
        gravar_vinculos(&dir, &m).unwrap();
        assert_eq!(ler_estado(&dir).unwrap().arquivo_id, "Z1");
        assert_eq!(ler_vinculos(&dir), m);
        let sobras: Vec<_> = fs::read_dir(dir.join(".eguatex")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(sobras.iter().all(|n| !n.ends_with(".eguatex-tmp")), "{sobras:?}");
    }

    #[test]
    fn desligar_remove_estado_e_vinculos_e_projeto_com_vinculos_conta_como_do_drive() {
        let dir = dir_temp("desligar");
        let mut m = Vinculos::new();
        m.insert("a.tex".into(), v("m", "r", "s"));
        gravar_vinculos(&dir, &m).unwrap();
        let caminho = dir.display().to_string();
        assert_eq!(cmd_drive_projeto(caminho.clone()).as_deref(), Some("ID1"));
        cmd_drive_desligar(caminho.clone()).unwrap();
        assert_eq!(cmd_drive_projeto(caminho), None);
    }
}

#[cfg(test)]
#[path = "drive_real.rs"]
mod drive_real;

#[cfg(test)]
#[path = "drive_borda.rs"]
mod drive_borda;
