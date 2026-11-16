//! Testes de borda de `cmd_importar_zip` (zip do disco): embrulho, zip slip, nomes, repeticao.
use super::*;
use std::io::Write;

fn pasta(rotulo: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eguatex-arq-borda-{rotulo}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Termina em "/" = entrada de pasta.
fn grava_zip(caminho: &Path, entradas: &[(&str, &str)]) {
    let mut z = zip::ZipWriter::new(File::create(caminho).unwrap());
    let o = zip::write::SimpleFileOptions::default();
    for (nome, txt) in entradas {
        if nome.ends_with('/') {
            z.add_directory(nome.trim_end_matches('/'), o).unwrap();
        } else {
            z.start_file(*nome, o).unwrap();
            z.write_all(txt.as_bytes()).unwrap();
        }
    }
    z.finish().unwrap();
}

fn importa(zip_caminho: &Path, pai: &Path) -> Result<PathBuf, String> {
    cmd_importar_zip(zip_caminho.display().to_string(), pai.display().to_string()).map(PathBuf::from)
}

fn arquivos_em(raiz: &Path) -> Vec<String> {
    fn rec(base: &Path, atual: &Path, saida: &mut Vec<String>) {
        for e in fs::read_dir(atual).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rec(base, &p, saida);
            } else {
                saida.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut v = Vec::new();
    rec(raiz, raiz, &mut v);
    v.sort();
    v
}

#[test]
fn zip_de_um_so_arquivo_em_pasta_fica_na_raiz() {
    let t = pasta("um");
    grava_zip(&t.join("a.zip"), &[("embrulho/x.tex", "x")]);
    let p = importa(&t.join("a.zip"), &t.join("pai")).unwrap();
    assert_eq!(arquivos_em(&p), vec!["x.tex".to_string()]);
}

#[test]
fn zip_de_um_so_arquivo_na_raiz_continua_na_raiz() {
    let t = pasta("um-raiz");
    grava_zip(&t.join("a.zip"), &[("x.tex", "x")]);
    let p = importa(&t.join("a.zip"), &t.join("pai")).unwrap();
    assert_eq!(arquivos_em(&p), vec!["x.tex".to_string()]);
}

#[test]
fn zip_vazio_da_erro_claro() {
    let t = pasta("vazio");
    grava_zip(&t.join("a.zip"), &[]);
    let e = importa(&t.join("a.zip"), &t.join("pai")).unwrap_err();
    assert_eq!(e, "O zip está vazio");
}

#[test]
fn zip_so_com_pastas_nao_vira_projeto() {
    let t = pasta("so-pastas");
    grava_zip(&t.join("a.zip"), &[("a/", ""), ("a/b/", "")]);
    let e = importa(&t.join("a.zip"), &t.join("pai")).unwrap_err();
    assert!(e.contains("não tem arquivos LaTeX"), "{e}");
    assert!(!t.join("pai").join("a").exists());
}

/// Caso real do HighScore3.0e: um .rar, um PDF e o ._Icon_ do macOS, nenhum .tex.
#[test]
fn zip_sem_tex_da_mensagem_clara_e_nao_cria_pasta() {
    let t = pasta("sem-tex");
    let pai = t.join("pai");
    grava_zip(
        &t.join("HighScore3.0e.zip"),
        &[("HighScore3.0e/._Icon_", "i"), ("HighScore3.0e/Instalação.pdf", "pdf"), ("HighScore3.0e/HighScore Plus.rar", "rar")],
    );
    for _ in 0..2 {
        let e = importa(&t.join("HighScore3.0e.zip"), &pai).unwrap_err();
        assert!(e.starts_with("Este .zip não tem arquivos LaTeX (.tex)"), "{e}");
        assert!(e.contains("HighScore Plus.rar"), "{e}");
        assert!(!e.contains("._Icon_"), "{e}");
    }
    assert!(!pai.join("HighScore3.0e").exists());
    assert!(!pai.join("HighScore3.0e (2)").exists());
}

#[test]
fn mesmo_zip_importado_de_novo_reabre_a_pasta() {
    let t = pasta("reabre");
    let pai = t.join("pai");
    // como no MUL_poster.zip: mul_poster.cls e MUL_poster.cls viram um arquivo so no Windows
    grava_zip(
        &t.join("TU.zip"),
        &[("poster.tex", "x"), ("bilder/a.pdf", "p"), ("bilder/", ""), ("mul_poster.cls", "velho"), ("MUL_poster.cls", "novo")],
    );
    let p1 = importa(&t.join("TU.zip"), &pai).unwrap();
    fs::write(p1.join("poster.pdf"), "gerado").unwrap();
    let p2 = importa(&t.join("TU.zip"), &pai).unwrap();
    assert_eq!(p1, p2);
    assert!(!pai.join("TU (2)").exists());
    // outro zip com o mesmo nome e conteudo diferente: pasta nova
    grava_zip(&t.join("TU.zip"), &[("poster.tex", "y")]);
    let p3 = importa(&t.join("TU.zip"), &pai).unwrap();
    assert_eq!(p3.file_name().unwrap().to_string_lossy(), "TU (2)");
}

#[test]
fn zip_com_dois_tex_mantem_os_dois_sem_o_embrulho() {
    let t = pasta("dois");
    grava_zip(&t.join("a.zip"), &[("m/T/a.tex", "\\documentclass{article}"), ("m/T/sub/b.tex", "\\documentclass{book}")]);
    let p = importa(&t.join("a.zip"), &t.join("pai")).unwrap();
    assert_eq!(arquivos_em(&p), vec!["a.tex".to_string(), "sub/b.tex".to_string()]);
}

#[test]
fn zip_slip_e_caminho_absoluto_nao_escapam() {
    let t = pasta("slip");
    grava_zip(
        &t.join("a.zip"),
        &[
            ("../fora-arq-borda-1.tex", "x"),
            ("/abs-arq-borda-2.tex", "x"),
            ("m/T/../../fora-arq-borda-3.tex", "x"),
            ("m/T/x.tex", "ok"),
            ("m/T/y.txt", "ok"),
        ],
    );
    let pai = t.join("pai");
    let p = importa(&t.join("a.zip"), &pai).unwrap();
    assert_eq!(arquivos_em(&p), vec!["x.tex".to_string(), "y.txt".to_string()]);
    for n in ["fora-arq-borda-1.tex", "abs-arq-borda-2.tex", "fora-arq-borda-3.tex"] {
        assert!(!t.join(n).exists(), "{n} escapou para {}", t.display());
        assert!(!pai.join(n).exists(), "{n} escapou para {}", pai.display());
        assert!(!std::env::temp_dir().join(n).exists(), "{n} escapou para o temp");
    }
}

#[test]
fn zip_so_com_caminhos_perigosos_da_projeto_vazio_ou_erro_sem_escapar() {
    let t = pasta("slip-total");
    grava_zip(&t.join("a.zip"), &[("../so-fora-arq-borda.tex", "x")]);
    let r = importa(&t.join("a.zip"), &t.join("pai"));
    assert!(!t.join("so-fora-arq-borda.tex").exists());
    match r {
        Ok(p) => assert!(arquivos_em(&p).is_empty()),
        Err(e) => assert_eq!(e, "O zip está vazio"),
    }
}

#[test]
fn nomes_com_acento_e_espaco() {
    let t = pasta("acento");
    grava_zip(&t.join("Relatório Final.zip"), &[("Pasta Única/Seção 1/capítulo um.tex", "x"), ("Pasta Única/Seção 1/figura é.png", "p")]);
    let p = importa(&t.join("Relatório Final.zip"), &t.join("pai")).unwrap();
    assert_eq!(p.file_name().unwrap().to_string_lossy(), "Relatório Final");
    assert_eq!(arquivos_em(&p), vec!["capítulo um.tex".to_string(), "figura é.png".to_string()]);
}

#[test]
fn zip_com_nome_repetido_vira_pasta_numerada() {
    let t = pasta("repete");
    let pai = t.join("pai");
    fs::create_dir_all(pai.join("124")).unwrap();
    fs::write(pai.join("124/meu.tex"), "MEU").unwrap();
    grava_zip(&t.join("124.zip"), &[("w/x.tex", "x")]);
    let p = importa(&t.join("124.zip"), &pai).unwrap();
    assert_eq!(p.file_name().unwrap().to_string_lossy(), "124 (2)");
    assert_eq!(fs::read_to_string(pai.join("124/meu.tex")).unwrap(), "MEU");
    assert_eq!(arquivos_em(&p), vec!["x.tex".to_string()]);
}

/// Zips reais (nao entra no cargo test comum): EGUATEX_ZIPS_REAIS lista um zip por linha e
/// EGUATEX_ZIPS_DESTINO e a pasta pai. Importa cada um duas vezes e mostra a pasta e o principal.
/// Rodar: cargo test importar_zips_reais -- --ignored --nocapture
#[test]
#[ignore]
fn importar_zips_reais() {
    let lista = std::env::var("EGUATEX_ZIPS_REAIS").expect("EGUATEX_ZIPS_REAIS");
    let pai = PathBuf::from(std::env::var("EGUATEX_ZIPS_DESTINO").expect("EGUATEX_ZIPS_DESTINO"));
    for z in fs::read_to_string(lista).unwrap().lines().map(str::trim).filter(|l| !l.is_empty()) {
        let inicio = std::time::Instant::now();
        let r1 = importa(Path::new(z), &pai);
        let r2 = importa(Path::new(z), &pai);
        let principal = r1.as_ref().ok().and_then(|p| principal_do_projeto(p));
        eprintln!("ZIP {z}: {:?} | de novo: {:?} | principal: {:?} | {} ms", r1, r2, principal, inicio.elapsed().as_millis());
    }
}
