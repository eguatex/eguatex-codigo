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
    cmd_importar_zip(zip_caminho.display().to_string(), pai.display().to_string()).map(|r| PathBuf::from(r.caminho))
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

// ---------------------------------------------------------------------------
// Rodadas A e B de seguranca: limites, nomes perigosos e entrada hostil do zip.
// ---------------------------------------------------------------------------

mod seguranca {
    use super::*;
    use crate::limites_zip::{self, Limites, MIB};
    use std::collections::BTreeSet;
    use std::io::{Cursor, Read};

    fn opcoes() -> zip::write::SimpleFileOptions {
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
    }

    /// Zip em memoria cujo diretorio central declara (tamanho descompactado, tamanho compactado)
    /// diferentes do real. Cada item e gravado com 1 byte. Nome com "/" no fim vira pasta.
    fn zip_declarando(itens: &[(&str, u64, u64)]) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (nome, _, _) in itens {
            if nome.ends_with('/') {
                z.add_directory(nome.trim_end_matches('/'), opcoes()).unwrap();
            } else {
                z.start_file(*nome, opcoes()).unwrap();
                z.write_all(b"x").unwrap();
            }
        }
        let mut bytes = z.finish().unwrap().into_inner();
        let assinatura = [0x50u8, 0x4b, 0x01, 0x02];
        let mut inicio = 0;
        for (_, tamanho, compactado) in itens {
            let pos = inicio + bytes[inicio..].windows(4).position(|j| j == assinatura).expect("diretorio central");
            bytes[pos + 20..pos + 24].copy_from_slice(&u32::try_from(*compactado).unwrap().to_le_bytes());
            bytes[pos + 24..pos + 28].copy_from_slice(&u32::try_from(*tamanho).unwrap().to_le_bytes());
            inicio = pos + 4;
        }
        bytes
    }

    fn conferir_bytes(bytes: Vec<u8>, lim: &Limites) -> Result<u64, String> {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        limites_zip::conferir(&mut z, lim)
    }

    /// Zip em disco com entradas de `(nome, bytes reais de zeros)`; o diretorio central declara
    /// `declarado` bytes para todas elas (para o cabecalho mentir).
    fn zip_de_zeros_mentindo(caminho: &Path, itens: &[(&str, u64)], declarado: u32) {
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut z = zip::ZipWriter::new(File::create(caminho).unwrap());
        for (nome, bytes) in itens {
            z.start_file(*nome, o).unwrap();
            io::copy(&mut io::repeat(0).take(*bytes), &mut z).unwrap();
        }
        z.finish().unwrap();
        let mut dados = fs::read(caminho).unwrap();
        let assinatura = [0x50u8, 0x4b, 0x01, 0x02];
        let mut inicio = 0;
        for _ in itens {
            let pos = inicio + dados[inicio..].windows(4).position(|j| j == assinatura).expect("diretorio central");
            dados[pos + 24..pos + 28].copy_from_slice(&declarado.to_le_bytes());
            inicio = pos + 4;
        }
        fs::write(caminho, dados).unwrap();
    }

    fn nada_sobrou(pai: &Path) -> bool {
        !pai.exists() || fs::read_dir(pai).unwrap().next().is_none()
    }

    fn normalizados(nomes: &[String]) -> BTreeSet<String> {
        nomes.iter().map(|n| n.replace('\\', "/")).collect()
    }

    #[test]
    fn exatamente_5000_entradas_passam_e_5001_sao_recusadas() {
        // .tex: um zip sem .tex nao vira projeto e pararia antes do limite testado aqui
        let nomes5000: Vec<String> = (0..5000).map(|i| format!("p/f{i}.tex")).collect();
        fn itens(n: &[String]) -> Vec<(&str, u64, u64)> {
            n.iter().map(|s| (s.as_str(), 1u64, 1u64)).collect()
        }
        assert_eq!(conferir_bytes(zip_declarando(&itens(&nomes5000)), &Limites::PADRAO), Ok(5000));
        let mut nomes5001 = nomes5000.clone();
        nomes5001.push("p/extra.txt".into());
        let erro = conferir_bytes(zip_declarando(&itens(&nomes5001)), &Limites::PADRAO).unwrap_err();
        assert!(erro.contains("5001") && erro.contains("5000"), "{erro}");

        // de ponta a ponta: 5000 arquivos entram, 5001 nao deixam pasta nenhuma
        let t = pasta("entradas-5000");
        grava_zip(&t.join("ok.zip"), &nomes5000.iter().map(|n| (n.as_str(), "")).collect::<Vec<_>>());
        let pasta_ok = importa(&t.join("ok.zip"), &t.join("pai")).unwrap();
        assert_eq!(arquivos_em(&pasta_ok).len(), 5000);
        grava_zip(&t.join("demais.zip"), &nomes5001.iter().map(|n| (n.as_str(), "")).collect::<Vec<_>>());
        let pai2 = t.join("pai2");
        let erro = importa(&t.join("demais.zip"), &pai2).unwrap_err();
        assert!(erro.contains("5000"), "{erro}");
        assert!(nada_sobrou(&pai2), "sobrou pasta de uma importacao recusada");
    }

    #[test]
    fn exatamente_500_mb_descompactados_passam_e_500_mb_mais_1_byte_e_recusado() {
        // 5 arquivos de 100 MB (o teto por arquivo) somam 500 MB. Compactado igual ao real: razao 1:1
        let cinco: Vec<(&str, u64, u64)> = vec![
            ("a", 100 * MIB, 100 * MIB),
            ("b", 100 * MIB, 100 * MIB),
            ("c", 100 * MIB, 100 * MIB),
            ("d", 100 * MIB, 100 * MIB),
            ("e", 100 * MIB, 100 * MIB),
        ];
        assert_eq!(conferir_bytes(zip_declarando(&cinco), &Limites::PADRAO), Ok(500 * MIB));
        let mut seis = cinco.clone();
        seis.push(("f", 1, 1));
        let erro = conferir_bytes(zip_declarando(&seis), &Limites::PADRAO).unwrap_err();
        assert!(erro.contains("500 MB"), "{erro}");
    }

    #[test]
    fn arquivo_de_100_mb_passa_e_100_mb_mais_1_byte_e_recusado() {
        assert_eq!(conferir_bytes(zip_declarando(&[("a", 100 * MIB, 100 * MIB)]), &Limites::PADRAO), Ok(100 * MIB));
        let erro = conferir_bytes(zip_declarando(&[("a.bin", 100 * MIB + 1, 100 * MIB + 1)]), &Limites::PADRAO).unwrap_err();
        assert!(erro.contains("a.bin") && erro.contains("100 MB"), "{erro}");
    }

    #[test]
    fn razao_exatamente_100_para_1_passa_e_acima_e_recusada() {
        // 20480 bytes compactados: 2 048 000 e exatamente 100:1, e passa de 1 MB (a razao so vale dai)
        assert_eq!(conferir_bytes(zip_declarando(&[("a", 2_048_000, 20_480)]), &Limites::PADRAO), Ok(2_048_000));
        let erro = conferir_bytes(zip_declarando(&[("a", 2_048_001, 20_480)]), &Limites::PADRAO).unwrap_err();
        assert!(erro.contains("100:1"), "{erro}");
        // abaixo de 1 MB a razao nao conta: 1000 bytes de 1 byte compactado entram
        assert_eq!(conferir_bytes(zip_declarando(&[("a", 1000, 1)]), &Limites::PADRAO), Ok(1000));
    }

    #[test]
    fn muitos_arquivos_abaixo_de_1_mb_nao_escapam_da_razao_do_total() {
        // cada um tem 1 MB menos 1 byte (a razao por arquivo nao vale), 1 KB compactado;
        // juntos: 4 MB - 4 bytes para 4 KB, razao de ~1000:1
        let itens = [("a", MIB - 1, 1024), ("b", MIB - 1, 1024), ("c", MIB - 1, 1024), ("d", MIB - 1, 1024)];
        let erro = conferir_bytes(zip_declarando(&itens), &Limites::PADRAO).unwrap_err();
        assert!(erro.contains("100:1"), "{erro}");
    }

    #[test]
    fn profundidade_16_passa_e_17_e_recusada() {
        let fundo = |n: usize| format!("{}f.txt", "d/".repeat(n));
        assert!(conferir_bytes(zip_declarando(&[(&fundo(16), 1, 1)]), &Limites::PADRAO).is_ok());
        let erro = conferir_bytes(zip_declarando(&[(&fundo(17), 1, 1)]), &Limites::PADRAO).unwrap_err();
        assert!(erro.contains("16 níveis"), "{erro}");
        // entrada de pasta: a propria pasta conta como um nivel
        let pasta16 = "d/".repeat(16);
        let pasta17 = "d/".repeat(17);
        assert!(conferir_bytes(zip_declarando(&[(&pasta16, 0, 0)]), &Limites::PADRAO).is_ok());
        assert!(conferir_bytes(zip_declarando(&[(&pasta17, 0, 0)]), &Limites::PADRAO).is_err());
        // de ponta a ponta
        let t = pasta("fundo");
        grava_zip(&t.join("a.zip"), &[(fundo(17).as_str(), "x")]);
        let pai = t.join("pai");
        assert!(importa(&t.join("a.zip"), &pai).unwrap_err().contains("16 níveis"));
        assert!(nada_sobrou(&pai));
    }

    #[test]
    fn importa_so_grava_o_que_nao_roda_programa_e_devolve_os_ignorados() {
        let t = pasta("nomes-exe");
        grava_zip(
            &t.join("a.zip"),
            &[
                ("main.tex", "x"),
                ("a.EXE", "x"),
                ("b.exe.", "x"),
                ("c.exe ", "x"),
                ("d.exe:stream", "x"),
                ("e.tar.exe", "x"),
                (".exe", "x"),
                ("exe", "ok"),
                ("f.lnk.txt", "ok"),
            ],
        );
        let r = cmd_importar_zip(t.join("a.zip").display().to_string(), t.join("pai").display().to_string()).unwrap();
        let gravados: BTreeSet<String> = arquivos_em(Path::new(&r.caminho)).into_iter().collect();
        assert_eq!(gravados, BTreeSet::from(["main.tex".to_string(), "exe".to_string(), "f.lnk.txt".to_string()]));
        let esperados: BTreeSet<String> = ["a.EXE", "b.exe.", "c.exe ", "d.exe:stream", "e.tar.exe", ".exe"].map(String::from).into();
        assert_eq!(normalizados(&r.ignorados), esperados);
    }

    #[test]
    fn extensao_bloqueada_nos_casos_da_lista_do_paulo() {
        for ruim in ["a.EXE", "a.exe.", "a.exe ", "a.exe:stream", "a.tar.exe", ".exe", "A.Lnk", "x.URL", "y.Scf"] {
            assert!(limites_zip::extensao_bloqueada(ruim), "deixou passar {ruim:?}");
            assert_eq!(limites_zip::motivo_de_bloqueio(ruim), Some(limites_zip::Motivo::Executavel), "{ruim:?}");
        }
        for bom in ["exe", "lnk", "a.lnk.txt", "a.exe.txt", "exe.tex", "a.exec", "a.js.map", "a.jsx", "a.sh", "a.bat.tex"] {
            assert!(!limites_zip::extensao_bloqueada(bom), "bloqueou {bom:?}");
            assert_eq!(limites_zip::motivo_de_bloqueio(bom), None, "{bom:?}");
        }
        // so o ultimo componente conta para a extensao: uma pasta "x.exe" com um .tex dentro entra
        assert_eq!(limites_zip::motivo_de_bloqueio("x.exe/a.tex"), None);
    }

    #[test]
    fn caminhos_hostis_do_zip_nao_gravam_fora_da_pasta_da_importacao() {
        let t = pasta("hostis");
        grava_zip(
            &t.join("a.zip"),
            &[
                ("main.tex", "x"),
                ("../fora-hostil-1.tex", "x"),
                ("/abs-hostil-2.tex", "x"),
                ("C:\\eguatex-hostil-3.tex", "x"),
                ("\\\\servidor\\compartilhado\\eguatex-hostil-4.tex", "x"),
                ("a/../../fora-hostil-5.tex", "x"),
            ],
        );
        let pai = t.join("pai");
        let r = importa(&t.join("a.zip"), &pai).unwrap();
        let gravados = arquivos_em(&r);
        assert!(gravados.contains(&"main.tex".to_string()), "{gravados:?}");
        // nada fora da pasta do projeto, nem no pai, nem acima, nem na raiz da unidade
        assert_eq!(fs::read_dir(&pai).unwrap().count(), 1, "o pai deve ter so a pasta da importacao");
        for n in ["fora-hostil-1.tex", "abs-hostil-2.tex", "fora-hostil-5.tex"] {
            assert!(!t.join(n).exists() && !pai.join(n).exists() && !Path::new("/").join(n).exists(), "{n} escapou");
        }
        assert!(!Path::new("C:\\eguatex-hostil-3.tex").exists());
        assert!(!Path::new("C:/eguatex-hostil-4.tex").exists());
        // no Windows os caminhos de unidade e UNC sao descartados; so main.tex entra
        if cfg!(windows) {
            assert_eq!(gravados, vec!["main.tex".to_string()]);
        }
    }

    #[test]
    fn nome_com_hifen_inicial_e_ignorado_inclusive_em_pasta() {
        let t = pasta("hifen");
        grava_zip(
            &t.join("a.zip"),
            &[
                ("main.tex", "x"),
                ("--lua-filter=x.tex", "x"),
                ("-x/a.tex", "x"),
                ("cap/-c.tex", "x"),
                ("-", "x"),
                ("cap/a-b.tex", "ok"),
                ("x-/y-.tex", "ok"),
            ],
        );
        let r = cmd_importar_zip(t.join("a.zip").display().to_string(), t.join("pai").display().to_string()).unwrap();
        let gravados: BTreeSet<String> = arquivos_em(Path::new(&r.caminho)).into_iter().collect();
        assert_eq!(gravados, BTreeSet::from(["main.tex".to_string(), "cap/a-b.tex".to_string(), "x-/y-.tex".to_string()]));
        let esperados: BTreeSet<String> = ["--lua-filter=x.tex", "-x/a.tex", "cap/-c.tex", "-"].map(String::from).into();
        assert_eq!(normalizados(&r.ignorados), esperados);
        assert!(!Path::new(&r.caminho).join("-x").exists(), "a pasta com hifen foi criada");
    }

    #[test]
    fn zip_vazio_e_recusado_sem_criar_pasta() {
        let t = pasta("vazio");
        zip::ZipWriter::new(File::create(t.join("v.zip")).unwrap()).finish().unwrap();
        let pai = t.join("pai");
        let erro = importa(&t.join("v.zip"), &pai).unwrap_err();
        assert!(erro.contains("vazio"), "{erro}");
        assert!(nada_sobrou(&pai));
    }

    #[test]
    fn zip_so_com_programas_e_recusado_com_a_lista_do_que_foi_ignorado() {
        let t = pasta("so-programas");
        grava_zip(&t.join("p.zip"), &[("a.exe", "x"), ("b.LNK", "x")]);
        let pai = t.join("pai");
        let erro = importa(&t.join("p.zip"), &pai).unwrap_err();
        assert!(erro.contains("a.exe") && erro.contains("b.LNK"), "{erro}");
        assert!(nada_sobrou(&pai));
    }

    #[test]
    fn zip_corrompido_e_recusado_sem_deixar_pasta() {
        let t = pasta("corrompido");
        let pai = t.join("pai");
        // 1) lixo que nao e zip
        fs::write(t.join("lixo.zip"), b"isto nao e um zip, so texto qualquer que ocupa espaco").unwrap();
        assert!(importa(&t.join("lixo.zip"), &pai).unwrap_err().contains("zip válido"));
        // 2) zip valido cortado ao meio (perde o diretorio central)
        grava_zip(&t.join("inteiro.zip"), &[("a.tex", "um"), ("b.tex", "dois"), ("c.tex", "tres")]);
        let bytes = fs::read(t.join("inteiro.zip")).unwrap();
        fs::write(t.join("cortado.zip"), &bytes[..bytes.len() / 2]).unwrap();
        assert!(importa(&t.join("cortado.zip"), &pai).is_err());
        // 3) arquivo vazio e arquivo que nao existe
        fs::write(t.join("zero.zip"), b"").unwrap();
        assert!(importa(&t.join("zero.zip"), &pai).is_err());
        assert!(importa(&t.join("nao-existe.zip"), &pai).unwrap_err().contains("abrir"));
        assert!(nada_sobrou(&pai));
        // 4) dados de uma entrada estragados (CRC): a extracao falha no meio e a pasta parcial some
        // sem compressao, para o texto aparecer inteiro nos bytes do zip
        let mut z = zip::ZipWriter::new(File::create(t.join("crc.zip")).unwrap());
        for (nome, texto) in [("a.tex", "primeiro arquivo"), ("b.tex", "MARCADOR-DA-CORRUPCAO")] {
            z.start_file(nome, opcoes()).unwrap();
            z.write_all(texto.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        let mut dados = fs::read(t.join("crc.zip")).unwrap();
        let pos = dados.windows(9).position(|j| j == b"MARCADOR-").expect("conteudo no zip");
        dados[pos] ^= 0xff;
        fs::write(t.join("crc.zip"), dados).unwrap();
        let erro = importa(&t.join("crc.zip"), &pai).unwrap_err();
        assert!(erro.contains("extrair"), "{erro}");
        assert!(nada_sobrou(&pai), "sobrou a pasta parcial: {:?}", fs::read_dir(&pai).map(|d| d.count()));
    }

    fn lim_pequeno() -> Limites {
        Limites { total: 3 * MIB, arquivo: 2 * MIB, entradas: 10, razao: 1_000_000, profundidade: 4 }
    }

    #[test]
    fn cabecalho_que_mente_o_tamanho_e_cortado_nos_bytes_reais_e_apaga_a_pasta_parcial() {
        let t = pasta("mentira-parcial");
        let pai = t.join("pai");
        // pasta com o mesmo nome ja existe: tem de continuar intacta depois do erro
        fs::create_dir_all(pai.join("z")).unwrap();
        fs::write(pai.join("z").join("meu.tex"), "MEU").unwrap();
        // a.tex entra; big.bin tem 3 MB reais e jura 10 bytes
        zip_de_zeros_mentindo(&t.join("z.zip"), &[("a.tex", 1000), ("big.bin", 3 * MIB)], 10);
        let erro = importar_zip_com_limites(&t.join("z.zip").display().to_string(), &pai.display().to_string(), &lim_pequeno()).unwrap_err();
        assert!(erro.contains("big.bin") && erro.contains("2 MB"), "{erro}");
        assert_eq!(fs::read_to_string(pai.join("z").join("meu.tex")).unwrap(), "MEU");
        assert!(!pai.join("z (2)").exists(), "a pasta parcial nao foi apagada");
        assert_eq!(fs::read_dir(&pai).unwrap().count(), 1);
    }

    #[test]
    fn cabecalho_que_mente_o_total_para_no_teto_do_total() {
        let t = pasta("mentira-total");
        let pai = t.join("pai");
        // dois arquivos de 2 MB reais (cabem no teto por arquivo), 4 MB juntos contra 3 MB de total
        // .tex: um zip sem .tex nao vira projeto e pararia antes do limite testado aqui
        zip_de_zeros_mentindo(&t.join("z.zip"), &[("a.tex", 2 * MIB), ("b.tex", 2 * MIB)], 10);
        let erro = importar_zip_com_limites(&t.join("z.zip").display().to_string(), &pai.display().to_string(), &lim_pequeno()).unwrap_err();
        assert!(erro.contains("3 MB"), "{erro}");
        assert!(nada_sobrou(&pai));
    }

    #[test]
    fn bytes_reais_no_teto_passam_e_um_byte_a_mais_nao() {
        let t = pasta("mentira-borda");
        let pai = t.join("pai");
        let lim = lim_pequeno();
        // .tex: um zip sem .tex nao vira projeto e pararia antes do limite testado aqui
        zip_de_zeros_mentindo(&t.join("exato.zip"), &[("a.tex", 2 * MIB)], 10);
        let r = importar_zip_com_limites(&t.join("exato.zip").display().to_string(), &pai.display().to_string(), &lim).unwrap();
        assert_eq!(fs::metadata(Path::new(&r.caminho).join("a.tex")).unwrap().len(), 2 * MIB);
        zip_de_zeros_mentindo(&t.join("mais1.zip"), &[("a.tex", 2 * MIB + 1)], 10);
        let pai2 = t.join("pai2");
        let erro = importar_zip_com_limites(&t.join("mais1.zip").display().to_string(), &pai2.display().to_string(), &lim).unwrap_err();
        assert!(erro.contains("2 MB"), "{erro}");
        assert!(nada_sobrou(&pai2));
    }

    #[test]
    fn copiar_limitado_com_destino_que_falha_devolve_erro_e_nao_conta_o_restante() {
        struct Quebrado;
        impl Write for Quebrado {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("disco cheio"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut restante = 100;
        let erro = limites_zip::copiar_limitado(&b"abc"[..], &mut Quebrado, &Limites::PADRAO, &mut restante).unwrap_err();
        assert!(erro.contains("disco cheio"), "{erro}");
        assert_eq!(restante, 100);
    }

    #[test]
    fn validar_nome_recusa_hifen_inicial_nas_criacoes_do_projeto() {
        let t = pasta("criar-hifen");
        let projeto = t.join("proj");
        fs::create_dir_all(&projeto).unwrap();
        let p = projeto.display().to_string();
        for ruim in ["--lua-filter=x.tex", "-x.tex", "-"] {
            let erro = cmd_criar_arquivo(p.clone(), ruim.to_string()).unwrap_err();
            assert!(erro.contains("hífen"), "{ruim}: {erro}");
            assert!(cmd_criar_pasta(p.clone(), ruim.to_string()).is_err(), "{ruim}");
        }
        // em subpasta
        assert!(cmd_criar_arquivo(p.clone(), "cap/-x.tex".to_string()).unwrap_err().contains("hífen"));
        assert!(cmd_criar_arquivo(p.clone(), "-pasta/a.tex".to_string()).unwrap_err().contains("hífen"));
        assert_eq!(fs::read_dir(&projeto).unwrap().count(), 0, "algo foi criado");
        // hifen no meio e permitido
        assert!(cmd_criar_arquivo(p, "cap-1.tex".to_string()).is_ok());
    }
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
