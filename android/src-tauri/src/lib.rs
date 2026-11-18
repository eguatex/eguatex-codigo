mod arquivos;
mod atualizacao;
mod compilador;
mod config_projeto;
mod drive;
mod drive_picker;
mod exportar;
mod http_local;
mod ia;
mod limites_zip;
mod motor_web;
mod ortografia;
mod ponte_uri;
mod projetos;
mod registro;
mod traducao;
mod versoes;

use compilador::{InfoMotores, ResultadoCompilacao};
use ia::google::InfoConta;
use ia::ConfigIA;

use projetos::{ArquivoNo, Projeto};

/// Numero da versao que a tela mostra (rodape, Sobre, Ajuda e o e-mail de feedback).
/// E o proximo numero publico (X.Y.Z), o mesmo da `version` do Cargo.toml, do
/// tauri.conf.json e do package.json.
pub const NOME_VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Numero de compilacao (rodada de teste da oficina: v44, v45...). Fica nos
/// bastidores e nunca aparece na tela (versionCode do Android, versao interna do instalador).
pub const NUMERO_COMPILACAO: &str = "49";

#[tauri::command]
fn cmd_versao() -> String {
    format!("ÉguaTeX {NOME_VERSAO}")
}


#[tauri::command]
fn cmd_detectar_motores() -> InfoMotores {
    compilador::detectar_motores()
}

#[tauri::command(async)]
fn cmd_compilar(
    arquivo: String,
    diretorio: String,
    motor: String,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    compilador::compilar(&arquivo, &diretorio, &motor, parar_no_erro)
}

#[tauri::command(async)]
fn cmd_compilar_completo(
    arquivo: String,
    diretorio: String,
    motor: String,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    compilador::compilar_com_bibtex(&arquivo, &diretorio, &motor, parar_no_erro)
}

/// Motor da compilacao automatica para `arquivo` (com o motivo), pela mesma regra do Windows:
/// comentario % !TeX program, latexmkrc, pacotes e comandos (principal, \input, .cls e .sty
/// locais) e, sem nada, pdfLaTeX. Um arquivo sem \begin{document} troca pelo principal do projeto.
#[tauri::command(async)]
fn cmd_detectar_motor(arquivo: String, diretorio: String) -> MotorDoProjeto {
    let raiz = std::path::Path::new(&diretorio);
    let arquivo = arquivo.trim_start_matches('/').to_string();
    let tem_begin = std::fs::read(raiz.join(&arquivo))
        .map(|b| arquivos::tem_begin_document(&String::from_utf8_lossy(&b)))
        .unwrap_or(false);
    let arquivo = if tem_begin { arquivo } else { arquivos::principal_do_projeto(raiz).unwrap_or(arquivo) };
    let principal = raiz.join(arquivo);
    let auto = config_projeto::detectar_motor(raiz, &principal);
    MotorDoProjeto {
        motor: auto.motor,
        motivo: auto.motivo,
        // % !TeX program ou latexmkrc fixam o motor: sem motor lembrado nem outra tentativa (como no Windows)
        fixo: config_projeto::ler(raiz, &principal).motor.is_some(),
        impressao: config_projeto::impressao_do_documento(raiz, &principal),
    }
}

/// Motor automatico do celular: o detectado, se o projeto o fixa e a impressao do documento (preambulo,
/// pacotes e estilos) que valida o motor lembrado, pela mesma regra do Windows.
#[derive(Debug, serde::Serialize)]
struct MotorDoProjeto {
    motor: &'static str,
    motivo: String,
    fixo: bool,
    impressao: String,
}

/// O log mostra erro que outro motor resolve (o mesmo `erro_de_motor` do compilador.rs do Windows):
/// fontspec no pdfLaTeX, \directlua indefinido, fonte nao carregada, caractere Unicode, memoria do TeX.
/// \input ausente, comando desconhecido comum e pacote que falta nao contam. `motor_certo` e o motor
/// que o log aponta, se houver.
#[tauri::command]
fn cmd_erro_de_motor(log: String, motor: String) -> ErroDeMotor {
    let motor_certo = config_projeto::motor_pelo_log(&log, &motor);
    let plano = log.replace(['\r', '\n'], "");
    let tipicos = [
        "is required",
        "not loadable",
        "Missing character",
        "not set up for use with LaTeX",
        "TeX capacity exceeded",
        "Package fontspec Error",
        "Package inputenc Error: Unicode",
    ];
    ErroDeMotor { erro: motor_certo.is_some() || tipicos.iter().any(|t| plano.contains(t)), motor_certo }
}

#[derive(Debug, serde::Serialize)]
struct ErroDeMotor {
    erro: bool,
    motor_certo: Option<&'static str>,
}

#[cfg(test)]
mod testes_motor {
    use super::cmd_detectar_motor;

    /// O TU Berlin compila poster.tex (preambulo em 00-preambel.tex); o Inria pede XeLaTeX pelo
    /// mathspec; um preambulo sem \begin{document} troca pelo principal do projeto.
    #[test]
    fn detectar_motor_le_o_disco_e_troca_o_preambulo_pelo_principal() {
        let raiz = std::env::temp_dir().join(format!("eguatex-motor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::write(raiz.join("00-preambel.tex"), "\\documentclass{baposter}\n\\usepackage{fontspec}\n").unwrap();
        std::fs::write(raiz.join("poster.tex"), "\\input{00-preambel}\n\\begin{document}\n\\end{document}\n").unwrap();
        // fora da raiz: dentro dela o comum.tex (com \documentclass) viraria o principal
        let outra = raiz.with_extension("outra");
        std::fs::create_dir_all(&outra).unwrap();
        std::fs::write(outra.join("comum.tex"), "\\documentclass{article}\n\\begin{document}\n\\end{document}\n").unwrap();
        let d = raiz.display().to_string();
        let m = cmd_detectar_motor("00-preambel.tex".into(), d.clone());
        assert_eq!(m.motor, "xelatex", "{m:?}");
        assert!(!m.fixo && !m.impressao.is_empty(), "{m:?}");
        assert_eq!(cmd_detectar_motor("/poster.tex".into(), d.clone()).motor, "xelatex");
        assert_eq!(cmd_detectar_motor("comum.tex".into(), outra.display().to_string()).motor, "pdflatex");
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(&outra);
    }

    #[test]
    fn erro_de_motor_so_nos_erros_que_outro_motor_resolve() {
        use super::cmd_erro_de_motor;
        let r = cmd_erro_de_motor("! Package fontspec Error: The fontspec package requires either XeTeX or\n(fontspec) LuaTeX.".into(), "pdflatex".into());
        assert!(r.erro);
        assert_eq!(r.motor_certo, Some("xelatex"));
        assert!(cmd_erro_de_motor("Missing character: There is no ç in font cmr10!".into(), "pdflatex".into()).erro);
        for log in ["! LaTeX Error: File `geometry.sty' not found.", "! Undefined control sequence.", "! Emergency stop."] {
            assert!(!cmd_erro_de_motor(log.into(), "pdflatex".into()).erro, "{log}");
        }
    }
}

#[tauri::command]
fn cmd_listar_arvore(diretorio: String) -> Vec<ArquivoNo> {
    projetos::listar_arvore(&diretorio)
}

#[tauri::command]
fn cmd_ler_arquivo(caminho_projeto: String, caminho_relativo: String) -> Result<String, String> {
    projetos::ler_arquivo(&caminho_projeto, &caminho_relativo)
}

#[tauri::command]
fn cmd_salvar_arquivo(
    caminho_projeto: String,
    caminho_relativo: String,
    conteudo: String,
) -> Result<(), String> {
    projetos::salvar_arquivo(&caminho_projeto, &caminho_relativo, &conteudo)
}

#[tauri::command]
fn cmd_criar_projeto(diretorio: String, nome: String) -> Result<Projeto, String> {
    projetos::criar_projeto(&diretorio, &nome)
}

#[tauri::command]
fn cmd_listar_projetos(diretorio: String) -> Vec<projetos::ProjetoNaPasta> {
    projetos::listar_projetos(&diretorio)
}

/// Ultima modificacao de cada projeto da tela inicial (ms desde 1970; `None` se a pasta sumiu).
#[tauri::command(async)]
fn cmd_modificacao_projetos(caminhos: Vec<String>) -> Vec<Option<i64>> {
    caminhos.iter().map(|c| projetos::modificacao_do_projeto(std::path::Path::new(c))).collect()
}

#[tauri::command]
fn cmd_pasta_projetos() -> String {
    projetos::pasta_projetos()
}

#[tauri::command]
fn cmd_renomear_projeto(caminho: String, novo_nome: String) -> Result<String, String> {
    projetos::renomear_projeto(&caminho, &novo_nome)
}

/// Verdadeiro para um caminho que termina em .pdf (maiusculas ou minusculas).
fn pdf_pelo_nome(caminho: &str) -> bool {
    std::path::Path::new(caminho).extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Devolve o PDF em binario (chega ao frontend como ArrayBuffer). Roda fora da thread da
/// janela: um PDF grande, com fotos, nao congela a interface enquanto e lido.
#[tauri::command(async)]
fn cmd_ler_pdf_bytes(caminho: String) -> Result<tauri::ipc::Response, String> {
    // so o PDF compilado passa por aqui: qualquer outro arquivo (cofre, chaves) fica de fora
    if !pdf_pelo_nome(&caminho) {
        return Err("Erro ao ler PDF: o arquivo não é um .pdf.".into());
    }
    std::fs::read(&caminho)
        .map(tauri::ipc::Response::new)
        .map_err(|e| format!("Erro ao ler PDF: {}", e))
}

/// Bytes de uma imagem do projeto (chega ao frontend como ArrayBuffer). Resolve o caminho dentro
/// do projeto e recusa arquivo acima de 25 MB. Roda fora da thread da janela.
#[tauri::command(async)]
fn cmd_ler_bytes_projeto(caminho_projeto: String, caminho_relativo: String) -> Result<tauri::ipc::Response, String> {
    projetos::ler_bytes_projeto(&caminho_projeto, &caminho_relativo).map(tauri::ipc::Response::new)
}

#[tauri::command]
fn cmd_ia_carregar_config() -> ConfigIA {
    ia::carregar_config()
}

#[tauri::command]
fn cmd_ia_salvar_config(config: ConfigIA) -> Result<(), String> {
    ia::salvar_config(&config)
}

/// Testa a chave do Gemini colada pela pessoa e, se deu certo (ou so falta cota), grava no
/// cofre e liga a IA. Devolve um codigo; o texto vem do i18n.
#[tauri::command]
async fn cmd_ia_chave_ativar(chave: String) -> String {
    ia::chave::ativar(&chave).await
}

/// Ha chave do Gemini guardada no cofre? (nunca a chave)
#[tauri::command]
fn cmd_ia_chave_status() -> bool {
    ia::chave::status()
}

/// Apaga a chave do Gemini e desliga a IA.
#[tauri::command]
fn cmd_ia_chave_remover() -> Result<(), String> {
    ia::chave::remover()
}

/// Agente: corpo completo do pedido ao Gemini (com ferramentas) e devolve o `content` da resposta.
#[tauri::command]
async fn cmd_ia_agente(
    app: tauri::AppHandle,
    id: String,
    modelo: String,
    corpo: serde_json::Value,
) -> Result<serde_json::Value, String> {
    ia::google::chamar_agente(&app, &id, &modelo, corpo).await
}

/// Botao Parar: corta o stream da conversa `id`.
#[tauri::command]
fn cmd_ia_cancelar(id: String) {
    ia::google::cancelar_agente(&id);
}

/// Login na conta Google unica (icone de conta): Drive, compartilhamento e arquivos do Google.
#[tauri::command]
async fn cmd_conta_google_entrar(app: tauri::AppHandle) -> Result<InfoConta, String> {
    ia::google::entrar_conta(&app).await?;
    Ok(ia::google::info())
}

#[tauri::command]
async fn cmd_ia_google_sair() -> Result<(), String> {
    ia::google::sair().await
}

#[tauri::command]
fn cmd_ia_google_info() -> InfoConta {
    ia::google::info()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .register_uri_scheme_protocol("busytex", |_ctx, pedido| motor_web::responder(pedido.uri().path()))
        .setup(|app| {
            use tauri::Manager;
            // janela guardada para os eventos tarefa-progresso (zip e Drive)
            crate::drive::registrar_app_progresso(app.handle());
            // Pasta privada do app: projetos, configuracao da IA e cofre de credenciais.
            if let Ok(dados) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dados);
                // SAFETY: roda no setup, antes das threads que leem o ambiente.
                unsafe { std::env::set_var("EGUATEX_DADOS", &dados) };
            }
            if let Ok(recursos) = app.path().resource_dir() {
                compilador::usar_tex_embutido(&recursos);
                exportar::localizar_pandoc(&recursos);
            }
            // Conta antiga, com o Drive inteiro:
            // revoga e pede novo login com menos acesso.
            tauri::async_runtime::spawn(async {
                let _ = ia::google::migrar_conta_antiga().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ortografia::cmd_ortografia_verificar,
            ortografia::cmd_ortografia_sugerir,
            versoes::cmd_versoes_gravar,
            versoes::cmd_versoes_listar,
            versoes::cmd_versoes_ler,
            versoes::cmd_versoes_restaurar,
            cmd_versao,
            atualizacao::cmd_procurar_atualizacao,
            atualizacao::cmd_aviso_do_instalador,
            cmd_detectar_motores,
            cmd_compilar,
            cmd_compilar_completo,
            compilador::cmd_parar_compilacao,
            cmd_listar_arvore,
            cmd_ler_arquivo,
            cmd_salvar_arquivo,
            cmd_criar_projeto,
            cmd_pasta_projetos,
            cmd_listar_projetos,
            cmd_modificacao_projetos,
            cmd_renomear_projeto,
            cmd_ler_pdf_bytes,
            cmd_ler_bytes_projeto,
            cmd_ia_carregar_config,
            cmd_ia_salvar_config,
            cmd_ia_chave_ativar,
            cmd_ia_chave_status,
            cmd_ia_chave_remover,
            cmd_ia_agente,
            cmd_ia_cancelar,
            cmd_ia_google_sair,
            cmd_conta_google_entrar,
            cmd_ia_google_info,
            motor_web::cmd_motor_arquivos,
            motor_web::cmd_motor_gravar,
            motor_web::cmd_motor_gravar_base64,
            motor_web::cmd_motor_gravar_texto,
            motor_web::cmd_motor_erros,
            motor_web::cmd_motor_cache_ler,
            motor_web::cmd_motor_cache_gravar,
            motor_web::cmd_motor_cache_apagar,
            motor_web::cmd_motor_pacotes_gravar,
            motor_web::cmd_motor_resumo,
            cmd_detectar_motor,
            cmd_erro_de_motor,
            arquivos::cmd_criar_arquivo,
            arquivos::cmd_criar_pasta,
            ponte_uri::cmd_importar_arquivo,
            ponte_uri::cmd_importar_figura,
            arquivos::cmd_renomear_item,
            arquivos::cmd_excluir_item,
            ponte_uri::cmd_copiar_para,
            ponte_uri::cmd_gravar_bytes,
            ponte_uri::cmd_ler_texto_importado,
            arquivos::cmd_copiar_projeto,
            ponte_uri::cmd_exportar_zip,
            ponte_uri::cmd_importar_zip,
            ponte_uri::cmd_escolher_pasta_android,
            arquivos::cmd_detectar_principal,
            traducao::cmd_traduzir,
            exportar::cmd_exportar_documento,
            exportar::cmd_pandoc_disponivel,
            drive::cmd_drive_info,
            drive::cmd_drive_entrar,
            drive::cmd_drive_sair,
            drive::cmd_drive_projeto,
            drive::cmd_drive_sincronizar,
            drive::cmd_drive_compartilhamento,
            drive::cmd_drive_convidar,
            drive::cmd_drive_remover_pessoa,
            drive::cmd_drive_link,
            drive::cmd_drive_reabrir_seletor,
            drive::cmd_drive_meus_projetos,
            drive::cmd_drive_enviar_projeto,
            drive::cmd_drive_escolher,
            drive::cmd_drive_planilha_csv,
            drive::cmd_drive_documento_latex,
            drive::cmd_drive_baixar_projeto,
            drive::cmd_drive_desligar,
            drive::cmd_drive_abrir_arquivos,
            drive::cmd_drive_abrir_figura,
            arquivos::cmd_limpar_auxiliares,
            arquivos::cmd_synctex_disponivel,
            arquivos::cmd_synctex_ir_pdf,
            arquivos::cmd_synctex_ir_codigo,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o EguaTex");
}

#[cfg(test)]
mod testes_ler_pdf {
    #[test]
    fn ler_pdf_bytes_so_aceita_pdf() {
        assert!(super::pdf_pelo_nome("C:/proj/main.pdf"));
        assert!(super::pdf_pelo_nome("/data/proj/MAIN.PDF"));
        assert!(!super::pdf_pelo_nome("C:/Users/x/AppData/cofre.json"));
        assert!(!super::pdf_pelo_nome("/data/proj/main.pdf.txt"));
        assert!(!super::pdf_pelo_nome("/data/proj/pdf"));
        assert!(super::cmd_ler_pdf_bytes("/nao/existe/segredo.json".into()).is_err());
    }
}

#[cfg(test)]
mod testes_versao {
    #[test]
    fn tela_mostra_o_nome_da_versao() {
        assert_eq!(super::NOME_VERSAO, "1.0.3");
        assert_eq!(super::cmd_versao(), "ÉguaTeX 1.0.3");
        assert!(!super::cmd_versao().contains("Tauri"));
    }

    #[test]
    fn numeros_batem_com_os_arquivos_de_configuracao() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let versao = conf["version"].as_str().unwrap().to_string();
        assert!(versao.starts_with(super::NOME_VERSAO));
        assert_eq!(conf["bundle"]["android"]["versionCode"].as_u64(), Some(super::NUMERO_COMPILACAO.parse().unwrap()));
        let pacote: serde_json::Value = serde_json::from_str(include_str!("../../package.json")).unwrap();
        assert_eq!(pacote["version"].as_str(), Some(super::NOME_VERSAO));
    }
}
