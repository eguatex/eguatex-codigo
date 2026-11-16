//! Limites de seguranca para zip: o do "Abrir .zip" (arquivos.rs), o do Drive (drive.rs) e a
//! leitura de enderecos content:// (ponte_uri.rs) passam por aqui.
//!
//! Um zip pequeno pode virar centenas de MB no disco (bomba de descompressao: 0,5 MB viram
//! 512 MB), ter dezenas de milhares de entradas ou trazer atalhos e programas. Por isso:
//! - antes de extrair, o diretorio central e somado e conferido (`conferir`);
//! - durante a copia, os bytes reais sao contados com `take` (`copiar_limitado`), porque o
//!   cabecalho do zip pode mentir sobre o tamanho;
//! - entradas que rodam programas (.exe, .lnk...) ou cujo nome comeca com "-" sao ignoradas
//!   (`motivo_de_bloqueio`) e voltam numa lista, para a pessoa ser avisada.
//!
//! Copia do limites_zip.rs do Android. No Windows so o Drive usa `conferir`, `Limites`,
//! `motivo_de_bloqueio` e `aviso_de_ignorados`; o resto (importar .zip, espaco em disco) fica sem uso
//! aqui, e os testes que passam por `arquivos::cmd_importar_zip` (a versao do Android) ficam so no Android.

#![allow(dead_code)]

use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path};

/// Limites escolhidos pelo Paulo. Nos testes entram valores menores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limites {
    /// Bytes descompactados no total.
    pub total: u64,
    /// Bytes descompactados em cada arquivo.
    pub arquivo: u64,
    /// Entradas (arquivos e pastas) do zip.
    pub entradas: usize,
    /// Razao maxima entre o tamanho descompactado e o compactado.
    pub razao: u64,
    /// Pastas aninhadas num caminho.
    pub profundidade: usize,
}

pub const MIB: u64 = 1024 * 1024;

impl Limites {
    pub const PADRAO: Limites = Limites { total: 500 * MIB, arquivo: 100 * MIB, entradas: 5000, razao: 100, profundidade: 16 };
}

/// A razao so vale de 1 MB para cima: um arquivo pequeno de zeros comprime muito, mas nao
/// faz mal, e os limites de total e de arquivo ja cobrem o resto.
const MINIMO_PARA_RAZAO: u64 = MIB;

/// Folga que precisa sobrar no disco depois da extracao.
const FOLGA_DISCO: u64 = 64 * MIB;

/// Extensoes que rodam programas no Windows, ou levam a eles ao abrir (.lnk, .url, .scf).
const EXTENSOES_BLOQUEADAS: &[&str] = &[
    ".lnk", ".exe", ".bat", ".cmd", ".ps1", ".msi", ".scr", ".vbs", ".js", ".url", ".scf",
];

/// Por que uma entrada do zip nao e gravada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// Pode rodar um programa (.exe, .lnk, .bat...).
    Executavel,
    /// Nome que comeca com "-": um programa o leria como opcao (`--lua-filter=x.tex`).
    HifenInicial,
}

/// "100 MB", ou "512 KB" para limites pequenos (so nos testes).
pub fn texto_tamanho(bytes: u64) -> String {
    if bytes >= MIB { format!("{} MB", bytes / MIB) } else { format!("{} KB", bytes.div_ceil(1024)) }
}

/// Uma parte do caminho (ou um fluxo alternativo do NTFS) termina com extensao bloqueada?
/// Ignora maiusculas, espacos e pontos no fim ("a.Bat ", "x.ps1.") e confere cada trecho
/// separado por ":" ("leia.txt:escondido.exe", "a.exe::$DATA").
pub fn extensao_bloqueada(nome: &str) -> bool {
    nome.split(':').any(|trecho| {
        let limpo = trecho.trim_end_matches([' ', '.']).to_lowercase();
        EXTENSOES_BLOQUEADAS.iter().any(|e| limpo.ends_with(e))
    })
}

/// Decide se a entrada `caminho` (do zip, com "/" ou "\\") fica de fora. So o ultimo
/// componente conta para a extensao; qualquer componente com "-" no inicio e recusado, como
/// em `validar_nome`, fora a pasta "-texmf" na raiz do projeto.
pub fn motivo_de_bloqueio(caminho: &str) -> Option<Motivo> {
    let partes: Vec<&str> = caminho.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    // excecao unica: "-texmf" na raiz e a convencao do Overleaf para classes e pacotes do projeto
    // (config_projeto.rs so a procura na raiz; o TU Delft traz a classe tudelft-beamer nela).
    // Nenhum programa a recebe como argumento: o motor so recebe o nome do documento principal.
    let n = partes.len();
    let texmf_na_raiz = |i: usize, p: &str| i == 0 && n > 1 && p.eq_ignore_ascii_case("-texmf");
    if partes.iter().enumerate().any(|(i, p)| p.starts_with('-') && !texmf_na_raiz(i, p)) {
        return Some(Motivo::HifenInicial);
    }
    partes.last().filter(|ultimo| extensao_bloqueada(ultimo)).map(|_| Motivo::Executavel)
}

/// Pastas aninhadas de um caminho de entrada (a ultima parte conta como pasta so se `e_pasta`).
fn profundidade(caminho: &str, e_pasta: bool) -> usize {
    let partes = Path::new(caminho).components().filter(|c| matches!(c, Component::Normal(_))).count();
    if e_pasta { partes } else { partes.saturating_sub(1) }
}

/// Confere o diretorio central do zip antes de extrair qualquer coisa: numero de entradas,
/// profundidade, tamanho de cada arquivo, soma e razao de compressao. Devolve a soma dos
/// tamanhos declarados. O cabecalho pode mentir: `copiar_limitado` confere os bytes reais.
pub fn conferir<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, lim: &Limites) -> Result<u64, String> {
    if zip.len() > lim.entradas {
        return Err(format!(
            "O zip tem {} itens e o limite é {}. Compacte só o projeto, sem pastas de dados ou de cópias.",
            zip.len(),
            lim.entradas
        ));
    }
    let mut total: u64 = 0;
    let mut compactado: u64 = 0;
    for i in 0..zip.len() {
        let item = zip.by_index_raw(i).map_err(|e| format!("O zip está corrompido: {e}"))?;
        let nome = item.name().to_string();
        if profundidade(&nome, item.is_dir()) > lim.profundidade {
            return Err(format!("O zip tem pastas dentro de pastas demais ({nome}). O limite é {} níveis.", lim.profundidade));
        }
        let tamanho = item.size();
        if tamanho > lim.arquivo {
            return Err(format!("{nome} passa do limite de {} por arquivo.", texto_tamanho(lim.arquivo)));
        }
        if tamanho >= MINIMO_PARA_RAZAO && tamanho > item.compressed_size().saturating_mul(lim.razao) {
            return Err(format!(
                "{nome} se expande mais de {}:1 ao abrir. Parece uma bomba de descompressão e foi recusado.",
                lim.razao
            ));
        }
        total = total.saturating_add(tamanho);
        compactado = compactado.saturating_add(item.compressed_size());
        if total > lim.total {
            return Err(format!("O zip passa do limite de {} descompactado.", texto_tamanho(lim.total)));
        }
    }
    if total >= MINIMO_PARA_RAZAO && total > compactado.saturating_mul(lim.razao) {
        return Err(format!(
            "O zip se expande mais de {}:1 ao abrir. Parece uma bomba de descompressão e foi recusado.",
            lim.razao
        ));
    }
    Ok(total)
}

/// Copia `origem` em `destino` contando os bytes reais. Para ao passar do limite por arquivo
/// ou do que sobra do limite total (`restante`, que diminui a cada copia). Le no maximo
/// `teto + 1` bytes: nunca grava mais do que o limite permite, mesmo que o cabecalho minta.
pub fn copiar_limitado<R: Read, W: Write>(
    origem: R,
    destino: &mut W,
    lim: &Limites,
    restante: &mut u64,
) -> Result<u64, String> {
    let teto = lim.arquivo.min(*restante);
    let mut limitado = origem.take(teto.saturating_add(1));
    let copiados = io::copy(&mut limitado, destino).map_err(|e| e.to_string())?;
    if copiados > teto {
        return Err(if teto == lim.arquivo {
            format!("Um arquivo do zip passa do limite de {} descompactado.", texto_tamanho(lim.arquivo))
        } else {
            format!("O zip passa do limite de {} descompactado.", texto_tamanho(lim.total))
        });
    }
    *restante -= copiados;
    Ok(copiados)
}

/// Espaco livre, em bytes, no disco da pasta `onde` (ou do primeiro ancestral que existe).
/// `None` se o sistema nao informa.
pub fn espaco_livre(onde: &Path) -> Option<u64> {
    let existente = onde.ancestors().find(|p| p.exists())?;
    espaco_livre_do_sistema(existente)
}

#[cfg(unix)]
fn espaco_livre_do_sistema(pasta: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let caminho = std::ffi::CString::new(pasta.as_os_str().as_bytes()).ok()?;
    // SAFETY: `caminho` termina em NUL e `info` tem o tamanho de `statvfs`.
    let mut info: libc::statvfs = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::statvfs(caminho.as_ptr(), &mut info) };
    if r != 0 {
        return None;
    }
    Some((info.f_bavail as u64).saturating_mul(info.f_frsize as u64))
}

#[cfg(windows)]
fn espaco_livre_do_sistema(pasta: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            diretorio: *const u16,
            livre_para_o_usuario: *mut u64,
            total: *mut u64,
            livre_total: *mut u64,
        ) -> i32;
    }
    let mut largo: Vec<u16> = pasta.as_os_str().encode_wide().collect();
    largo.push(0);
    let (mut livre, mut total, mut livre_total) = (0u64, 0u64, 0u64);
    // SAFETY: `largo` termina em NUL e os tres ponteiros apontam para variaveis locais.
    let ok = unsafe { GetDiskFreeSpaceExW(largo.as_ptr(), &mut livre, &mut total, &mut livre_total) };
    (ok != 0).then_some(livre)
}

#[cfg(not(any(unix, windows)))]
fn espaco_livre_do_sistema(_pasta: &Path) -> Option<u64> {
    None
}

/// Confere se cabem `necessario` bytes (mais uma folga) no disco de `onde`.
pub fn conferir_espaco(onde: &Path, necessario: u64) -> Result<(), String> {
    conferir_espaco_com(espaco_livre(onde), necessario)
}

fn conferir_espaco_com(livre: Option<u64>, necessario: u64) -> Result<(), String> {
    match livre {
        Some(livre) if livre < necessario.saturating_add(FOLGA_DISCO) => Err(format!(
            "Não há espaço livre suficiente: o zip precisa de {} e o aparelho tem {} livres.",
            texto_tamanho(necessario.saturating_add(FOLGA_DISCO)),
            texto_tamanho(livre)
        )),
        _ => Ok(()),
    }
}

/// Nome seguro para mostrar na lista de ignorados (sem quebras de linha nem controles).
pub fn nome_para_aviso(nome: &str) -> String {
    nome.chars().map(|c| if c.is_control() { '?' } else { c }).collect()
}

/// Texto do aviso para a pessoa, com o que foi ignorado (no maximo 10 nomes).
pub fn aviso_de_ignorados(ignorados: &[String]) -> String {
    let mostrados: Vec<String> = ignorados.iter().take(10).map(|n| nome_para_aviso(n)).collect();
    let mais = ignorados.len().saturating_sub(mostrados.len());
    let sufixo = if mais > 0 { format!(" e mais {mais}") } else { String::new() };
    format!(
        "{} arquivo(s) do zip foram ignorados por poderem rodar programas ou por terem nome que começa com hífen: {}{}.",
        ignorados.len(),
        mostrados.join(", "),
        sufixo
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn opcoes() -> zip::write::SimpleFileOptions {
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
    }

    #[test]
    fn extensoes_bloqueadas_sem_diferenciar_maiusculas_espaco_ponto_e_ads() {
        for ruim in [
            "a.exe", "A.EXE", "a.Lnk", "x.bat ", "x.ps1.", "x.cmd. .", "a.msi", "a.scr", "a.vbs", "a.js", "a.url", "a.scf",
            "leia.txt:escondido.exe", "a.exe::$DATA", "a.txt:Zone.Identifier:x.lnk", ".exe",
        ] {
            assert!(extensao_bloqueada(ruim), "deixou passar {ruim:?}");
        }
        for bom in ["main.tex", "a.exe.txt", "exe", "a.jsx", "a.json", "leia.txt:x", "a.txt:Zone.Identifier", "bat", "a.sh"] {
            assert!(!extensao_bloqueada(bom), "bloqueou {bom:?}");
        }
    }

    #[test]
    fn motivo_olha_o_ultimo_nome_para_extensao_e_todos_para_o_hifen() {
        assert_eq!(motivo_de_bloqueio("a/b/run.exe"), Some(Motivo::Executavel));
        assert_eq!(motivo_de_bloqueio("a\\b\\RUN.EXE"), Some(Motivo::Executavel));
        assert_eq!(motivo_de_bloqueio("--lua-filter=x.tex"), Some(Motivo::HifenInicial));
        assert_eq!(motivo_de_bloqueio("-pasta/x.tex"), Some(Motivo::HifenInicial));
        // a pasta -texmf do Overleaf entra so na raiz; um arquivo -texmf, outra pasta com hifen
        // ou -texmf dentro de outra pasta, nao
        assert_eq!(motivo_de_bloqueio("-texmf/tex/latex/tudelft-beamer/tudelft-beamer.cls"), None);
        assert_eq!(motivo_de_bloqueio("-TEXMF/tex/x.sty"), None);
        assert_eq!(motivo_de_bloqueio("-texmf"), Some(Motivo::HifenInicial));
        assert_eq!(motivo_de_bloqueio("-texmf/-x.tex"), Some(Motivo::HifenInicial));
        assert_eq!(motivo_de_bloqueio("capitulos/-texmf/x.sty"), Some(Motivo::HifenInicial));
        assert_eq!(motivo_de_bloqueio("pasta.exe/x.tex"), None);
        assert_eq!(motivo_de_bloqueio("cap/a-b.tex"), None);
    }

    #[test]
    fn copiar_limitado_nunca_le_mais_que_o_teto_mais_um() {
        let lim = Limites { arquivo: 1000, total: 5000, ..Limites::PADRAO };
        let mut restante = lim.total;
        let mut origem = io::repeat(7).take(400 * MIB);
        let mut saida = Vec::new();
        let erro = copiar_limitado(&mut origem, &mut saida, &lim, &mut restante).unwrap_err();
        assert!(erro.contains("limite"), "{erro}");
        // 400 MB de entrada: so 1001 bytes foram puxados da origem
        assert_eq!(origem.limit(), 400 * MIB - 1001);
        assert_eq!(saida.len(), 1001);
    }

    #[test]
    fn copiar_limitado_respeita_o_que_sobra_do_total() {
        let lim = Limites { arquivo: 1000, total: 1500, ..Limites::PADRAO };
        let mut restante = lim.total;
        let mut saida = Vec::new();
        assert_eq!(copiar_limitado(&b"a".repeat(900)[..], &mut saida, &lim, &mut restante), Ok(900));
        assert_eq!(restante, 600);
        let erro = copiar_limitado(&b"a".repeat(700)[..], &mut saida, &lim, &mut restante).unwrap_err();
        assert!(erro.contains("limite"), "{erro}");
    }

    #[test]
    fn conferir_recusa_total_arquivo_razao_profundidade_e_entradas() {
        fn zip_com_zeros(itens: &[(&str, u64)]) -> zip::ZipArchive<io::Cursor<Vec<u8>>> {
            let mut z = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
            for (nome, bytes) in itens {
                z.start_file(*nome, opcoes()).unwrap();
                io::copy(&mut io::repeat(0).take(*bytes), &mut z).unwrap();
            }
            zip::ZipArchive::new(z.finish().unwrap()).unwrap()
        }
        let pequeno = Limites { total: 3 * MIB, arquivo: 2 * MIB, entradas: 3, razao: 100, profundidade: 2 };
        // 2 arquivos de 1,5 MB: cada um cabe, a soma (3 MB + 1) passa de 3 MB
        let mut z = zip_com_zeros(&[("a", MIB + MIB / 2), ("b", MIB + MIB / 2 + 1)]);
        assert!(conferir(&mut z, &Limites { razao: 100_000, ..pequeno }).unwrap_err().contains("3 MB"));
        // arquivo de 3 MB passa do limite por arquivo
        let mut z = zip_com_zeros(&[("a", 3 * MIB)]);
        assert!(conferir(&mut z, &Limites { razao: 100_000, ..pequeno }).unwrap_err().contains("2 MB"));
        // zeros: razao de ~1000:1
        let mut z = zip_com_zeros(&[("a", MIB + 1)]);
        assert!(conferir(&mut z, &pequeno).unwrap_err().contains("100:1"));
        // profundidade 3 com limite 2
        let mut z = zip_com_zeros(&[("a/b/c/d.txt", 1)]);
        assert!(conferir(&mut z, &pequeno).unwrap_err().contains("2 níveis"));
        // 4 entradas com limite 3
        let mut z = zip_com_zeros(&[("a", 1), ("b", 1), ("c", 1), ("d", 1)]);
        assert!(conferir(&mut z, &pequeno).unwrap_err().contains("limite é 3"));
        // dentro de tudo: devolve a soma
        let mut z = zip_com_zeros(&[("a/b/c.txt", 10), ("d", 5)]);
        assert_eq!(conferir(&mut z, &pequeno), Ok(15));
    }

    #[test]
    fn espaco_livre_do_disco_e_conferido() {
        assert!(espaco_livre(&std::env::temp_dir()).is_some_and(|livre| livre > 0));
        // pasta que ainda nao existe: usa o ancestral
        assert!(espaco_livre(&std::env::temp_dir().join("nao-existe-ainda").join("x")).is_some());
        assert!(conferir_espaco_com(Some(10 * MIB), MIB).unwrap_err().contains("espaço"));
        assert!(conferir_espaco_com(Some(10 * MIB * 100), MIB).is_ok());
        assert!(conferir_espaco_com(None, u64::MAX).is_ok());
    }

    #[test]
    fn aviso_lista_os_ignorados() {
        let nomes: Vec<String> = (0..12).map(|i| format!("a{i}.exe")).collect();
        let aviso = aviso_de_ignorados(&nomes);
        assert!(aviso.contains("12 arquivo(s)") && aviso.contains("a9.exe") && !aviso.contains("a10.exe") && aviso.contains("e mais 2"));
        assert_eq!(nome_para_aviso("a\nb"), "a?b");
    }
}
