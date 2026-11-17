//! Chave de API do Gemini (Google AI Studio), colada pela propria pessoa.
//! A chave fica so no Gerenciador de Credenciais do Windows (crate `keyring`), no servico "eguatex"
//! e no usuario "gemini" (entrada "gemini.eguatex", que o desinstalador apaga). Ela nunca vai
//! para ia-config.json, arquivo de projeto, log, mensagem de erro ou URL: o tipo `ChaveApi` nao
//! se imprime, nao se serializa e so sai daqui dentro de um cabecalho HTTP marcado como sensivel.

use std::time::Duration;

use reqwest::header::HeaderValue;

use super::comum::Codigo;

#[cfg_attr(test, allow(dead_code))]
const SERVICO_COFRE: &str = "eguatex";
/// Usuario do cofre e nome do provedor no ia-config.json.
pub const GEMINI: &str = "gemini";
/// Limite do teste da chave.
const LIMITE_TESTE: Duration = Duration::from_secs(20);
/// Maior chave que o cofre guarda: o Gerenciador de Credenciais do Windows aceita 2560 bytes por
/// senha e o keyring grava em UTF-16, entao cabem 1280 caracteres ASCII. Chave maior e recusada
/// antes de qualquer pedido de rede.
pub const LIMITE_CHAVE: usize = 1280;
/// Codigo do Ativar quando o Gemini aceitou a chave e o cofre nao a gravou.
pub const COFRE: &str = "cofre";
/// Chave de API. Sem Debug, Display, Clone ou Serialize derivados: o texto so sai por
/// `cabecalho` (marcado como sensivel) e por `limpar`, que a troca por `***`.
pub struct ChaveApi(String);

impl std::fmt::Debug for ChaveApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChaveApi(***)")
    }
}

impl ChaveApi {
    /// Chave colada pela pessoa, sem espacos e quebras de linha nas pontas. Vazia: `None`.
    pub fn nova(texto: &str) -> Option<ChaveApi> {
        let limpo = texto.trim();
        (!limpo.is_empty()).then(|| ChaveApi(limpo.to_string()))
    }

    /// Valor do cabecalho HTTP com a chave (`prefixo` + chave), marcado como sensivel: o reqwest
    /// e o hyper nao o mostram em log nem em Debug. Chave com byte fora do ASCII visivel (acento,
    /// espaco ou quebra de linha no meio, U+200B, BOM, NBSP) devolve `None`: o Google nao emite
    /// chave assim, e o HeaderValue aceitaria parte desses bytes.
    pub fn cabecalho(&self, prefixo: &str) -> Option<HeaderValue> {
        if !self.valida() {
            return None;
        }
        let mut valor = HeaderValue::from_str(&format!("{prefixo}{}", self.0)).ok()?;
        valor.set_sensitive(true);
        Some(valor)
    }

    /// Todos os bytes no ASCII visivel (de `!` a `~`).
    pub fn valida(&self) -> bool {
        !self.0.is_empty() && self.0.bytes().all(|b| b.is_ascii_graphic())
    }

    /// Troca a chave por `***` em `texto`.
    pub fn limpar(&self, texto: &str) -> String {
        limpar(texto, self)
    }
}

/// Troca a chave por `***` em todo o texto. Vale para toda mensagem de erro que sai do modulo
/// de IA: um servidor que ecoe a chave no corpo do erro nao a faz chegar a tela.
pub fn limpar(texto: &str, chave: &ChaveApi) -> String {
    if chave.0.is_empty() {
        return texto.to_string();
    }
    texto.replace(&chave.0, "***")
}

/// Servico do cofre. Nos testes, um servico so deste processo (`eguatex-teste-<aleatorio>`), para
/// nunca tocar o login Google nem a chave reais guardados no Windows.
pub(crate) fn servico_cofre() -> &'static str {
    #[cfg(test)]
    {
        static SERVICO: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        SERVICO.get_or_init(|| format!("eguatex-teste-{}", super::google::texto_aleatorio(8)))
    }
    #[cfg(not(test))]
    {
        SERVICO_COFRE
    }
}

fn entrada() -> Result<keyring::Entry, String> {
    keyring::Entry::new(servico_cofre(), GEMINI).map_err(|_| "Erro ao acessar o cofre de credenciais.".to_string())
}

/// So nos testes: faz `salvar` falhar, como um cofre que nao grava.
#[cfg(test)]
pub(crate) static COFRE_QUEBRADO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Grava a chave no cofre. O erro nunca traz a chave.
pub fn salvar(chave: &ChaveApi) -> Result<(), String> {
    #[cfg(test)]
    if COFRE_QUEBRADO.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Erro ao gravar a chave no cofre: cofre de teste quebrado".into());
    }
    entrada()?
        .set_password(&chave.0)
        .map_err(|e| limpar(&format!("Erro ao gravar a chave no cofre: {e}"), chave))
}

/// Chave guardada, ou `None` se nao houver (ou o cofre nao abrir).
pub fn ler() -> Option<ChaveApi> {
    let texto = entrada().ok()?.get_password().ok()?;
    ChaveApi::nova(&texto)
}

/// Apaga a chave. Sem chave guardada tambem da certo.
pub fn apagar() -> Result<(), String> {
    match entrada()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Erro ao apagar a chave do cofre.".into()),
    }
}

/// `cmd_ia_chave_status`: ha chave guardada? (nunca a chave)
pub fn status() -> bool {
    ler().is_some()
}

/// `cmd_ia_chave_ativar`: testa a chave e so a grava quando deu certo ou quando a conta esta
/// no limite (a chave vale, falta cota). Grava tambem a configuracao: IA ligada.
/// Devolve o codigo ("ok", "invalida", "sem-internet", "limite", "tempo", "ocupado",
/// "bloqueada", "erro", "cofre"); o texto vem do i18n.
pub async fn ativar(chave: &str) -> String {
    ativar_com(chave, super::google::URL_GEMINI, LIMITE_TESTE).await
}

pub(crate) async fn ativar_com(texto: &str, base: &str, limite: Duration) -> String {
    // chave vazia, com caractere fora do ASCII visivel ou maior que o cofre: recusada sem tocar
    // a rede (ASCII: um byte por caractere)
    let Some(chave) = ChaveApi::nova(texto).filter(|c| c.valida() && c.0.len() <= LIMITE_CHAVE) else {
        return Codigo::Invalida.texto().into();
    };
    let codigo = super::google::testar_chave(&chave, base, limite).await;
    if matches!(codigo, Codigo::Ok | Codigo::Limite) {
        if salvar(&chave).is_err() {
            return COFRE.into();
        }
        let mut cfg = super::carregar_config();
        cfg.provedor = GEMINI.into();
        cfg.ativa = true;
        if super::salvar_config(&cfg).is_err() {
            return Codigo::Erro.texto().into();
        }
    }
    codigo.texto().into()
}

/// `cmd_ia_chave_remover`: apaga a chave e desliga a IA.
pub fn remover() -> Result<(), String> {
    apagar()?;
    let mut cfg = super::carregar_config();
    if cfg.ativa {
        cfg.ativa = false;
        super::salvar_config(&cfg)?;
    }
    Ok(())
}

/// So nos testes: ia-config.json numa pasta temporaria (EGUATEX_DADOS) e o cofre no servico de
/// teste (`servico_cofre`), nunca nos dados reais. A variavel e global do processo: e definida
/// uma vez e os testes que gravam no cofre ou na configuracao rodam um por vez, segurando a trava
/// devolvida. Cada teste apaga no fim o que gravou no cofre.
#[cfg(test)]
pub(crate) fn cofre_de_teste() -> std::sync::MutexGuard<'static, ()> {
    static TRAVA: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static PASTA: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let guarda = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
    PASTA.get_or_init(|| {
        let pasta = std::env::temp_dir().join(format!("eguatex-teste-chaves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        // SAFETY: definida uma vez, antes de qualquer teste do cofre ler a variavel.
        unsafe { std::env::set_var("EGUATEX_DADOS", &pasta) };
        pasta
    });
    guarda
}

#[cfg(test)]
pub(crate) fn chave_de_teste(texto: &str) -> ChaveApi {
    ChaveApi(texto.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::ia::comum::teste::{json as resp_json, porta_fechada, servidor};

    fn zerar() {
        let _ = apagar();
        let _ = crate::ia::salvar_config(&crate::ia::ConfigIA::default());
    }

    #[test]
    fn salva_le_apaga_e_nao_ha_mais_nada() {
        let _vez = cofre_de_teste();
        let chave = ChaveApi::nova("  AIza-teste-123  ").unwrap();
        salvar(&chave).unwrap();
        assert_eq!(ler().unwrap().0, "AIza-teste-123");
        assert!(status());
        // a chave fica no servico de teste do Gerenciador de Credenciais, nunca no real
        assert!(servico_cofre().starts_with("eguatex-teste-"));
        assert_eq!(keyring::Entry::new(servico_cofre(), "gemini").unwrap().get_password().unwrap(), "AIza-teste-123");
        apagar().unwrap();
        assert!(ler().is_none());
        assert!(!status());
        // apagar de novo, sem chave, tambem da certo
        apagar().unwrap();
        // a conta Google (usuario google-oauth) nao e tocada pela chave
        assert!(keyring::Entry::new(servico_cofre(), "google-oauth").unwrap().get_password().is_err());
    }

    #[test]
    fn debug_nao_mostra_a_chave() {
        let chave = chave_de_teste("AIza-segredo");
        let texto = format!("{chave:?}");
        assert_eq!(texto, "ChaveApi(***)");
        assert!(!format!("{:#?}", Some(&chave)).contains("segredo"));
    }

    #[test]
    fn limpar_troca_a_chave_em_todo_o_texto() {
        let chave = chave_de_teste("AIzaSEGREDO");
        assert_eq!(limpar("erro: AIzaSEGREDO e de novo AIzaSEGREDO", &chave), "erro: *** e de novo ***");
        assert_eq!(chave.limpar("nada aqui"), "nada aqui");
    }

    #[test]
    fn cabecalho_e_sensivel_e_recusa_chave_com_quebra() {
        let chave = chave_de_teste("abc");
        let valor = chave.cabecalho("").unwrap();
        assert!(valor.is_sensitive());
        assert_eq!(valor.to_str().unwrap(), "abc");
        assert!(!format!("{valor:?}").contains("abc"));
        assert!(chave_de_teste("a\nb").cabecalho("").is_none());
    }

    #[test]
    fn chave_fora_do_ascii_visivel_nao_vale_e_as_pontas_sao_aparadas() {
        for ruim in ["AIza\u{a0}b", "AIza b", "AIza\tb", "AIzaç", "\u{feff}AIza"] {
            let c = ChaveApi::nova(ruim).unwrap();
            assert!(!c.valida() && c.cabecalho("").is_none(), "{ruim:?}");
        }
        let c = ChaveApi::nova("\u{a0} AIza-boa\r\n").unwrap();
        assert!(c.valida());
        assert_eq!(c.cabecalho("").unwrap().to_str().unwrap(), "AIza-boa");
        assert!(ChaveApi::nova("   ").is_none());
    }

    #[tokio::test]
    async fn ativar_grava_so_quando_o_teste_da_certo_ou_da_limite() {
        let _vez = cofre_de_teste();
        zerar();
        let limite = Duration::from_secs(5);
        // chave recusada: nada gravado, IA continua desligada
        let invalida = r#"{"error":{"code":400,"message":"API key not valid.","details":[{"reason":"API_KEY_INVALID"}]}}"#;
        let (base, _p) = servidor(vec![resp_json(400, invalida)]);
        assert_eq!(ativar_com("AIza-ruim", &base, limite).await, "invalida");
        assert!(!status());
        assert!(!crate::ia::carregar_config().ativa);
        // sem internet: nada gravado
        assert_eq!(ativar_com("AIza-x", &porta_fechada(), limite).await, "sem-internet");
        assert!(!status());
        // chave vazia: invalida sem tocar a rede
        assert_eq!(ativar_com("   ", "http://127.0.0.1:1", limite).await, "invalida");
        // limite: a chave vale e e gravada
        let (base, _p) = servidor(vec![resp_json(429, r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED"}}"#)]);
        assert_eq!(ativar_com("AIza-cota", &base, limite).await, "limite");
        assert!(status());
        // deu certo: grava e liga a IA
        let (base, _p) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
        assert_eq!(ativar_com("  AIza-boa  ", &base, limite).await, "ok");
        assert_eq!(ler().unwrap().limpar("AIza-boa"), "***");
        let cfg = crate::ia::carregar_config();
        assert!(cfg.ativa);
        assert_eq!(cfg.provedor, "gemini");
        // a configuracao em disco nao guarda a chave
        let disco = std::fs::read_to_string(std::path::PathBuf::from(std::env::var_os("EGUATEX_DADOS").unwrap()).join("eguatex").join("ia-config.json")).unwrap();
        assert!(!disco.contains("AIza-boa") && !disco.contains("AIza-cota"));
        zerar();
    }

    #[tokio::test]
    async fn cofre_que_nao_grava_depois_do_teste_certo_devolve_cofre() {
        let _vez = cofre_de_teste();
        zerar();
        let (base, _p) = servidor(vec![resp_json(200, r#"{"candidates":[]}"#)]);
        COFRE_QUEBRADO.store(true, std::sync::atomic::Ordering::SeqCst);
        let codigo = ativar_com("AIza-boa", &base, Duration::from_secs(5)).await;
        COFRE_QUEBRADO.store(false, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(codigo, COFRE);
        assert!(!status());
        assert!(!crate::ia::carregar_config().ativa);
        let i18n: serde_json::Value = serde_json::from_str(include_str!("../../../dados/i18n/pt-BR.json")).unwrap();
        assert_eq!(i18n["ia.chave.resultado.cofre"], "Não foi possível guardar a chave neste computador. Tente de novo.");
        zerar();
    }

    #[test]
    fn remover_apaga_a_chave_e_desliga_a_ia() {
        let _vez = cofre_de_teste();
        zerar();
        salvar(&chave_de_teste("AIza-1")).unwrap();
        let cfg = crate::ia::ConfigIA { ativa: true, ..crate::ia::ConfigIA::default() };
        crate::ia::salvar_config(&cfg).unwrap();
        remover().unwrap();
        assert!(!crate::ia::carregar_config().ativa);
        assert!(!status());
        // remover sem chave tambem da certo
        remover().unwrap();
        zerar();
    }
}
