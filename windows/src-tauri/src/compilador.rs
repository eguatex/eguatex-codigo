use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoCompilacao {
    pub sucesso: bool,
    pub pdf: Option<String>,
    pub log: String,
    pub erros: Vec<ErroCompilacao>,
    pub motor_usado: String,
    pub tempo_ms: u64,
    /// No automatico: motor que gerou o PDF quando nao e o detectado (o motor lembrado ou um
    /// dos outros tentados depois de uma falha sem PDF). A interface o guarda para o projeto e
    /// manda de volta na proxima compilacao. `None`: vale o detectado.
    #[serde(default)]
    pub motor_lembrar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErroCompilacao {
    pub linha: Option<u32>,
    pub arquivo: Option<String>,
    pub mensagem: String,
    pub tipo: String,
    /// So nos avisos agrupados no painel ("caixa-larga", "rotulo-repetido"...). Vazio nos demais.
    #[serde(default)]
    pub categoria: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorDetectado {
    pub nome: String,
    pub caminho: String,
    pub versao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoMotores {
    pub distribuicao: Option<String>,
    pub motores: Vec<MotorDetectado>,
}

/// Pasta do TinyTeX que vem junto com o instalador, relativa aos recursos do app.
const TEX_EMBUTIDO: &str = "tex/TinyTeX/bin/windows";

/// %PROGRAMDATA% e gravavel por qualquer usuario local. Antes de usar o TinyTeX de la, confere
/// que cada pasta de `bin\windows` ate `...\EguaTex` e TODO arquivo dentro de `bin\windows`
/// (exe, com, dll, scripts) tem o mesmo dono do ÉguaTeX em execucao (quem o instalou). Basta um
/// arquivo plantado por outro usuario para a pasta inteira ser recusada. A pasta de recursos
/// (desenvolvimento) nao passa por esta conferencia.
pub(crate) fn tinytex_confiavel(bin: &Path) -> bool {
    let Some(dados) = std::env::var_os("PROGRAMDATA") else {
        return true;
    };
    let raiz = PathBuf::from(dados).join("EguaTex");
    if !bin.starts_with(&raiz) {
        return true;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    tinytex_confiavel_com(bin, &raiz, &exe, &dono::mesmo_dono)
}

/// Nucleo da conferencia, com a comparacao de dono injetada para os testes.
pub(crate) fn tinytex_confiavel_com(
    bin: &Path,
    raiz: &Path,
    referencia: &Path,
    mesmo_dono: &dyn Fn(&Path, &Path) -> bool,
) -> bool {
    let Ok(entradas) = std::fs::read_dir(bin) else {
        return false;
    };
    let mut alvos: Vec<PathBuf> = Vec::new();
    for e in entradas {
        match e {
            Ok(e) => alvos.push(e.path()),
            Err(_) => return false,
        }
    }
    if alvos.is_empty() {
        return false;
    }
    let mut atual = Some(bin);
    while let Some(p) = atual {
        alvos.push(p.to_path_buf());
        if p == raiz {
            break;
        }
        atual = p.parent();
    }
    alvos.iter().all(|a| mesmo_dono(a, referencia))
}

/// Dono de um arquivo ou pasta do Windows, so para comparar com outro (sem crates novas).
#[cfg(windows)]
mod dono {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn GetNamedSecurityInfoW(
            nome: *const u16,
            tipo: u32,
            informacao: u32,
            dono: *mut *mut c_void,
            grupo: *mut *mut c_void,
            dacl: *mut *mut c_void,
            sacl: *mut *mut c_void,
            descritor: *mut *mut c_void,
        ) -> u32;
        fn EqualSid(a: *mut c_void, b: *mut c_void) -> i32;
        fn IsWellKnownSid(sid: *mut c_void, tipo: u32) -> i32;
        fn GetAclInformation(acl: *mut c_void, info: *mut c_void, tamanho: u32, classe: u32) -> i32;
        fn GetAce(acl: *mut c_void, indice: u32, ace: *mut *mut c_void) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memoria: *mut c_void) -> *mut c_void;
    }

    const SE_FILE_OBJECT: u32 = 1;
    const OWNER_SECURITY_INFORMATION: u32 = 1;
    const DACL_SECURITY_INFORMATION: u32 = 4;
    const ACL_SIZE_INFORMATION: u32 = 2;
    const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
    const ACCESS_DENIED_ACE_TYPE: u8 = 1;
    const WIN_LOCAL_SYSTEM_SID: u32 = 22;
    const WIN_BUILTIN_ADMINISTRATORS_SID: u32 = 26;

    /// Le o dono de `caminho`. O ponteiro do SID mora dentro do descritor devolvido.
    fn ler(caminho: &Path) -> Option<(*mut c_void, *mut c_void)> {
        let nome: Vec<u16> = caminho.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut sid: *mut c_void = std::ptr::null_mut();
        let mut descritor: *mut c_void = std::ptr::null_mut();
        // SAFETY: `nome` termina em zero e vive ate o fim da chamada; as saidas sao ponteiros locais.
        let erro = unsafe {
            GetNamedSecurityInfoW(
                nome.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut sid,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descritor,
            )
        };
        if erro != 0 || sid.is_null() || descritor.is_null() {
            return None;
        }
        Some((sid, descritor))
    }

    pub fn mesmo_dono(a: &Path, b: &Path) -> bool {
        let Some((sid_a, desc_a)) = ler(a) else {
            return false;
        };
        let Some((sid_b, desc_b)) = ler(b) else {
            // SAFETY: `desc_a` veio de GetNamedSecurityInfoW e nao foi liberado.
            unsafe { LocalFree(desc_a) };
            return false;
        };
        // SAFETY: os dois SIDs estao dentro de descritores ainda nao liberados.
        let igual = unsafe { EqualSid(sid_a, sid_b) } != 0;
        // SAFETY: os descritores vieram de GetNamedSecurityInfoW e sao liberados uma vez.
        unsafe {
            LocalFree(desc_a);
            LocalFree(desc_b);
        }
        igual
    }

    /// `pasta` e da mesma conta que `referencia` (um arquivo que a conta atual acabou de criar)
    /// e so essa conta, o SISTEMA e os Administradores tem permissao nela. ACE de negacao nao
    /// abre nada; qualquer outro tipo de ACE (objeto, callback) recusa.
    pub fn so_da_conta(pasta: &Path, referencia: &Path) -> bool {
        if !mesmo_dono(pasta, referencia) {
            return false;
        }
        let Some((conta, desc_ref)) = ler(referencia) else {
            return false;
        };
        let nome: Vec<u16> = pasta.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut dacl: *mut c_void = std::ptr::null_mut();
        let mut descritor: *mut c_void = std::ptr::null_mut();
        // SAFETY: `nome` termina em zero; as saidas sao ponteiros locais.
        let erro = unsafe {
            GetNamedSecurityInfoW(
                nome.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descritor,
            )
        };
        // DACL nula da acesso total a todos
        let mut certo = erro == 0 && !dacl.is_null();
        if certo {
            // ACL_SIZE_INFORMATION: AceCount, AclBytesInUse, AclBytesFree
            let mut info = [0u32; 3];
            // SAFETY: `dacl` vem do descritor ainda vivo; `info` tem os 12 bytes informados.
            certo = unsafe { GetAclInformation(dacl, info.as_mut_ptr().cast(), 12, ACL_SIZE_INFORMATION) } != 0;
            for indice in 0..if certo { info[0] } else { 0 } {
                let mut ace: *mut c_void = std::ptr::null_mut();
                // SAFETY: indice < AceCount; `ace` aponta para dentro da DACL viva.
                if unsafe { GetAce(dacl, indice, &mut ace) } == 0 || ace.is_null() {
                    certo = false;
                    break;
                }
                // ACE_HEADER (tipo, flags, tamanho) e a mascara: o SID comeca no byte 8
                // SAFETY: toda ACE tem ao menos o cabecalho de 4 bytes.
                let tipo = unsafe { *(ace as *const u8) };
                match tipo {
                    ACCESS_DENIED_ACE_TYPE => {}
                    ACCESS_ALLOWED_ACE_TYPE => {
                        // SAFETY: ACE de permissao: o SID fica logo depois do cabecalho e da mascara.
                        let sid = unsafe { (ace as *mut u8).add(8) }.cast::<c_void>();
                        // SAFETY: `sid` e `conta` sao SIDs validos dentro de descritores vivos.
                        let permitido = unsafe {
                            EqualSid(sid, conta) != 0
                                || IsWellKnownSid(sid, WIN_LOCAL_SYSTEM_SID) != 0
                                || IsWellKnownSid(sid, WIN_BUILTIN_ADMINISTRATORS_SID) != 0
                        };
                        if !permitido {
                            certo = false;
                            break;
                        }
                    }
                    _ => {
                        certo = false;
                        break;
                    }
                }
            }
        }
        // SAFETY: os descritores vieram de GetNamedSecurityInfoW e sao liberados uma vez.
        unsafe {
            if !descritor.is_null() {
                LocalFree(descritor);
            }
            LocalFree(desc_ref);
        }
        certo
    }
}

#[cfg(not(windows))]
mod dono {
    use std::path::Path;

    pub fn mesmo_dono(_a: &Path, _b: &Path) -> bool {
        true
    }

    pub fn so_da_conta(_pasta: &Path, _referencia: &Path) -> bool {
        true
    }
}

/// Coloca o TinyTeX embutido no fim do PATH. Um MiKTeX ou TeX Live ja instalado
/// continua com prioridade; o embutido so e usado quando nao ha outro.
/// Chamada uma vez no setup, antes de qualquer compilacao.
pub fn usar_tex_embutido(recursos: &Path) {
    // O instalador move o TinyTeX para %PROGRAMDATA%\EguaTex, pasta sem acento.
    // A pasta de recursos so vale no desenvolvimento.
    let mut candidatos: Vec<PathBuf> = Vec::new();
    if let Some(dados) = std::env::var_os("PROGRAMDATA") {
        candidatos.push(PathBuf::from(dados).join("EguaTex").join("TinyTeX").join("bin").join("windows"));
    }
    candidatos.push(recursos.join(TEX_EMBUTIDO));
    let Some(bin) = candidatos
        .into_iter()
        .find(|b| b.join("pdflatex.exe").exists() && tinytex_confiavel(b))
    else {
        return;
    };
    let mut caminhos: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if caminhos.contains(&bin) {
        return;
    }
    caminhos.push(bin);
    if let Ok(novo) = std::env::join_paths(caminhos) {
        // SAFETY: roda no setup do Tauri, antes de criar as threads que leem o PATH.
        unsafe { std::env::set_var("PATH", novo) };
    }
}

/// Preparo da primeira abertura, numa thread de prioridade baixa: o do TinyTeX (se e ele que
/// compila) e o do biber. O biber.exe e um Perl empacotado que se desempacota (105 MB em
/// %TEMP%\par-*) na primeira vez que roda: no TinyTeX novo isso levou 36 s dentro da primeira
/// compilacao com bibliografia. Rodar `biber --version` antes deixa o pacote pronto.
pub fn preparar_em_segundo_plano() {
    iniciar_preparo(ESPERA_MAXIMA_PREPARO);
    std::thread::spawn(|| {
        // libera quem espera mesmo se o preparo der panic
        struct Fim;
        impl Drop for Fim {
            fn drop(&mut self) {
                terminar_preparo();
            }
        }
        let _fim = Fim;
        if let Some(bin) = detectar_executavel("pdflatex").and_then(|p| p.parent().map(Path::to_path_buf))
            && bin.parent().and_then(Path::parent).is_some_and(|raiz| raiz.join("texmf-var").is_dir())
            && bin.to_string_lossy().to_lowercase().contains("tinytex")
        {
            aquecer_tinytex(&bin);
        }
        if let Some(biber) = detectar_executavel("biber") {
            let mut comando = Command::new(biber);
            comando.arg("--version");
            rodar_em_segundo_plano(&mut comando);
        }
    });
}

/// Estado do preparo da primeira abertura. A compilacao espera ele acabar: o fc-cache, o
/// luaotfload-tool e o biber desempacotando ao mesmo tempo que o motor disputam o disco e os
/// mesmos caches.
struct EstadoPreparo {
    rodando: bool,
    /// Fim da espera, contado do inicio do preparo: depois dele nenhuma compilacao espera mais.
    prazo: Option<Instant>,
    /// Uma compilacao esta esperando; o botao Parar cancela a espera.
    esperando: bool,
    cancelado: bool,
}

static PREPARO: (Mutex<EstadoPreparo>, Condvar) = (
    Mutex::new(EstadoPreparo { rodando: false, prazo: None, esperando: false, cancelado: false }),
    Condvar::new(),
);

/// Limite da espera pelo preparo inteiro: se ele enroscar, as compilacoes seguem assim mesmo.
const ESPERA_MAXIMA_PREPARO: Duration = Duration::from_secs(90);

/// Marca o preparo como em andamento, com o prazo contado de agora.
fn iniciar_preparo(limite: Duration) {
    let mut estado = travar(&PREPARO.0);
    estado.rodando = true;
    estado.prazo = Some(Instant::now() + limite);
}

/// Marca o fim do preparo e acorda quem espera.
fn terminar_preparo() {
    travar(&PREPARO.0).rodando = false;
    PREPARO.1.notify_all();
}

/// Espera o preparo terminar, ate o prazo dele. Devolve false se o botao Parar cancelou a espera.
fn esperar_preparo() -> bool {
    let mut estado = travar(&PREPARO.0);
    estado.cancelado = false;
    estado.esperando = true;
    while estado.rodando && !estado.cancelado {
        let Some(falta) = estado.prazo.and_then(|prazo| prazo.checked_duration_since(Instant::now())).filter(|f| !f.is_zero())
        else {
            break;
        };
        estado = PREPARO.1.wait_timeout(estado, falta).unwrap_or_else(|erro| erro.into_inner()).0;
    }
    estado.esperando = false;
    !std::mem::take(&mut estado.cancelado)
}

/// Botao Parar durante a espera pelo preparo: cancela a espera. Devolve se havia espera.
fn cancelar_espera_do_preparo() -> bool {
    let mut estado = travar(&PREPARO.0);
    if !estado.esperando {
        return false;
    }
    estado.cancelado = true;
    PREPARO.1.notify_all();
    true
}

/// Resultado da compilacao parada ainda na espera pelo preparo, antes de rodar o motor.
fn espera_cancelada(motor: &str) -> ResultadoCompilacao {
    interrompida(ResultadoCompilacao {
        sucesso: false,
        pdf: None,
        log: String::new(),
        erros: Vec::new(),
        motor_usado: motor.to_string(),
        tempo_ms: 0,
        motor_lembrar: None,
    })
}

/// Roda e espera um programa auxiliar sem janela, com prioridade baixa e sem saida.
fn rodar_em_segundo_plano(comando: &mut Command) {
    comando.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    ocultar_janela(comando);
    com_ambiente_da_conta(comando);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW com BELOW_NORMAL_PRIORITY_CLASS: nao atrapalha a compilacao
        comando.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    let _ = comando.status();
}

/// Versao do preparo do TinyTeX gravada em texmf-var; mudar o numero refaz o preparo.
const PREPARO_TINYTEX: &str = "eguatex-preparo-1";

/// Ambiente do TeX desta conta do Windows quando ela nao grava no texmf-var do TinyTeX (que e
/// de quem instalou): TEXMFVAR e FONTCONFIG_FILE numa pasta so desta conta. Vazio quando a
/// conta grava no texmf-var. Os formatos (.fmt) continuam vindo do texmf-var do TinyTeX
/// (TEXMFSYSVAR), so para leitura.
static AMBIENTE_CONTA: Mutex<Vec<(&'static str, PathBuf)>> = Mutex::new(Vec::new());

/// Pasta desta conta que o AMBIENTE_CONTA usa, para a conferencia antes de cada compilacao.
static PASTA_CONTA: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Antes de cada compilacao: a pasta da conta continua so desta conta (dono e ACL). Se outra
/// conta mexeu nela, o TeX deixa de usa-la (sem TEXMFVAR nem fonts.conf proprios) e o log registra.
fn conferir_pasta_da_conta() {
    let Some(pasta) = travar(&PASTA_CONTA).clone() else { return };
    if pasta_so_da_conta(&pasta) {
        return;
    }
    crate::registro::tex("conta", &format!("{} mudou de dono ou de permissão; o TeX deixa de usá-la", pasta.display()));
    travar(&AMBIENTE_CONTA).clear();
    *travar(&PASTA_CONTA) = None;
}

/// Passa o ambiente desta conta, se houver, ao programa do TeX.
fn com_ambiente_da_conta(comando: &mut Command) {
    for (nome, valor) in travar(&AMBIENTE_CONTA).iter() {
        comando.env(nome, valor);
    }
}

/// A conta atual consegue criar arquivo em `pasta`.
fn pasta_gravavel(pasta: &Path) -> bool {
    let teste = pasta.join(format!(".eguatex-gravavel-{}", std::process::id()));
    let gravou = std::fs::File::create(&teste).is_ok();
    let _ = std::fs::remove_file(&teste);
    gravou
}

fn so_ascii(caminho: &Path) -> bool {
    caminho.to_str().is_some_and(str::is_ascii)
}

/// Pasta desta conta para o TeX, sempre num caminho so ASCII (o TeX quebra com acento):
/// `local\EguaTex`; com acento, o nome curto 8.3 dela; sem nome curto, a pasta de
/// `em_programdata`.
fn escolher_pasta_da_conta(
    local: Option<&Path>,
    curto: &dyn Fn(&Path) -> Option<PathBuf>,
    em_programdata: &dyn Fn() -> Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(local) = local {
        let pasta = local.join("EguaTex");
        if std::fs::create_dir_all(&pasta).is_ok() {
            if so_ascii(&pasta) {
                return Some(pasta);
            }
            if let Some(curta) = curto(&pasta).filter(|c| so_ascii(c)) {
                return Some(curta);
            }
        }
    }
    em_programdata()
}

/// %LOCALAPPDATA%\EguaTex, o nome curto dela ou %PROGRAMDATA%\EguaTex\contas\<hash da conta>.
fn pasta_da_conta() -> Option<PathBuf> {
    let local = dirs::data_local_dir();
    escolher_pasta_da_conta(local.as_deref(), &caminho_curto, &|| {
        let base = PathBuf::from(std::env::var_os("PROGRAMDATA")?).join("EguaTex").join("contas");
        let usuario = std::env::var("USERNAME").ok()?;
        let conta = match std::env::var("USERDOMAIN") {
            Ok(dominio) if !dominio.is_empty() => format!("{dominio}\\{usuario}"),
            _ => usuario,
        };
        pasta_restrita(&base, &conta)
    })
}

/// Nome curto 8.3 de um caminho que existe (GetShortPathNameW). Com os nomes curtos desligados
/// no disco, o Windows devolve o proprio caminho longo.
#[cfg(windows)]
fn caminho_curto(caminho: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetShortPathNameW(longo: *const u16, curto: *mut u16, tamanho: u32) -> u32;
    }

    let longo: Vec<u16> = caminho.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut curto = vec![0u16; 1024];
    // SAFETY: `longo` termina em zero; `curto` tem `curto.len()` posicoes, o tamanho informado.
    let n = unsafe { GetShortPathNameW(longo.as_ptr(), curto.as_mut_ptr(), curto.len() as u32) } as usize;
    if n == 0 || n >= curto.len() {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(&curto[..n])))
}

#[cfg(not(windows))]
fn caminho_curto(_caminho: &Path) -> Option<PathBuf> {
    None
}

/// `pasta` e da conta atual e so ela (com o SISTEMA e os Administradores) tem permissao nela.
fn pasta_so_da_conta(pasta: &Path) -> bool {
    let referencia = std::env::temp_dir().join(format!("eguatex-dono-{}", std::process::id()));
    let certo = std::fs::write(&referencia, b"").is_ok() && dono::so_da_conta(pasta, &referencia);
    let _ = std::fs::remove_file(&referencia);
    certo
}

/// Pasta `base\<hash da conta>` so da conta atual. `base` e a pasta contas que o instalador cria
/// sem heranca (os Usuarios so criam subpasta nela); o app nao a cria, para ela nunca ser de uma
/// conta comum. Criada agora: o icacls tira a heranca e deixa so a conta, com controle total.
/// Ja existente ou recem-criada, so vale se o dono e a ACL forem so desta conta.
fn pasta_restrita(base: &Path, conta: &str) -> Option<PathBuf> {
    // FNV-1a de 64 bits: o mesmo nome de pasta em qualquer versao do Rust
    let hash = conta
        .to_lowercase()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    let pasta = base.join(format!("{hash:016x}"));
    if !base.is_dir() {
        return None;
    }
    match std::fs::create_dir(&pasta) {
        Ok(()) => {
            let icacls = std::env::var_os("SystemRoot")
                .map(|raiz| PathBuf::from(raiz).join("System32").join("icacls.exe"))
                .unwrap_or_else(|| PathBuf::from("icacls.exe"));
            let mut comando = Command::new(icacls);
            comando
                .arg(&pasta)
                .args(["/inheritance:r", "/grant:r"])
                .arg(format!("{conta}:(OI)(CI)F"))
                .arg("/Q")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            ocultar_janela(&mut comando);
            if comando.status().is_ok_and(|s| s.success()) && pasta_so_da_conta(&pasta) {
                Some(pasta)
            } else {
                crate::registro::tex("conta", &format!("icacls falhou em {}; pasta descartada", pasta.display()));
                let _ = std::fs::remove_dir(&pasta);
                None
            }
        }
        Err(erro) if erro.kind() == std::io::ErrorKind::AlreadyExists => {
            if pasta_so_da_conta(&pasta) {
                Some(pasta)
            } else {
                crate::registro::tex("conta", &format!("{} tem dono ou permissão de outra conta; não usada", pasta.display()));
                None
            }
        }
        Err(_) => None,
    }
}

/// Texto do fonts.conf de `raiz` com as pastas de fontes apontando para `raiz`, a quebra de
/// linha do arquivo e se algo mudou. O fonts.conf do TinyTeX aponta as pastas da maquina onde ele
/// foi montado (`C:/Users/runneradmin/AppData/Roaming/TinyTeX/...`): sem a correcao o XeLaTeX nao
/// acha pelo nome as fontes OpenType do proprio TinyTeX (Fira, TeX Gyre, Libertine...).
fn fonts_conf_corrigido(raiz: &Path) -> Option<(String, &'static str, bool)> {
    let caminho = raiz.join("texmf-var").join("fonts").join("conf").join("fonts.conf");
    let texto = std::fs::read_to_string(&caminho).ok()?;
    let base = raiz.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string();
    let mut mudou = false;
    let novo: Vec<String> = texto
        .lines()
        .map(|linha| {
            let limpo = linha.trim();
            let Some(dentro) = limpo.strip_prefix("<dir>").and_then(|r| r.strip_suffix("</dir>")) else {
                return linha.to_string();
            };
            let barra = dentro.replace('\\', "/");
            let Some(pos) = barra.to_lowercase().find("/texmf-dist/fonts/") else {
                return linha.to_string();
            };
            let certo = format!("{base}{}", &barra[pos..]);
            if barra.eq_ignore_ascii_case(&certo) {
                return linha.to_string();
            }
            mudou = true;
            format!("<dir>{certo}</dir>")
        })
        .collect();
    let quebra = if texto.contains("\r\n") { "\r\n" } else { "\n" };
    let fim = if texto.ends_with('\n') { quebra } else { "" };
    Some((novo.join(quebra) + fim, quebra, mudou))
}

/// Corrige no lugar o fonts.conf do TinyTeX em `raiz` (conta que grava no texmf-var).
/// Devolve se o arquivo mudou.
#[cfg(test)]
pub(crate) fn corrigir_fonts_conf(raiz: &Path) -> bool {
    corrigir_fonts_conf_em(raiz, None)
}

/// Prepara a configuracao de fontes. Sem `conta`, corrige o fonts.conf do TinyTeX no lugar. Com
/// `conta` (a conta nao grava no texmf-var do TinyTeX), nada do TinyTeX muda: grava a copia
/// corrigida em `conta\fontconfig\fonts.conf`, com o conf.d do TinyTeX pelo caminho absoluto e
/// o cache de fontes em `conta\fontconfig\cache`, e passa TEXMFVAR=`conta\texmf-var` e
/// FONTCONFIG_FILE aos programas do TeX. Devolve se a configuracao que vale mudou.
pub(crate) fn corrigir_fonts_conf_em(raiz: &Path, conta: Option<&Path>) -> bool {
    let mut ambiente: Vec<(&'static str, PathBuf)> = Vec::new();
    let mudou = match conta {
        None => corrigir_no_tinytex(raiz),
        Some(conta) => {
            let texmf_var = conta.join("texmf-var");
            match std::fs::create_dir_all(&texmf_var) {
                Ok(()) => ambiente.push(("TEXMFVAR", texmf_var)),
                Err(erro) => crate::registro::tex("texmf-var", &format!("não foi possível criar {}: {erro}", texmf_var.display())),
            }
            match copiar_fonts_conf(raiz, &conta.join("fontconfig")) {
                Some((destino, mudou)) => {
                    ambiente.push(("FONTCONFIG_FILE", destino));
                    mudou
                }
                None => false,
            }
        }
    };
    *travar(&AMBIENTE_CONTA) = ambiente;
    *travar(&PASTA_CONTA) = conta.map(Path::to_path_buf);
    mudou
}

fn corrigir_no_tinytex(raiz: &Path) -> bool {
    let Some((corrigido, _, true)) = fonts_conf_corrigido(raiz) else {
        return false;
    };
    let caminho = raiz.join("texmf-var").join("fonts").join("conf").join("fonts.conf");
    match std::fs::write(&caminho, corrigido) {
        Ok(()) => true,
        Err(erro) => {
            crate::registro::tex("fonts.conf", &format!("não foi possível gravar {}: {erro}", caminho.display()));
            false
        }
    }
}

/// Grava em `propria\fonts.conf` a copia do fonts.conf do TinyTeX para esta conta. Devolve o
/// caminho e se o conteudo mudou; `None` se nao houver fonts.conf ou a gravacao falhar.
fn copiar_fonts_conf(raiz: &Path, propria: &Path) -> Option<(PathBuf, bool)> {
    let (corrigido, quebra, _) = fonts_conf_corrigido(raiz)?;
    let conf_d = raiz.join("texmf-var").join("fonts").join("conf").join("conf.d").to_string_lossy().replace('\\', "/");
    let cache = propria.join("cache").to_string_lossy().replace('\\', "/");
    let copia = corrigido
        .replace(">conf.d</include>", &format!(">{conf_d}</include>"))
        .replacen("</fontconfig>", &format!("<cachedir>{cache}</cachedir>{quebra}</fontconfig>"), 1);
    let destino = propria.join("fonts.conf");
    let igual = std::fs::read_to_string(&destino).is_ok_and(|atual| atual == copia);
    if !igual
        && let Err(erro) = std::fs::create_dir_all(propria).and_then(|()| std::fs::write(&destino, &copia))
    {
        crate::registro::tex("fonts.conf", &format!("não foi possível gravar {}: {erro}", destino.display()));
        return None;
    }
    Some((destino, !igual))
}

/// Primeira abertura com o TinyTeX embutido, em segundo plano e com prioridade baixa: corrige o
/// fonts.conf, monta o cache de fontes do XeLaTeX (fc-cache) e a base de nomes de fontes do
/// LuaLaTeX (luaotfload-tool). Sem isso a primeira compilacao com fontspec gasta esse tempo.
/// Roda uma vez por TinyTeX (marca em texmf-var) e de novo se o fonts.conf precisar de correcao.
/// Numa conta que nao grava no texmf-var (o TinyTeX e de quem instalou), tudo isso vai para a
/// pasta desta conta, com a marca junto.
fn aquecer_tinytex(bin: &Path) {
    let Some(raiz) = bin.parent().and_then(Path::parent) else { return };
    let texmf_var = raiz.join("texmf-var");
    let conta = if pasta_gravavel(&texmf_var) {
        None
    } else {
        let conta = pasta_da_conta();
        crate::registro::tex(
            "texmf-var",
            &match &conta {
                Some(pasta) => format!("sem permissão de gravar em {}; TEXMFVAR e fonts.conf em {}", texmf_var.display(), pasta.display()),
                None => format!("sem permissão de gravar em {} e sem pasta própria para esta conta", texmf_var.display()),
            },
        );
        conta
    };
    let corrigiu = corrigir_fonts_conf_em(raiz, conta.as_deref());
    let marca = conta.as_deref().unwrap_or(&texmf_var).join(PREPARO_TINYTEX);
    if marca.is_file() && !corrigiu {
        return;
    }
    for (programa, argumentos) in [("fc-cache.exe", &["-v"][..]), ("luaotfload-tool.exe", &["--update"][..])] {
        let exe = bin.join(programa);
        if !exe.is_file() {
            continue;
        }
        let mut comando = Command::new(&exe);
        comando.args(argumentos);
        rodar_em_segundo_plano(&mut comando);
    }
    let _ = std::fs::write(&marca, "fc-cache e luaotfload-tool --update\n");
}

/// Preferencia avancada "Permitir shell-escape". Desligada ate o front-end ligar.
static SHELL_ESCAPE: AtomicBool = AtomicBool::new(false);

/// Liga ou desliga o shell-escape nas proximas compilacoes.
pub fn definir_shell_escape(liberado: bool) {
    SHELL_ESCAPE.store(liberado, Ordering::SeqCst);
}

fn shell_escape_liberado() -> bool {
    SHELL_ESCAPE.load(Ordering::SeqCst) || std::env::var_os("EGUATEX_SHELL_ESCAPE").is_some_and(|v| v == "1")
}

fn detectar_executavel(nome: &str) -> Option<PathBuf> {
    which::which(nome).ok().map(preferir_exe)
}

/// O `which` segue o PATHEXT e pode escolher um `.com` antes do `.exe`. Quando existe o `.exe`
/// irmao, o comando roda esse caminho completo.
pub(crate) fn preferir_exe(achado: PathBuf) -> PathBuf {
    let e_com = achado.extension().is_some_and(|e| e.eq_ignore_ascii_case("com"));
    if e_com {
        let exe = achado.with_extension("exe");
        if exe.is_file() {
            return exe;
        }
    }
    achado
}

/// Nome do motor que roda: o que a interface manda ou `pdflatex` se nao reconhecer.
pub(crate) fn nome_do_motor(motor: &str) -> &'static str {
    match motor {
        "pdflatex" | "pdfLaTeX" => "pdflatex",
        "xelatex" | "XeLaTeX" => "xelatex",
        "lualatex" | "LuaLaTeX" => "lualatex",
        _ => "pdflatex",
    }
}

/// Argumentos do motor. Sem a preferencia avancada o motor recebe `-shell-restricted`: so os
/// programas da lista segura do TeX Live (shell_escape_commands do texmf.cnf: latexminted,
/// bibtex, makeindex, kpsewhich...) rodam. Assim o minted 3 colore o codigo sem nenhuma opcao,
/// como no Overleaf, e um projeto baixado continua sem poder rodar um comando qualquer.
pub(crate) fn argumentos_motor(arquivo: &Path, parar_no_erro: bool, shell_escape: bool) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec!["-interaction=nonstopmode".into()];
    if parar_no_erro {
        args.push("-halt-on-error".into());
    }
    args.push(if shell_escape { "-shell-escape" } else { "-shell-restricted" }.into());
    args.push("-file-line-error".into());
    args.push("-synctex=1".into());
    args.push(arquivo.as_os_str().to_os_string());
    args
}

/// Primeira linha da saída de `caminho --version`. No Windows o processo roda sem janela
/// de terminal (`ocultar_janela`). Devolve vazio se o programa não rodar ou se a saída
/// não for UTF-8.
fn obter_versao(caminho: &Path) -> String {
    let mut comando = Command::new(caminho);
    comando.arg("--version").stdin(Stdio::null());
    ocultar_janela(&mut comando);
    comando
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

fn detectar_distribuicao() -> Option<String> {
    if detectar_executavel("miktex-pdflatex").is_some()
        || detectar_executavel("mpm").is_some()
    {
        return Some("MiKTeX".to_string());
    }
    if detectar_executavel("tlmgr").is_some() {
        return Some("TeX Live".to_string());
    }
    None
}

pub fn detectar_motores() -> InfoMotores {
    let nomes = ["pdflatex", "xelatex", "lualatex", "latexmk", "bibtex", "biber"];
    let mut motores = Vec::new();

    for nome in &nomes {
        if let Some(caminho) = detectar_executavel(nome) {
            let versao = obter_versao(&caminho);
            motores.push(MotorDetectado {
                nome: nome.to_string(),
                caminho: caminho.display().to_string(),
                versao,
            });
        }
    }

    InfoMotores {
        distribuicao: detectar_distribuicao(),
        motores,
    }
}

/// Frase que aparece no log quando o usuario interrompe a compilacao.
const MSG_INTERROMPIDA: &str = "Compilação interrompida pelo usuário.";

/// PID do processo filho em execucao (motor LaTeX ou bibtex/biber), se houver.
static PID_ATUAL: Mutex<Option<u32>> = Mutex::new(None);

/// Ligada por `cmd_parar_compilacao` quando mata o processo em execucao. A compilacao
/// le este valor depois que o processo termina, para saber que a saida foi cortada
/// de proposito e nao rodar as etapas seguintes.
static INTERROMPIDA: AtomicBool = AtomicBool::new(false);

/// Variaveis de ambiente do projeto em compilacao (pastas texmf do latexmkrc). Valem
/// para todos os programas da compilacao; so ha uma compilacao por vez.
static AMBIENTE: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// A interface pede o motor automatico ("auto", o padrao) ou um dos tres pelo nome.
pub(crate) fn motor_automatico(motor: &str) -> bool {
    !matches!(motor, "pdflatex" | "pdfLaTeX" | "xelatex" | "XeLaTeX" | "lualatex" | "LuaLaTeX")
}

/// Como no Overleaf, so um `.tex` com `\begin{document}` e documento principal. Se a interface
/// mandar um preambulo ou um capitulo (sem `\begin{document}`), compila o principal do projeto.
/// Devolve o arquivo que vale e, se trocou, a nota para o log.
pub(crate) fn resolver_principal(arquivo: &str, diretorio: &str) -> (String, Option<String>) {
    let raiz = Path::new(diretorio);
    let Ok(bytes) = std::fs::read(raiz.join(arquivo)) else {
        return (arquivo.to_string(), None);
    };
    if crate::arquivos::tem_begin_document(&String::from_utf8_lossy(&bytes)) {
        return (arquivo.to_string(), None);
    }
    match crate::arquivos::principal_do_projeto(raiz) {
        Some(p) if p != arquivo.replace('\\', "/") => {
            let nota = format!("{arquivo} não tem \\begin{{document}}; compilado o documento principal {p}.");
            (p, Some(nota))
        }
        _ => (arquivo.to_string(), None),
    }
}

/// Motor que a compilacao automatica usaria para `arquivo` (ja trocado pelo principal, se
/// preciso): comentario % !TeX program, latexmkrc, pacotes e, sem nada, pdfLaTeX.
pub fn detectar_motor(arquivo: &str, diretorio: &str) -> crate::config_projeto::MotorAutomatico {
    let (arquivo, _) = resolver_principal(arquivo, diretorio);
    let raiz = Path::new(diretorio);
    crate::config_projeto::detectar_motor(raiz, &raiz.join(arquivo))
}

/// Le a configuracao do projeto (latexmkrc, % !TeX program, pasta texmf), guarda as
/// variaveis de ambiente e devolve o motor que vale e as notas para o log. No modo
/// automatico o motor vem de `config_projeto::detectar_motor`; um motor escolhido a mao
/// vale mesmo que o projeto peca outro.
fn preparar_projeto(arquivo: &str, diretorio: &str, motor: &str) -> (String, String) {
    let raiz = Path::new(diretorio);
    let cfg = crate::config_projeto::ler(raiz, &raiz.join(arquivo));
    *travar(&AMBIENTE) = cfg.variaveis;
    let mut notas: Vec<String> = cfg.notas.iter().filter(|n| !n.starts_with("Motor ")).cloned().collect();
    let motor = if motor_automatico(motor) {
        let auto = crate::config_projeto::detectar_motor(raiz, &raiz.join(arquivo));
        notas.insert(0, format!("Motor automático: {} ({}).", nome_exibido(auto.motor), auto.motivo));
        auto.motor.to_string()
    } else {
        let escolhido = nome_do_motor(motor);
        if let Some(pedido) = cfg.motor.filter(|m| *m != escolhido) {
            notas.insert(0, format!("Motor {} escolhido na barra de status; o projeto pede {}.", nome_exibido(escolhido), nome_exibido(pedido)));
        }
        escolhido.to_string()
    };
    let notas: String = notas.iter().map(|n| format!("ÉguaTeX: {n}\n")).collect();
    (motor, notas)
}

/// Nome do motor como aparece na interface.
pub(crate) fn nome_exibido(motor: &str) -> &'static str {
    match nome_do_motor(motor) {
        "xelatex" => "XeLaTeX",
        "lualatex" => "LuaLaTeX",
        _ => "pdfLaTeX",
    }
}

fn com_notas(mut resultado: ResultadoCompilacao, notas: &str) -> ResultadoCompilacao {
    if !notas.is_empty() {
        resultado.log = juntar_notas(notas, &resultado.log);
    }
    resultado
}

/// Poe as notas do projeto no comeco do log, depois da linha `ÉguaTeX: tempos:` quando ela existe.
pub(crate) fn juntar_notas(notas: &str, log: &str) -> String {
    if log.starts_with("ÉguaTeX: tempos:")
        && let Some((tempos, resto)) = log.split_once('\n')
    {
        return format!("{tempos}\n{notas}\n{resto}");
    }
    format!("{notas}\n{log}")
}

/// Uma compilacao por vez. Os comandos de compilar sao assincronos e o frontend pode
/// pedir uma segunda enquanto a primeira roda; dois LaTeX na mesma pasta estragariam
/// os arquivos auxiliares um do outro (e so existe um PID guardado).
static UMA_POR_VEZ: Mutex<()> = Mutex::new(());

/// Nos testes, quem compila de verdade segura esta trava, para que um teste que
/// interrompe a compilacao nunca mate o processo de outro teste.
#[cfg(test)]
static TESTE_COMPILACAO: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn travar_teste() -> MutexGuard<'static, ()> {
    travar(&TESTE_COMPILACAO)
}

/// Pega a trava mesmo que outra thread tenha dado panic com ela na mao.
fn travar<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|erro| erro.into_inner())
}

fn foi_interrompida() -> bool {
    INTERROMPIDA.load(Ordering::SeqCst)
}

/// No Windows, impede que o processo filho abra uma janela de console.
fn ocultar_janela(comando: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        comando.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = comando;
}

/// Roda o comando e espera o fim, guardando o PID enquanto ele executa para que
/// `cmd_parar_compilacao` consiga interrompe-lo.
fn executar(comando: &mut Command) -> std::io::Result<Output> {
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    ocultar_janela(comando);
    com_ambiente_da_conta(comando);
    for (nome, valor) in travar(&AMBIENTE).iter() {
        comando.env(nome, valor);
    }
    let mut filho = comando.spawn()?;
    *travar(&PID_ATUAL) = Some(filho.id());
    // a saida e lida em threads com teto: um \typeout num laco sem fim nao enche a memoria
    let ler = |fonte: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut guardado = Vec::new();
            if let Some(mut f) = fonte {
                let mut bloco = [0u8; 64 * 1024];
                while let Ok(n) = f.read(&mut bloco) {
                    if n == 0 {
                        break;
                    }
                    if guardado.len() < LIMITE_SAIDA {
                        guardado.extend_from_slice(&bloco[..n]);
                    }
                }
            }
            guardado
        })
    };
    let t_saida = ler(filho.stdout.take().map(|f| Box::new(f) as Box<dyn std::io::Read + Send>));
    let t_erro = ler(filho.stderr.take().map(|f| Box::new(f) as Box<dyn std::io::Read + Send>));
    let inicio = Instant::now();
    let mut estourou = false;
    let status = loop {
        if let Some(status) = filho.try_wait()? {
            break status;
        }
        if inicio.elapsed() > LIMITE_TEMPO {
            estourou = true;
            matar_arvore(filho.id());
            let _ = filho.kill();
            break filho.wait()?;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    *travar(&PID_ATUAL) = None;
    let stdout = t_saida.join().unwrap_or_default();
    let mut stderr = t_erro.join().unwrap_or_default();
    if estourou {
        stderr.extend_from_slice(
            format!(
                "\nÉguaTeX: a compilação passou de {} minutos e foi interrompida. Procure um laço sem fim no documento.\n",
                LIMITE_TEMPO.as_secs() / 60
            )
            .as_bytes(),
        );
    }
    Ok(Output { status, stdout, stderr })
}

/// Teto da saida guardada de cada programa (o .log completo continua no disco).
const LIMITE_SAIDA: usize = 20 * 1024 * 1024;
/// Tempo maximo de um programa (motor, bibtex, biber, makeindex).
const LIMITE_TEMPO: Duration = Duration::from_secs(10 * 60);

/// Mata o processo e a arvore de filhos dele (o motor LaTeX chama outros programas).
fn matar_arvore(pid: u32) -> bool {
    let (programa, argumentos): (&str, Vec<String>) = if cfg!(windows) {
        (
            "taskkill",
            vec![
                "/PID".to_string(),
                pid.to_string(),
                "/T".to_string(),
                "/F".to_string(),
            ],
        )
    } else {
        ("kill", vec!["-9".to_string(), pid.to_string()])
    };
    let mut comando = Command::new(programa);
    comando
        .args(argumentos)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    ocultar_janela(&mut comando);
    comando
        .status()
        .map(|estado| estado.success())
        .unwrap_or(false)
}

/// Termo de busca de uma fonte pedida pelo nome ao fontspec: "Fira Code" vira "firacode",
/// "EBGaramond-Regular.otf" vira "ebgaramond-regular". None para nome invalido.
pub fn termo_da_fonte(nome: &str) -> Option<String> {
    let nome = nome.trim();
    let sem_ext = ["otf", "ttf", "pfb", "OTF", "TTF"]
        .iter()
        .find_map(|e| nome.strip_suffix(&format!(".{e}")))
        .unwrap_or(nome);
    let valido = !sem_ext.is_empty()
        && sem_ext.len() <= 80
        && sem_ext.chars().all(|c| c.is_ascii_alphanumeric() || " ._-+".contains(c));
    valido.then(|| sem_ext.chars().filter(|c| *c != ' ').collect::<String>().to_ascii_lowercase())
}

/// Na saida do `tlmgr search --global --file`, o primeiro pacote com um arquivo de fonte
/// (.otf, .ttf, .pfb) cujo nome comeca pelo termo.
pub fn pacote_da_fonte(busca: &str, termo: &str) -> Option<String> {
    let mut atual = "";
    for linha in busca.lines() {
        if let Some(p) = linha.strip_suffix(':').filter(|p| !p.starts_with(' ') && !p.starts_with('\t') && !p.contains(' ')) {
            atual = p;
            continue;
        }
        let arquivo = linha.trim().rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        let e_fonte = [".otf", ".ttf", ".pfb"].iter().any(|e| arquivo.ends_with(e));
        if e_fonte && !atual.is_empty() && arquivo.replace(' ', "").starts_with(termo) {
            return Some(atual.to_string());
        }
    }
    None
}

/// Instala o pacote do TeX Live que tem `arquivo` (por exemplo `physics.sty`), pelo
/// tlmgr do TinyTeX embutido ou de um TeX Live instalado. Precisa de internet. Devolve o
/// nome do pacote instalado. `fonte:Nome` procura a fonte que o fontspec nao achou
/// ("fonte:Fira Code") e, depois de instalar, refaz os caches de fontes do XeLaTeX e do LuaLaTeX.
#[tauri::command(async)]
pub fn cmd_instalar_pacote_tex(arquivo: String) -> Result<String, String> {
    let fonte = arquivo.trim().strip_prefix("fonte:").map(str::to_string);
    let termo = match &fonte {
        Some(f) => termo_da_fonte(f).ok_or_else(|| format!("Nome de fonte inválido: {f}"))?,
        None => String::new(),
    };
    let nome = if fonte.is_some() { termo.as_str() } else { arquivo.trim() };
    let valido = !nome.is_empty()
        && nome.len() <= 120
        && nome.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b));
    if !valido {
        return Err(format!("Nome de arquivo inválido: {nome}"));
    }
    if detectar_executavel("miktex-pdflatex").is_some() || detectar_executavel("mpm").is_some() {
        return Err("O MiKTeX instala sozinho os pacotes que faltam. Abra o MiKTeX Console, em Configurações escolha \"Instalar pacotes que faltam: sempre\" e compile de novo.".into());
    }
    let Some(tlmgr) = detectar_executavel("tlmgr") else {
        return Err("O instalador de pacotes (tlmgr) não foi encontrado.".into());
    };
    // o tlmgr roda os programas da mesma instalacao: a pasta dele vem primeiro no PATH
    let pasta_bin = tlmgr.parent().map(Path::to_path_buf).unwrap_or_default();
    let caminho = std::env::var_os("PATH")
        .map(|p| {
            let mut v = vec![pasta_bin.clone()];
            v.extend(std::env::split_paths(&p));
            std::env::join_paths(v).unwrap_or(p)
        })
        .unwrap_or_default();
    let rodar = |args: &[&str]| -> Result<String, String> {
        let mut c = Command::new(&tlmgr);
        c.args(args).env("PATH", &caminho);
        let saida = executar(&mut c).map_err(|e| format!("Falha ao executar o tlmgr: {e}"))?;
        let texto = format!("{}{}", String::from_utf8_lossy(&saida.stdout), String::from_utf8_lossy(&saida.stderr));
        if saida.status.success() { Ok(texto) } else { Err(texto) }
    };
    let busca = rodar(&["search", "--global", "--file", nome])
        .map_err(|e| format!("Não foi possível consultar o repositório do TeX Live (precisa de internet).\n{e}"))?;
    if let Some(f) = &fonte {
        let Some(pacote) = pacote_da_fonte(&busca, &termo) else {
            return Err(format!(
                "A fonte \"{f}\" não existe no TeX Live. Instale a fonte no Windows (arquivo .otf ou .ttf, clique direito, Instalar para todos os usuários) ou troque por outra no documento."
            ));
        };
        rodar(&["install", &pacote]).map_err(|e| format!("A instalação do pacote {pacote} falhou.\n{e}"))?;
        // o XeLaTeX acha fontes pelo cache do fontconfig e o LuaLaTeX pela base do luaotfload
        for (programa, argumentos) in [("fc-cache.exe", &["-f"][..]), ("luaotfload-tool.exe", &["--update"][..])] {
            let exe = pasta_bin.join(programa);
            if exe.is_file() {
                let mut c = Command::new(&exe);
                c.args(argumentos).env("PATH", &caminho);
                let _ = executar(&mut c);
            }
        }
        return Ok(pacote);
    }
    // a saida lista "pacote:" seguido dos caminhos; vale o primeiro cujo caminho termina no arquivo
    let mut pacote = None;
    let mut atual = "";
    for linha in busca.lines() {
        if let Some(p) = linha.strip_suffix(':').filter(|p| !p.starts_with(' ') && !p.contains(' ')) {
            atual = p;
        } else if linha.trim_end().ends_with(&format!("/{nome}")) && !atual.is_empty() {
            pacote = Some(atual.to_string());
            break;
        }
    }
    let Some(pacote) = pacote else {
        return Err(format!("Nenhum pacote do TeX Live tem o arquivo {nome}. Confira o nome no documento."));
    };
    rodar(&["install", &pacote]).map_err(|e| format!("A instalação do pacote {pacote} falhou.\n{e}"))?;
    Ok(pacote)
}

/// Interrompe o processo da compilacao em andamento (motor LaTeX ou bibtex/biber) e
/// cancela as etapas que viriam depois. Devolve true se havia um processo rodando e ele
/// foi interrompido; false se nao havia nada rodando.
#[tauri::command]
pub fn cmd_parar_compilacao() -> bool {
    // A trava do PID fica presa enquanto o kill roda: a compilacao so confere o sinal de
    // interrupcao depois de soltar o PID, entao nunca o ve pela metade.
    let guarda = travar(&PID_ATUAL);
    let Some(pid) = *guarda else {
        // a compilacao ainda espera o preparo da primeira abertura: para a espera
        return cancelar_espera_do_preparo();
    };
    INTERROMPIDA.store(true, Ordering::SeqCst);
    let interrompeu = matar_arvore(pid);
    if !interrompeu {
        INTERROMPIDA.store(false, Ordering::SeqCst);
    }
    interrompeu
}

/// Resultado de uma compilacao interrompida: falha, sem PDF, sem erros do LaTeX e com a
/// frase combinada no log (uma vez so).
fn interrompida(mut resultado: ResultadoCompilacao) -> ResultadoCompilacao {
    resultado.sucesso = false;
    resultado.pdf = None;
    resultado.erros.clear();
    if !resultado.log.contains(MSG_INTERROMPIDA) {
        resultado.log.push_str(&format!("\n{}\n", MSG_INTERROMPIDA));
    }
    resultado
}

/// Compila uma vez. Com `parar_no_erro` o motor recebe `-halt-on-error` e para no
/// primeiro erro; sem ele continua apesar dos erros e ainda gera o PDF.
pub fn compilar(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
) -> ResultadoCompilacao {
    let _vez = travar(&UMA_POR_VEZ);
    INTERROMPIDA.store(false, Ordering::SeqCst);
    if !esperar_preparo() {
        return espera_cancelada(motor);
    }
    conferir_pasta_da_conta();
    let (arquivo, troca) = resolver_principal(arquivo, diretorio);
    let (motor, notas) = preparar_projeto(&arquivo, diretorio, motor);
    let notas = format!("{}{notas}", troca.map(|t| format!("ÉguaTeX: {t}\n")).unwrap_or_default());
    let (pasta, nome, sub) = separar_pasta(&arquivo, diretorio);
    let resultado = compilar_uma_vez(&nome, &pasta, &motor, parar_no_erro, false);
    com_notas(com_subpasta(resultado, &sub), &notas)
}

/// O LaTeX procura .sty, .cls, .bib e imagens na pasta onde roda. Quando o documento
/// principal esta numa subpasta do projeto (ex.: template-latex/main.tex), a compilacao
/// roda dentro dela. Devolve (pasta onde rodar, nome do arquivo, subpasta relativa).
fn separar_pasta(arquivo: &str, diretorio: &str) -> (String, String, String) {
    let caminho = Path::new(arquivo);
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| arquivo.to_string());
    match caminho.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(sub) => (
            Path::new(diretorio).join(sub).to_string_lossy().to_string(),
            nome,
            sub.to_string_lossy().replace('\\', "/"),
        ),
        None => (diretorio.to_string(), nome, String::new()),
    }
}

/// Os erros saem com caminhos relativos a pasta onde o LaTeX rodou. Acrescenta a
/// subpasta para que o clique no erro abra o arquivo certo do projeto.
fn com_subpasta(mut resultado: ResultadoCompilacao, sub: &str) -> ResultadoCompilacao {
    if sub.is_empty() {
        return resultado;
    }
    for erro in &mut resultado.erros {
        if let Some(a) = &erro.arquivo {
            let limpo = a.trim_start_matches("./");
            if !Path::new(limpo).is_absolute() {
                erro.arquivo = Some(format!("./{sub}/{limpo}"));
            }
        }
    }
    resultado
}

/// Argumento de rascunho de cada motor: o pdfLaTeX e o LuaLaTeX nao gravam o PDF
/// (-draftmode) e o XeLaTeX grava so o .xdv, sem chamar o xdvipdfmx (-no-pdf).
pub(crate) fn argumento_rascunho(motor: &str) -> &'static str {
    if nome_do_motor(motor) == "xelatex" { "-no-pdf" } else { "-draftmode" }
}

/// Uma passada do motor. Com `rascunho` o motor nao grava o PDF (passada intermediaria, que
/// so serve para gravar o .aux, o sumario e as referencias): o resultado sai sem PDF.
fn compilar_uma_vez(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
    rascunho: bool,
) -> ResultadoCompilacao {
    let marco = std::time::SystemTime::now();
    let inicio = std::time::Instant::now();
    let dir = Path::new(diretorio);
    let arquivo_path = Path::new(arquivo);

    let nome_motor = nome_do_motor(motor);

    let executavel = match detectar_executavel(nome_motor) {
        Some(p) => p,
        None => {
            return ResultadoCompilacao {
                sucesso: false,
                pdf: None,
                log: format!("Motor '{}' não encontrado no sistema. Instale o MiKTeX ou o TeX Live.", nome_motor),
                erros: vec![ErroCompilacao {
                    linha: None,
                    arquivo: None,
                    mensagem: format!("Motor '{}' não encontrado", nome_motor),
                    tipo: "fatal".to_string(),
                    categoria: None,
                }],
                motor_usado: nome_motor.to_string(),
                tempo_ms: inicio.elapsed().as_millis() as u64,
                motor_lembrar: None,
            };
        }
    };

    let mut argumentos = argumentos_motor(arquivo_path, parar_no_erro, shell_escape_liberado());
    if rascunho {
        // o rascunho nao grava PDF, entao o .synctex.gz dele nao serve: a passada que grava o PDF faz o dela
        argumentos.retain(|a| a != "-synctex=1");
        argumentos.insert(0, argumento_rascunho(nome_motor).into());
    }
    let mut comando = Command::new(&executavel);
    comando.current_dir(dir).args(argumentos);
    let resultado = executar(&mut comando);

    let tempo_ms = inicio.elapsed().as_millis() as u64;

    match resultado {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let log_completo = format!("{}\n{}", stdout, stderr);

            if foi_interrompida() {
                return interrompida(ResultadoCompilacao {
                    sucesso: false,
                    pdf: None,
                    log: log_completo,
                    erros: Vec::new(),
                    motor_usado: nome_motor.to_string(),
                    tempo_ms,
                    motor_lembrar: None,
                });
            }

            let principal = arquivo_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let erros = extrair_erros(&log_completo, &principal);

            let nome_base = arquivo_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let pdf_path = dir.join(format!("{}.pdf", nome_base));
            // so vale o PDF gravado nesta passada; o de uma compilacao anterior nao conta
            let gravado_agora = std::fs::metadata(&pdf_path)
                .and_then(|m| m.modified())
                .is_ok_and(|t| t + Duration::from_secs(2) >= marco);
            // "No pages of output": o PDF que esta la e de uma compilacao de ate 2 s atras (a folga
            // do relogio do sistema de arquivos), por exemplo do motor que rodou antes desta tentativa
            let sem_paginas = log_completo.contains("No pages of output");
            let pdf = if gravado_agora && !rascunho && !sem_paginas {
                Some(pdf_path.display().to_string())
            } else {
                None
            };

            ResultadoCompilacao {
                sucesso: output.status.success() && pdf.is_some(),
                pdf,
                log: log_completo,
                erros,
                motor_usado: nome_motor.to_string(),
                tempo_ms,
                motor_lembrar: None,
            }
        }
        Err(e) => ResultadoCompilacao {
            sucesso: false,
            pdf: None,
            log: format!("Falha ao executar {}: {}", nome_motor, e),
            erros: vec![ErroCompilacao {
                linha: None,
                arquivo: None,
                mensagem: e.to_string(),
                tipo: "fatal".to_string(),
                categoria: None,
            }],
            motor_usado: nome_motor.to_string(),
            tempo_ms,
            motor_lembrar: None,
        },
    }
}

/// Sufixos dos arquivos que o LaTeX grava numa passada e le na seguinte (referencias,
/// sumario, listas, bibliografia, paginas do backref). Se nenhum mudou numa passada, a
/// proxima daria o mesmo PDF.
const SUFIXOS_ENTRE_PASSADAS: &[&str] = &[
    ".aux", ".toc", ".lof", ".lot", ".out", ".brf", ".bbl", ".nav", ".snm", ".ind", ".loa",
    ".lol", ".nls",
];

/// Maximo de passadas do motor numa compilacao completa (o mesmo limite do latexmk).
const MAX_PASSADAS: u32 = 5;

/// Hash do conteudo de cada arquivo auxiliar da pasta e das subpastas (sem pastas ocultas).
fn retrato_auxiliares(dir: &Path) -> BTreeMap<PathBuf, u64> {
    let mut retrato = BTreeMap::new();
    juntar_auxiliares(dir, 0, &mut retrato);
    retrato
}

fn juntar_auxiliares(pasta: &Path, nivel: u32, retrato: &mut BTreeMap<PathBuf, u64>) {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let Ok(tipo) = entrada.file_type() else {
            continue;
        };
        let nome = entrada.file_name().to_string_lossy().to_lowercase();
        if tipo.is_dir() {
            if nivel < 6 && !nome.starts_with('.') {
                juntar_auxiliares(&entrada.path(), nivel + 1, retrato);
            }
        } else if tipo.is_file() && SUFIXOS_ENTRE_PASSADAS.iter().any(|s| nome.ends_with(s))
            && let Ok(bytes) = std::fs::read(entrada.path()) {
                let mut hash = DefaultHasher::new();
                bytes.hash(&mut hash);
                retrato.insert(entrada.path(), hash.finish());
            }
    }
}

/// Programa que monta a bibliografia: bibtex quando algum .aux tem \bibdata (o
/// \bibliography do documento), biber quando o documento usa biblatex com biber (marcas
/// \abx@aux no .aux principal e o .bcf ao lado), nenhum quando nao ha bibliografia.
fn ferramenta_bibliografia(
    dir: &Path,
    nome_base: &str,
    auxiliares: &BTreeMap<PathBuf, u64>,
) -> Option<&'static str> {
    let ler = |caminho: &Path| String::from_utf8_lossy(&std::fs::read(caminho).unwrap_or_default()).into_owned();
    let _ = auxiliares;
    // so o .aux do documento e os dos \include dele: um .aux velho de outro documento
    // numa subpasta nao escolhe o programa errado
    let principal = ler(&dir.join(format!("{nome_base}.aux")));
    let incluidos: Vec<String> = principal
        .match_indices("\\@input{")
        .filter_map(|(i, _)| {
            let resto = &principal[i + "\\@input{".len()..];
            resto.find('}').map(|fim| ler(&dir.join(&resto[..fim])))
        })
        .collect();
    let pede_bibtex = principal.contains("\\bibdata") || incluidos.iter().any(|t| t.contains("\\bibdata"));
    if pede_bibtex {
        return Some("bibtex");
    }
    let usa_biblatex = ler(&dir.join(format!("{nome_base}.aux"))).contains("\\abx@aux");
    if usa_biblatex && dir.join(format!("{nome_base}.bcf")).is_file() {
        return Some("biber");
    }
    None
}

/// Entrada da bibliografia da ultima vez que o bibtex ou o biber rodou em cada documento
/// (pasta e nome base). Com a mesma entrada e o .bbl no lugar, a bibliografia sairia igual.
static BIBLIOGRAFIA_VISTA: Mutex<BTreeMap<PathBuf, u64>> = Mutex::new(BTreeMap::new());

/// Linhas de todos os .aux que o bibtex le: o do documento e os que ele chama por
/// `\@input{...}` (os dos `\include`), em profundidade, com o caminho relativo a pasta de saida.
fn auxiliares_do_bibtex(dir: &Path, aux: &str, vistos: &mut Vec<PathBuf>, linhas: &mut Vec<String>) {
    let caminho = dir.join(aux);
    if vistos.contains(&caminho) || vistos.len() > 500 {
        return;
    }
    vistos.push(caminho.clone());
    let texto = String::from_utf8_lossy(&std::fs::read(&caminho).unwrap_or_default()).into_owned();
    for linha in texto.lines() {
        if linha.starts_with("\\citation") || linha.starts_with("\\bibdata") || linha.starts_with("\\bibstyle") {
            linhas.push(linha.to_string());
        }
        for (i, _) in linha.match_indices("\\@input{") {
            let resto = &linha[i + "\\@input{".len()..];
            if let Some(fim) = resto.find('}') {
                auxiliares_do_bibtex(dir, &resto[..fim], vistos, linhas);
            }
        }
    }
}

/// Valor do primeiro argumento entre chaves depois de `comando` em `linha`.
fn argumento_de<'a>(linha: &'a str, comando: &str) -> Option<&'a str> {
    let resto = linha.strip_prefix(comando)?.strip_prefix('{')?;
    resto.find('}').map(|fim| &resto[..fim])
}

/// Arquivos .bib que o biber le: o texto de cada `<bcf:datasource ...>nome</bcf:datasource>`.
fn bibs_do_bcf(bcf: &str) -> Vec<String> {
    let mut nomes = Vec::new();
    for (i, _) in bcf.match_indices("<bcf:datasource") {
        let resto = &bcf[i..];
        let (Some(abre), Some(fecha)) = (resto.find('>'), resto.find("</bcf:datasource>")) else { continue };
        if abre < fecha {
            let nome = resto[abre + 1..fecha].trim().replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
            if !nome.is_empty() {
                nomes.push(nome);
            }
        }
    }
    nomes
}

/// Hash do que o bibtex ou o biber leem: o .bcf (biber) ou as linhas \citation, \bibdata e
/// \bibstyle do .aux e dos .aux dos `\include` (bibtex), e o conteudo de cada .bib nomeado ali
/// (\bibdata ou datasource do .bcf), pelo caminho relativo a pasta de saida ou absoluto, dentro
/// ou fora do projeto. Devolve `None` se algum .bib nomeado nao foi achado (pode vir da
/// distribuicao TeX pelo kpathsea): ai a bibliografia roda sempre.
fn entrada_bibliografia(dir: &Path, nome_base: &str, programa: &str) -> Option<u64> {
    let mut hash = DefaultHasher::new();
    programa.hash(&mut hash);
    let mut bibs: Vec<String> = Vec::new();
    if programa == "biber" {
        let bcf = std::fs::read(dir.join(format!("{nome_base}.bcf"))).ok()?;
        bcf.hash(&mut hash);
        bibs = bibs_do_bcf(&String::from_utf8_lossy(&bcf));
    } else {
        let mut linhas = Vec::new();
        auxiliares_do_bibtex(dir, &format!("{nome_base}.aux"), &mut Vec::new(), &mut linhas);
        for linha in &linhas {
            linha.hash(&mut hash);
            if let Some(lista) = argumento_de(linha, "\\bibdata") {
                for nome in lista.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                    let com_ext = if nome.to_lowercase().ends_with(".bib") { nome.to_string() } else { format!("{nome}.bib") };
                    bibs.push(com_ext);
                }
            }
            // o .bst do projeto entra no hash; o da distribuicao nao muda entre compilacoes
            if let Some(estilo) = argumento_de(linha, "\\bibstyle") {
                let bst = dir.join(format!("{}.bst", estilo.trim()));
                if let Ok(conteudo) = std::fs::read(&bst) {
                    conteudo.hash(&mut hash);
                }
            }
        }
    }
    for nome in bibs {
        // Path::join com caminho absoluto devolve o proprio caminho absoluto
        let conteudo = std::fs::read(dir.join(&nome)).ok()?;
        nome.hash(&mut hash);
        conteudo.hash(&mut hash);
    }
    Some(hash.finish())
}

/// Roda bibtex ou biber na pasta do projeto. Devolve a saida para o log e, se ele faltar
/// ou terminar com erro, a mensagem para a lista de erros.
fn rodar_bibliografia(programa: &str, dir: &Path, nome_base: &str) -> (String, Option<String>) {
    let Some(executavel) = detectar_executavel(programa) else {
        let msg = format!("{programa} não encontrado no sistema: a bibliografia não foi montada.");
        return (format!("{msg}\n"), Some(msg));
    };
    let mut comando = Command::new(&executavel);
    comando.current_dir(dir).arg(nome_base);
    match executar(&mut comando) {
        Ok(saida) => {
            let texto = format!(
                "{}{}",
                String::from_utf8_lossy(&saida.stdout),
                String::from_utf8_lossy(&saida.stderr)
            );
            let problema = (!saida.status.success()).then(|| {
                let detalhe = texto
                    .lines()
                    .find(|l| l.contains("ERROR") || l.contains("error") || l.starts_with("I couldn't") || l.contains("I found no"))
                    .unwrap_or("veja o log")
                    .trim()
                    .to_string();
                format!("O {programa} terminou com erro e a bibliografia pode estar incompleta: {detalhe}")
            });
            (texto, problema)
        }
        Err(e) => {
            let msg = format!("Falha ao executar {programa}: {e}");
            (format!("{msg}\n"), Some(msg))
        }
    }
}

/// Indices que o makeindex monta: o indice remissivo (\makeindex, .idx vira .ind) e a
/// lista de simbolos do pacote nomencl (.nlo vira .nls). Cada item: entrada, saida e
/// estilo opcional.
const INDICES: &[(&str, &str, Option<&str>)] =
    &[("idx", "ind", None), ("nlo", "nls", Some("nomencl.ist"))];

/// Hash de um arquivo, ou None se ele nao existir ou estiver vazio.
fn hash_arquivo(caminho: &Path) -> Option<u64> {
    let bytes = std::fs::read(caminho).ok().filter(|b| !b.is_empty())?;
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    Some(hash.finish())
}

/// Roda o makeindex para cada indice cujo arquivo de entrada mudou desde a ultima vez.
/// `vistos` guarda o hash da entrada de cada indice ja montado. Devolve o texto do log.
fn rodar_indices(dir: &Path, nome_base: &str, vistos: &mut BTreeMap<&'static str, u64>) -> String {
    let mut log = String::new();
    for &(entrada, saida, estilo) in INDICES {
        let arquivo = dir.join(format!("{nome_base}.{entrada}"));
        let Some(hash) = hash_arquivo(&arquivo) else { continue };
        if vistos.get(entrada) == Some(&hash) {
            continue;
        }
        vistos.insert(entrada, hash);
        log.push_str(&format!("\n--- makeindex ({entrada}) ---\n"));
        let Some(executavel) = detectar_executavel("makeindex") else {
            log.push_str("makeindex não encontrado no sistema: o índice não foi montado.\n");
            continue;
        };
        let mut comando = Command::new(&executavel);
        comando.current_dir(dir);
        if let Some(ist) = estilo {
            comando.arg("-s").arg(ist);
        }
        comando
            .arg("-o")
            .arg(format!("{nome_base}.{saida}"))
            .arg(format!("{nome_base}.{entrada}"));
        match executar(&mut comando) {
            Ok(s) => {
                log.push_str(&String::from_utf8_lossy(&s.stdout));
                log.push_str(&String::from_utf8_lossy(&s.stderr));
            }
            Err(e) => log.push_str(&format!("Falha ao executar makeindex: {e}\n")),
        }
    }
    log
}

/// Compilacao completa, como o latexmk: roda o motor, monta a bibliografia (bibtex ou
/// biber, se o documento tiver uma) e repete o motor enquanto os arquivos auxiliares
/// mudarem, ate MAX_PASSADAS. Documento sem mudanca compila uma vez so. Com `parar_no_erro`
/// desligado, o ciclo segue apesar dos erros sempre que sai PDF, para o sumario e as
/// citacoes ficarem certos. Se o usuario interromper em qualquer etapa, as seguintes nao
/// rodam.
    #[allow(dead_code)]
    pub fn compilar_com_bibtex(
        arquivo: &str,
        diretorio: &str,
        motor: &str,
        parar_no_erro: bool,
    ) -> ResultadoCompilacao {
        compilar_com_bibtex_ate(arquivo, diretorio, motor, parar_no_erro, None)
    }

    /// Igual a `compilar_com_bibtex`, com o teto de passadas escolhido pelo usuario
    /// (1 a MAX_PASSADAS; `None` ou fora da faixa vale MAX_PASSADAS).
    #[allow(dead_code)]
    pub fn compilar_com_bibtex_ate(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
    max_passadas: Option<u32>,
) -> ResultadoCompilacao {
    compilar_completo(arquivo, diretorio, motor, parar_no_erro, max_passadas, None)
}

/// O log mostra um erro tipico de motor errado, que outro motor pode resolver: fontspec ou
/// "XeTeX/LuaTeX is required" (`motor_pelo_log`), fonte que o motor nao carrega, caractere
/// Unicode que o pdfLaTeX nao conhece, estouro de memoria do TeX. Erros do documento (arquivo do
/// `\input` ausente, Runaway argument, comando desconhecido comum, pacote nao instalado) nao contam:
/// os tres motores dariam o mesmo erro.
pub(crate) fn erro_de_motor(log: &str, motor: &str) -> bool {
    if crate::config_projeto::motor_pelo_log(log, motor).is_some() {
        return true;
    }
    // o TeX quebra as linhas do log em 79 colunas
    let log = log.replace(['\r', '\n'], "");
    let tipicos = [
        "is required",
        "not loadable",
        "Missing character",
        "not set up for use with LaTeX",
        "TeX capacity exceeded",
        "Package fontspec Error",
        "Package inputenc Error: Unicode",
    ];
    tipicos.iter().any(|t| log.contains(t))
}

/// Motores para tentar no automatico depois que `primeiro` falhou, na ordem: o que o log apontou
/// (fontspec no pdfLaTeX, \directlua indefinido), o detectado (quando o primeiro era o lembrado),
/// XeLaTeX e LuaLaTeX, sem repetir o `primeiro`. Com PDF, so o que o log apontou. Sem PDF, os
/// outros so quando o erro e de motor (`erro_de_motor`).
pub(crate) fn motores_alternativos(
    primeiro: &str,
    detectado: &str,
    log: &str,
    sucesso: bool,
    com_pdf: bool,
) -> Vec<&'static str> {
    let primeiro = nome_do_motor(primeiro);
    let pelo_log = if sucesso { None } else { crate::config_projeto::motor_pelo_log(log, primeiro) };
    let mut lista: Vec<&'static str> = pelo_log.into_iter().collect();
    if !com_pdf && erro_de_motor(log, primeiro) {
        lista.extend([nome_do_motor(detectado), "xelatex", "lualatex"]);
    }
    let mut saida: Vec<&'static str> = Vec::new();
    for m in lista {
        if m != primeiro && !saida.contains(&m) {
            saida.push(m);
        }
    }
    saida
}

/// Compilacao completa com o motor lembrado do projeto. No automatico (como no Overleaf, sem
/// escolha de motor na interface) roda primeiro o `lembrado` (o motor que funcionou da ultima
/// vez, se difere do detectado) ou o detectado. Se essa compilacao falha sem gerar o PDF, tenta
/// os outros motores (`motores_alternativos`) e fica com o primeiro que gerar PDF; o
/// `motor_lembrar` do resultado diz qual guardar ("motor@impressao"). O lembrado so vale enquanto a
/// impressao do documento (`config_projeto::impressao_do_documento`) for a mesma. Se todos falham,
/// mostra o erro e o .log do primeiro motor (ou do que o log apontou como certo). Com
/// `% !TeX program` ou motor no latexmkrc vale so o motor pedido.
pub fn compilar_completo(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
    max_passadas: Option<u32>,
    lembrado: Option<&str>,
) -> ResultadoCompilacao {
    let _vez = travar(&UMA_POR_VEZ);
    INTERROMPIDA.store(false, Ordering::SeqCst);
    if !esperar_preparo() {
        return espera_cancelada(motor);
    }
    conferir_pasta_da_conta();
    let automatico = motor_automatico(motor);
    let (arquivo, troca) = resolver_principal(arquivo, diretorio);
    let (detectado, notas) = preparar_projeto(&arquivo, diretorio, motor);
    let detectado = nome_do_motor(&detectado);
    let mut notas = format!("{}{notas}", troca.map(|t| format!("ÉguaTeX: {t}\n")).unwrap_or_default());
    let (pasta, nome, sub) = separar_pasta(&arquivo, diretorio);
    let teto = limitar_passadas(max_passadas);
    // % !TeX program ou motor no latexmkrc ($pdf_mode, $pdflatex) mandam: nem motor lembrado nem
    // outro motor depois de uma falha
    let raiz = Path::new(diretorio);
    let caminho_principal = raiz.join(&arquivo);
    let fixo = automatico && crate::config_projeto::ler(raiz, &caminho_principal).motor.is_some();
    let alternativas = automatico && !fixo;
    let impressao = if alternativas {
        crate::config_projeto::impressao_do_documento(raiz, &caminho_principal)
    } else {
        String::new()
    };
    let lembrado = match lembrado.filter(|_| alternativas).and_then(|l| l.split_once('@')) {
        Some((m, imp)) if !motor_automatico(m) && nome_do_motor(m) != detectado => {
            if imp == impressao {
                notas.push_str(&format!(
                    "ÉguaTeX: motor lembrado deste projeto: {} (gerou o PDF da última vez).\n",
                    nome_exibido(m)
                ));
                Some(nome_do_motor(m))
            } else {
                notas.push_str(&format!(
                    "ÉguaTeX: o preâmbulo ou os pacotes mudaram; o {} lembrado foi esquecido e o motor volta a ser detectado.\n",
                    nome_exibido(m)
                ));
                None
            }
        }
        _ => None,
    };
    let primeiro = lembrado.unwrap_or(detectado);
    let mut resultado = compilar_ciclo(&nome, &pasta, primeiro, parar_no_erro, teto);
    if alternativas && !foi_interrompida() {
        // .log do motor cujo erro aparece se nenhum outro gerar o PDF: os outros motores gravam por cima
        let caminho_log = Path::new(&pasta).join(format!(
            "{}.log",
            Path::new(&nome).file_stem().unwrap_or_default().to_string_lossy()
        ));
        let mut log_mostrado = std::fs::read(&caminho_log).ok();
        let pelo_log = crate::config_projeto::motor_pelo_log(&resultado.log, primeiro).filter(|_| !resultado.sucesso);
        let candidatos =
            motores_alternativos(primeiro, detectado, &resultado.log, resultado.sucesso, resultado.pdf.is_some());
        let mut tempo = resultado.tempo_ms;
        let mut tentados: Vec<&str> = Vec::new();
        let mut achou = false;
        for m in candidatos {
            if foi_interrompida() {
                break;
            }
            let r = compilar_ciclo(&nome, &pasta, m, parar_no_erro, teto);
            tempo += r.tempo_ms;
            tentados.push(m);
            if r.pdf.is_some() {
                let motivo = if pelo_log == Some(m) { "acusou motor errado" } else { "não gerou o PDF" };
                notas.push_str(&format!(
                    "ÉguaTeX: o {} {motivo}; compilado de novo com o {}.\n",
                    nome_exibido(primeiro),
                    nome_exibido(m)
                ));
                resultado = r;
                achou = true;
                break;
            }
            // o motor que o log apontou como certo tem o erro mais util para mostrar
            if pelo_log == Some(m) || foi_interrompida() {
                resultado = r;
                log_mostrado = std::fs::read(&caminho_log).ok();
            }
        }
        if !achou && !tentados.is_empty() {
            if let Some(bytes) = &log_mostrado {
                let _ = std::fs::write(&caminho_log, bytes);
            }
            let nomes: Vec<&str> = tentados.iter().map(|m| nome_exibido(m)).collect();
            notas.push_str(&format!(
                "ÉguaTeX: tentado também com {} e nenhum gerou o PDF; os erros abaixo são do {}.\n",
                nomes.join(" e "),
                nome_exibido(&resultado.motor_usado)
            ));
        }
        resultado.tempo_ms = tempo;
    }
    resultado.motor_lembrar = (alternativas && resultado.pdf.is_some() && resultado.motor_usado != detectado)
        .then(|| format!("{}@{impressao}", resultado.motor_usado));
    com_notas(com_subpasta(resultado, &sub), &notas)
}

/// Linha `ÉguaTeX: tempos: ...` do comeco do log: total, cada passada do motor, bibliografia
/// e indices, em milissegundos. Partes que nao rodaram ficam de fora.
pub(crate) fn formatar_tempos(total_ms: u64, passadas_ms: &[u64], bibliografia_ms: u64, indices_ms: u64) -> String {
    let mut partes = vec![format!("total {total_ms} ms")];
    for (i, ms) in passadas_ms.iter().enumerate() {
        partes.push(format!("passada {} {ms} ms", i + 1));
    }
    if bibliografia_ms > 0 {
        partes.push(format!("bibliografia {bibliografia_ms} ms"));
    }
    if indices_ms > 0 {
        partes.push(format!("indices {indices_ms} ms"));
    }
    format!("ÉguaTeX: tempos: {}", partes.join(", "))
}

pub(crate) fn limitar_passadas(pedido: Option<u32>) -> u32 {
    match pedido {
        Some(n) if (1..=MAX_PASSADAS).contains(&n) => n,
        _ => MAX_PASSADAS,
    }
}

/// A passada de rascunho terminou sem erro fatal (o motor chegou ao fim do documento).
fn rascunho_completo(log: &str) -> bool {
    !log.contains("Fatal error occurred") && !log.contains("Emergency stop") && !log.contains("No pages of output")
}

fn compilar_ciclo(
    arquivo: &str,
    diretorio: &str,
    motor: &str,
    parar_no_erro: bool,
    max_passadas: u32,
) -> ResultadoCompilacao {
    let inicio = std::time::Instant::now();

    let dir = Path::new(diretorio);
    let nome_base = Path::new(arquivo)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut antes = retrato_auxiliares(dir);
    // Sem o .aux do documento (primeira compilacao ou depois de limpar os auxiliares) a primeira
    // passada sempre e seguida de outra, entao ela roda em rascunho, sem gravar o PDF: so o .aux,
    // o sumario e as referencias. No LuaLaTeX de uma tese de 58 paginas isso leva 3 s em vez de 18 s.
    let mut rascunho = max_passadas > 1 && !dir.join(format!("{nome_base}.aux")).is_file();
    let mut resultado = compilar_uma_vez(arquivo, diretorio, motor, parar_no_erro, rascunho);
    if rascunho && !foi_interrompida() && !rascunho_completo(&resultado.log) {
        // o rascunho parou num erro fatal: a passada normal mostra o mesmo erro, com o log de sempre
        rascunho = false;
        resultado = compilar_uma_vez(arquivo, diretorio, motor, parar_no_erro, false);
    }
    let mut log = resultado.log.clone();
    if rascunho {
        log = format!("ÉguaTeX: passada 1 em rascunho ({}), sem gravar o PDF.\n{log}", argumento_rascunho(motor));
    }
    let mut passadas = 1;
    // so medicao: tempo de cada passada do motor, da bibliografia e dos indices
    let mut tempos_passadas = vec![resultado.tempo_ms];
    let mut tempo_bibliografia: u64 = 0;
    let mut tempo_indices: u64 = 0;
    let mut indices_vistos = BTreeMap::new();
    let mut aviso_bibliografia: Option<String> = None;

    loop {
        // a bibliografia rodou nesta volta do ciclo e gravou um .bbl novo
        let mut bbl_novo = false;
        if foi_interrompida() {
            resultado.log = log;
            return interrompida(resultado);
        }
        // sem PDF novo nao ha o que repetir: motor ausente, erro fatal ou parou no erro
        // (o rascunho que chegou ate aqui terminou sem erro fatal e nunca grava PDF)
        let sem_pdf_novo =
            !rascunho && (resultado.pdf.is_none() || resultado.log.contains("no output PDF file produced"));
        if sem_pdf_novo || (!rascunho && parar_no_erro && !resultado.sucesso) {
            break;
        }
        let mut depois = retrato_auxiliares(dir);
        let chave_bib = dir.join(&nome_base);
        let bib_sem_mudanca = |programa: &str| {
            dir.join(format!("{nome_base}.bbl")).is_file()
                && entrada_bibliografia(dir, &nome_base, programa)
                    .is_some_and(|entrada| travar(&BIBLIOGRAFIA_VISTA).get(&chave_bib) == Some(&entrada))
        };
        if passadas == 1
            && let Some(programa) = ferramenta_bibliografia(dir, &nome_base, &depois)
            && bib_sem_mudanca(programa)
        {
            // citacoes e .bib iguais aos da ultima vez: o .bbl que esta la ja e o certo (como no latexmk)
            log.push_str(&format!("\n--- {programa}: citações e .bib sem mudança, não rodou ---\n"));
        } else if passadas == 1
            && let Some(programa) = ferramenta_bibliografia(dir, &nome_base, &depois) {
                let entrada = entrada_bibliografia(dir, &nome_base, programa);
                log.push_str(&format!("\n--- {programa} ---\n"));
                let marca = Instant::now();
                let (saida, problema) = rodar_bibliografia(programa, dir, &nome_base);
                tempo_bibliografia += marca.elapsed().as_millis() as u64;
                log.push_str(&saida);
                // so uma bibliografia montada sem erro, com todos os .bib achados, dispensa a proxima rodada
                if let (None, Some(entrada)) = (&problema, entrada) {
                    travar(&BIBLIOGRAFIA_VISTA).insert(chave_bib.clone(), entrada);
                } else {
                    travar(&BIBLIOGRAFIA_VISTA).remove(&chave_bib);
                }
                aviso_bibliografia = problema;
                bbl_novo = true;
                if foi_interrompida() {
                    resultado.log = log;
                    return interrompida(resultado);
                }
                depois = retrato_auxiliares(dir);
            }
        let marca = Instant::now();
        let log_indices = rodar_indices(dir, &nome_base, &mut indices_vistos);
        tempo_indices += marca.elapsed().as_millis() as u64;
        if !log_indices.is_empty() {
            log.push_str(&log_indices);
            if foi_interrompida() {
                resultado.log = log;
                return interrompida(resultado);
            }
            depois = retrato_auxiliares(dir);
        }
        // depois de um rascunho sempre ha outra passada: e ela que grava o PDF
        if !rascunho && (depois == antes || passadas >= max_passadas) {
            break;
        }
        antes = depois;
        let rascunho_anterior = rascunho;
        // Depois do rascunho a bibliografia acabou de gravar o .bbl. A passada que o le muda o .aux
        // (citacoes e paginas) em quase todo documento (TU Berlin, Inria, ACM, TU Delft e o TCC
        // pediam 3 passadas ou mais), entao ela tambem roda em rascunho e a seguinte grava o PDF.
        // Precisa caber mais uma passada no teto.
        rascunho = rascunho_anterior && bbl_novo && passadas + 2 <= max_passadas;
        resultado = compilar_uma_vez(arquivo, diretorio, motor, parar_no_erro, rascunho);
        if rascunho && !foi_interrompida() && !rascunho_completo(&resultado.log) {
            // erro fatal no rascunho: a passada normal mostra o mesmo erro, com o log de sempre
            rascunho = false;
            resultado = compilar_uma_vez(arquivo, diretorio, motor, parar_no_erro, false);
        }
        tempos_passadas.push(resultado.tempo_ms);
        passadas += 1;
        log.push_str(&format!("\n--- Passada {passadas} ---\n"));
        if rascunho {
            log.push_str(&format!("ÉguaTeX: passada {passadas} em rascunho ({}), sem gravar o PDF.\n", argumento_rascunho(motor)));
        }
        log.push_str(&resultado.log);
        // o XeLaTeX em rascunho deixa o .xdv, que a passada normal nao usa
        if rascunho_anterior && nome_do_motor(motor) == "xelatex" {
            let _ = std::fs::remove_file(dir.join(format!("{nome_base}.xdv")));
        }
    }

    resultado.tempo_ms = inicio.elapsed().as_millis() as u64;
    let linha = formatar_tempos(resultado.tempo_ms, &tempos_passadas, tempo_bibliografia, tempo_indices);
    resultado.log = format!("{linha}\n{log}");
    if let Some(mensagem) = aviso_bibliografia {
        resultado.erros.push(ErroCompilacao { linha: None, arquivo: None, mensagem, tipo: "aviso".to_string(), categoria: None });
    }
    resultado
}

/// Avisos que o painel agrupa e recolhe (campo `categoria` do `ErroCompilacao`).
const CATEGORIA_CAIXA_LARGA: &str = "caixa-larga";
const CATEGORIA_CAIXA_FROUXA: &str = "caixa-frouxa";
const CATEGORIA_CAIXA_VERTICAL: &str = "caixa-vertical";
const CATEGORIA_ROTULO_REPETIDO: &str = "rotulo-repetido";

/// Aviso de caixa ou de rotulo repetido lido de uma linha do log.
struct AvisoLido {
    categoria: &'static str,
    linha: Option<u32>,
}

/// Reconhece `Overfull \hbox (Xpt too wide) ... at lines A--B`, `Underfull \hbox (badness N) ...`,
/// as mesmas com `\vbox` e `detected at line N`, e `LaTeX Warning: Label `x' multiply defined`.
fn aviso_de_caixa_ou_rotulo(linha: &str) -> Option<AvisoLido> {
    let categoria = if linha.starts_with("Overfull \\hbox (") {
        CATEGORIA_CAIXA_LARGA
    } else if linha.starts_with("Underfull \\hbox (") {
        CATEGORIA_CAIXA_FROUXA
    } else if linha.starts_with("Overfull \\vbox (") || linha.starts_with("Underfull \\vbox (") {
        CATEGORIA_CAIXA_VERTICAL
    } else if linha.starts_with("LaTeX Warning: Label `") && linha.contains("multiply defined") {
        // a linha chega inteira: `juntar_linhas_quebradas` ja uniu o nome longo quebrado em 79 colunas
        return Some(AvisoLido { categoria: CATEGORIA_ROTULO_REPETIDO, linha: None });
    } else {
        return None;
    };
    Some(AvisoLido { categoria, linha: primeira_linha_citada(linha) })
}

/// `... at lines 12--14` e `... detected at line 7` dao 12 e 7. Sem numero, `None`.
fn primeira_linha_citada(texto: &str) -> Option<u32> {
    let (_, resto) = texto.split_once(" at lines ").or_else(|| texto.split_once(" at line "))?;
    let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();
    digitos.parse().ok()
}

/// Acompanha quais arquivos o LaTeX tem abertos enquanto o log e lido: cada `(./x.tex` abre
/// um arquivo e o `)` correspondente fecha. Parenteses que nao abrem arquivo (`(12pt too wide)`)
/// entram como `None` so para o `)` fechar o par certo.
#[derive(Default)]
struct RastroDeArquivos {
    abertos: Vec<Option<String>>,
    /// A linha seguinte ao cabecalho de uma caixa horizontal e o texto da caixa (do documento,
    /// com parenteses soltos): nao conta parenteses. Vale para o log em arquivo, que tem ` []` e
    /// uma linha em branco depois, e para o do console, que segue direto para o proximo `)`.
    pular_conteudo: bool,
}

/// Largura em que o TeX quebra as linhas do log (`max_print_line`), em bytes.
const COLUNAS_DO_LOG: usize = 79;

/// O TeX quebra o log em 79 colunas no meio de nomes de arquivo, de rotulos e de textos.
/// Uma linha com exatamente 79 bytes continua na seguinte: as duas viram uma so.
fn juntar_linhas_quebradas(log: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual: Option<String> = None;
    for linha in log.lines() {
        let mut texto = atual.take().unwrap_or_default();
        texto.push_str(linha);
        if linha.len() == COLUNAS_DO_LOG {
            atual = Some(texto);
        } else {
            saida.push(texto);
        }
    }
    saida.extend(atual);
    saida
}

/// Cabecalho do relatorio de caixa horizontal: Overfull, Underfull, Loose ou Tight seguido de
/// `\hbox (`. O TeX imprime o texto da caixa numa linha so (quebrada em 79 colunas,
/// que `juntar_linhas_quebradas` ja juntou) logo depois do cabecalho.
fn cabecalho_de_caixa_horizontal(linha: &str) -> bool {
    ["Overfull", "Underfull", "Loose", "Tight"]
        .iter()
        .any(|p| linha.strip_prefix(p).is_some_and(|r| r.starts_with(" \\hbox (")))
}

/// Nome do arquivo logo depois de um `(`. Com aspas, vai ate a aspa de fechamento. Sem aspas,
/// vai ate o proximo parentese e pode ter espacos (`./pasta com espaco/capitulo tres.tex`) se
/// o trecho todo comecar como caminho e terminar em extensao; senao vale a primeira palavra.
/// Devolve o nome e quantos bytes de `resto` ele ocupa.
fn nome_apos_parentese(resto: &str) -> (&str, usize) {
    if let Some(sem_aspa) = resto.strip_prefix('"')
        && let Some(fim) = sem_aspa.find('"') {
        return (&sem_aspa[..fim], fim + 2);
    }
    let ate_parentese = resto.find(['(', ')']).unwrap_or(resto.len());
    let trecho = resto[..ate_parentese].trim_end();
    let parece_caminho = trecho.starts_with("./") || trecho.starts_with("../") || trecho.starts_with('/') || trecho.as_bytes().get(1) == Some(&b':');
    if parece_caminho && trecho.contains(' ') && tem_extensao(trecho) {
        return (trecho, trecho.len());
    }
    let fim = resto.find(|c: char| c.is_whitespace() || c == '(' || c == ')').unwrap_or(resto.len());
    (&resto[..fim], fim)
}

impl RastroDeArquivos {
    fn ler_linha(&mut self, linha: &str) {
        if self.pular_conteudo {
            self.pular_conteudo = false;
            return;
        }
        if cabecalho_de_caixa_horizontal(linha) {
            self.pular_conteudo = true;
            return;
        }
        // contexto de erro e os proprios avisos trazem parenteses soltos do texto do documento
        const PULAR: [&str; 7] = ["[]", "\\", "!", "l.", "<", "Overfull", "Underfull"];
        if PULAR.iter().any(|p| linha.starts_with(p)) {
            return;
        }
        let mut resto = linha;
        while let Some(pos) = resto.find(['(', ')']) {
            let fecha = resto.as_bytes()[pos] == b')';
            resto = &resto[pos + 1..];
            if fecha {
                self.abertos.pop();
                continue;
            }
            let (token, usados) = nome_apos_parentese(resto);
            self.abertos.push(tem_extensao(token).then(|| token.to_string()));
            resto = &resto[usados..];
        }
    }

    /// O arquivo aberto mais recente, se for um `.tex` do projeto. Se o aviso nasceu num `.aux`,
    /// `.toc`, `.bbl` ou classe, os numeros de linha nao valem para o texto do usuario: `None`.
    fn arquivo_tex_aberto(&self) -> Option<&str> {
        self.abertos
            .iter()
            .rev()
            .find_map(|a| a.as_deref())
            .filter(|a| a.to_ascii_lowercase().ends_with(".tex"))
    }
}

/// O nome (sem a pasta) termina em `.ext` de 1 a 5 letras ou digitos.
fn tem_extensao(token: &str) -> bool {
    let nome = token.rsplit(['/', '\\']).next().unwrap_or(token);
    nome.rsplit_once('.')
        .is_some_and(|(base, ext)| !base.is_empty() && (1..=5).contains(&ext.len()) && ext.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// `principal` e o nome do documento principal: onde caem os avisos sem arquivo `.tex` aberto.
fn extrair_erros(log: &str, principal: &str) -> Vec<ErroCompilacao> {
    let mut erros = Vec::new();
    let mut rastro = RastroDeArquivos::default();

    for line in juntar_linhas_quebradas(log).iter().map(String::as_str) {
        if let Some(resto) = line.strip_prefix('!') {
            erros.push(ErroCompilacao {
                linha: None,
                arquivo: None,
                mensagem: resto.trim().to_string(),
                tipo: "erro".to_string(),
                categoria: None,
            });
        } else if let Some((arquivo, num, msg)) = arquivo_linha_mensagem(line) {
            erros.push(ErroCompilacao {
                linha: Some(num),
                arquivo: Some(arquivo.to_string()),
                mensagem: msg.to_string(),
                tipo: if msg.to_lowercase().contains("warning") {
                    "aviso".to_string()
                } else {
                    "erro".to_string()
                },
                categoria: None,
            });
        } else if let Some(aviso) = aviso_de_caixa_ou_rotulo(line) {
            let (arquivo, linha) = match rastro.arquivo_tex_aberto() {
                Some(a) => (Some(a.to_string()), aviso.linha),
                None if principal.is_empty() => (None, None),
                None => (Some(format!("./{principal}")), None),
            };
            erros.push(ErroCompilacao {
                linha,
                arquivo,
                mensagem: line.trim().to_string(),
                tipo: "aviso".to_string(),
                categoria: Some(aviso.categoria.to_string()),
            });
        }
        rastro.ler_linha(line);
    }

    erros
}

/// Separa `arquivo:linha: mensagem` (formato do -file-line-error). O arquivo pode ter
/// ":" na letra da unidade (`C:/ProgramData/.../x.cls:12: ...`): vale o primeiro ":"
/// seguido de numero e ":" cujo trecho anterior termine num arquivo com extensao.
fn arquivo_linha_mensagem(linha: &str) -> Option<(&str, u32, &str)> {
    for (pos, _) in linha.match_indices(':') {
        let antes = &linha[..pos];
        let depois = &linha[pos + 1..];
        let digitos = depois.bytes().take_while(u8::is_ascii_digit).count();
        if digitos == 0 || !depois[digitos..].starts_with(':') {
            continue;
        }
        let nome = antes.rsplit(['/', '\\']).next().unwrap_or(antes);
        let tem_extensao = nome.rsplit_once('.').is_some_and(|(base, ext)| {
            !base.is_empty() && !ext.is_empty() && ext.len() <= 8 && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        });
        if !tem_extensao {
            continue;
        }
        let msg = depois[digitos + 1..].trim();
        let num = depois[..digitos].parse().ok()?;
        return (!msg.is_empty()).then_some((antes, num, msg));
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn shell_escape_fica_desligado_por_padrao_e_liga_pela_preferencia() {
        definir_shell_escape(false);
        if std::env::var_os("EGUATEX_SHELL_ESCAPE").is_none() {
            assert!(!shell_escape_liberado());
        }
        definir_shell_escape(true);
        assert!(shell_escape_liberado());
        definir_shell_escape(false);
    }

    #[test]
    fn argumentos_dos_tres_motores_com_e_sem_shell_escape() {
        let arq = Path::new("main.tex");
        for motor in ["pdfLaTeX", "XeLaTeX", "LuaLaTeX", "pdflatex", "xelatex", "lualatex"] {
            assert!(["pdflatex", "xelatex", "lualatex"].contains(&nome_do_motor(motor)));
            let sem: Vec<String> = argumentos_motor(arq, false, false).iter().map(|a| a.to_string_lossy().into()).collect();
            assert_eq!(
                sem,
                ["-interaction=nonstopmode", "-shell-restricted", "-file-line-error", "-synctex=1", "main.tex"]
            );
            let com: Vec<String> = argumentos_motor(arq, true, true).iter().map(|a| a.to_string_lossy().into()).collect();
            assert_eq!(
                com,
                ["-interaction=nonstopmode", "-halt-on-error", "-shell-escape", "-file-line-error", "-synctex=1", "main.tex"]
            );
            assert!(!com.iter().any(|a| a == "-shell-restricted"));
        }
        assert_eq!(nome_do_motor("xelatex"), "xelatex");
        assert_eq!(nome_do_motor("qualquer"), "pdflatex");
    }

    fn pasta_tinytex_de_teste(nome: &str, arquivos: &[&str]) -> (PathBuf, PathBuf) {
        let raiz = std::env::temp_dir().join(format!("eguatex-tt-{nome}-{}", std::process::id())).join("EguaTex");
        let bin = raiz.join("TinyTeX").join("bin").join("windows");
        std::fs::create_dir_all(&bin).unwrap();
        for a in arquivos {
            std::fs::write(bin.join(a), "x").unwrap();
        }
        (raiz, bin)
    }

    #[test]
    fn tinytex_com_todos_os_arquivos_do_mesmo_dono_e_aceito() {
        let (raiz, bin) = pasta_tinytex_de_teste("aceita", &["pdflatex.exe", "bibtex.exe", "kpathsea.dll"]);
        assert!(tinytex_confiavel_com(&bin, &raiz, Path::new("ref.exe"), &|_, _| true));
        let _ = std::fs::remove_dir_all(raiz.parent().unwrap());
    }

    #[test]
    fn tinytex_com_um_arquivo_de_outro_dono_e_recusado() {
        for plantado in ["pdflatex.exe", "pdflatex.com", "biber.exe", "kpathsea.dll", "latexmk.exe"] {
            let (raiz, bin) = pasta_tinytex_de_teste(
                &plantado.replace('.', "_"),
                &["pdflatex.exe", "xelatex.exe", "lualatex.exe", "bibtex.exe", "biber.exe", "makeindex.exe", "latexmk.exe", "pdflatex.com", "kpathsea.dll"],
            );
            let recusa = |a: &Path, _: &Path| a.file_name().is_none_or(|n| n != plantado);
            assert!(!tinytex_confiavel_com(&bin, &raiz, Path::new("ref.exe"), &recusa), "{plantado}");
            let _ = std::fs::remove_dir_all(raiz.parent().unwrap());
        }
    }

    #[test]
    fn tinytex_com_pasta_acima_de_outro_dono_ou_vazia_e_recusado() {
        let (raiz, bin) = pasta_tinytex_de_teste("acima", &["pdflatex.exe"]);
        let recusa_raiz = |a: &Path, _: &Path| a != raiz;
        assert!(!tinytex_confiavel_com(&bin, &raiz, Path::new("ref.exe"), &recusa_raiz));
        let _ = std::fs::remove_dir_all(raiz.parent().unwrap());
        let (raiz, bin) = pasta_tinytex_de_teste("vazia", &[]);
        assert!(!tinytex_confiavel_com(&bin, &raiz, Path::new("ref.exe"), &|_, _| true));
        assert!(!tinytex_confiavel_com(&bin.join("nao-existe"), &raiz, Path::new("ref.exe"), &|_, _| true));
        let _ = std::fs::remove_dir_all(raiz.parent().unwrap());
    }

    #[test]
    fn preferir_exe_troca_o_com_pelo_exe_irmao() {
        let pasta = std::env::temp_dir().join(format!("eguatex-pe-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("a.exe"), "x").unwrap();
        std::fs::write(pasta.join("a.com"), "x").unwrap();
        std::fs::write(pasta.join("b.com"), "x").unwrap();
        assert_eq!(preferir_exe(pasta.join("a.com")), pasta.join("a.exe"));
        assert_eq!(preferir_exe(pasta.join("b.com")), pasta.join("b.com"));
        assert_eq!(preferir_exe(pasta.join("a.exe")), pasta.join("a.exe"));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[cfg(windows)]
    #[test]
    fn dono_de_arquivo_do_mesmo_usuario_confere() {
        let exe = std::env::current_exe().unwrap();
        let pasta = std::env::temp_dir().join(format!("eguatex-dono-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("a.txt");
        std::fs::write(&arquivo, "x").unwrap();
        assert!(dono::mesmo_dono(&arquivo, &exe));
        assert!(dono::mesmo_dono(&pasta, &arquivo));
        assert!(!dono::mesmo_dono(&pasta.join("nao-existe"), &exe));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn linha_de_tempos_traz_total_e_cada_passada() {
        let linha = formatar_tempos(2500, &[1200, 900], 150, 0);
        assert_eq!(linha, "ÉguaTeX: tempos: total 2500 ms, passada 1 1200 ms, passada 2 900 ms, bibliografia 150 ms");
        assert!(!linha.contains('\n'));
        assert_eq!(formatar_tempos(10, &[10], 0, 0), "ÉguaTeX: tempos: total 10 ms, passada 1 10 ms");
        assert!(formatar_tempos(10, &[10], 0, 7).ends_with("indices 7 ms"));
        // as notas do projeto entram depois da linha de tempos
        let log = format!("{linha}\n--- log ---");
        assert_eq!(juntar_notas("ÉguaTeX: nota\n", &log), format!("{linha}\nÉguaTeX: nota\n\n--- log ---"));
        assert_eq!(juntar_notas("ÉguaTeX: nota\n", "so log"), "ÉguaTeX: nota\n\nso log");
    }

    #[test]
    fn limitar_passadas_aceita_1_a_5_e_cai_no_maximo_fora_disso() {
        assert_eq!(limitar_passadas(Some(1)), 1);
        assert_eq!(limitar_passadas(Some(3)), 3);
        assert_eq!(limitar_passadas(Some(5)), MAX_PASSADAS);
        assert_eq!(limitar_passadas(Some(0)), MAX_PASSADAS);
        assert_eq!(limitar_passadas(Some(9)), MAX_PASSADAS);
        assert_eq!(limitar_passadas(None), MAX_PASSADAS);
    }

    /// Log de exemplo: main.tex abre cap1.tex, a classe e o .aux; os avisos de caixa vem de arquivos diferentes.
    const LOG_AVISOS: &str = "This is pdfTeX, Version 3.141592653-2.6-1.40.27 (TeX Live 2026)
(./main.tex
LaTeX2e <2024-11-01>
(/usr/share/texlive/article.cls
Document Class: article 2024/06/29 v1.4n Standard LaTeX document class
(/usr/share/texlive/size10.clo))
(./main.aux)
(./cap1.tex
Overfull \\hbox (12.5pt too wide) in paragraph at lines 12--14
[]\\T1/cmr/m/n/10 palavra (longa
 []

Underfull \\hbox (badness 10000) in paragraph at lines 20--21

Underfull \\vbox (badness 10000) detected at line 30
)
Overfull \\hbox (3.0pt too wide) in alignment at lines 40--41
LaTeX Warning: Label `eq:a' multiply defined.
./main.tex:50: Undefined control sequence.
! Undefined control sequence.
";

    fn so_erros(erros: &[ErroCompilacao]) -> Vec<&ErroCompilacao> {
        erros.iter().filter(|e| e.tipo != "aviso").collect()
    }

    #[test]
    fn avisos_de_caixa_e_rotulo_saem_com_arquivo_linha_e_categoria() {
        let erros = extrair_erros(LOG_AVISOS, "main.tex");
        let avisos: Vec<_> = erros.iter().filter(|e| e.categoria.is_some()).collect();
        assert_eq!(avisos.len(), 5, "{erros:?}");
        assert!(avisos.iter().all(|e| e.tipo == "aviso"), "{avisos:?}");

        // Overfull \hbox ... at lines 12--14 dentro de cap1.tex
        assert_eq!(avisos[0].categoria.as_deref(), Some("caixa-larga"));
        assert_eq!(avisos[0].arquivo.as_deref(), Some("./cap1.tex"));
        assert_eq!(avisos[0].linha, Some(12));
        assert_eq!(avisos[0].mensagem, "Overfull \\hbox (12.5pt too wide) in paragraph at lines 12--14");
        // Underfull \hbox
        assert_eq!(avisos[1].categoria.as_deref(), Some("caixa-frouxa"));
        assert_eq!(avisos[1].linha, Some(20));
        // Underfull \vbox ... detected at line 30
        assert_eq!(avisos[2].categoria.as_deref(), Some("caixa-vertical"));
        assert_eq!(avisos[2].arquivo.as_deref(), Some("./cap1.tex"));
        assert_eq!(avisos[2].linha, Some(30));
        // depois do ")" que fecha cap1.tex, volta a valer o main.tex
        assert_eq!(avisos[3].categoria.as_deref(), Some("caixa-larga"));
        assert_eq!(avisos[3].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(avisos[3].linha, Some(40));
        // rotulo repetido: sem linha, no documento principal
        assert_eq!(avisos[4].categoria.as_deref(), Some("rotulo-repetido"));
        assert_eq!(avisos[4].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(avisos[4].linha, None);
    }

    #[test]
    fn avisos_de_caixa_nao_mudam_a_contagem_de_erros() {
        let com = extrair_erros(LOG_AVISOS, "main.tex");
        let sem_avisos: String = LOG_AVISOS
            .lines()
            .filter(|l| !l.starts_with("Overfull") && !l.starts_with("Underfull") && !l.contains("multiply defined"))
            .map(|l| format!("{l}\n"))
            .collect();
        let sem = extrair_erros(&sem_avisos, "main.tex");
        assert_eq!(so_erros(&com).len(), 2, "{com:?}");
        assert_eq!(so_erros(&com).len(), so_erros(&sem).len());
        assert!(so_erros(&com).iter().all(|e| e.categoria.is_none()));
    }

    #[test]
    fn aviso_em_aux_ou_sem_arquivo_aberto_cai_no_documento_principal() {
        // aberto so o .aux (nao e texto do usuario): arquivo = principal, sem linha
        let no_aux = "(./main.aux\nLaTeX Warning: Label `x' multiply defined.\n)\n";
        let e = extrair_erros(no_aux, "tese.tex");
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].arquivo.as_deref(), Some("./tese.tex"));
        assert_eq!(e[0].linha, None);
        // nenhum arquivo aberto no log
        let solto = "Overfull \\hbox (1.0pt too wide) in paragraph at lines 3--4\n";
        let e = extrair_erros(solto, "tese.tex");
        assert_eq!(e[0].arquivo.as_deref(), Some("./tese.tex"));
        assert_eq!(e[0].linha, None);
        assert_eq!(e[0].categoria.as_deref(), Some("caixa-larga"));
    }

    #[test]
    fn parenteses_do_texto_nao_desalinham_os_arquivos() {
        let log = "(./main.tex\nTexto com (parenteses) e (outro\n(./a.tex\n)\nOverfull \\hbox (2.0pt too wide) in paragraph at lines 8--9\n)\n";
        let e = extrair_erros(log, "main.tex");
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(e[0].linha, Some(8));
    }

    #[test]
    fn linha_citada_aceita_lines_line_e_ausencia() {
        assert_eq!(primeira_linha_citada("Overfull \\hbox (1pt too wide) in paragraph at lines 12--14"), Some(12));
        assert_eq!(primeira_linha_citada("Underfull \\hbox (badness 5) in paragraph at line 7"), Some(7));
        assert_eq!(primeira_linha_citada("Overfull \\vbox (2pt too high) detected at line 99"), Some(99));
        assert_eq!(primeira_linha_citada("Overfull \\vbox (2pt too high) has occurred while \\output is active"), None);
    }

    #[test]
    fn categoria_e_opcional_ao_ler_json_antigo() {
        let json = r#"{"linha":3,"arquivo":"./a.tex","mensagem":"m","tipo":"erro"}"#;
        let e: ErroCompilacao = serde_json::from_str(json).expect("json sem categoria");
        assert_eq!(e.categoria, None);
    }

    #[test]
    fn erro_com_caminho_de_unidade_entra_na_lista() {
        let log = "C:/ProgramData/EguaTex/TinyTeX/texmf-dist/tex/latex/x/x.cls:123: LaTeX Error: File `y.sty' not found.\n./main.tex:7: Undefined control sequence.\nlinha comum: sem numero\n";
        let erros = extrair_erros(log, "main.tex");
        assert_eq!(erros.len(), 2, "{erros:?}");
        assert_eq!(erros[0].arquivo.as_deref(), Some("C:/ProgramData/EguaTex/TinyTeX/texmf-dist/tex/latex/x/x.cls"));
        assert_eq!(erros[0].linha, Some(123));
        assert_eq!(erros[1].arquivo.as_deref(), Some("./main.tex"));
        assert_eq!(erros[1].linha, Some(7));
    }
    use std::fs;
    use std::time::{Duration, Instant};

    /// Pasta temporaria que se apaga sozinha.
    struct PastaTemp(PathBuf);

    impl PastaTemp {
        fn nova(nome: &str) -> Self {
            let caminho = std::env::temp_dir().join(format!(
                "eguatex-teste-compilador-{}-{}",
                std::process::id(),
                nome
            ));
            let _ = fs::remove_dir_all(&caminho);
            fs::create_dir_all(&caminho).expect("criar pasta de teste");
            PastaTemp(caminho)
        }

        fn texto(&self) -> String {
            self.0.display().to_string()
        }
    }

    impl Drop for PastaTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tex_instalado() -> bool {
        detectar_executavel("pdflatex").is_some()
    }

    /// Documento com um comando que nao existe no meio do texto.
    const TEX_COM_ERRO: &str = r"\documentclass{article}
\begin{document}
Antes do erro.

\comandoquenaoexiste

Depois do erro.
\end{document}
";

    /// Na primeira passada cria trava.txt e termina. Da segunda em diante (trava.txt ja
    /// existe) avisa em passo2.txt e entra num laco infinito: so sai se for morto.
    const TEX_QUE_TRAVA_NA_SEGUNDA: &str = r"\documentclass{article}
\newwrite\f
\begin{document}
\IfFileExists{trava.txt}{%
  \immediate\openout\f=passo2.txt
  \immediate\write\f{segunda passada}
  \immediate\closeout\f
  \def\a{\a}\a
}{%
  \immediate\openout\f=trava.txt
  \immediate\write\f{primeira passada}
  \immediate\closeout\f
}
Texto.
\end{document}
";

    #[test]
    fn documento_em_subpasta_acha_o_sty_ao_lado() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("subpasta");
        let sub = pasta.0.join("template-latex");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("meu-estilo.sty"),
            r"\ProvidesPackage{meu-estilo}
\newcommand{\olamundo}{Ola}
",
        )
        .unwrap();
        fs::write(
            sub.join("main.tex"),
            r"\documentclass{article}
\usepackage{meu-estilo}
\begin{document}
\olamundo \erroproposital
\end{document}
",
        )
        .unwrap();
        let r = compilar_com_bibtex("template-latex/main.tex", &pasta.texto(), "pdflatex", false);
        assert!(!r.log.contains("meu-estilo.sty' not found"), "{}", r.log);
        assert!(sub.join("main.pdf").is_file(), "{}", r.log);
        let erro = r.erros.iter().find(|e| e.mensagem.contains("Undefined control sequence")).expect("sem erro");
        assert_eq!(erro.arquivo.as_deref(), Some("./template-latex/main.tex"), "{:?}", erro);
    }

    #[test]
    fn parar_no_erro_liga_e_desliga_o_halt_on_error() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("parar-no-erro");
        fs::write(pasta.0.join("main.tex"), TEX_COM_ERRO).unwrap();
        let dir = pasta.texto();

        // true: para no primeiro erro e nem chega a gerar o PDF
        let parou = compilar("main.tex", &dir, "pdflatex", true);
        assert!(!parou.sucesso, "{}", parou.log);
        assert!(parou.pdf.is_none(), "gerou PDF mesmo parando no erro");
        assert!(!pasta.0.join("main.pdf").exists());
        assert!(!parou.log.contains("Output written on"), "{}", parou.log);
        assert!(
            parou.erros.iter().any(|e| e.mensagem.contains("Undefined control sequence")),
            "{}",
            parou.log
        );

        // false: continua apesar do erro e gera o PDF mesmo assim
        let seguiu = compilar("main.tex", &dir, "pdflatex", false);
        assert!(seguiu.pdf.is_some(), "{}", seguiu.log);
        assert!(pasta.0.join("main.pdf").is_file());
        assert!(seguiu.log.contains("Output written on"), "{}", seguiu.log);
        assert!(
            seguiu.erros.iter().any(|e| e.mensagem.contains("Undefined control sequence")),
            "{}",
            seguiu.log
        );

        // a compilacao completa repassa a mesma opcao para o motor
        let _ = fs::remove_file(pasta.0.join("main.pdf"));
        let completa_parando = compilar_com_bibtex("main.tex", &dir, "pdflatex", true);
        assert!(completa_parando.pdf.is_none(), "{}", completa_parando.log);
        let completa_seguindo = compilar_com_bibtex("main.tex", &dir, "pdflatex", false);
        assert!(completa_seguindo.pdf.is_some(), "{}", completa_seguindo.log);
    }

    #[test]
    fn parar_compilacao_sem_nada_rodando_devolve_false() {
        let _vez = travar_teste();
        assert!(!cmd_parar_compilacao());
        assert!(!cmd_parar_compilacao());
        assert!(travar(&PID_ATUAL).is_none());
    }

    #[test]
    fn parar_compilacao_interrompe_a_etapa_e_cancela_as_seguintes() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("interromper");
        fs::write(pasta.0.join("main.tex"), TEX_QUE_TRAVA_NA_SEGUNDA).unwrap();
        let dir = pasta.texto();

        let dir_da_thread = dir.clone();
        let compilacao = std::thread::spawn(move || {
            compilar_com_bibtex("main.tex", &dir_da_thread, "pdflatex", true)
        });

        // espera a segunda passada (a que trava) avisar que chegou ao laco infinito
        let marca = pasta.0.join("passo2.txt");
        let limite = Instant::now() + Duration::from_secs(90);
        while !marca.is_file() && !compilacao.is_finished() && Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(50));
        }
        let interrompeu = cmd_parar_compilacao();

        // nunca deixa o teste pendurado: espera a thread acabar e insiste se precisar
        let limite = Instant::now() + Duration::from_secs(30);
        while !compilacao.is_finished() && Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(50));
        }
        if !compilacao.is_finished() {
            cmd_parar_compilacao();
        }
        let resultado = compilacao.join().expect("thread da compilação");

        assert!(marca.is_file(), "a segunda passada não chegou a rodar: {}", resultado.log);
        assert!(interrompeu, "cmd_parar_compilacao devolveu false com a segunda passada rodando");
        assert!(!resultado.sucesso);
        assert!(resultado.pdf.is_none());
        assert!(resultado.erros.is_empty());
        assert!(
            resultado.log.contains("Compilação interrompida pelo usuário."),
            "{}",
            resultado.log
        );
        assert!(resultado.log.contains("--- Passada 2 ---"), "{}", resultado.log);
        assert!(
            !resultado.log.contains("--- Passada 3 ---"),
            "a terceira passada rodou depois da interrupcao"
        );
        assert!(travar(&PID_ATUAL).is_none(), "o PID continuou guardado");
        assert!(!cmd_parar_compilacao());

        // a compilacao seguinte nao herda a interrupcao
        fs::remove_file(pasta.0.join("trava.txt")).unwrap();
        let normal = compilar("main.tex", &dir, "pdflatex", true);
        assert!(normal.sucesso, "{}", normal.log);
        assert!(!normal.log.contains("Compilação interrompida pelo usuário."));
    }

    /// Log da ultima passada (o log completo junta todas).
    fn ultima_passada(log: &str) -> &str {
        log.rsplit("--- Passada ").next().unwrap_or(log)
    }

    const BIB_KNUTH: &str =
        "@book{knuth, author={Donald Knuth}, title={The TeXbook}, year={1984}, publisher={Addison-Wesley}}\n";

    #[test]
    fn documento_com_biblatex_usa_o_biber() {
        if !tex_instalado() || detectar_executavel("biber").is_none() {
            eprintln!("pdflatex ou biber não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("biblatex");
        fs::write(pasta.0.join("ref.bib"), BIB_KNUTH).unwrap();
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\usepackage[backend=biber]{biblatex}\n\\addbibresource{ref.bib}\n\
             \\begin{document}\nTexto \\cite{knuth}.\n\\printbibliography\n\\end{document}\n",
        )
        .unwrap();

        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("--- biber ---"), "{}", r.log);
        assert!(!r.log.contains("--- bibtex ---"), "{}", r.log);
        let ultima = ultima_passada(&r.log);
        assert!(!ultima.contains("undefined"), "citação sem resolver: {}", ultima);

        // recompilar sem mudar citacoes nem .bib nao roda o biber de novo (como no latexmk)
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("biber: citações e .bib sem mudança, não rodou"), "{}", r.log);
        assert!(!r.log.contains("--- biber ---"), "{}", r.log);
        // .bib mudou: roda
        fs::write(pasta.0.join("ref.bib"), format!("{BIB_KNUTH}\n% nova\n")).unwrap();
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.log.contains("--- biber ---"), "{}", r.log);
    }

    #[test]
    fn classe_na_pasta_texmf_do_latexmkrc_e_achada() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("texmf-projeto");
        let cls = pasta.0.join("-texmf/tex/latex/minha");
        fs::create_dir_all(&cls).unwrap();
        fs::write(cls.join("minha-classe.cls"), "\\LoadClass{article}\n").unwrap();
        fs::write(pasta.0.join("latexmkrc"), "ensure_path( 'TEXMFHOME', '/compile/-texmf/' );\n").unwrap();
        fs::write(pasta.0.join("main.tex"), "\\documentclass{minha-classe}\n\\begin{document}\nOi\n\\end{document}\n").unwrap();

        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("-texmf"), "{}", r.log);
    }

    #[test]
    fn documento_com_makeindex_monta_o_indice() {
        if !tex_instalado() || detectar_executavel("makeindex").is_none() {
            eprintln!("pdflatex ou makeindex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("makeindex");
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\usepackage{makeidx}\n\\makeindex\n\
             \\begin{document}\nFônon\\index{fonon} e Raman\\index{Raman}.\n\\printindex\n\\end{document}\n",
        )
        .unwrap();

        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("--- makeindex (idx) ---"), "{}", r.log);
        let ind = fs::read_to_string(pasta.0.join("main.ind")).unwrap();
        assert!(ind.contains("fonon") && ind.contains("Raman"), "{}", ind);
        // o .ind entrou no PDF: a passada seguinte leu o indice
        assert!(r.log.contains("main.ind"), "{}", r.log);
    }

    #[test]
    fn documento_em_subpasta_monta_bibliografia_e_indice() {
        if !tex_instalado() || detectar_executavel("bibtex").is_none() || detectar_executavel("makeindex").is_none() {
            eprintln!("pdflatex, bibtex ou makeindex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("subpasta-bib-indice");
        let sub = pasta.0.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("ref.bib"), BIB_KNUTH).unwrap();
        fs::write(
            sub.join("main.tex"),
            "\\documentclass{article}\n\\usepackage{makeidx}\n\\makeindex\n\\begin{document}\n\
             Texto \\cite{knuth} e Raman\\index{Raman}.\n\\bibliographystyle{plain}\n\\bibliography{ref}\n\
             \\printindex\n\\end{document}\n",
        )
        .unwrap();

        let r = compilar_com_bibtex("sub/main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("--- bibtex ---"), "{}", r.log);
        assert!(sub.join("main.bbl").is_file(), "{}", r.log);
        let ind = fs::read_to_string(sub.join("main.ind")).unwrap();
        assert!(ind.contains("Raman"), "{}", ind);
        assert!(!ultima_passada(&r.log).contains("Citation `knuth' undefined"), "{}", r.log);
    }

    #[test]
    fn erro_no_texto_nao_impede_a_bibliografia_sem_parar_no_erro() {
        if !tex_instalado() || detectar_executavel("bibtex").is_none() {
            eprintln!("pdflatex ou bibtex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("erro-com-bib");
        fs::write(pasta.0.join("ref.bib"), BIB_KNUTH).unwrap();
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\nTexto \\cite{knuth}.\n\\comandoquenaoexiste\n\
             \\bibliographystyle{plain}\n\\bibliography{ref}\n\\end{document}\n",
        )
        .unwrap();

        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(!r.sucesso, "o erro do texto sumiu: {}", r.log);
        assert!(r.pdf.is_some(), "{}", r.log);
        assert!(r.log.contains("--- bibtex ---"), "{}", r.log);
        let ultima = ultima_passada(&r.log);
        assert!(!ultima.contains("Citation `knuth' undefined"), "citação sem resolver: {}", ultima);
    }

    #[test]
    fn documento_sem_mudanca_compila_uma_vez_so() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("estavel");
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\n\\tableofcontents\n\\section{Um}\\label{s}\n\
             Veja a secao \\ref{s}.\n\\end{document}\n",
        )
        .unwrap();
        let dir = pasta.texto();

        // primeira vez: o sumario e a referencia pedem uma segunda passada
        let primeira = compilar_com_bibtex("main.tex", &dir, "pdflatex", false);
        assert!(primeira.sucesso, "{}", primeira.log);
        assert!(primeira.log.contains("--- Passada 2 ---"), "{}", primeira.log);
        assert!(!primeira.log.contains("--- bibtex ---"), "rodou bibtex sem bibliografia");
        let ultima = ultima_passada(&primeira.log);
        assert!(!ultima.contains("Rerun"), "{}", ultima);

        // sem mudar nada, uma passada basta
        let segunda = compilar_com_bibtex("main.tex", &dir, "pdflatex", false);
        assert!(segunda.sucesso, "{}", segunda.log);
        assert!(!segunda.log.contains("--- Passada 2 ---"), "{}", segunda.log);
    }

    /// O hash do bibtex le os .aux dos `\include` (\@input, em profundidade) e os .bib nomeados
    /// em \bibdata, inclusive fora do projeto; .bib nomeado e nao achado da `None`.
    #[test]
    fn entrada_do_bibtex_le_os_aux_incluidos_e_os_bib_nomeados() {
        let pasta = PastaTemp::nova("entrada-bibtex");
        let proj = pasta.0.join("proj");
        let fora = pasta.0.join("fora");
        fs::create_dir_all(proj.join("caps")).unwrap();
        fs::create_dir_all(&fora).unwrap();
        fs::write(fora.join("ref.bib"), BIB_KNUTH).unwrap();
        fs::write(proj.join("main.aux"), "\\relax\n\\@input{caps/cap1.aux}\n\\bibstyle{plain}\n\\bibdata{../fora/ref}\n").unwrap();
        fs::write(proj.join("caps/cap1.aux"), "\\relax\n\\citation{knuth}\n\\@input{caps/cap2.aux}\n").unwrap();
        fs::write(proj.join("caps/cap2.aux"), "\\relax\n").unwrap();
        let antes = entrada_bibliografia(&proj, "main", "bibtex").expect("todos os .bib achados");
        assert_eq!(entrada_bibliografia(&proj, "main", "bibtex"), Some(antes));
        // \cite novo num .aux de segundo nivel
        fs::write(proj.join("caps/cap2.aux"), "\\relax\n\\citation{lamport}\n").unwrap();
        let citou = entrada_bibliografia(&proj, "main", "bibtex").unwrap();
        assert_ne!(citou, antes);
        // .bib fora da pasta do projeto mudou
        fs::write(fora.join("ref.bib"), format!("{BIB_KNUTH}% nova\n")).unwrap();
        assert_ne!(entrada_bibliografia(&proj, "main", "bibtex"), Some(citou));
        // caminho absoluto em \bibdata
        let absoluto = fora.join("ref").to_string_lossy().replace('\\', "/");
        fs::write(proj.join("main.aux"), format!("\\relax\n\\bibdata{{{absoluto}}}\n")).unwrap();
        assert!(entrada_bibliografia(&proj, "main", "bibtex").is_some());
        // .bib nomeado que nao existe: roda sempre
        fs::write(proj.join("main.aux"), "\\relax\n\\bibdata{../fora/ref,sumiu}\n").unwrap();
        assert_eq!(entrada_bibliografia(&proj, "main", "bibtex"), None);
    }

    /// O hash do biber le os .bib dos datasource do .bcf, relativos ou absolutos.
    #[test]
    fn entrada_do_biber_le_os_bib_do_bcf() {
        let pasta = PastaTemp::nova("entrada-biber");
        let proj = pasta.0.join("proj");
        let fora = pasta.0.join("fora");
        fs::create_dir_all(&proj).unwrap();
        fs::create_dir_all(&fora).unwrap();
        fs::write(fora.join("a.bib"), BIB_KNUTH).unwrap();
        fs::write(proj.join("b.bib"), BIB_KNUTH).unwrap();
        let absoluto = fora.join("a.bib").to_string_lossy().replace('\\', "/");
        let bcf = |extra: &str| {
            format!(
                "<bcf:controlfile>\n  <bcf:bibdata section=\"0\">\n    <bcf:datasource type=\"file\" datatype=\"bibtex\" glob=\"false\">{absoluto}</bcf:datasource>\n    <bcf:datasource type=\"file\" datatype=\"bibtex\" glob=\"false\">b.bib</bcf:datasource>\n{extra}  </bcf:bibdata>\n</bcf:controlfile>\n"
            )
        };
        assert_eq!(bibs_do_bcf(&bcf("")), vec![absoluto.clone(), "b.bib".to_string()]);
        fs::write(proj.join("main.bcf"), bcf("")).unwrap();
        let antes = entrada_bibliografia(&proj, "main", "biber").expect("todos os .bib achados");
        fs::write(fora.join("a.bib"), format!("{BIB_KNUTH}% nova\n")).unwrap();
        assert_ne!(entrada_bibliografia(&proj, "main", "biber"), Some(antes));
        fs::write(proj.join("main.bcf"), bcf("    <bcf:datasource type=\"file\" datatype=\"bibtex\" glob=\"false\">sumiu.bib</bcf:datasource>\n")).unwrap();
        assert_eq!(entrada_bibliografia(&proj, "main", "biber"), None);
    }

    /// \cite novo num capitulo do \include: a citacao fica no cap1.aux e o bibtex roda de novo.
    #[test]
    fn cite_novo_num_include_roda_o_bibtex() {
        if !tex_instalado() || detectar_executavel("bibtex").is_none() {
            eprintln!("pdflatex ou bibtex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("include-bibtex");
        let bib = format!(
            "{BIB_KNUTH}@book{{lamport, author={{Leslie Lamport}}, title={{LaTeX}}, year={{1994}}, publisher={{Addison-Wesley}}}}\n"
        );
        fs::write(pasta.0.join("ref.bib"), bib).unwrap();
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\n\\include{cap1}\n\\bibliographystyle{plain}\n\\bibliography{ref}\n\\end{document}\n",
        )
        .unwrap();
        fs::write(pasta.0.join("cap1.tex"), "Texto \\cite{knuth}.\n").unwrap();
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("--- bibtex ---"), "{}", r.log);
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.log.contains("bibtex: citações e .bib sem mudança, não rodou"), "{}", r.log);

        fs::write(pasta.0.join("cap1.tex"), "Texto \\cite{knuth} e \\cite{lamport}.\n").unwrap();
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("--- bibtex ---"), "{}", r.log);
        assert!(!ultima_passada(&r.log).contains("Citation `lamport' undefined"), "{}", r.log);
    }

    /// A compilacao espera o preparo, ate o prazo do preparo inteiro; estourado o prazo, as
    /// proximas nao esperam; o botao Parar cancela a espera.
    #[test]
    fn compilacao_espera_o_preparo() {
        let _vez = travar_teste();
        iniciar_preparo(Duration::from_secs(30));
        let soltar = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(300));
            terminar_preparo();
        });
        let inicio = Instant::now();
        assert!(esperar_preparo());
        soltar.join().unwrap();
        let esperou = inicio.elapsed();
        assert!(esperou >= Duration::from_millis(250) && esperou < Duration::from_secs(10), "{esperou:?}");

        // preparo enroscado: segue no prazo; a proxima compilacao nem espera
        iniciar_preparo(Duration::from_millis(200));
        let inicio = Instant::now();
        assert!(esperar_preparo());
        let esperou = inicio.elapsed();
        assert!(esperou >= Duration::from_millis(150) && esperou < Duration::from_secs(10), "{esperou:?}");
        let inicio = Instant::now();
        assert!(esperar_preparo());
        assert!(inicio.elapsed() < Duration::from_millis(100), "{:?}", inicio.elapsed());

        // Parar durante a espera: a compilacao sai interrompida, sem rodar o motor
        iniciar_preparo(Duration::from_secs(30));
        assert!(!cmd_parar_compilacao(), "sem ninguem esperando nao ha o que parar");
        let parar = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(300));
            cmd_parar_compilacao()
        });
        let inicio = Instant::now();
        let r = compilar_com_bibtex("nao-existe.tex", &std::env::temp_dir().display().to_string(), "pdflatex", false);
        assert!(parar.join().unwrap());
        assert!(inicio.elapsed() < Duration::from_secs(10), "{:?}", inicio.elapsed());
        assert!(!r.sucesso);
        assert!(r.log.contains(MSG_INTERROMPIDA), "{}", r.log);
        terminar_preparo();
        // sem preparo: nao espera
        let inicio = Instant::now();
        assert!(esperar_preparo());
        assert!(inicio.elapsed() < Duration::from_secs(1));
    }

    /// O fonts.conf do TinyTeX v2026.10 aponta a pasta do runneradmin; passa a apontar a raiz real.
    #[test]
    fn fonts_conf_do_tinytex_aponta_a_pasta_real() {
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("fonts-conf");
        let conf = pasta.0.join("texmf-var/fonts/conf");
        fs::create_dir_all(&conf).unwrap();
        let original = "<fontconfig>\n<!--\n<dir>c:/usr/local/share/texmf/fonts/opentype/adobe/kozuka</dir>\n-->\n\n<dir>C:/Windows/fonts</dir>\n\
                        <dir>C:/Users/runneradmin/AppData/Roaming/TinyTeX/texmf-dist/fonts/opentype</dir>\n\
                        <dir>C:/Users/runneradmin/AppData/Roaming/TinyTeX/texmf-dist/fonts/truetype</dir>\n</fontconfig>\n";
        fs::write(conf.join("fonts.conf"), original).unwrap();
        assert!(corrigir_fonts_conf(&pasta.0));
        let novo = fs::read_to_string(conf.join("fonts.conf")).unwrap();
        let base = pasta.0.to_string_lossy().replace('\\', "/");
        assert!(novo.contains(&format!("<dir>{base}/texmf-dist/fonts/opentype</dir>")), "{novo}");
        assert!(novo.contains(&format!("<dir>{base}/texmf-dist/fonts/truetype</dir>")), "{novo}");
        assert!(novo.contains("<dir>C:/Windows/fonts</dir>"));
        assert!(!novo.contains("runneradmin"));
        // o exemplo comentado (sem texmf-dist) fica como esta; a segunda vez nao muda nada
        assert!(novo.contains("kozuka"));
        assert!(!corrigir_fonts_conf(&pasta.0));
    }

    /// Conta sem gravacao no texmf-var do TinyTeX (outra conta do Windows): nada do TinyTeX muda;
    /// a copia corrigida do fonts.conf (conf.d absoluto, cachedir proprio) e o TEXMFVAR ficam na
    /// pasta da conta e vao para o TeX pelo ambiente.
    #[test]
    fn conta_sem_gravacao_usa_texmfvar_e_fonts_conf_proprios() {
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("fonts-conf-conta");
        let tinytex = pasta.0.join("TinyTeX");
        let conf = tinytex.join("texmf-var/fonts/conf");
        fs::create_dir_all(&conf).unwrap();
        let original = "<fontconfig>\r\n<dir>C:/Users/runneradmin/AppData/Roaming/TinyTeX/texmf-dist/fonts/opentype</dir>\r\n\
                        <include ignore_missing=\"yes\">conf.d</include>\r\n</fontconfig>\r\n";
        fs::write(conf.join("fonts.conf"), original).unwrap();
        let conta = pasta.0.join("conta");

        assert!(corrigir_fonts_conf_em(&tinytex, Some(&conta)));
        assert_eq!(fs::read_to_string(conf.join("fonts.conf")).unwrap(), original);
        let copia = fs::read_to_string(conta.join("fontconfig/fonts.conf")).unwrap();
        let base = tinytex.to_string_lossy().replace('\\', "/");
        assert!(copia.contains(&format!("<dir>{base}/texmf-dist/fonts/opentype</dir>\r\n")), "{copia}");
        assert!(copia.contains(&format!(">{base}/texmf-var/fonts/conf/conf.d</include>")), "{copia}");
        assert!(copia.contains("/conta/fontconfig/cache</cachedir>\r\n</fontconfig>\r\n"), "{copia}");
        assert!(conta.join("texmf-var").is_dir());
        let mut comando = Command::new("x");
        com_ambiente_da_conta(&mut comando);
        let ambiente: Vec<(String, PathBuf)> = comando
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_string_lossy().into_owned(), PathBuf::from(v?))))
            .collect();
        assert!(ambiente.contains(&("TEXMFVAR".to_string(), conta.join("texmf-var"))), "{ambiente:?}");
        assert!(ambiente.contains(&("FONTCONFIG_FILE".to_string(), conta.join("fontconfig").join("fonts.conf"))), "{ambiente:?}");
        // segunda vez, copia igual: nada mudou, o ambiente continua
        assert!(!corrigir_fonts_conf_em(&tinytex, Some(&conta)));
        assert_eq!(travar(&AMBIENTE_CONTA).len(), 2);
        // a conta que grava no texmf-var corrige o do TinyTeX e nao usa ambiente proprio
        assert!(corrigir_fonts_conf(&tinytex));
        assert!(travar(&AMBIENTE_CONTA).is_empty());
        assert!(fs::read_to_string(conf.join("fonts.conf")).unwrap().ends_with("</fontconfig>\r\n"));
        assert!(pasta_gravavel(&conf));
    }

    /// LOCALAPPDATA so ASCII vale direto; com acento vale o nome curto; sem nome curto, o
    /// ProgramData.
    #[test]
    fn pasta_da_conta_sem_acento() {
        let pasta = PastaTemp::nova("pasta-conta");
        let ascii = pasta.0.join("Local");
        let acento = pasta.0.join("Usuário").join("Local");
        let curta = pasta.0.join("USUARI~1");
        let programdata = pasta.0.join("contas");
        let sem_curto = |_: &Path| None;
        let com_curto = |_: &Path| Some(curta.clone());
        let pd = || Some(programdata.clone());
        assert_eq!(escolher_pasta_da_conta(Some(&ascii), &sem_curto, &pd), Some(ascii.join("EguaTex")));
        assert_eq!(escolher_pasta_da_conta(Some(&acento), &com_curto, &pd), Some(curta.clone()));
        assert_eq!(escolher_pasta_da_conta(Some(&acento), &sem_curto, &pd), Some(programdata.clone()));
        assert_eq!(escolher_pasta_da_conta(None, &com_curto, &pd), Some(programdata));
        // o nome curto de verdade: nada, ou um caminho que existe
        if let Some(c) = caminho_curto(&acento.join("EguaTex")) {
            assert!(c.is_dir(), "{c:?}");
        }
    }

    /// A pasta do ProgramData fica so com a conta atual; criada de novo, e a mesma.
    #[cfg(windows)]
    #[test]
    fn pasta_restrita_so_da_conta_atual() {
        let pasta = PastaTemp::nova("pasta-restrita");
        let usuario = std::env::var("USERNAME").unwrap();
        let conta = match std::env::var("USERDOMAIN") {
            Ok(d) if !d.is_empty() => format!("{d}\\{usuario}"),
            _ => usuario.clone(),
        };
        let criada = pasta_restrita(&pasta.0, &conta).expect("pasta criada");
        assert!(criada.starts_with(&pasta.0));
        let saida = Command::new("icacls").arg(&criada).output().unwrap();
        let texto = String::from_utf8_lossy(&saida.stdout).to_string();
        assert!(!texto.contains("BUILTIN"), "{texto}");
        assert!(!texto.contains("(I)"), "heranca ficou: {texto}");
        assert!(texto.contains(&usuario), "{texto}");
        assert_eq!(pasta_restrita(&pasta.0, &conta), Some(criada.clone()));
        fs::write(criada.join("x"), "ok").unwrap();
        // sem a pasta contas (o instalador cria) nao ha pasta restrita
        assert_eq!(pasta_restrita(&pasta.0.join("sem-contas"), &conta), None);

        // outra conta (Usuarios) ganhou permissao: a pasta deixa de valer, na escolha e antes de compilar
        let _vez = travar_teste();
        *travar(&PASTA_CONTA) = Some(criada.clone());
        *travar(&AMBIENTE_CONTA) = vec![("TEXMFVAR", criada.join("texmf-var"))];
        conferir_pasta_da_conta();
        assert_eq!(travar(&AMBIENTE_CONTA).len(), 1, "pasta certa continua valendo");
        let ok = Command::new("icacls").arg(&criada).args(["/grant", "*S-1-5-32-545:(OI)(CI)M", "/Q"]).status().unwrap();
        assert!(ok.success());
        assert!(!pasta_so_da_conta(&criada));
        assert_eq!(pasta_restrita(&pasta.0, &conta), None);
        conferir_pasta_da_conta();
        assert!(travar(&AMBIENTE_CONTA).is_empty());
        assert!(travar(&PASTA_CONTA).is_none());
    }

    /// Sem .aux, a primeira passada roda em rascunho e a segunda grava o PDF; com o .aux no lugar
    /// nao ha rascunho. Um erro fatal no rascunho cai na passada normal, com o mesmo erro.
    #[test]
    fn primeira_compilacao_faz_rascunho_e_depois_o_pdf() {
        if !tex_instalado() {
            eprintln!("pdflatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("rascunho");
        fs::write(pasta.0.join("main.tex"), "\\documentclass{article}\n\\begin{document}\n\\section{Um}\\label{s}\nVeja \\ref{s}.\n\\end{document}\n").unwrap();
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.pdf.is_some(), "{}", r.log);
        assert!(r.log.contains("passada 1 em rascunho (-draftmode)"), "{}", r.log);
        assert!(r.log.contains("--- Passada 2 ---"), "{}", r.log);
        assert!(!ultima_passada(&r.log).contains("undefined"), "{}", r.log);
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(!r.log.contains("rascunho"), "{}", r.log);

        fs::write(pasta.0.join("ruim.tex"), "\\documentclass{article}\n\\usepackage{pacotequenaoexiste}\n\\begin{document}\nOi\n\\end{document}\n").unwrap();
        let r = compilar_com_bibtex("ruim.tex", &pasta.texto(), "pdflatex", false);
        assert!(!r.sucesso);
        assert!(r.pdf.is_none());
        assert!(!r.log.contains("passada 1 em rascunho"), "{}", r.log);
        assert!(r.erros.iter().any(|e| e.mensagem.contains("pacotequenaoexiste")), "{:?}", r.erros);
    }

    /// Primeira compilacao com bibliografia: rascunho, bibtex, segundo rascunho (le o .bbl novo) e
    /// a passada que grava o PDF, com o SyncTeX dela. Os rascunhos nao passam -synctex=1.
    #[test]
    fn bibliografia_nova_faz_o_segundo_rascunho_e_o_pdf_sai_certo() {
        if !tex_instalado() || detectar_executavel("bibtex").is_none() {
            eprintln!("pdflatex ou bibtex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("segundo-rascunho");
        fs::write(pasta.0.join("refs.bib"), "@book{a, author={Ana Silva}, title={Livro}, year={2020}, publisher={X}}\n").unwrap();
        fs::write(
            pasta.0.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\nVer \\cite{a}.\n\\bibliographystyle{plain}\n\\bibliography{refs}\n\\end{document}\n",
        )
        .unwrap();
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.log.contains("passada 1 em rascunho"), "{}", r.log);
        assert!(r.log.contains("passada 2 em rascunho"), "{}", r.log);
        assert!(r.log.contains("--- Passada 3 ---"), "{}", r.log);
        assert!(!ultima_passada(&r.log).contains("undefined"), "{}", r.log);
        assert!(pasta.0.join("main.synctex.gz").is_file());
        assert!(argumentos_motor(Path::new("a.tex"), false, false).iter().any(|a| a == "-synctex=1"));
        // sem mudanca: uma passada so, sem rascunho
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "pdflatex", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(!r.log.contains("rascunho"), "{}", r.log);
        // teto de 2 passadas: nao cabe o segundo rascunho
        let _ = crate::arquivos::cmd_limpar_auxiliares(pasta.texto());
        let r = compilar_com_bibtex_ate("main.tex", &pasta.texto(), "pdflatex", false, Some(2));
        assert!(r.pdf.is_some(), "{}", r.log);
        assert!(!r.log.contains("passada 2 em rascunho"), "{}", r.log);
    }

    #[test]
    fn motor_automatico_so_nos_nomes_desconhecidos() {
        assert!(motor_automatico("auto"));
        assert!(motor_automatico(""));
        for m in ["pdflatex", "pdfLaTeX", "xelatex", "XeLaTeX", "lualatex", "LuaLaTeX"] {
            assert!(!motor_automatico(m), "{m}");
        }
    }

    /// TU Berlin: a interface manda o preambulo (sem \begin{document}); compila o poster.tex.
    #[test]
    fn preambulo_sem_begin_document_compila_o_principal() {
        let pasta = PastaTemp::nova("preambulo");
        fs::write(pasta.0.join("00-preambel.tex"), "\\documentclass{article}\n").unwrap();
        fs::write(pasta.0.join("poster.tex"), "\\input{00-preambel}\n\\begin{document}\nOi\n\\end{document}\n").unwrap();
        let (arquivo, nota) = resolver_principal("00-preambel.tex", &pasta.texto());
        assert_eq!(arquivo, "poster.tex");
        assert!(nota.unwrap().contains("00-preambel.tex"));
        // o principal fica como esta
        assert_eq!(resolver_principal("poster.tex", &pasta.texto()), ("poster.tex".to_string(), None));
        if !tex_instalado() {
            return;
        }
        let _vez = travar_teste();
        let r = compilar_com_bibtex("00-preambel.tex", &pasta.texto(), "auto", false);
        assert!(r.sucesso, "{}", r.log);
        assert!(r.pdf.as_deref().is_some_and(|p| p.ends_with("poster.pdf")), "{:?}", r.pdf);
        assert!(r.log.contains("Motor automático: pdfLaTeX"), "{}", r.log);
    }

    /// fontspec visivel no documento: o automatico ja escolhe o XeLaTeX. Escondido atras de uma
    /// macro (a deteccao nao ve), o pdfLaTeX falha com o erro do fontspec e a compilacao se repete
    /// uma vez com o XeLaTeX.
    #[test]
    fn motor_automatico_escolhe_e_repete_com_o_motor_certo() {
        if detectar_executavel("xelatex").is_none() || !tex_instalado() {
            eprintln!("xelatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("auto-xe");
        fs::write(pasta.0.join("main.tex"), "\\documentclass{article}\n\\usepackage{fontspec}\n\\begin{document}\nOlá\n\\end{document}\n").unwrap();
        let r = compilar_com_bibtex("main.tex", &pasta.texto(), "auto", false);
        assert!(r.sucesso, "{}", r.log);
        assert_eq!(r.motor_usado, "xelatex");
        assert!(r.log.contains("Motor automático: XeLaTeX"), "{}", r.log);

        fs::write(pasta.0.join("esc.tex"), "\\documentclass{article}\n\\def\\p{fontspec}\n\\usepackage{\\p}\n\\begin{document}\nOlá\n\\end{document}\n").unwrap();
        let r = compilar_com_bibtex("esc.tex", &pasta.texto(), "auto", false);
        assert!(r.sucesso, "{}", r.log);
        assert_eq!(r.motor_usado, "xelatex");
        assert!(r.log.contains("compilado de novo com o XeLaTeX"), "{}", r.log);

        // escolhido a mao: nao repete
        let r = compilar_com_bibtex("esc.tex", &pasta.texto(), "pdflatex", false);
        assert!(!r.sucesso);
        assert_eq!(r.motor_usado, "pdflatex");
    }

    #[test]
    fn motores_alternativos_na_ordem_sem_repetir() {
        // erro de motor sem PDF: XeLaTeX e LuaLaTeX
        let fonte = "! Font \\x=foo at 10pt not loadable: Metric (TFM) file not found.";
        assert_eq!(motores_alternativos("pdflatex", "pdflatex", fonte, false, false), vec!["xelatex", "lualatex"]);
        // o lembrado falhou: volta ao detectado antes dos outros
        assert_eq!(motores_alternativos("xelatex", "pdflatex", fonte, false, false), vec!["pdflatex", "lualatex"]);
        // erro do documento: nenhum outro motor
        assert!(motores_alternativos("pdflatex", "pdflatex", "! Emergency stop.", false, false).is_empty());
        // o log aponta o motor certo: ele vem primeiro
        let log = "! Package fontspec Error: The fontspec package requires either XeTeX or LuaTeX.";
        assert_eq!(motores_alternativos("pdflatex", "pdflatex", log, false, false), vec!["xelatex", "lualatex"]);
        let lua = "! Undefined control sequence.\nl.3 \\directlua";
        assert_eq!(motores_alternativos("xelatex", "xelatex", lua, false, false), vec!["lualatex"]);
        // com PDF so repete se o log apontou o motor errado
        assert!(motores_alternativos("pdflatex", "pdflatex", "", false, true).is_empty());
        assert_eq!(motores_alternativos("pdflatex", "pdflatex", log, false, true), vec!["xelatex"]);
        // falta um .sty: os outros motores tambem nao o acham
        let falta = "! LaTeX Error: File `uarial.sty' not found.";
        assert!(motores_alternativos("pdflatex", "pdflatex", falta, false, false).is_empty());
    }

    #[test]
    fn erro_de_motor_so_nos_erros_que_outro_motor_resolve() {
        for log in [
            "! Package fontspec Error: The fontspec package requires either XeTeX or\n(fontspec) LuaTeX.",
            "! Package mathspec Error: XeTeX is required.",
            "! Font \\x=foo at 10pt not loadable: Metric (TFM) file not found.",
            "Missing character: There is no ç in font cmr10!",
            "! LaTeX Error: Unicode character 中 (U+4E2D)\n               not set up for use with LaTeX.",
            "! TeX capacity exceeded, sorry [main memory size=5000000].",
        ] {
            assert!(erro_de_motor(log, "pdflatex"), "{log}");
        }
        for log in [
            "! LaTeX Error: File `capitulo.tex' not found.",
            "! LaTeX Error: File `geometry.sty' not found.",
            "! Paragraph ended before \\textbf was complete.\nRunaway argument?",
            "! Undefined control sequence.\nl.5 \\comandoqualquer",
            "! Emergency stop.",
        ] {
            assert!(!erro_de_motor(log, "pdflatex"), "{log}");
        }
    }

    /// O pdfLaTeX (detectado) para sem PDF com erro de fonte que a deteccao nao ve: o automatico
    /// tenta o XeLaTeX, fica com o PDF dele e devolve "xelatex@impressao" para lembrar. Na compilacao
    /// seguinte o lembrado roda primeiro; com o preambulo mudado ele e esquecido. Diretiva e latexmkrc
    /// com motor nao tentam outro. Erro do documento nao tenta outro. Sem PDF em nenhum, valem o erro
    /// e o .log do detectado.
    #[test]
    fn automatico_tenta_os_outros_motores_e_lembra_o_que_funcionou() {
        if detectar_executavel("xelatex").is_none() || detectar_executavel("lualatex").is_none() || !tex_instalado() {
            eprintln!("xelatex ou lualatex não instalado: teste pulado");
            return;
        }
        let _vez = travar_teste();
        let pasta = PastaTemp::nova("auto-alternativo");
        let so_xe = "\\documentclass{article}\n\\ifx\\XeTeXrevision\\undefined\\PackageError{teste}{Font x not loadable}{}\\expandafter\\stop\\fi\n\\begin{document}\nOi\n\\end{document}\n";
        fs::write(pasta.0.join("main.tex"), so_xe).unwrap();
        assert_eq!(detectar_motor("main.tex", &pasta.texto()).motor, "pdflatex");
        let r = compilar_completo("main.tex", &pasta.texto(), "auto", false, None, None);
        assert!(r.pdf.is_some(), "{}", r.log);
        assert_eq!(r.motor_usado, "xelatex");
        let lembrar = r.motor_lembrar.clone().unwrap();
        assert!(lembrar.starts_with("xelatex@"), "{lembrar}");
        assert!(r.log.contains("pdfLaTeX não gerou o PDF; compilado de novo com o XeLaTeX"), "{}", r.log);

        let r = compilar_completo("main.tex", &pasta.texto(), "auto", false, None, Some(&lembrar));
        assert!(r.pdf.is_some(), "{}", r.log);
        assert_eq!(r.motor_usado, "xelatex");
        assert_eq!(r.motor_lembrar.as_deref(), Some(lembrar.as_str()));
        assert!(r.log.contains("motor lembrado deste projeto: XeLaTeX"), "{}", r.log);
        // sem impressao (formato antigo) o lembrado nao vale
        let r = compilar_completo("main.tex", &pasta.texto(), "auto", false, None, Some("xelatex"));
        assert!(!r.log.contains("motor lembrado"), "{}", r.log);

        // preambulo mudou: o lembrado e esquecido, a deteccao decide e o ciclo recomeca
        fs::write(pasta.0.join("main.tex"), so_xe.replace("\\begin{document}", "\\usepackage{amsmath}\n\\begin{document}")).unwrap();
        let r = compilar_completo("main.tex", &pasta.texto(), "auto", false, None, Some(&lembrar));
        assert!(r.log.contains("o XeLaTeX lembrado foi esquecido"), "{}", r.log);
        assert_eq!(r.motor_usado, "xelatex");
        assert_ne!(r.motor_lembrar.as_deref(), Some(lembrar.as_str()));

        // a diretiva manda: nem lembrado nem outra tentativa
        fs::write(pasta.0.join("dir.tex"), format!("% !TeX program = pdflatex\n{so_xe}")).unwrap();
        let r = compilar_completo("dir.tex", &pasta.texto(), "auto", false, None, Some(&lembrar));
        assert!(r.pdf.is_none());
        assert_eq!(r.motor_usado, "pdflatex");
        assert_eq!(r.motor_lembrar, None);
        assert!(!r.log.contains("compilado de novo"), "{}", r.log);

        // erro do documento (\input ausente): nenhum outro motor
        fs::write(pasta.0.join("doc.tex"), "\\documentclass{article}\n\\begin{document}\n\\input{naoexiste}\n\\end{document}\n").unwrap();
        let r = compilar_completo("doc.tex", &pasta.texto(), "auto", false, None, None);
        assert_eq!(r.motor_usado, "pdflatex");
        assert!(!r.log.contains("tentado também"), "{}", r.log);

        // nenhum motor gera PDF: ficam o erro e o .log do detectado
        fs::write(pasta.0.join("nada.tex"), "\\documentclass{article}\n\\PackageError{teste}{Font x not loadable}{}\n\\stop\n\\begin{document}\nOi\n\\end{document}\n").unwrap();
        let r = compilar_completo("nada.tex", &pasta.texto(), "auto", false, None, None);
        assert!(r.pdf.is_none());
        assert_eq!(r.motor_usado, "pdflatex");
        assert_eq!(r.motor_lembrar, None);
        assert!(r.log.contains("tentado também com XeLaTeX e LuaLaTeX"), "{}", r.log);
        let log_disco = fs::read_to_string(pasta.0.join("nada.log")).unwrap();
        assert!(log_disco.contains("pdfTeX"), "{log_disco}");

        // latexmkrc com motor: igual a diretiva
        let outra = PastaTemp::nova("auto-latexmkrc");
        fs::write(outra.0.join("main.tex"), so_xe).unwrap();
        fs::write(outra.0.join("latexmkrc"), "$pdflatex = 'pdflatex %O %S';\n").unwrap();
        let r = compilar_completo("main.tex", &outra.texto(), "auto", false, None, Some(&lembrar));
        assert!(r.pdf.is_none());
        assert_eq!(r.motor_usado, "pdflatex");
        assert_eq!(r.motor_lembrar, None);
        assert!(!r.log.contains("compilado de novo") && !r.log.contains("motor lembrado"), "{}", r.log);
    }

    /// Projetos reais (nao entra no `cargo test` comum): `EGUATEX_PROJETOS_REAIS` aponta um arquivo
    /// com uma pasta por linha; `EGUATEX_RELATORIO` recebe a tabela. Para cada pasta: apaga os
    /// auxiliares, escolhe o principal e o motor como o app e compila duas vezes no automatico.
    /// Rodar: `cargo test projetos_reais -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn projetos_reais() {
        let lista = std::env::var("EGUATEX_PROJETOS_REAIS").expect("EGUATEX_PROJETOS_REAIS");
        // com EGUATEX_AQUECER apontando o bin\windows de um TinyTeX, faz o preparo da primeira abertura
        if let Ok(bin) = std::env::var("EGUATEX_AQUECER") {
            let marca = Instant::now();
            aquecer_tinytex(Path::new(&bin));
            rodar_em_segundo_plano(Command::new(Path::new(&bin).join("biber.exe")).arg("--version"));
            eprintln!("preparo do TinyTeX: {} ms", marca.elapsed().as_millis());
        }
        let saida = std::env::var("EGUATEX_RELATORIO").unwrap_or_else(|_| "relatorio-projetos.md".into());
        let mut tabela = String::from(
            "| Projeto | Principal | Motor | 1a (ms) | 2a (ms) | Paginas | Erros | Tempos da 1a |\n|---|---|---|---|---|---|---|---|\n",
        );
        let paginas = |log: &str| {
            log.rmatch_indices("Output written on")
                .next()
                .and_then(|(i, _)| log[i..].split('(').nth(1))
                .and_then(|r| r.split_whitespace().next())
                .unwrap_or("0")
                .to_string()
        };
        for pasta in std::fs::read_to_string(lista).unwrap().lines().map(str::trim).filter(|l| !l.is_empty()) {
            let raiz = Path::new(pasta);
            let nome = raiz.file_name().unwrap().to_string_lossy().to_string();
            let Some(principal) = crate::arquivos::principal_do_projeto(raiz) else {
                tabela.push_str(&format!("| {nome} | nenhum | | | | | | |\n"));
                continue;
            };
            let _ = crate::arquivos::cmd_limpar_auxiliares(pasta.to_string());
            let motor = detectar_motor(&principal, pasta);
            let r1 = compilar_com_bibtex(&principal, pasta, "auto", false);
            let r2 = compilar_com_bibtex(&principal, pasta, "auto", false);
            let n_erros = r1.erros.iter().filter(|e| e.tipo != "aviso").count();
            let primeiros: Vec<String> = r1.erros.iter().filter(|e| e.tipo != "aviso").take(2).map(|e| e.mensagem.replace('|', "/").chars().take(70).collect()).collect();
            let tempos = r1.log.lines().next().unwrap_or("").trim_start_matches("ÉguaTeX: tempos: ").to_string();
            let linha = format!(
                "| {nome} | {principal} | {} ({}) | {} | {} | {} | {n_erros} {} | {tempos} |\n",
                r1.motor_usado,
                motor.motivo.replace('|', "/"),
                r1.tempo_ms,
                r2.tempo_ms,
                paginas(&r1.log),
                primeiros.join("; "),
            );
            eprint!("{linha}");
            tabela.push_str(&linha);
            std::fs::write(&saida, &tabela).unwrap();
        }
    }

    #[test]
    fn obter_versao_de_executavel_real_devolve_primeira_linha() {
        let cargo = std::env::var("CARGO").expect("CARGO definido pelo cargo test");
        let v = obter_versao(Path::new(&cargo));
        assert!(v.starts_with("cargo"), "{v:?}");
        assert!(!v.contains('\n'));
    }

    #[test]
    fn obter_versao_de_caminho_inexistente_devolve_vazio() {
        assert_eq!(obter_versao(Path::new("C:/nao/existe/biber-fantasma.exe")), "");
        assert_eq!(obter_versao(Path::new("")), "");
    }
}

#[cfg(test)]
#[path = "compilador_avisos_borda.rs"]
mod compilador_avisos_borda;

#[cfg(test)]
mod testes_fonte_que_falta {
    use super::{pacote_da_fonte, termo_da_fonte};

    // saida real do `tlmgr search --global --file` (12/11/2026)
    const BUSCA: &str = "tlmgr.pl: package repository https://exemplo/tlnet (not verified: pubkey missing)\n\
00texlive.image:\n\ttlpkg/tlpsrc/lstfiracode.tlpsrc\n\
lstfiracode:\n\ttexmf-dist/tex/latex/lstfiracode/lstfiracode.sty\n\
tex-gyre:\n\ttexmf-dist/fonts/opentype/public/tex-gyre/texgyrebonum-bold.otf\n\
\ttexmf-dist/fonts/opentype/public/tex-gyre/texgyrebonum-regular.otf\n";

    #[test]
    fn termo_tira_espacos_e_extensao() {
        assert_eq!(termo_da_fonte("TeX Gyre Bonum").as_deref(), Some("texgyrebonum"));
        assert_eq!(termo_da_fonte("EBGaramond-Regular.otf").as_deref(), Some("ebgaramond-regular"));
        assert_eq!(termo_da_fonte("Fira Code").as_deref(), Some("firacode"));
        assert_eq!(termo_da_fonte(""), None);
        assert_eq!(termo_da_fonte("x;rm -rf"), None);
        assert_eq!(termo_da_fonte("../../a"), None);
    }

    #[test]
    fn acha_o_pacote_pela_fonte_e_ignora_estilos() {
        assert_eq!(pacote_da_fonte(BUSCA, "texgyrebonum").as_deref(), Some("tex-gyre"));
        // lstfiracode.sty nao e fonte: Fira Code nao existe no TeX Live
        assert_eq!(pacote_da_fonte(BUSCA, "firacode"), None);
    }
}
