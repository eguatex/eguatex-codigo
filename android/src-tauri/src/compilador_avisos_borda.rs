//! Testes de borda dos avisos de caixa e de rotulo lidos por `extrair_erros`.
//! Os logs das constantes sao trechos copiados sem mudanca de execucoes reais do pdfTeX 2026
//! (TeX Live), com a quebra de linha em 79 colunas que o motor faz. Os testes de integracao
//! compilam os mesmos projetos com o pdflatex do computador.
use super::*;
use std::fs;

/// Projeto com `\include` em subpastas, pasta com espaco e acento e arquivo de nome comprido.
/// Segunda passada: o `.aux` ja existe e o rotulo `a` repetido aparece.
const LOG_INCLUDE: &str = r#"(./main.tex
LaTeX2e <2026-06-01>
L3 programming layer <2026-05-26>
(c:/texlive/2026/texmf-dist/tex/latex/base/article.cls
Document Class: article 2025/01/22 v1.4n Standard LaTeX document class
(c:/texlive/2026/texmf-dist/tex/latex/base/size10.clo
File: size10.clo 2025/01/22 v1.4n Standard LaTeX file (size option)
)
\c@part=\count276
)
(./main.aux (./cap/um.aux

LaTeX Warning: Label `a' multiply defined.

) (./cap/sub/dois.aux))
\openout1 = `main.aux'.

 [1

{c:/texlive/2026/texmf-var/fonts/map/pdftex/updmap/pdftex.map}{c:/texlive/2026/
texmf-dist/fonts/enc/dvips/cm-super/cm-super-t1.enc}]
\openout2 = `cap/um.aux'.

(./cap/um.tex
Overfull \hbox (195.81604pt too wide) in paragraph at lines 3--4
[][]
 []

) [2

]
\openout2 = `cap/sub/dois.aux'.

 (./cap/sub/dois.tex
Overfull \hbox (219.6991pt too wide) in paragraph at lines 3--5
[]\T1/cmr/m/n/10 linha []
 []

) [3



] (./pasta com espaço/capítulo três.tex
Overfull \hbox (195.81604pt too wide) in paragraph at lines 3--4
[][]
 []

)
(./cap/capitulo-com-nome-muito-comprido-que-estoura-a-largura-do-log-do-latex-e
m-79-colunas.tex
Overfull \hbox (195.81604pt too wide) in paragraph at lines 3--4
[][]
 []

) [4


] (./main.aux (./cap/um.aux) (./cap/sub/dois.aux))
"#;

const ARQUIVO_LONGO: &str = "./cap/capitulo-com-nome-muito-comprido-que-estoura-a-largura-do-log-do-latex-em-79-colunas.tex";

/// Linhas de conteudo de caixa que o TeX quebra em 79 colunas: a continuacao ("l) (m)")
/// nao comeca com `[]` nem com `\` e traz parenteses do texto do documento.
const LOG_PARENTESES_NA_CAIXA: &str = r#"(./main.tex
LaTeX2e <2026-06-01>
(c:/texlive/2026/texmf-dist/tex/latex/base/article.cls
Document Class: article 2025/01/22 v1.4n Standard LaTeX document class
(c:/texlive/2026/texmf-dist/tex/latex/base/size10.clo
File: size10.clo 2025/01/22 v1.4n Standard LaTeX file (size option)
)
)
(./cap.tex
Overfull \hbox (55.4065pt too wide) detected at line 1
|\T1/cmr/m/n/10 texto (com parenteses)  texto (outro)
 []


Loose \hbox (badness 21) in paragraph at lines 1--1
\T1/cmr/m/it/10 Quando \T1/cmr/m/n/10 (a) o texto (b) tem (c) muitos (d) parent
eses (e) fechando (f) a (g)
 []


Underfull \hbox (badness 1496) in paragraph at lines 2--2
\T1/cmr/m/n/10 Segundo para-grafo (a) (b) (c) (d) (e) (f) (g) (h) (i) (j) (k) (
l) (m)
 []


Underfull \hbox (badness 1540) in paragraph at lines 2--2
\T1/cmr/m/n/10 (n) (o) (p) (q) (r) (s) (t) (u) (v) (w) (x) (y) (z) fi-nal longo
 longo
 []


Overfull \hbox (204.6991pt too wide) in paragraph at lines 2--2
\T1/cmr/m/n/10 mbox[]
 []

)
Overfull \hbox (40.0pt too wide) detected at line 8
|
 []

[1

{c:/texlive/2026/texmf-var/fonts/map/pdftex/updmap/pdftex.map}{c:/texlive/2026/
texmf-dist/fonts/enc/dvips/cm-super/cm-super-t1.enc}] (./main.aux)
 ***********
 )
"#;

/// Rotulo com nome comprido: o TeX quebra a linha no meio do nome, antes do "multiply defined".
const LOG_ROTULO_LONGO: &str = r#"(./main.tex
LaTeX2e <2026-06-01>
(./main.aux

LaTeX Warning: Label `eq:rotulo-muito-comprido-para-quebrar-a-linha-do-log-do-l
atex' multiply defined.

) [1{c:/texlive/2026/texmf-var/fonts/map/pdftex/updmap/pdftex.map}] (./main.aux
)

LaTeX Warning: There were multiply-defined labels.

 )
"#;

fn avisos(erros: &[ErroCompilacao]) -> Vec<&ErroCompilacao> {
    erros.iter().filter(|e| e.categoria.is_some()).collect()
}

fn so_erros(erros: &[ErroCompilacao]) -> Vec<&ErroCompilacao> {
    erros.iter().filter(|e| e.tipo != "aviso").collect()
}

// ---------- log lido de texto ----------

#[test]
fn include_em_subpastas_aponta_para_o_arquivo_de_cada_aviso() {
    let erros = extrair_erros(LOG_INCLUDE, "main.tex");
    let a = avisos(&erros);
    assert_eq!(a.len(), 5, "{erros:?}");
    // rotulo repetido lido no .aux: vale o documento principal, sem linha
    assert_eq!(a[0].categoria.as_deref(), Some("rotulo-repetido"));
    assert_eq!(a[0].arquivo.as_deref(), Some("./main.tex"));
    assert_eq!(a[0].linha, None);
    assert_eq!((a[1].arquivo.as_deref(), a[1].linha), (Some("./cap/um.tex"), Some(3)));
    assert_eq!((a[2].arquivo.as_deref(), a[2].linha), (Some("./cap/sub/dois.tex"), Some(3)));
    assert!(a[1..].iter().all(|e| e.categoria.as_deref() == Some("caixa-larga") && e.tipo == "aviso"), "{a:?}");
}

#[test]
fn nome_com_espaco_e_acento_aponta_para_o_arquivo_certo() {
    let erros = extrair_erros(LOG_INCLUDE, "main.tex");
    let a = avisos(&erros);
    assert_eq!(a.len(), 5, "{erros:?}");
    assert_eq!(a[3].arquivo.as_deref(), Some("./pasta com espaço/capítulo três.tex"), "{:?}", a[3]);
    assert_eq!(a[3].linha, Some(3), "{:?}", a[3]);
}

#[test]
fn nome_de_arquivo_quebrado_em_79_colunas_aponta_para_o_arquivo_certo() {
    let erros = extrair_erros(LOG_INCLUDE, "main.tex");
    let a = avisos(&erros);
    assert_eq!(a.len(), 5, "{erros:?}");
    assert_eq!(a[4].arquivo.as_deref(), Some(ARQUIVO_LONGO), "{:?}", a[4]);
    assert_eq!(a[4].linha, Some(3), "{:?}", a[4]);
}

#[test]
fn continuacao_de_caixa_com_parenteses_nao_desalinha_os_arquivos() {
    let erros = extrair_erros(LOG_PARENTESES_NA_CAIXA, "main.tex");
    let a = avisos(&erros);
    // Overfull (linha 1), Underfull (2), Underfull (2), Overfull (2) em cap.tex e Overfull (8) em main.tex
    assert_eq!(a.len(), 5, "{erros:?}");
    let achados: Vec<_> = a.iter().map(|e| (e.arquivo.as_deref().unwrap_or(""), e.linha)).collect();
    assert_eq!(
        achados,
        vec![
            ("./cap.tex", Some(1)),
            ("./cap.tex", Some(2)),
            ("./cap.tex", Some(2)),
            ("./cap.tex", Some(2)),
            ("./main.tex", Some(8)),
        ],
        "{erros:?}"
    );
}

#[test]
fn rotulo_com_nome_longo_mantem_a_mensagem_inteira() {
    let erros = extrair_erros(LOG_ROTULO_LONGO, "main.tex");
    let a = avisos(&erros);
    // "There were multiply-defined labels" nao e um segundo aviso
    assert_eq!(a.len(), 1, "{erros:?}");
    assert_eq!(a[0].categoria.as_deref(), Some("rotulo-repetido"));
    assert!(a[0].mensagem.contains("multiply defined"), "mensagem cortada: {:?}", a[0].mensagem);
    assert!(a[0].mensagem.contains("do-log-do-latex"), "nome do rotulo cortado: {:?}", a[0].mensagem);
}

#[test]
fn log_com_crlf_da_o_mesmo_resultado_do_log_com_lf() {
    let lf = extrair_erros(LOG_INCLUDE, "main.tex");
    let crlf = extrair_erros(&LOG_INCLUDE.replace('\n', "\r\n"), "main.tex");
    let resumo = |e: &[ErroCompilacao]| -> Vec<(Option<String>, Option<u32>, String, Option<String>)> {
        e.iter().map(|x| (x.arquivo.clone(), x.linha, x.mensagem.clone(), x.categoria.clone())).collect()
    };
    assert_eq!(resumo(&lf), resumo(&crlf));
    assert!(crlf.iter().all(|e| !e.mensagem.contains('\r')), "{crlf:?}");
}

#[test]
fn log_vazio_ou_so_com_espacos_nao_gera_nada() {
    assert!(extrair_erros("", "main.tex").is_empty());
    assert!(extrair_erros("", "").is_empty());
    assert!(extrair_erros("\n\n   \n\t\n", "main.tex").is_empty());
    assert!(extrair_erros("\r\n\r\n", "main.tex").is_empty());
}

#[test]
fn log_sem_aviso_nem_erro_nao_gera_nada() {
    let log = "(./main.tex\nLaTeX2e <2026-06-01>\n(./main.aux)\nOutput written on main.pdf (1 page, 1234 bytes).\n)\n";
    assert!(extrair_erros(log, "main.tex").is_empty());
}

#[test]
fn aviso_sem_arquivo_e_sem_principal_fica_sem_arquivo_e_sem_linha() {
    let e = extrair_erros("Overfull \\hbox (1.0pt too wide) in paragraph at lines 3--4\n", "");
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!((e[0].arquivo.as_deref(), e[0].linha), (None, None));
    assert_eq!(e[0].categoria.as_deref(), Some("caixa-larga"));
    assert_eq!(e[0].tipo, "aviso");
}

#[test]
fn caixa_vertical_sem_numero_de_linha_vira_aviso_sem_linha() {
    let log = "(./main.tex\nOverfull \\vbox (2.0pt too high) has occurred while \\output is active\nUnderfull \\vbox (badness 10000) has occurred while \\output is active\n)\n";
    let e = extrair_erros(log, "main.tex");
    assert_eq!(e.len(), 2, "{e:?}");
    for aviso in &e {
        assert_eq!(aviso.categoria.as_deref(), Some("caixa-vertical"));
        assert_eq!(aviso.linha, None);
        assert_eq!(aviso.arquivo.as_deref(), Some("./main.tex"));
    }
}

#[test]
fn caixa_frouxa_e_apertada_fora_da_lista_nao_viram_aviso_agrupado() {
    let log = "(./main.tex\nLoose \\hbox (badness 21) in paragraph at lines 1--1\nTight \\hbox (badness 5000) in paragraph at lines 4--4\n)\n";
    assert!(extrair_erros(log, "main.tex").is_empty());
}

#[test]
fn numero_de_linha_gigante_ou_ausente_nao_derruba_a_leitura() {
    assert_eq!(primeira_linha_citada("Overfull \\hbox (1pt too wide) in paragraph at lines 99999999999999999999--1"), None);
    assert_eq!(primeira_linha_citada("Overfull \\hbox (1pt too wide) in paragraph at lines --5"), None);
    assert_eq!(primeira_linha_citada("Overfull \\hbox (1pt too wide) in paragraph at line "), None);
    assert_eq!(primeira_linha_citada(""), None);
    assert_eq!(primeira_linha_citada("Overfull \\hbox (1pt too wide) in paragraph at lines 4294967295--4294967295"), Some(u32::MAX));
    let e = extrair_erros("(./a.tex\nOverfull \\hbox (1pt too wide) in paragraph at lines 99999999999999999999--1\n)\n", "a.tex");
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!((e[0].arquivo.as_deref(), e[0].linha), (Some("./a.tex"), None));
}

#[test]
fn extensao_em_maiuscula_e_aceita_como_tex() {
    let log = "(./CAP.TEX\nOverfull \\hbox (1pt too wide) in paragraph at lines 7--8\n)\n";
    let e = extrair_erros(log, "main.tex");
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!((e[0].arquivo.as_deref(), e[0].linha), (Some("./CAP.TEX"), Some(7)));
}

#[test]
fn erro_e_aviso_do_latex_continuam_com_o_tipo_de_antes() {
    let log = "(./main.tex\n./main.tex:9: LaTeX Warning: Citation `x' on page 1 undefined.\n! Undefined control sequence.\n./main.tex:12: Undefined control sequence.\n)\n";
    let e = extrair_erros(log, "main.tex");
    assert_eq!(e.len(), 3, "{e:?}");
    assert_eq!((e[0].tipo.as_str(), e[0].categoria.as_deref()), ("aviso", None));
    assert_eq!((e[1].tipo.as_str(), e[1].categoria.as_deref()), ("erro", None));
    assert_eq!((e[2].tipo.as_str(), e[2].categoria.as_deref(), e[2].linha), ("erro", None, Some(12)));
    assert_eq!(so_erros(&e).len(), 2);
}

// ---------- compilacao de verdade, com o pdflatex do computador ----------

fn tex_instalado() -> bool {
    detectar_executavel("pdflatex").is_some()
}

/// Pasta fixa no temp do sistema; os arquivos sao sobrescritos e os auxiliares conhecidos
/// apagados no inicio de cada teste, para a passada nao herdar nada de uma rodada anterior.
fn pasta(nome: &str, auxiliares: &[&str]) -> PathBuf {
    let p = std::env::temp_dir().join(format!("eguatex-teste-avisos-{nome}"));
    fs::create_dir_all(&p).unwrap();
    for base in auxiliares {
        for ext in ["aux", "log", "pdf", "synctex.gz"] {
            let _ = fs::remove_file(p.join(format!("{base}.{ext}")));
        }
    }
    p
}

fn gravar(raiz: &Path, relativo: &str, texto: &str) {
    let caminho = raiz.join(relativo);
    fs::create_dir_all(caminho.parent().unwrap()).unwrap();
    fs::write(caminho, texto).unwrap();
}

fn palavra_larga() -> String {
    format!("\\mbox{{palavraqueestoura{}}}", "a".repeat(90))
}

#[test]
fn compilando_projeto_com_include_espaco_acento_e_nome_longo() {
    if !tex_instalado() {
        eprintln!("pdflatex não instalado: teste pulado");
        return;
    }
    let _vez = travar_teste();
    let raiz = pasta("include", &["main", "cap/um", "cap/sub/dois"]);
    let larga = palavra_larga();
    let nome_longo = "capitulo-com-nome-muito-comprido-que-estoura-a-largura-do-log-do-latex-em-79-colunas";
    gravar(&raiz, "main.tex", &format!("\\documentclass{{article}}\n\\usepackage[T1]{{fontenc}}\n\\begin{{document}}\nInicio.\n\n\\include{{cap/um}}\n\\include{{cap/sub/dois}}\n\\input{{\"pasta com espaço/capítulo três\"}}\n\\input{{cap/{nome_longo}}}\nFim.\n\\end{{document}}\n"));
    gravar(&raiz, "cap/um.tex", &format!("Capitulo um.\n\n{larga}\n\n\\section{{A}}\\label{{a}}\n\\section{{B}}\\label{{a}}\n"));
    gravar(&raiz, "cap/sub/dois.tex", &format!("Capitulo dois.\n\nlinha\n{larga}\n\n"));
    gravar(&raiz, "pasta com espaço/capítulo três.tex", &format!("Tres.\n\n{larga}\n\n"));
    gravar(&raiz, &format!("cap/{nome_longo}.tex"), &format!("Longo.\n\n{larga}\n\n"));

    let dir = raiz.display().to_string();
    let _primeira = compilar("main.tex", &dir, "pdflatex", false);
    let r = compilar("main.tex", &dir, "pdflatex", false);
    assert!(r.sucesso, "{}", r.log);
    let a = avisos(&r.erros);
    assert_eq!(a.len(), 5, "{:?}", r.erros);
    let achados: Vec<_> = a.iter().map(|e| (e.categoria.as_deref().unwrap_or(""), e.arquivo.as_deref().unwrap_or(""), e.linha)).collect();
    assert_eq!(
        achados,
        vec![
            ("rotulo-repetido", "./main.tex", None),
            ("caixa-larga", "./cap/um.tex", Some(3)),
            ("caixa-larga", "./cap/sub/dois.tex", Some(3)),
            ("caixa-larga", "./pasta com espaço/capítulo três.tex", Some(3)),
            ("caixa-larga", ARQUIVO_LONGO, Some(3)),
        ],
        "{:?}",
        r.erros
    );
    assert!(so_erros(&r.erros).is_empty(), "{:?}", r.erros);
}

#[test]
fn compilando_caixa_com_parenteses_quebrada_em_79_colunas() {
    if !tex_instalado() {
        eprintln!("pdflatex não instalado: teste pulado");
        return;
    }
    let _vez = travar_teste();
    let raiz = pasta("parenteses", &["main"]);
    gravar(&raiz, "main.tex", "\\documentclass{article}\n\\usepackage[T1]{fontenc}\n\\begin{document}\n\\sloppy\n\\hbadness=0\n\\input{cap}\nFim com Overfull depois.\\par\n\\noindent\\hbox to 10pt{\\vrule width 50pt}\n\\end{document}\n");
    gravar(&raiz, "cap.tex", &format!("\\noindent Segundo paragrafo (a) (b) (c) (d) (e) (f) (g) (h) (i) (j) (k) (l) (m) (n) (o) (p) (q) (r) (s) (t) (u) (v) (w) (x) (y) (z) final longo longo {}\\par\n", palavra_larga()));
    let r = compilar("main.tex", &raiz.display().to_string(), "pdflatex", false);
    let a = avisos(&r.erros);
    assert!(!a.is_empty(), "{}", r.log);
    let da_caixa_do_cap = a.iter().filter(|e| e.arquivo.as_deref() == Some("./cap.tex")).count();
    assert!(da_caixa_do_cap >= 1, "nenhum aviso foi para ./cap.tex: {:?}", r.erros);
    let ultimo = a.last().unwrap();
    assert_eq!((ultimo.arquivo.as_deref(), ultimo.linha), (Some("./main.tex"), Some(8)), "{:?}", r.erros);
}

#[test]
fn compilando_avisos_nao_mudam_a_contagem_de_erros() {
    if !tex_instalado() {
        eprintln!("pdflatex não instalado: teste pulado");
        return;
    }
    let _vez = travar_teste();
    let raiz = pasta("contagem", &["com", "sem"]);
    let corpo = |extra: &str| format!("\\documentclass{{article}}\n\\begin{{document}}\nAntes.\n\n\\comandoquenaoexiste\n\n{extra}\n\nDepois.\n\\end{{document}}\n");
    gravar(&raiz, "sem.tex", &corpo(""));
    gravar(&raiz, "com.tex", &corpo(&palavra_larga()));
    let dir = raiz.display().to_string();
    let sem = compilar("sem.tex", &dir, "pdflatex", false);
    let com = compilar("com.tex", &dir, "pdflatex", false);
    assert!(!so_erros(&sem.erros).is_empty(), "{}", sem.log);
    assert_eq!(so_erros(&com.erros).len(), so_erros(&sem.erros).len(), "{:?} contra {:?}", com.erros, sem.erros);
    assert!(avisos(&sem.erros).is_empty(), "{:?}", sem.erros);
    assert_eq!(avisos(&com.erros).len(), 1, "{:?}", com.erros);
}

#[test]
fn compilando_documento_em_subpasta_os_avisos_ganham_a_subpasta() {
    if !tex_instalado() {
        eprintln!("pdflatex não instalado: teste pulado");
        return;
    }
    let _vez = travar_teste();
    let raiz = pasta("subpasta", &["t/main", "t/cap"]);
    gravar(&raiz, "t/main.tex", &format!("\\documentclass{{article}}\n\\begin{{document}}\nTexto.\n\n{}\n\n\\input{{cap}}\n\\end{{document}}\n", palavra_larga()));
    gravar(&raiz, "t/cap.tex", &format!("Capitulo.\n\n{}\n\n", palavra_larga()));
    let r = compilar("t/main.tex", &raiz.display().to_string(), "pdflatex", false);
    assert!(r.sucesso, "{}", r.log);
    let achados: Vec<_> = avisos(&r.erros).iter().map(|e| (e.arquivo.as_deref().unwrap_or("").to_string(), e.linha)).collect();
    assert_eq!(
        achados,
        vec![("./t/main.tex".to_string(), Some(5)), ("./t/cap.tex".to_string(), Some(3))],
        "{:?}",
        r.erros
    );
}

// ---------- cmd_motor_erros (motor web do Android), com os mesmos logs ----------

use crate::motor_web::cmd_motor_erros;

#[test]
fn motor_erros_acrescenta_a_subpasta_do_principal_aos_avisos() {
    let erros = cmd_motor_erros(LOG_INCLUDE.to_string(), "livro/main.tex".to_string());
    let a = avisos(&erros);
    assert_eq!(a.len(), 5, "{erros:?}");
    assert_eq!(a[0].arquivo.as_deref(), Some("./livro/main.tex"));
    assert_eq!((a[1].arquivo.as_deref(), a[1].linha), (Some("./livro/cap/um.tex"), Some(3)));
    assert_eq!((a[2].arquivo.as_deref(), a[2].linha), (Some("./livro/cap/sub/dois.tex"), Some(3)));
}

#[test]
fn motor_erros_com_principal_na_raiz_ou_com_barra_na_frente() {
    let na_raiz = cmd_motor_erros(LOG_ROTULO_LONGO.to_string(), "main.tex".to_string());
    assert_eq!(avisos(&na_raiz)[0].arquivo.as_deref(), Some("./main.tex"));
    let com_barra = cmd_motor_erros(LOG_ROTULO_LONGO.to_string(), "/x/y/main.tex".to_string());
    assert_eq!(avisos(&com_barra)[0].arquivo.as_deref(), Some("./x/y/main.tex"));
}

#[test]
fn motor_erros_com_log_vazio_ou_principal_vazio_nao_quebra() {
    assert!(cmd_motor_erros(String::new(), "main.tex".to_string()).is_empty());
    assert!(cmd_motor_erros(String::new(), String::new()).is_empty());
    let sem_principal = cmd_motor_erros("Overfull \\hbox (1.0pt too wide) in paragraph at lines 3--4\n".to_string(), String::new());
    assert_eq!(sem_principal.len(), 1, "{sem_principal:?}");
    assert_eq!((sem_principal[0].arquivo.as_deref(), sem_principal[0].linha), (None, None));
}

#[test]
fn motor_erros_nao_muda_a_contagem_de_erros() {
    let log = "(./main.tex\n! Undefined control sequence.\nl.5 \\foo\nOverfull \\hbox (1.0pt too wide) in paragraph at lines 3--4\n./main.tex:9: Undefined control sequence.\n)\n";
    let e = cmd_motor_erros(log.to_string(), "main.tex".to_string());
    assert_eq!(so_erros(&e).len(), 2, "{e:?}");
    assert_eq!(avisos(&e).len(), 1, "{e:?}");
}
