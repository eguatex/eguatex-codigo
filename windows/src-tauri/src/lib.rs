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
mod projetos;
mod registro;
mod traducao;
mod ortografia;
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
/// bastidores: so aparece na linha de detalhes tecnicos da tela Sobre.
pub const NUMERO_COMPILACAO: &str = "44";

#[tauri::command]
fn cmd_versao() -> String {
    format!("ÉguaTex {NOME_VERSAO}")
}

/// Detalhes tecnicos da tela Sobre: numero de compilacao e versao do Tauri.
#[derive(serde::Serialize)]
struct DetalhesTecnicos {
    compilacao: &'static str,
    tauri: &'static str,
}

#[tauri::command]
fn cmd_detalhes_tecnicos() -> DetalhesTecnicos {
    DetalhesTecnicos { compilacao: NUMERO_COMPILACAO, tauri: tauri::VERSION }
}

#[tauri::command(async)]
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
    max_passadas: Option<u32>,
    motor_lembrado: Option<String>,
) -> ResultadoCompilacao {
    compilador::compilar_completo(&arquivo, &diretorio, &motor, parar_no_erro, max_passadas, motor_lembrado.as_deref())
}

/// Motor que a compilacao automatica usaria, com o motivo, para a barra de status.
#[tauri::command(async)]
fn cmd_detectar_motor(arquivo: String, diretorio: String) -> config_projeto::MotorAutomatico {
    compilador::detectar_motor(&arquivo, &diretorio)
}

/// Preferencia avancada "Permitir shell-escape": vale para as proximas compilacoes.
#[tauri::command]
fn cmd_definir_shell_escape(liberado: bool) {
    compilador::definir_shell_escape(liberado);
}

#[tauri::command(async)]
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

#[tauri::command(async)]
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
/// Tira da barra de titulo do Windows o nome e o icone. Os botoes de minimizar, maximizar e fechar
/// ficam, e o nome continua na barra de tarefas e no Alt+Tab.
#[cfg(windows)]
fn esconder_nome_e_icone_da_barra(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::Controls::{
        SetWindowThemeAttribute, WTA_NONCLIENT, WTA_OPTIONS, WTNCA_NODRAWCAPTION, WTNCA_NODRAWICON,
    };
    let flags = WTNCA_NODRAWCAPTION | WTNCA_NODRAWICON;
    let opcoes = WTA_OPTIONS { dwFlags: flags, dwMask: flags };
    // SAFETY: hwnd e a janela principal viva; opcoes vive ate o fim da chamada
    unsafe {
        SetWindowThemeAttribute(hwnd, WTA_NONCLIENT, &opcoes as *const WTA_OPTIONS as *const _, size_of::<WTA_OPTIONS>() as u32);
    }
    // O Windows 11 ainda desenha o icone pequeno: ele vira um icone transparente de 16 px. O icone
    // grande (barra de tarefas e Alt+Tab) continua o do app.
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIcon, ICON_SMALL, SendMessageW, WM_SETICON};
    let mascara_e = [0xFFu8; 16 * 16 / 8]; // 1 = transparente
    let mascara_xor = [0u8; 16 * 16 / 8];
    // SAFETY: as mascaras tem 16x16 bits; o icone fica com a janela ate o fim do app
    unsafe {
        let vazio = CreateIcon(std::ptr::null_mut(), 16, 16, 1, 1, mascara_e.as_ptr(), mascara_xor.as_ptr());
        if !vazio.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, vazio as isize);
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            // janela guardada para os eventos tarefa-progresso (zip e Drive)
            crate::drive::registrar_app_progresso(app.handle());
            // barra de titulo do Windows so com os botoes: o nome e a logo de 16 px ficavam ilegiveis
            #[cfg(windows)]
            if let Some(janela) = app.get_webview_window("main") {
                if let Ok(hwnd) = janela.hwnd() {
                    esconder_nome_e_icone_da_barra(hwnd.0 as _);
                }
            }
            if let Ok(recursos) = app.path().resource_dir() {
                compilador::usar_tex_embutido(&recursos);
                exportar::localizar_pandoc(&recursos);
            }
            // cache de fontes do TinyTeX e biber prontos antes da primeira compilacao
            compilador::preparar_em_segundo_plano();
            // Chaves de provedores antigos saem do cofre. Conta antiga, com o Drive inteiro:
            // revoga e pede novo login com menos acesso.
            tauri::async_runtime::spawn(async {
                ia::chave::apagar_provedores_antigos();
                let _ = ia::google::migrar_conta_antiga().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_versao,
            cmd_detalhes_tecnicos,
            atualizacao::cmd_procurar_atualizacao,
            cmd_detectar_motores,
            cmd_compilar,
            cmd_compilar_completo,
            cmd_definir_shell_escape,
            cmd_detectar_motor,
            compilador::cmd_parar_compilacao,
            compilador::cmd_instalar_pacote_tex,
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
            arquivos::cmd_criar_arquivo,
            arquivos::cmd_criar_pasta,
            arquivos::cmd_importar_arquivo,
            arquivos::cmd_importar_figura,
            arquivos::cmd_renomear_item,
            arquivos::cmd_excluir_item,
            arquivos::cmd_copiar_para,
            arquivos::cmd_copiar_projeto,
            arquivos::cmd_exportar_zip,
            arquivos::cmd_importar_zip,
            arquivos::cmd_detectar_principal,
            traducao::cmd_traduzir,
            ortografia::cmd_ortografia_verificar,
            ortografia::cmd_ortografia_sugerir,
            versoes::cmd_versoes_gravar,
            versoes::cmd_versoes_listar,
            versoes::cmd_versoes_ler,
            versoes::cmd_versoes_restaurar,
            exportar::cmd_exportar_documento,
            exportar::cmd_gravar_bytes,
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
        assert_eq!(super::NOME_VERSAO, "1.0.0");
        assert_eq!(super::cmd_versao(), "ÉguaTex 1.0.0");
        assert!(!super::cmd_versao().contains("Tauri"));
        let d = super::cmd_detalhes_tecnicos();
        assert_eq!(d.compilacao, "44");
        assert_eq!(d.tauri, tauri::VERSION);
    }

    #[test]
    fn numeros_batem_com_os_arquivos_de_configuracao() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let versao = conf["version"].as_str().unwrap().to_string();
        assert!(versao.starts_with(super::NOME_VERSAO));
        assert_eq!(versao, format!("{}+{}", super::NOME_VERSAO, super::NUMERO_COMPILACAO));
        let pacote: serde_json::Value = serde_json::from_str(include_str!("../../package.json")).unwrap();
        assert_eq!(pacote["version"].as_str(), Some(super::NOME_VERSAO));
    }
}
