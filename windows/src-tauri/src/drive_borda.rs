//! Testes de borda da extracao de zip no projeto aberto (embrulho, zip slip, nomes, repeticao).
//! Arquivo so de testes, igual no ÉguaTeX e no ÉguaTeX Android.
use super::*;
use std::io::{Cursor, Write};

fn dir_temp(rotulo: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eguatex-borda-{rotulo}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Termina em "/" = entrada de pasta.
fn zip_de(entradas: &[(&str, &str)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    for (nome, txt) in entradas {
        if nome.ends_with('/') {
            z.add_directory(nome.trim_end_matches('/'), o).unwrap();
        } else {
            z.start_file(*nome, o).unwrap();
            z.write_all(txt.as_bytes()).unwrap();
        }
    }
    z.finish().unwrap().into_inner()
}

const DOC: &str = "\\documentclass{article}\n\\begin{document}x\\end{document}";

fn meta(nome: &str) -> MetaRemota {
    MetaRemota { id: "Zb".into(), nome: nome.into(), mime: "application/zip".into(), ..Default::default() }
}

async fn trazer(dir: &Path, nome: &str, bytes: Vec<u8>, novo: bool) -> ResultadoAbrir {
    let mut r = ResultadoAbrir { novo, ..Default::default() };
    trazer_para_o_projeto(dir, vec![Baixado::solto(meta(nome), bytes)], None, &mut r).await.unwrap();
    r
}

#[test]
fn zip_com_um_so_arquivo_perde_a_pasta_de_embrulho() {
    let dir = dir_temp("um-arquivo");
    let g = extrair_zip(&dir, &zip_de(&[("pasta/x.tex", DOC)]), "124").unwrap();
    assert_eq!(g, vec!["124/x.tex".to_string()]);
    assert!(dir.join("124/x.tex").is_file());
    assert!(!dir.join("124/pasta").exists());

    let dir2 = dir_temp("um-arquivo-raiz");
    let g2 = extrair_zip(&dir2, &zip_de(&[("x.tex", DOC)]), "124").unwrap();
    assert_eq!(g2, vec!["124/x.tex".to_string()]);
}

#[test]
fn zip_vazio_nao_grava_nada() {
    let dir = dir_temp("vazio");
    let g = extrair_zip(&dir, &zip_de(&[]), "124").unwrap();
    assert!(g.is_empty());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[tokio::test]
async fn zip_vazio_nao_sugere_principal() {
    let dir = dir_temp("vazio-trazer");
    let r = trazer(&dir, "124.zip", zip_de(&[]), false).await;
    assert_eq!(r.principal_sugerido, None);
    assert!(r.adicionados.is_empty());
}

#[test]
fn zip_so_com_pastas_nao_grava_nada() {
    let dir = dir_temp("so-pastas");
    let g = extrair_zip(&dir, &zip_de(&[("a/", ""), ("a/b/", ""), ("a/b/c/", "")]), "124").unwrap();
    assert!(g.is_empty());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[tokio::test]
async fn dois_tex_com_documentclass_main_vence_senao_o_mais_raso() {
    // main.tex mais fundo vence o arquivo da raiz do zip
    let dir = dir_temp("dois-main");
    let z = zip_de(&[("w/raiz.tex", DOC), ("w/sub/fundo/main.tex", DOC), ("w/x.txt", "t")]);
    let r = trazer(&dir, "124.zip", z, false).await;
    assert_eq!(r.principal_sugerido.as_deref(), Some("124/sub/fundo/main.tex"));

    // sem main.tex: o mais raso, depois a ordem alfabetica
    let dir2 = dir_temp("dois-raso");
    let z2 = zip_de(&[("w/z/b.tex", DOC), ("w/z/a.tex", DOC), ("w/c.tex", DOC), ("w/d.tex", DOC)]);
    let r2 = trazer(&dir2, "124.zip", z2, false).await;
    assert_eq!(r2.principal_sugerido.as_deref(), Some("124/c.tex"));
}

#[tokio::test]
async fn documentclass_em_comentario_nao_conta() {
    let dir = dir_temp("comentario");
    let z = zip_de(&[("w/a.tex", "%\\documentclass{article}\nTexto"), ("w/b.tex", DOC)]);
    let r = trazer(&dir, "124.zip", z, false).await;
    assert_eq!(r.principal_sugerido.as_deref(), Some("124/b.tex"));

    let dir2 = dir_temp("so-comentario");
    let z2 = zip_de(&[("w/a.tex", "  % \\documentclass{article}"), ("w/b.tex", "sem nada")]);
    let r2 = trazer(&dir2, "124.zip", z2, false).await;
    assert_eq!(r2.principal_sugerido, None);
    assert!(dir2.join("124/a.tex").is_file());
}

#[test]
fn zip_slip_e_caminho_absoluto_sao_descartados_antes_de_tirar_o_embrulho() {
    let dir = dir_temp("slip");
    let fora = dir.parent().unwrap();
    let z = zip_de(&[
        ("../fora-borda-1.tex", "x"),
        ("/abs-borda-2.tex", "x"),
        ("m/T/../../fora-borda-3.tex", "x"),
        ("m\\T\\..\\..\\fora-borda-4.tex", "x"),
        ("m/T/x.tex", DOC),
        ("m/T/y.txt", "t"),
    ]);
    let mut g = extrair_zip(&dir, &z, "124").unwrap();
    g.sort();
    assert_eq!(g, vec!["124/x.tex".to_string(), "124/y.txt".to_string()]);
    for n in ["fora-borda-1.tex", "abs-borda-2.tex", "fora-borda-3.tex", "fora-borda-4.tex"] {
        assert!(!fora.join(n).exists(), "{n} escapou");
        assert!(!dir.join(n).exists(), "{n} escapou");
        assert!(!dir.join("124").join(n).exists(), "{n} escapou");
    }
}

#[test]
fn zip_so_com_caminhos_perigosos_nao_grava_nada() {
    let dir = dir_temp("slip-total");
    let g = extrair_zip(&dir, &zip_de(&[("../a-borda.tex", "x"), ("/b-borda.tex", "x")]), "124").unwrap();
    assert!(g.is_empty());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[tokio::test]
async fn nomes_com_acento_e_espaco_sobrevivem() {
    let dir = dir_temp("acento");
    let z = zip_de(&[
        ("Relatório Final/Seção 1/capítulo um.tex", DOC),
        ("Relatório Final/Seção 1/figura é.png", "png"),
    ]);
    let r = trazer(&dir, "Relatório de Física.zip", z, false).await;
    assert_eq!(r.principal_sugerido.as_deref(), Some("Relatório de Física/capítulo um.tex"));
    assert!(dir.join("Relatório de Física/capítulo um.tex").is_file());
    assert!(dir.join("Relatório de Física/figura é.png").is_file());
    assert!(!dir.join("Relatório de Física/Relatório Final").exists());
}

#[tokio::test]
async fn zip_com_nome_de_pasta_ja_existente_nao_sobrescreve() {
    let dir = dir_temp("repete-124");
    fs::create_dir_all(dir.join("124/template-latex")).unwrap();
    fs::write(dir.join("124/template-latex/sbc-template.tex"), "MEU ARQUIVO").unwrap();
    let z = zip_de(&[("w/Template/template-latex/sbc-template.tex", DOC), ("w/Template/sbc.pdf", "pdf")]);
    let r = trazer(&dir, "124.zip", z, false).await;
    assert_eq!(fs::read_to_string(dir.join("124/template-latex/sbc-template.tex")).unwrap(), "MEU ARQUIVO");
    assert!(dir.join("124/template-latex/sbc-template (2).tex").is_file());
    assert!(dir.join("124/sbc.pdf").is_file());
    assert_eq!(r.principal_sugerido.as_deref(), Some("124/template-latex/sbc-template (2).tex"));
}

#[test]
fn nome_da_subpasta_do_zip_casos_de_borda() {
    assert_eq!(nome_da_subpasta_do_zip("124.zip"), "124");
    assert_eq!(nome_da_subpasta_do_zip("RELATÓRIO.ZIP"), "RELATÓRIO");
    assert_eq!(nome_da_subpasta_do_zip("a/b.zip"), "a-b");
    assert_eq!(nome_da_subpasta_do_zip(".zip"), "Arquivos do Drive");
    assert_eq!(nome_da_subpasta_do_zip(""), "Arquivos do Drive");
    assert_eq!(nome_da_subpasta_do_zip("ação"), "ação");
    assert_eq!(nome_da_subpasta_do_zip("ç.zip"), "ç");
    assert_eq!(nome_da_subpasta_do_zip("sem-extensao"), "sem-extensao");
    assert_eq!(nome_da_subpasta_do_zip("..zip"), "Arquivos do Drive");
}
