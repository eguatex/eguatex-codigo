//! Exportacao do documento para Word (.docx), Markdown (.md) e HTML (.html) pelo
//! Pandoc, como o menu "Download" do Overleaf. O Pandoc le o LaTeX e converte as
//! formulas para equacoes nativas do Word (docx) ou MathML (html).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static PANDOC: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Pasta do Pandoc dentro dos recursos do app (vem junto com o instalador).
const PANDOC_EMBUTIDO: &str = "pandoc/pandoc.exe";

/// Chamada no setup: guarda onde esta o Pandoc (o embutido primeiro, depois o do PATH).
pub fn localizar_pandoc(recursos: &Path) {
    let embutido = recursos.join(PANDOC_EMBUTIDO);
    let achado = if embutido.is_file() {
        Some(embutido)
    } else {
        which::which("pandoc").ok()
    };
    let _ = PANDOC.set(achado);
}

pub(crate) fn pandoc() -> Option<PathBuf> {
    PANDOC
        .get()
        .cloned()
        .unwrap_or_else(|| which::which("pandoc").ok())
}

#[tauri::command]
pub fn cmd_pandoc_disponivel() -> bool {
    pandoc().is_some()
}

/// Argumentos do Pandoc para cada formato. `pasta_saida` recebe as imagens do Markdown.
fn argumentos(formato: &str, entrada: &str, destino: &Path, bibs: &[String]) -> Result<Vec<String>, String> {
    let mut args = vec![
        "--from=latex".to_string(),
        "--resource-path=.".to_string(),
        format!("--output={}", destino.display()),
    ];
    match formato {
        "docx" => args.push("--to=docx".into()),
        "html" => {
            args.push("--to=html5".into());
            args.push("--standalone".into());
            args.push("--embed-resources".into());
            args.push("--math-method=mathml".into());
        }
        "md" => {
            args.push("--to=markdown".into());
            // as figuras vao para uma pasta ao lado do .md
            let stem = destino.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let pasta = destino.with_file_name(format!("{stem}_figuras"));
            args.push(format!("--extract-media={}", pasta.display()));
        }
        outro => return Err(format!("Formato desconhecido: {outro}")),
    }
    if !bibs.is_empty() {
        args.push("--citeproc".into());
        for b in bibs {
            args.push(format!("--bibliography={b}"));
        }
    }
    // "--" encerra as opcoes: um arquivo chamado "--lua-filter=x.tex" nao vira opcao do Pandoc
    args.push("--".into());
    args.push(entrada.to_string());
    Ok(args)
}

/// Os .bib da pasta do documento, para as citacoes virarem referencias no arquivo exportado.
fn bibliografias(pasta: &Path) -> Vec<String> {
    let mut lista: Vec<String> = std::fs::read_dir(pasta)
        .map(|it| {
            it.flatten()
                .filter_map(|e| {
                    let nome = e.file_name().to_string_lossy().into_owned();
                    nome.to_lowercase().ends_with(".bib").then_some(nome)
                })
                .collect()
        })
        .unwrap_or_default();
    lista.sort();
    lista
}

/// Converte o documento principal (`arquivo`, relativo a `diretorio`) e grava em `destino`.
#[tauri::command(async)]
pub fn cmd_exportar_documento(
    diretorio: String,
    arquivo: String,
    formato: String,
    destino: String,
) -> Result<(), String> {
    let pandoc = pandoc().ok_or_else(|| {
        "O conversor Pandoc não foi encontrado. Reinstale o ÉguaTeX ou instale o Pandoc (pandoc.org).".to_string()
    })?;
    let caminho = Path::new(diretorio.trim_end_matches(['/', '\\'])).join(arquivo.trim_start_matches(['/', '\\']));
    if !caminho.is_file() {
        return Err(format!("Documento não encontrado: {}", caminho.display()));
    }
    let pasta = caminho.parent().map(Path::to_path_buf).unwrap_or_default();
    let entrada = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let destino = PathBuf::from(destino);
    let args = argumentos(&formato, &entrada, &destino, &bibliografias(&pasta))?;

    let mut comando = Command::new(pandoc);
    comando.current_dir(&pasta).args(&args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        comando.creation_flags(CREATE_NO_WINDOW);
    }
    let saida = comando
        .output()
        .map_err(|e| format!("Não foi possível rodar o Pandoc: {e}"))?;
    if !saida.status.success() {
        let erro = String::from_utf8_lossy(&saida.stderr);
        return Err(format!("O Pandoc não conseguiu converter o documento:\n{}", erro.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn argumentos_de_cada_formato() {
        let destino = Path::new("C:/saida/artigo.md");
        let md = argumentos("md", "main.tex", destino, &["refs.bib".into()]).unwrap();
        assert!(md.contains(&"--to=markdown".to_string()));
        assert!(md.iter().any(|a| a.starts_with("--extract-media=") && a.ends_with("artigo_figuras")));
        assert!(md.contains(&"--citeproc".to_string()));
        assert!(md.contains(&"--bibliography=refs.bib".to_string()));

        let html = argumentos("html", "main.tex", Path::new("a.html"), &[]).unwrap();
        assert!(html.contains(&"--embed-resources".to_string()));
        assert!(!html.contains(&"--citeproc".to_string()));

        assert!(argumentos("docx", "main.tex", Path::new("a.docx"), &[]).unwrap().contains(&"--to=docx".to_string()));
        assert!(argumentos("odt", "main.tex", Path::new("a.odt"), &[]).is_err());
    }

    #[test]
    fn entrada_vem_depois_de_dois_tracos() {
        for formato in ["docx", "html", "md"] {
            let args = argumentos(formato, "--lua-filter=x.tex", Path::new("a.out"), &["refs.bib".into()]).unwrap();
            let n = args.len();
            assert_eq!(args[n - 2], "--");
            assert_eq!(args[n - 1], "--lua-filter=x.tex");
            // o nome do arquivo nao aparece em nenhuma posicao anterior
            assert!(!args[..n - 1].iter().any(|a| a == "--lua-filter=x.tex"));
        }
    }
}
