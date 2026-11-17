//! Testes do testador para a versao 42_API (chave de API do Gemini).
//! Cobrem bordas da chave, o Gemini contra servidor falso, conta Google antiga e configuracao.
//! So codigo de teste.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde_json::{Value, json};

use super::chave::{self, ChaveApi, chave_de_teste, cofre_de_teste};
use super::comum::MSG_SEM_REDE;
use super::comum::teste::{Gravador, Pedido, json as resp_json, porta_fechada, sem_cancelar, servidor};
use super::google;

const K_GEMINI: &str = "AIzaSy-SEGREDO_testador-987";
const MODELO: &str = "gemini-flash-lite-latest";

fn zerar() {
    let _ = chave::apagar();
    let _ = super::salvar_config(&super::ConfigIA::default());
}

fn dados() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("EGUATEX_DADOS").unwrap())
}

/// Todo arquivo dentro de EGUATEX_DADOS, em texto (lossy), para procurar a chave.
fn tudo_no_disco() -> Vec<(std::path::PathBuf, String)> {
    fn andar(p: &std::path::Path, saida: &mut Vec<(std::path::PathBuf, String)>) {
        if let Ok(lista) = std::fs::read_dir(p) {
            for e in lista.flatten() {
                let c = e.path();
                if c.is_dir() {
                    andar(&c, saida);
                } else if let Ok(b) = std::fs::read(&c) {
                    saida.push((c, String::from_utf8_lossy(&b).into_owned()));
                }
            }
        }
    }
    let mut v = Vec::new();
    andar(&dados(), &mut v);
    v
}

fn texto_cabecalho(c: &ChaveApi) -> String {
    c.cabecalho("").unwrap().to_str().unwrap().to_string()
}

// ---------- servidores crus: stream que para ou cai no meio ----------

fn ler_pedido(s: &std::net::TcpStream) -> Pedido {
    let mut leitor = BufReader::new(s.try_clone().unwrap());
    let mut linha = String::new();
    leitor.read_line(&mut linha).ok();
    let mut cabecalhos = Vec::new();
    let mut tamanho = 0usize;
    loop {
        let mut l = String::new();
        if leitor.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
            break;
        }
        if let Some((n, v)) = l.trim_end().split_once(':') {
            let n = n.trim().to_lowercase();
            if n == "content-length" {
                tamanho = v.trim().parse().unwrap_or(0);
            }
            cabecalhos.push((n, v.trim().to_string()));
        }
    }
    let mut corpo = vec![0u8; tamanho];
    leitor.read_exact(&mut corpo).ok();
    Pedido { linha: linha.trim_end().to_string(), cabecalhos, corpo: String::from_utf8_lossy(&corpo).into_owned() }
}

/// Responde 200 com Content-Length grande, manda `inicio` e entao: `cair` fecha a conexao
/// (corpo incompleto); senao fica calado por 30 s.
fn servidor_parcial(inicio: &str, cair: bool) -> (String, Receiver<Pedido>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let (tx, rx) = channel();
    let inicio = inicio.to_string();
    std::thread::spawn(move || {
        let Ok((mut s, _)) = l.accept() else { return };
        let _ = tx.send(ler_pedido(&s));
        let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 100000\r\n\r\n{inicio}");
        let _ = s.flush();
        if cair {
            let _ = s.shutdown(std::net::Shutdown::Both);
        } else {
            std::thread::sleep(Duration::from_secs(30));
        }
    });
    (base, rx)
}

const COMECO: &str = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Começo\"}]}}]}\n\n";

fn corpo_simples() -> Value {
    json!({"contents": [{"role": "user", "parts": [{"text": "oi"}]}], "tools": [{"functionDeclarations": [{"name": "ler_selecao", "description": "Le a selecao"}]}]})
}

// =====================================================================================
// 1. Chave: cofre, bordas e vazamento
// =====================================================================================

#[test]
fn chave_especial_longa_e_com_espacos_ida_e_volta() {
    let _vez = cofre_de_teste();
    zerar();
    let longa: String = "AIza-L".chars().chain(std::iter::repeat_n('x', 20_000)).chain("FIM".chars()).collect();
    let casos: Vec<(String, String)> = vec![
        (r#"AIza-a+b/c=d~!@#$%^&*()[]{}|;:'",.<>?`"#.into(), r#"AIza-a+b/c=d~!@#$%^&*()[]{}|;:'",.<>?`"#.into()),
        ("  \t AIza-com-espacos \r\n".into(), "AIza-com-espacos".into()),
        ("AIza-quebra-colada\n".into(), "AIza-quebra-colada".into()),
        ("\u{a0}AIza-nbsp\u{a0}".into(), "AIza-nbsp".into()),
        (longa.clone(), longa.clone()),
    ];
    for (colada, guardada) in &casos {
        let c = ChaveApi::nova(colada).unwrap();
        chave::salvar(&c).unwrap();
        let lida = chave::ler().unwrap();
        assert_eq!(&texto_cabecalho(&lida), guardada);
        assert!(chave::status());
        // Debug nunca mostra a chave
        assert_eq!(format!("{lida:?}"), "ChaveApi(***)");
        assert!(!format!("{:#?}", Some(&lida)).contains(&guardada[..guardada.len().min(12)]));
        // nada no disco guarda a chave em texto claro
        for (arquivo, texto) in tudo_no_disco() {
            assert!(!texto.contains(guardada.as_str()), "{} tem a chave", arquivo.display());
        }
    }
    chave::apagar().unwrap();
    assert!(chave::ler().is_none());
    assert!(!chave::status());
    zerar();
}

#[test]
fn chave_vazia_so_espacos_ou_so_quebra_nao_vale() {
    for vazia in ["", " ", "\n", "\r\n", "\t \n ", "\u{a0}"] {
        assert!(ChaveApi::nova(vazia).is_none(), "{vazia:?}");
    }
}

#[tokio::test]
async fn ativar_com_chave_vazia_e_invalida_sem_tocar_o_cofre() {
    let _vez = cofre_de_teste();
    zerar();
    for vazia in ["", "  \n\t"] {
        assert_eq!(chave::ativar_com(vazia, &porta_fechada(), Duration::from_secs(2)).await, "invalida");
    }
    assert!(!chave::status());
    assert!(!super::carregar_config().ativa);
}

#[tokio::test]
async fn chave_com_quebra_no_meio_e_invalida_e_nada_sai_do_aparelho() {
    let _vez = cofre_de_teste();
    zerar();
    // porta fechada: se tentasse a rede, o codigo seria "sem-internet" ou "tempo"
    for ruim in ["AIza-a\nb", "AIza-a\r\nb", "AIza-a\u{7f}b"] {
        let r = chave::ativar_com(ruim, &porta_fechada(), Duration::from_secs(10)).await;
        assert_eq!(r, "invalida", "{ruim:?}");
    }
    assert!(!chave::status());
    assert!(!super::carregar_config().ativa);
}

#[test]
fn chave_com_acento_ou_caractere_invisivel_nao_vira_cabecalho() {
    // contrato escrito em chave.rs (doc de `cabecalho`): acento devolve None
    for ruim in ["AIza-ção", "AIza-\u{200b}zero", "\u{feff}AIza-bom", "AIza-é"] {
        let c = ChaveApi::nova(ruim).unwrap();
        assert!(c.cabecalho("").is_none(), "{ruim:?} virou cabecalho HTTP e seria enviado ao Google");
    }
}

#[tokio::test]
async fn ativar_com_chave_especial_e_longa_grava_e_nao_deixa_a_chave_na_config() {
    let _vez = cofre_de_teste();
    zerar();
    // a maior chave aceita: 1280 caracteres (chave::LIMITE_CHAVE)
    let longa: String = std::iter::repeat_n('k', 1_271).collect::<String>() + "-AIza-FIM";
    assert_eq!(longa.len(), chave::LIMITE_CHAVE);
    for (colada, guardada) in [(format!("  {K_GEMINI}\r\n"), K_GEMINI.to_string()), (longa.clone(), longa.clone())] {
        let (base, pedidos) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
        assert_eq!(chave::ativar_com(&colada, &base, Duration::from_secs(5)).await, "ok");
        let pedido = pedidos.recv().unwrap();
        assert_eq!(pedido.tudo().matches(guardada.as_str()).count(), 1);
        assert_eq!(pedido.cabecalho("x-goog-api-key"), Some(guardada.as_str()));
        assert!(!pedido.linha.contains(&guardada[..10]) && !pedido.linha.contains("key="));
        let cfg_texto = std::fs::read_to_string(dados().join("eguatex").join("ia-config.json")).unwrap();
        assert!(!cfg_texto.contains(guardada.as_str()));
        let cfg: Value = serde_json::from_str(&cfg_texto).unwrap();
        assert_eq!(cfg["provedor"], "gemini");
        assert_eq!(cfg["ativa"], true);
        assert_eq!(texto_cabecalho(&chave::ler().unwrap()), guardada);
    }
    for (arquivo, texto) in tudo_no_disco() {
        assert!(!texto.contains(K_GEMINI) && !texto.contains(&longa), "{}", arquivo.display());
    }
    zerar();
}

#[tokio::test]
async fn ativar_de_novo_mantem_o_modelo_escolhido() {
    let _vez = cofre_de_teste();
    zerar();
    let cfg = super::ConfigIA { ativa: false, modelo: "gemini-flash-latest".into(), ..super::ConfigIA::default() };
    super::salvar_config(&cfg).unwrap();
    let (base, _p) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
    assert_eq!(chave::ativar_com(K_GEMINI, &base, Duration::from_secs(5)).await, "ok");
    let c = super::carregar_config();
    assert_eq!((c.provedor.as_str(), c.modelo.as_str(), c.ativa), ("gemini", "gemini-flash-latest", true));
    zerar();
}

// =====================================================================================
// 2. Gemini contra servidor falso
// =====================================================================================

async fn erro_de(status: u16, corpo: &str, chave: &str) -> (String, Pedido, Gravador) {
    let (base, pedidos) = servidor(vec![resp_json(status, corpo), resp_json(status, corpo), resp_json(status, corpo)]);
    let mut rx = sem_cancelar();
    let tela = Gravador::default();
    let erro = google::chamar_gemini_stream(&tela, "t-erro", MODELO, &corpo_simples(), &mut rx, &chave_de_teste(chave), &base).await.unwrap_err();
    (erro, pedidos.recv().unwrap(), tela)
}

#[tokio::test]
async fn gemini_400_401_403_e_429_sem_a_chave_e_so_no_x_goog_api_key() {
    // 404 e 5xx marcam o modelo como lotado por 10 min (estado global) e quebrariam outros
    // testes que dependem da ordem dos modelos; ficam de fora deste teste HTTP.
    let invalida = format!(r#"{{"error":{{"code":400,"message":"API key not valid: {K_GEMINI}","details":[{{"reason":"API_KEY_INVALID"}}]}}}}"#);
    let generico = format!(r#"{{"error":{{"code":400,"message":"pedido ruim {K_GEMINI}"}}}}"#);
    let casos = [
        (400, invalida, "foi recusada"),
        (400, generico, "erro 400"),
        (401, format!(r#"{{"error":{{"code":401,"message":"{K_GEMINI}"}}}}"#), "foi recusada"),
        (403, format!(r#"{{"error":{{"code":403,"message":"denied {K_GEMINI}"}}}}"#), "não tem permissão"),
        (429, format!(r#"{{"error":{{"code":429,"status":"RESOURCE_EXHAUSTED","message":"{K_GEMINI}"}}}}"#), "Limite"),
    ];
    for (status, corpo, trecho) in casos {
        let (erro, p, tela) = erro_de(status, &corpo, K_GEMINI).await;
        let erro = chave_de_teste(K_GEMINI).limpar(&erro);
        assert!(erro.contains(trecho), "{status}: {erro}");
        assert!(!erro.contains(K_GEMINI) && !erro.contains("SEGREDO"), "{status}: {erro}");
        assert!(!erro.contains('\u{2014}') && !erro.contains('\u{2013}'));
        assert_eq!(p.cabecalho("x-goog-api-key"), Some(K_GEMINI));
        assert!(p.cabecalho("authorization").is_none() && p.cabecalho("x-goog-user-project").is_none());
        assert!(!p.linha.contains("key="));
        assert_eq!(p.tudo().matches(K_GEMINI).count(), 1);
        assert!(!format!("{:?}", tela.0.lock().unwrap()).contains(K_GEMINI));
    }
}

#[tokio::test]
async fn gemini_402_e_529_viram_mensagem_em_portugues() {
    // nenhum dos dois marca modelo como lotado: o Gemini devolve logo o erro
    let casos = [
        (402, format!(r#"{{"error":{{"code":402,"message":"Payment required {K_GEMINI}"}}}}"#)),
        (529, format!(r#"{{"error":{{"code":529,"message":"The model is overloaded {K_GEMINI}"}}}}"#)),
    ];
    for (status, corpo) in casos {
        let (erro, _, _) = erro_de(status, &corpo, K_GEMINI).await;
        let erro = chave_de_teste(K_GEMINI).limpar(&erro);
        assert!(!erro.contains(K_GEMINI), "{status}: {erro}");
        assert!(erro.contains("Gemini"), "status {status} chegou sem contexto em portugues: {erro:?}");
    }
}

#[tokio::test]
async fn sem_internet_no_gemini() {
    let mut rx = sem_cancelar();
    let erro = google::chamar_gemini_stream(&Gravador::default(), "t-rede", MODELO, &corpo_simples(), &mut rx, &chave_de_teste("k-rede"), &porta_fechada())
        .await
        .unwrap_err();
    assert_eq!(erro, MSG_SEM_REDE);
}

#[tokio::test]
async fn conexao_cai_no_meio_do_stream() {
    let (base, _p) = servidor_parcial(COMECO, true);
    let tela = Gravador::default();
    let mut rx = sem_cancelar();
    let erro = google::chamar_gemini_stream(&tela, "t-cai", MODELO, &corpo_simples(), &mut rx, &chave_de_teste("k"), &base).await.unwrap_err();
    assert!(erro.contains("caiu no meio"), "{erro}");
    assert_eq!(tela.texto(), "Começo");
}

#[tokio::test]
async fn cancelar_no_meio_do_stream() {
    let id = "t-cancelar-meio";
    let (base, _p) = servidor_parcial(COMECO, false);
    let mut rx = google::registrar_conversa(id);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        google::cancelar_agente(id);
    });
    let tela = Gravador::default();
    let comeco = std::time::Instant::now();
    let erro = google::chamar_gemini_stream(&tela, id, MODELO, &corpo_simples(), &mut rx, &chave_de_teste("k"), &base).await.unwrap_err();
    google::liberar_conversa(id);
    assert_eq!(erro, google::MSG_INTERROMPIDO);
    assert_eq!(tela.texto(), "Começo");
    assert!(comeco.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn erro_dentro_do_stream_que_ecoa_a_chave_sai_mascarado() {
    let stream = format!("data: {}\n\n", json!({"error": {"code": 400, "message": format!("ruim {K_GEMINI}")}}));
    let (base, _p) = servidor(vec![super::comum::teste::sse(&stream)]);
    let mut rx = sem_cancelar();
    let chave = chave_de_teste(K_GEMINI);
    let erro = google::chamar_gemini_stream(&Gravador::default(), "t-stream-erro", MODELO, &corpo_simples(), &mut rx, &chave, &base).await.unwrap_err();
    // o agente passa todo erro por `limpar` antes de devolver a tela
    let erro = chave.limpar(&erro);
    assert!(!erro.contains(K_GEMINI) && erro.contains("***"), "{erro}");
    assert!(!erro.contains("erro 0"), "{erro}");
}

#[tokio::test]
async fn assinatura_do_gemini_volta_intacta_no_pedido_seguinte() {
    // historico com chamada de ferramenta e thoughtSignature, como o ChatIA.svelte guarda
    let historico = json!({"contents": [
        {"role": "user", "parts": [{"text": "Leia a selecao"}]},
        {"role": "model", "parts": [{"functionCall": {"id": "g-1", "name": "ler_selecao", "args": {}}, "thoughtSignature": "SIG-GEMINI"}]},
        {"role": "user", "parts": [{"functionResponse": {"id": "g-1", "name": "ler_selecao", "response": {"texto": "x"}}}]}
    ]});
    let stream = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Fim.\"}]},\"finishReason\":\"STOP\"}]}\n\n";
    let (base, pedidos) = servidor(vec![super::comum::teste::sse(stream)]);
    let mut rx = sem_cancelar();
    let r = google::chamar_gemini_stream(&Gravador::default(), "t-assinatura", MODELO, &historico, &mut rx, &chave_de_teste("k"), &base).await.unwrap();
    assert_eq!(r["parts"][0]["text"], "Fim.");
    let enviado = pedidos.recv().unwrap().json();
    assert_eq!(enviado["contents"][1]["parts"][0]["thoughtSignature"], "SIG-GEMINI");
    assert_eq!(enviado["contents"][1]["parts"][0]["functionCall"]["id"], "g-1");
    assert_eq!(enviado["contents"][2], historico["contents"][2]);
}

// =====================================================================================
// 3. Login Google
// =====================================================================================

#[test]
fn login_pede_exatamente_openid_email_profile_e_drive_file() {
    let fonte = include_str!("google.rs").replace("\r\n", "\n");
    let i = fonte.find("const ESCOPOS_BASE: &str = \"").unwrap() + "const ESCOPOS_BASE: &str = \"".len();
    let escopos: Vec<&str> = fonte[i..i + fonte[i..].find('"').unwrap()].split_whitespace().collect();
    assert_eq!(escopos, vec!["openid", "email", "profile", "https://www.googleapis.com/auth/drive.file"]);
    let producao = &fonte[..fonte.find("#[cfg(test)]\nmod ").unwrap()];
    // "auth/drive" inteiro aparece so na migracao de contas antigas (tem_drive_inteiro); o pedido e ESCOPOS_BASE acima
    for proibido in ["include_granted_scopes", "preparar_projeto", "cloud-platform", "generative-language.retriever"] {
        assert!(!producao.contains(proibido), "{proibido}");
    }
    // o unico login de conta usa ESCOPOS_BASE
    assert_eq!(producao.matches("autorizar(app, ").count(), 1);
    assert!(producao.contains("autorizar(app, ESCOPOS_BASE,"));
}

#[tokio::test]
async fn conta_da_v41_no_cofre_carrega_o_drive_segue_e_a_chave_nao_a_toca() {
    let _vez = cofre_de_teste();
    zerar();
    if google::CONTA_TESTE.lock().unwrap().is_some() {
        return; // outro teste do Drive usa a conta de teste neste processo
    }
    let v41 = format!(
        r#"{{"access_token":"tok-v41","refresh_token":"r41","expira_em":{},"email":"pessoa@exemplo.com","nome":"P","foto":"","projeto":"eguatex-abc123","escopos":"openid email profile https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language.retriever https://www.googleapis.com/auth/drive.file"}}"#,
        google::agora() + 3600
    );
    let entrada = keyring::Entry::new("eguatex", "google-oauth").unwrap();
    entrada.set_password(&v41).unwrap();
    let info = google::info();
    assert!(info.conectado, "conta da v41 tem de continuar conectada");
    assert_eq!(info.email, "pessoa@exemplo.com");
    assert_eq!(google::token_acesso().await.unwrap(), "tok-v41", "o Drive usa o token da conta antiga");
    // ativar e remover a chave nao mexe na conta Google
    let (base, _p) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
    assert_eq!(chave::ativar_com(K_GEMINI, &base, Duration::from_secs(5)).await, "ok");
    chave::remover().unwrap();
    assert_eq!(entrada.get_password().unwrap(), v41);
    entrada.delete_credential().unwrap();
    zerar();
}

// =====================================================================================
// 4. Configuracao
// =====================================================================================

#[test]
fn config_ausente_corrompida_ou_com_campos_a_mais_carrega_desligada_ou_igual() {
    let _vez = cofre_de_teste();
    let caminho = dados().join("eguatex").join("ia-config.json");
    std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&caminho);
    assert!(!super::carregar_config().ativa, "sem arquivo: desligada");
    std::fs::write(&caminho, "{ corrompido").unwrap();
    assert!(!super::carregar_config().ativa, "arquivo corrompido: desligada");
    std::fs::write(&caminho, "").unwrap();
    assert!(!super::carregar_config().ativa);
    std::fs::write(&caminho, r#"{"ativa":true,"provedor":"gemini","modelo":"gemini-flash-latest","ollama_url":"","campo_futuro":123}"#).unwrap();
    let c = super::carregar_config();
    assert!(c.ativa);
    assert_eq!((c.provedor.as_str(), c.modelo.as_str()), ("gemini", "gemini-flash-latest"));
    // config antiga com outro provedor ativo: vira gemini com modelo automatico; sem chave a IA nao chama nada
    std::fs::write(&caminho, r#"{"ativa":true,"provedor":"ollama","modelo":"llama3","ollama_url":"http://localhost:11434"}"#).unwrap();
    let c = super::carregar_config();
    assert_eq!((c.provedor.as_str(), c.modelo.as_str()), ("gemini", ""));
    super::salvar_config(&super::ConfigIA::default()).unwrap();
}

#[tokio::test]
async fn sem_chave_ou_ia_desligada_o_agente_nao_manda_nada() {
    let _vez = cofre_de_teste();
    zerar();
    let tela = Gravador::default();
    let erro = google::chamar_agente(&tela, "t-desligada", "", json!({"contents": []})).await.unwrap_err();
    assert_eq!(erro, google::MSG_DESLIGADA);
    super::salvar_config(&super::ConfigIA { ativa: true, ..super::ConfigIA::default() }).unwrap();
    let erro = google::chamar_agente(&tela, "t-sem-chave", "", json!({"contents": []})).await.unwrap_err();
    assert_eq!(erro, super::comum::MSG_SEM_CHAVE);
    assert!(tela.0.lock().unwrap().is_empty());
    zerar();
}

#[tokio::test]
async fn remover_a_chave_em_uso_grava_ativa_false_no_arquivo() {
    let _vez = cofre_de_teste();
    zerar();
    chave::salvar(&chave_de_teste("k-rem")).unwrap();
    super::salvar_config(&super::ConfigIA { ativa: true, ..super::ConfigIA::default() }).unwrap();
    chave::remover().unwrap();
    let disco: Value = serde_json::from_str(&std::fs::read_to_string(dados().join("eguatex").join("ia-config.json")).unwrap()).unwrap();
    assert_eq!(disco["ativa"], false);
    assert_eq!(disco["provedor"], "gemini");
    // e o agente nao manda nada
    let erro = google::chamar_agente(&Gravador::default(), "t-rem", "", json!({"contents": []})).await.unwrap_err();
    assert_eq!(erro, google::MSG_DESLIGADA);
    zerar();
}

// =====================================================================================
// 5. Testador, rodada de 15/11: classificacao completa, provedores antigos, tres rodadas com
//    ferramenta e assinatura, e o limite do Gerenciador de Credenciais
// =====================================================================================

#[test]
fn classificacao_de_cada_status_e_mensagem_em_portugues() {
    use super::comum::{Codigo, mensagem};
    let invalida = r#"{"error":{"code":400,"message":"API key not valid.","details":[{"reason":"API_KEY_INVALID"}]}}"#;
    let casos = [
        (400, invalida, Codigo::Invalida, "invalida"),
        (400, r#"{"error":{"message":"pedido ruim"}}"#, Codigo::Erro, "erro"),
        (401, "{}", Codigo::Invalida, "invalida"),
        (403, "{}", Codigo::Bloqueada, "bloqueada"),
        (404, "{}", Codigo::Indisponivel, "erro"),
        (429, "{}", Codigo::Limite, "limite"),
        (500, "{}", Codigo::Ocupado, "ocupado"),
        (502, "", Codigo::Ocupado, "ocupado"),
        (503, "{}", Codigo::Ocupado, "ocupado"),
        (529, "{}", Codigo::Ocupado, "ocupado"),
    ];
    for (status, corpo, codigo, texto) in casos {
        assert_eq!(google::classificar_gemini(status, corpo), codigo, "{status} {corpo}");
        assert_eq!(codigo.texto(), texto);
        let m = mensagem(codigo, status, "detalhe");
        assert!(m.contains("Gemini"), "{status}: {m}");
        assert!(!m.contains('\u{2014}') && !m.contains('\u{2013}'), "{status}: {m}");
        assert!(!m.contains("erro 0"), "{status}: {m}");
    }
    assert_eq!(mensagem(Codigo::SemInternet, 0, ""), MSG_SEM_REDE);
    assert!(mensagem(Codigo::Tempo, 0, "").contains("demorou demais"));
}

#[tokio::test]
async fn teste_da_chave_com_529_e_404_nos_dois_modelos() {
    let _vez = cofre_de_teste();
    zerar();
    // 529: ocupado, sem trocar de modelo e sem gravar nada
    let (base, pedidos) = servidor(vec![resp_json(529, r#"{"error":{"code":529,"message":"overloaded"}}"#)]);
    assert_eq!(chave::ativar_com(K_GEMINI, &base, Duration::from_secs(5)).await, "ocupado");
    assert!(pedidos.recv().unwrap().linha.contains("gemini-flash-lite-latest"));
    assert!(!chave::status());
    // 404 no Flash Lite e no Flash: a frente recebe "erro" e nada fica guardado
    let nf = r#"{"error":{"code":404,"message":"not found"}}"#;
    let (base, pedidos) = servidor(vec![resp_json(404, nf), resp_json(404, nf)]);
    assert_eq!(chave::ativar_com(K_GEMINI, &base, Duration::from_secs(5)).await, "erro");
    assert!(pedidos.recv().unwrap().linha.contains("gemini-flash-lite-latest"));
    assert!(pedidos.recv().unwrap().linha.contains("gemini-flash-latest:generateContent"));
    assert!(!chave::status());
    assert!(!super::carregar_config().ativa);
    zerar();
}

#[tokio::test]
async fn config_de_provedor_antigo_carrega_como_gemini_e_sem_chave_nada_sai() {
    let _vez = cofre_de_teste();
    zerar();
    let caminho = dados().join("eguatex").join("ia-config.json");
    std::fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    // nomes montados aos pedacos: o grep de restos dos provedores antigos continua limpo
    let antigos = [format!("open{}", "ai"), "ollama".to_string()];
    for antigo in &antigos {
        std::fs::write(&caminho, format!(r#"{{"ativa":true,"provedor":"{antigo}","modelo":"modelo-antigo","ollama_url":"http://localhost:11434"}}"#)).unwrap();
        let c = super::carregar_config();
        assert_eq!(c.provedor, "gemini", "{antigo}");
        assert_eq!(c.modelo, "", "{antigo}: modelo de outro provedor nao vai ao Gemini");
        // sem chave no cofre o agente para antes da rede: nenhum evento, erro de chave faltando
        let tela = Gravador::default();
        let erro = google::chamar_agente(&tela, "t-antigo", &c.modelo, json!({"contents": [{"role": "user", "parts": [{"text": "oi"}]}]})).await.unwrap_err();
        assert_eq!(erro, super::comum::MSG_SEM_CHAVE, "{antigo}");
        assert!(tela.0.lock().unwrap().is_empty());
    }
    zerar();
}

#[tokio::test]
async fn tres_rodadas_com_ferramenta_e_assinatura_voltam_intactas() {
    // Faz o mesmo laco do ChatIA.svelte: cada rodada guarda a resposta inteira (sem modeloUsado)
    // e devolve o resultado da ferramenta casado pelo id. A quarta rodada responde em texto.
    let rodada = |n: usize| {
        format!(
            "data: {}\n\n",
            json!({"candidates": [{"content": {"role": "model", "parts": [
                {"text": format!("pensando {n}"), "thought": true},
                {"functionCall": {"id": format!("f-{n}"), "name": "ler_arquivo", "args": {"caminho": format!("cap{n}.tex")}}, "thoughtSignature": format!("SIG-{n}")}
            ]}, "finishReason": "STOP"}]})
        )
    };
    let fim = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Pronto.\",\"thoughtSignature\":\"SIG-FIM\"}]},\"finishReason\":\"STOP\"}]}\n\n";
    let (base, pedidos) = servidor(vec![
        super::comum::teste::sse(&rodada(1)),
        super::comum::teste::sse(&rodada(2)),
        super::comum::teste::sse(&rodada(3)),
        super::comum::teste::sse(fim),
    ]);
    let mut historico = vec![json!({"role": "user", "parts": [{"text": "Revise os capitulos"}]})];
    let mut rx = sem_cancelar();
    let chave = chave_de_teste(K_GEMINI);
    for n in 1..=4 {
        let corpo = json!({"contents": historico, "tools": corpo_simples()["tools"]});
        let tela = Gravador::default();
        let mut r = google::chamar_gemini_stream(&tela, "t-3-rodadas", MODELO, &corpo, &mut rx, &chave, &base).await.unwrap();
        let enviado = pedidos.recv().unwrap();
        assert_eq!(enviado.cabecalho("x-goog-api-key"), Some(K_GEMINI));
        assert_eq!(enviado.tudo().matches(K_GEMINI).count(), 1);
        // o pedido n leva todas as assinaturas anteriores, cada uma na sua chamada
        let contents = enviado.json()["contents"].clone();
        assert_eq!(contents, Value::Array(historico.clone()), "rodada {n}");
        for k in 1..n {
            let modelo = &contents[2 * k - 1];
            assert_eq!(modelo["role"], "model");
            assert_eq!(modelo["parts"][1]["thoughtSignature"], format!("SIG-{k}"));
            assert_eq!(modelo["parts"][1]["functionCall"]["id"], format!("f-{k}"));
            assert_eq!(contents[2 * k]["parts"][0]["functionResponse"]["id"], format!("f-{k}"));
        }
        // o pensamento nao vai para a tela
        assert!(!tela.texto().contains("pensando"));
        r.as_object_mut().unwrap().remove("modeloUsado");
        let partes = r["parts"].as_array().unwrap().clone();
        if n < 4 {
            let chamada = partes.iter().find(|p| p.get("functionCall").is_some()).unwrap();
            assert_eq!(chamada["thoughtSignature"], format!("SIG-{n}"));
            historico.push(json!({"role": "model", "parts": partes}));
            historico.push(json!({"role": "user", "parts": [{"functionResponse": {"id": format!("f-{n}"), "name": "ler_arquivo", "response": {"texto": format!("conteudo {n}")}}}]}));
        } else {
            assert_eq!(partes.len(), 1);
            assert_eq!(partes[0]["text"], "Pronto.");
            assert_eq!(partes[0]["thoughtSignature"], "SIG-FIM");
            assert_eq!(tela.texto(), "Pronto.");
        }
    }
}

/// O Ativar recusa chave com mais de 1280 caracteres (o limite do cofre do Windows) antes de
/// tocar a rede; 1280 caracteres passam.
#[tokio::test]
async fn chave_maior_que_o_limite_e_recusada_antes_da_rede_e_nao_deixa_lixo() {
    let _vez = cofre_de_teste();
    zerar();
    let no_limite: String = "AIza".chars().chain(std::iter::repeat_n('a', 1276)).collect();
    assert_eq!(no_limite.len(), chave::LIMITE_CHAVE);
    let um_a_mais = format!("{no_limite}b");
    let enorme: String = "AIza-E".chars().chain(std::iter::repeat_n('y', 3_000)).collect();
    // 1281 e 3000 caracteres, com o Gemini aceitando: "invalida" e nenhum pedido chega ao
    // servidor falso; nada no cofre, IA desligada e nada no disco
    for grande in [&um_a_mais, &enorme] {
        let (base, pedidos) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
        assert_eq!(chave::ativar_com(grande, &base, Duration::from_secs(5)).await, "invalida");
        assert!(pedidos.recv_timeout(Duration::from_millis(300)).is_err(), "{} caracteres", grande.len());
        assert!(!chave::status());
        assert!(!super::carregar_config().ativa);
    }
    for (arquivo, t) in tudo_no_disco() {
        assert!(!t.contains("yyyyyyyyyyyyyyyyyyyy"), "{}", arquivo.display());
    }
    // 1280 caracteres: o pedido sai e a chave e gravada
    let (base, pedidos) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
    assert_eq!(chave::ativar_com(&no_limite, &base, Duration::from_secs(5)).await, "ok");
    assert!(pedidos.recv_timeout(Duration::from_secs(2)).is_ok());
    assert_eq!(texto_cabecalho(&chave::ler().unwrap()), no_limite);
    zerar();
}
