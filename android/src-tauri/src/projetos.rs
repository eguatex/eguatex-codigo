use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projeto {
    pub nome: String,
    pub caminho: String,
    pub arquivo_principal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArquivoNo {
    pub nome: String,
    pub tipo: String,
    pub caminho: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filhos: Option<Vec<ArquivoNo>>,
    /// Bytes do arquivo; pastas nao tem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tamanho: Option<u64>,
}

pub fn listar_arvore(diretorio: &str) -> Vec<ArquivoNo> {
    let path = Path::new(diretorio);
    if !path.is_dir() {
        return Vec::new();
    }
    listar_recursivo(path, path)
}

fn listar_recursivo(dir: &Path, raiz: &Path) -> Vec<ArquivoNo> {
    let mut nos = Vec::new();

    let mut entradas: Vec<_> = match fs::read_dir(dir) {
        Ok(e) => e.filter_map(|r| r.ok()).collect(),
        Err(_) => return nos,
    };

    entradas.sort_by_key(|e| {
        let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(true);
        (is_file, e.file_name())
    });

    for entrada in entradas {
        let nome = entrada.file_name().to_string_lossy().to_string();

        if nome.starts_with('.') || nome == "node_modules" || nome == "target" {
            continue;
        }

        let caminho_rel = entrada
            .path()
            .strip_prefix(raiz)
            .unwrap_or(&entrada.path())
            .to_string_lossy()
            .replace('\\', "/");

        let caminho_display = format!("/{}", caminho_rel);

        if entrada.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let filhos = listar_recursivo(&entrada.path(), raiz);
            nos.push(ArquivoNo {
                nome,
                tipo: "pasta".to_string(),
                caminho: caminho_display,
                filhos: Some(filhos),
                tamanho: None,
            });
        } else {
            let tamanho = entrada.metadata().ok().map(|m| m.len());
            nos.push(ArquivoNo {
                nome,
                tipo: "arquivo".to_string(),
                caminho: caminho_display,
                filhos: None,
                tamanho,
            });
        }
    }

    nos
}

/// Junta o caminho relativo a pasta do projeto, recusando "..", unidade ou raiz,
/// para que nada (nem o agente de IA) leia ou grave fora do projeto.
pub fn caminho_seguro(caminho_projeto: &str, caminho_relativo: &str) -> Result<PathBuf, String> {
    use std::path::Component;
    let rel = Path::new(caminho_relativo.trim_start_matches(['/', '\\']));
    if rel.as_os_str().is_empty()
        || rel.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("Caminho fora do projeto: {caminho_relativo}"));
    }
    let base = Path::new(caminho_projeto);
    let path = base.join(rel);
    // Se ja existe, confere o caminho real (atalhos e links para fora sao recusados).
    if let (Ok(real), Ok(base_real)) = (path.canonicalize(), base.canonicalize()) {
        if !real.starts_with(&base_real) {
            return Err(format!("Caminho fora do projeto: {caminho_relativo}"));
        }
    }
    Ok(path)
}

pub fn ler_arquivo(caminho_projeto: &str, caminho_relativo: &str) -> Result<String, String> {
    let path = caminho_seguro(caminho_projeto, caminho_relativo)?;
    let bytes = fs::read(&path).map_err(|e| format!("Erro ao ler {}: {}", path.display(), e))?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    match std::str::from_utf8(bytes) {
        Ok(texto) => Ok(texto.to_string()),
        // binario de verdade (imagem, PDF) continua recusado
        Err(_) if bytes.contains(&0) => Err(format!("Erro ao ler {}: o arquivo não é texto", path.display())),
        // .tex e .bib antigos em Latin-1 ou Windows-1252: abrem e passam a ser gravados em
        // UTF-8; o inputenc do documento muda junto para os acentos continuarem certos
        Err(_) => Ok(trocar_inputenc_para_utf8(&decodificar_cp1252(bytes))),
    }
}

/// Maior arquivo que o visualizador de imagens le (25 MB). Acima disso so o botao Baixar.
pub const LIMITE_BYTES_PROJETO: u64 = 25 * 1024 * 1024;

/// Prefixo do erro de arquivo grande demais; o resto da mensagem e o tamanho em bytes. O
/// frontend monta o texto a partir do numero, entao o formato nao muda sem mudar `imagem.ts`.
pub const PREFIXO_ACIMA_DO_LIMITE: &str = "acima-do-limite:";

/// Bytes de um arquivo do projeto (imagem), sem passar por UTF-8. O caminho e resolvido com
/// `caminho_seguro` e o tamanho e conferido antes da leitura.
pub fn ler_bytes_projeto(caminho_projeto: &str, caminho_relativo: &str) -> Result<Vec<u8>, String> {
    ler_bytes_com_limite(caminho_projeto, caminho_relativo, LIMITE_BYTES_PROJETO)
}

fn ler_bytes_com_limite(caminho_projeto: &str, caminho_relativo: &str, limite: u64) -> Result<Vec<u8>, String> {
    let path = caminho_seguro(caminho_projeto, caminho_relativo)?;
    let meta = fs::metadata(&path).map_err(|e| format!("Erro ao ler {}: {}", path.display(), e))?;
    if !meta.is_file() {
        return Err(format!("Erro ao ler {}: não é um arquivo", path.display()));
    }
    if meta.len() > limite {
        return Err(format!("{PREFIXO_ACIMA_DO_LIMITE}{}", meta.len()));
    }
    fs::read(&path).map_err(|e| format!("Erro ao ler {}: {}", path.display(), e))
}

/// Windows-1252 (superconjunto do Latin-1 usado no Brasil) para texto.
fn decodificar_cp1252(bytes: &[u8]) -> String {
    const ALTOS: [char; 32] = [
        '\u{20AC}', '\u{81}', '\u{201A}', '\u{192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
        '\u{2C6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8D}', '\u{17D}', '\u{8F}',
        '\u{90}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
        '\u{2DC}', '\u{2122}', '\u{161}', '\u{203A}', '\u{153}', '\u{9D}', '\u{17E}', '\u{178}',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => ALTOS[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

/// `\usepackage[latin1]{inputenc}` (ou ansinew, cp1252) vira `[utf8]`, porque o arquivo
/// sera gravado em UTF-8.
fn trocar_inputenc_para_utf8(texto: &str) -> String {
    let mut saida = texto.to_string();
    for antigo in ["latin1", "latin9", "ansinew", "cp1252", "latin-1"] {
        saida = saida.replace(&format!("[{antigo}]{{inputenc}}"), "[utf8]{inputenc}");
    }
    saida
}

pub fn salvar_arquivo(
    caminho_projeto: &str,
    caminho_relativo: &str,
    conteudo: &str,
) -> Result<(), String> {
    let path = caminho_seguro(caminho_projeto, caminho_relativo)?;

    if let Some(pai) = path.parent() {
        fs::create_dir_all(pai)
            .map_err(|e| format!("Erro ao criar diretório: {}", e))?;
    }

    gravar_atomico(&path, conteudo.as_bytes())
        .map_err(|e| format!("Erro ao salvar {}: {}", path.display(), e))
}

/// Grava num arquivo temporario ao lado e troca pelo original. Se o computador desligar
/// ou o OneDrive segurar o arquivo no meio, o original continua inteiro.
pub fn gravar_atomico(path: &Path, conteudo: &[u8]) -> std::io::Result<()> {
    let nome = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temporario = path.with_file_name(format!(".{nome}.eguatex-tmp"));
    fs::write(&temporario, conteudo)?;
    let mut ultimo_erro = None;
    // o OneDrive e o antivirus seguram o arquivo por instantes (erro 32 no Windows)
    for tentativa in 0..5 {
        match fs::rename(&temporario, path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                ultimo_erro = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(100 * (tentativa + 1)));
            }
        }
    }
    let _ = fs::remove_file(&temporario);
    Err(ultimo_erro.unwrap_or_else(|| std::io::Error::other("falha ao gravar")))
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjetoNaPasta {
    pub nome: String,
    pub caminho: String,
    /// Ultima modificacao da pasta, em segundos desde 1970.
    pub modificado: i64,
}

/// Lista as pastas de projeto (com pelo menos um .tex na raiz) dentro da pasta de projetos.
pub fn listar_projetos(diretorio: &str) -> Vec<ProjetoNaPasta> {
    let Ok(entradas) = fs::read_dir(diretorio) else { return Vec::new() };
    let mut lista: Vec<ProjetoNaPasta> = entradas
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            fs::read_dir(e.path())
                .map(|itens| {
                    itens.flatten().any(|i| {
                        i.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("tex"))
                    })
                })
                .unwrap_or(false)
        })
        .map(|e| {
            let modificado = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            ProjetoNaPasta {
                nome: e.file_name().to_string_lossy().into_owned(),
                caminho: e.path().display().to_string(),
                modificado,
            }
        })
        .collect();
    lista.sort_by(|a, b| b.modificado.cmp(&a.modificado));
    lista
}

/// Sufixos dos arquivos que a compilacao grava: nao contam como modificacao do projeto.
const SUFIXOS_GERADOS: &[&str] = &[
    ".aux", ".log", ".synctex.gz", ".synctex", ".bbl", ".blg", ".bcf", ".run.xml", ".fls", ".fdb_latexmk",
    ".out", ".toc", ".lof", ".lot", ".nav", ".snm", ".xdv", ".idx", ".ind", ".ilg", ".vrb", ".brf", ".nlo", ".nls",
    ".glo", ".gls", ".glg", ".acn", ".acr", ".alg", ".ist", ".loa", ".lol", ".maf", ".ptc", ".thm", "-blx.bib",
    ".listing",
];

/// Arquivo gravado pela compilacao: sufixo da lista ou minitoc (.mtc, .mtc0, .mtc1...).
fn arquivo_gerado(nome: &str) -> bool {
    if SUFIXOS_GERADOS.iter().any(|s| nome.ends_with(s)) {
        return true;
    }
    nome.rsplit_once(".mtc").is_some_and(|(_, resto)| resto.chars().all(|c| c.is_ascii_digit()))
}
/// Arquivos lidos no maximo por projeto, para a lista da tela inicial nao travar numa pasta enorme.
const MAX_ARQUIVOS_MODIFICACAO: usize = 2000;

/// Ultima modificacao do projeto, em milissegundos desde 1970: o arquivo mais novo da pasta e das
/// subpastas (ate 3 niveis), como a "Última modificação" do Overleaf. Nao contam as pastas ocultas
/// (.eguatex, .git), os auxiliares da compilacao e o PDF que tem um .tex de mesmo nome ao lado.
/// `None` se a pasta nao existe.
pub fn modificacao_do_projeto(pasta: &Path) -> Option<i64> {
    if !pasta.is_dir() {
        return None;
    }
    let mut mais_novo: Option<i64> = None;
    let mut lidos = 0usize;
    let mut fila: Vec<(PathBuf, u32)> = vec![(pasta.to_path_buf(), 0)];
    while let Some((dir, nivel)) = fila.pop() {
        let Ok(entradas) = fs::read_dir(&dir) else { continue };
        for e in entradas.flatten() {
            let nome = e.file_name().to_string_lossy().to_lowercase();
            if nome.starts_with('.') {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                // _minted-* guarda o codigo colorido que o minted gera em cada compilacao
                if nivel < 3 && !nome.starts_with("_minted") {
                    fila.push((e.path(), nivel + 1));
                }
                continue;
            }
            lidos += 1;
            if lidos > MAX_ARQUIVOS_MODIFICACAO {
                return mais_novo;
            }
            if arquivo_gerado(&nome) {
                continue;
            }
            if let Some(base) = nome.strip_suffix(".pdf")
                && dir.join(format!("{base}.tex")).is_file()
            {
                continue;
            }
            let ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);
            if let Some(ms) = ms {
                mais_novo = Some(mais_novo.map_or(ms, |m| m.max(ms)));
            }
        }
    }
    mais_novo.or_else(|| {
        fs::metadata(pasta)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
    })
}

pub fn pasta_projetos() -> String {
    // No Android os projetos ficam na pasta privada do app (EGUATEX_DADOS).
    let base = std::env::var_os("EGUATEX_DADOS")
        .map(PathBuf::from)
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("EguaTex").display().to_string()
}

pub fn renomear_projeto(caminho: &str, novo_nome: &str) -> Result<String, String> {
    let nome = novo_nome.trim();
    if nome.is_empty() || nome.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("Nome inválido".to_string());
    }
    let origem = PathBuf::from(caminho);
    let pai = origem
        .parent()
        .ok_or_else(|| "Projeto sem pasta-mae".to_string())?;
    let destino = pai.join(nome);
    if destino.exists() {
        return Err(format!("Já existe uma pasta chamada {}", nome));
    }
    fs::rename(&origem, &destino).map_err(|e| format!("Erro ao renomear: {}", e))?;
    Ok(destino.display().to_string())
}

pub fn criar_projeto(diretorio: &str, nome: &str) -> Result<Projeto, String> {
    let caminho = PathBuf::from(diretorio).join(nome);
    if caminho.join("main.tex").exists() {
        return Err(format!("Já existe um projeto em {}", caminho.display()));
    }
    fs::create_dir_all(&caminho)
        .map_err(|e| format!("Erro ao criar projeto: {}", e))?;

    let tex_inicial = r#"\documentclass[12pt,a4paper]{article}
\usepackage[utf8]{inputenc}
\usepackage[brazil]{babel}

\title{Meu Documento}
\author{Seu Nome}
\date{\today}

\begin{document}
\maketitle

\section{Introdução}
Escreva aqui o texto do seu documento.

\end{document}"#;

    let main_path = caminho.join("main.tex");
    fs::write(&main_path, tex_inicial)
        .map_err(|e| format!("Erro ao criar main.tex: {}", e))?;

    Ok(Projeto {
        nome: nome.to_string(),
        caminho: caminho.display().to_string(),
        arquivo_principal: "main.tex".to_string(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_de_teste(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eguatex-projetos-{}-{}", std::process::id(), nome));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn caminho_seguro_recusa_sair_do_projeto() {
        let base = std::env::temp_dir().join("eguatex-teste-caminho");
        std::fs::create_dir_all(&base).unwrap();
        let b = base.to_str().unwrap();
        assert!(caminho_seguro(b, "main.tex").is_ok());
        assert!(caminho_seguro(b, "/capitulos/cap1.tex").is_ok());
        assert!(caminho_seguro(b, "../fora.tex").is_err());
        assert!(caminho_seguro(b, "capitulos/../../fora.tex").is_err());
        assert!(caminho_seguro(b, "C:/Windows/win.ini").is_err());
        assert!(caminho_seguro(b, "").is_err());
    }

    #[test]
    fn listar_arvore_devolve_tamanho_do_arquivo_e_nenhum_para_pasta() {
        let base = std::env::temp_dir().join(format!("eguatex-teste-tamanho-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("figuras")).unwrap();
        std::fs::write(base.join("main.tex"), "12345").unwrap();
        let nos = listar_arvore(base.to_str().unwrap());
        let arquivo = nos.iter().find(|n| n.nome == "main.tex").unwrap();
        assert_eq!(arquivo.tamanho, Some(5));
        let pasta = nos.iter().find(|n| n.nome == "figuras").unwrap();
        assert_eq!(pasta.tamanho, None);
        // o JSON de uma pasta nao leva o campo
        assert!(!serde_json::to_string(pasta).unwrap().contains("tamanho"));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A data da lista e a do arquivo de texto mais novo; o PDF e os auxiliares da compilacao e a
    /// pasta .eguatex nao contam.
    #[test]
    fn modificacao_do_projeto_ignora_o_que_a_compilacao_grava() {
        let dir = pasta_de_teste("modificacao");
        fs::create_dir_all(dir.join("cap")).unwrap();
        fs::create_dir_all(dir.join(".eguatex")).unwrap();
        fs::write(dir.join("main.tex"), "a").unwrap();
        fs::write(dir.join("cap/um.tex"), "b").unwrap();
        let antigo = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for f in ["main.tex", "cap/um.tex"] {
            fs::File::options().write(true).open(dir.join(f)).unwrap().set_modified(antigo).unwrap();
        }
        let base = antigo.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        fs::create_dir_all(dir.join("_minted-main")).unwrap();
        for f in [
            "main.pdf", "main.aux", "main.log", "main.synctex.gz", ".eguatex/chat.json", "main.glo", "main.gls",
            "main.glg", "main.acn", "main.acr", "main.alg", "main.ist", "main.loa", "main.lol", "main.mtc",
            "main.mtc0", "main.mtc12", "main.maf", "main.ptc", "main.thm", "main-blx.bib", "main.run.xml",
            "main.xdv", "main.nav", "main.snm", "main.vrb", "main.listing", "_minted-main/codigo.pygtex",
        ] {
            fs::write(dir.join(f), "x").unwrap();
        }
        assert!(!arquivo_gerado("refs.bib"));
        assert!(!arquivo_gerado("main.mtcx"));
        let m = modificacao_do_projeto(&dir).unwrap();
        assert!((m - base).abs() < 2000, "{m} {base}");
        // uma figura em PDF sem .tex de mesmo nome conta
        fs::write(dir.join("cap/figura.pdf"), "x").unwrap();
        assert!(modificacao_do_projeto(&dir).unwrap() > base + 60_000);
        assert_eq!(modificacao_do_projeto(&dir.join("nao-existe")), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
