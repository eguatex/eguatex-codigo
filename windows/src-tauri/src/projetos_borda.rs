//! Testes de borda de `ler_bytes_projeto` (visualizador de imagens): caminhos fora do projeto, juncao,
//! nomes com acento e espaco, arquivo no limite de 25 MB e dispositivos do Windows.
use super::*;

/// Pasta de teste. `EGUATEX_TMP_TESTES` aponta para outro disco quando o do sistema esta cheio.
fn pasta(rotulo: &str) -> PathBuf {
    let base = std::env::var_os("EGUATEX_TMP_TESTES").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("eguatex-bytes-borda-{rotulo}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn projeto_e_arquivo_com_acento_e_espaco_abrem() {
    let dir = pasta("acento");
    let projeto = dir.join("Ação & Reação do José");
    fs::create_dir_all(projeto.join("Capítulo 1/figuras finais")).unwrap();
    let bytes = [0x89, b'P', b'N', b'G', 0, 1, 2];
    fs::write(projeto.join("Capítulo 1/figuras finais/Foto Única.PNG"), bytes).unwrap();
    let p = projeto.to_str().unwrap();
    assert_eq!(ler_bytes_projeto(p, "/Capítulo 1/figuras finais/Foto Única.PNG").unwrap(), bytes);
    assert_eq!(ler_bytes_projeto(p, "Capítulo 1/figuras finais/Foto Única.PNG").unwrap(), bytes);
    // o mesmo arquivo com barra invertida e com ./ no meio
    assert_eq!(ler_bytes_projeto(p, r"\Capítulo 1\figuras finais\Foto Única.PNG").unwrap(), bytes);
    assert_eq!(ler_bytes_projeto(p, "/Capítulo 1/./figuras finais/Foto Única.PNG").unwrap(), bytes);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn nome_com_ponto_duplo_dentro_do_nome_abre_e_ponto_ponto_como_pasta_nao() {
    let dir = pasta("ponto");
    fs::write(dir.join("foto..final.png"), [1, 2, 3]).unwrap();
    fs::write(dir.join("..png"), [4]).unwrap();
    let p = dir.to_str().unwrap();
    assert_eq!(ler_bytes_projeto(p, "/foto..final.png").unwrap(), [1, 2, 3]);
    assert_eq!(ler_bytes_projeto(p, "/..png").unwrap(), [4]);
    assert!(ler_bytes_projeto(p, "/../foto..final.png").is_err());
    assert!(ler_bytes_projeto(p, "/./../foto..final.png").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn recusa_barra_invertida_unc_e_prefixo_estendido() {
    let dir = pasta("unc");
    let projeto = dir.join("projeto");
    fs::create_dir_all(&projeto).unwrap();
    fs::write(dir.join("segredo.png"), [1, 2, 3]).unwrap();
    let p = projeto.to_str().unwrap();
    assert!(ler_bytes_projeto(p, r"..\segredo.png").is_err());
    assert!(ler_bytes_projeto(p, r"\..\segredo.png").is_err());
    assert!(ler_bytes_projeto(p, r"fig\..\..\segredo.png").is_err());
    assert!(ler_bytes_projeto(p, r"\\servidor\compartilhamento\segredo.png").is_err());
    assert!(ler_bytes_projeto(p, r"\\?\C:\Windows\win.ini").is_err());
    assert!(ler_bytes_projeto(p, r"C:segredo.png").is_err());
    assert!(ler_bytes_projeto(p, "c:/windows/win.ini").is_err());
    assert!(ler_bytes_projeto(p, "/").is_err());
    assert!(ler_bytes_projeto(p, "//").is_err());
    assert!(ler_bytes_projeto(p, ".").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn caminho_absoluto_de_arquivo_que_existe_fora_do_projeto_e_recusado() {
    let dir = pasta("absoluto");
    let projeto = dir.join("projeto");
    fs::create_dir_all(&projeto).unwrap();
    let fora = dir.join("segredo.png");
    fs::write(&fora, [9, 9, 9]).unwrap();
    let p = projeto.to_str().unwrap();
    for forma in [fora.display().to_string(), fora.display().to_string().replace('\\', "/")] {
        assert!(ler_bytes_projeto(p, &forma).is_err(), "{forma}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(windows)]
fn juncao(link: &Path, alvo: &Path) -> bool {
    std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link.display().to_string().replace('/', "\\"))
        .arg(alvo.display().to_string().replace('/', "\\"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
#[test]
fn juncao_para_fora_do_projeto_e_recusada_e_para_dentro_funciona() {
    let dir = pasta("juncao");
    let projeto = dir.join("projeto");
    let fora = dir.join("fora");
    fs::create_dir_all(projeto.join("real")).unwrap();
    fs::create_dir_all(&fora).unwrap();
    fs::write(fora.join("segredo.png"), [7, 7, 7]).unwrap();
    fs::write(projeto.join("real/ok.png"), [1, 2]).unwrap();
    assert!(juncao(&projeto.join("atalho-fora"), &fora), "mklink /J falhou: o teste nao tem como criar juncao");
    assert!(juncao(&projeto.join("atalho-dentro"), &projeto.join("real")));
    let p = projeto.to_str().unwrap();
    let erro = ler_bytes_projeto(p, "/atalho-fora/segredo.png").unwrap_err();
    assert!(erro.contains("fora do projeto"), "{erro}");
    // a juncao para uma pasta do proprio projeto resolve para dentro: continua valida
    assert_eq!(ler_bytes_projeto(p, "/atalho-dentro/ok.png").unwrap(), [1, 2]);
    // juncao para a pasta fora: nem a pasta inteira abre como arquivo
    assert!(ler_bytes_projeto(p, "/atalho-fora").is_err());
    // desfaz as juncoes antes de apagar, para remove_dir_all nunca seguir o atalho para fora
    let _ = fs::remove_dir(projeto.join("atalho-fora"));
    let _ = fs::remove_dir(projeto.join("atalho-dentro"));
    assert!(fora.join("segredo.png").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn juncao_para_pasta_que_sumiu_da_erro_de_leitura() {
    let dir = pasta("juncao-quebrada");
    let projeto = dir.join("projeto");
    let alvo = dir.join("alvo-temporario");
    fs::create_dir_all(&projeto).unwrap();
    fs::create_dir_all(&alvo).unwrap();
    assert!(juncao(&projeto.join("quebrada"), &alvo));
    fs::remove_dir_all(&alvo).unwrap();
    assert!(ler_bytes_projeto(projeto.to_str().unwrap(), "/quebrada/x.png").is_err());
    let _ = fs::remove_dir(projeto.join("quebrada"));
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn dispositivo_do_windows_nao_e_lido_como_arquivo() {
    let dir = pasta("nul");
    let p = dir.to_str().unwrap();
    // NUL existe em qualquer pasta no Windows; nao pode virar "arquivo vazio lido com sucesso" de um disco
    let r = ler_bytes_projeto(p, "/NUL");
    assert!(r.is_err(), "{r:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn arquivo_inexistente_e_pasta_dao_erro_com_o_caminho() {
    let dir = pasta("inexistente");
    fs::create_dir_all(dir.join("figuras")).unwrap();
    let p = dir.to_str().unwrap();
    let e1 = ler_bytes_projeto(p, "/figuras/nao-existe.png").unwrap_err();
    assert!(e1.contains("nao-existe.png"), "{e1}");
    let e2 = ler_bytes_projeto(p, "/figuras").unwrap_err();
    assert!(e2.contains("não é um arquivo"), "{e2}");
    assert!(ler_bytes_projeto(p, "/figuras/").is_err());
    // projeto que nao existe mais
    assert!(ler_bytes_projeto(dir.join("sumiu").to_str().unwrap(), "/a.png").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn arquivo_vazio_devolve_zero_bytes() {
    let dir = pasta("vazio");
    fs::write(dir.join("vazio.png"), []).unwrap();
    assert_eq!(ler_bytes_projeto(dir.to_str().unwrap(), "/vazio.png").unwrap(), Vec::<u8>::new());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn limite_de_25_mb_exato_le_e_25_mb_mais_1_byte_recusa_com_o_tamanho() {
    let dir = pasta("limite-real");
    let p = dir.to_str().unwrap();
    assert_eq!(LIMITE_BYTES_PROJETO, 26_214_400);
    fs::write(dir.join("exato.png"), vec![3u8; LIMITE_BYTES_PROJETO as usize]).unwrap();
    let lido = ler_bytes_projeto(p, "/exato.png").unwrap();
    assert_eq!(lido.len() as u64, LIMITE_BYTES_PROJETO);
    assert!(lido.iter().all(|&b| b == 3));
    fs::write(dir.join("passou.png"), vec![3u8; LIMITE_BYTES_PROJETO as usize + 1]).unwrap();
    let erro = ler_bytes_projeto(p, "/passou.png").unwrap_err();
    assert_eq!(erro, format!("acima-do-limite:{}", LIMITE_BYTES_PROJETO + 1));
    assert!(erro.starts_with(PREFIXO_ACIMA_DO_LIMITE));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn prefixo_do_erro_de_limite_e_o_que_o_frontend_espera() {
    assert_eq!(PREFIXO_ACIMA_DO_LIMITE, "acima-do-limite:");
}

#[test]
fn gravacoes_simultaneas_do_mesmo_arquivo_nao_dao_erro() {
    let dir = pasta("simultaneas");
    let alvo = dir.join("a.tex");
    let falhas = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut fios = Vec::new();
    for n in 0..2 {
        let alvo = alvo.clone();
        let falhas = falhas.clone();
        fios.push(std::thread::spawn(move || {
            for i in 0..400 {
                let texto = format!("fio {n} volta {i}");
                if gravar_atomico(&alvo, texto.as_bytes()).is_err() {
                    falhas.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        }));
    }
    for f in fios {
        f.join().unwrap();
    }
    assert_eq!(falhas.load(std::sync::atomic::Ordering::SeqCst), 0, "gravacoes simultaneas falharam");
    let _ = fs::remove_dir_all(&dir);
}

fn nomes_na_pasta(dir: &Path) -> Vec<String> {
    let mut nomes: Vec<String> = fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    nomes.sort();
    nomes
}

#[test]
fn gravacoes_simultaneas_de_dois_arquivos_nao_deixam_temporario_e_o_conteudo_fica_inteiro() {
    let dir = pasta("sem-sobras");
    let mut fios = Vec::new();
    for n in 0..4 {
        let alvo = dir.join(if n % 2 == 0 { "a.tex" } else { "b.tex" });
        fios.push(std::thread::spawn(move || {
            for i in 0..100 {
                let texto = format!("fio {n} volta {i} {}", "x".repeat(2000));
                gravar_atomico(&alvo, texto.as_bytes()).expect("gravacao simultanea falhou");
            }
        }));
    }
    for f in fios {
        f.join().unwrap();
    }
    assert_eq!(nomes_na_pasta(&dir), vec!["a.tex".to_string(), "b.tex".to_string()]);
    for nome in ["a.tex", "b.tex"] {
        let texto = fs::read_to_string(dir.join(nome)).unwrap();
        assert!(texto.starts_with("fio ") && texto.ends_with(&"x".repeat(2000)), "conteudo misturado em {nome}");
        assert_eq!(texto.len(), texto.trim_end_matches('x').len() + 2000);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gravacao_que_falha_no_rename_nao_deixa_temporario() {
    let dir = pasta("falha-rename");
    // o destino e uma pasta nao vazia com o nome do arquivo: o rename falha nas 5 tentativas
    fs::create_dir_all(dir.join("alvo.tex")).unwrap();
    fs::write(dir.join("alvo.tex/dentro.txt"), "x").unwrap();
    assert!(gravar_atomico(&dir.join("alvo.tex"), b"novo").is_err());
    assert_eq!(nomes_na_pasta(&dir), vec!["alvo.tex".to_string()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gravacao_com_pasta_pai_inexistente_da_erro_sem_criar_nada() {
    let dir = pasta("sem-pai");
    assert!(gravar_atomico(&dir.join("nao-existe").join("a.tex"), b"x").is_err());
    assert_eq!(nomes_na_pasta(&dir), Vec::<String>::new());
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn gravar_atomico_funciona_com_o_arquivo_aberto_pelo_tex() {
    // o TeX abre o .tex com compartilhamento de leitura e escrita, sem o de apagar: o rename falha
    use std::os::windows::fs::OpenOptionsExt;
    let dir = pasta("aberto-pelo-tex");
    let alvo = dir.join("inria-poster.tex");
    fs::write(&alvo, "velho").unwrap();
    let aberto = fs::OpenOptions::new().read(true).share_mode(0x1 | 0x2).open(&alvo).unwrap();
    gravar_atomico(&alvo, "novo".as_bytes()).unwrap();
    drop(aberto);
    assert_eq!(fs::read_to_string(&alvo).unwrap(), "novo");
    assert_eq!(nomes_na_pasta(&dir), vec!["inria-poster.tex".to_string()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gravar_atomico_substitui_arquivo_existente_e_aceita_nome_com_acento_e_espaco() {
    let dir = pasta("substitui");
    let alvo = dir.join("Capítulo único.tex");
    fs::write(&alvo, "velho").unwrap();
    gravar_atomico(&alvo, "novo ção".as_bytes()).unwrap();
    assert_eq!(fs::read_to_string(&alvo).unwrap(), "novo ção");
    gravar_atomico(&alvo, b"").unwrap();
    assert_eq!(fs::read(&alvo).unwrap(), Vec::<u8>::new());
    assert_eq!(nomes_na_pasta(&dir), vec!["Capítulo único.tex".to_string()]);
    let _ = fs::remove_dir_all(&dir);
}
