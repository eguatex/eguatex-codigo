//! Configuracao de compilacao que vem dentro do projeto, como no Overleaf e no TeXstudio:
//! - `latexmkrc` (ou `.latexmkrc`): `ensure_path('TEXMFHOME', ...)` e `ensure_path('TEXINPUTS', ...)`
//!   dizem onde ficam classes e pacotes do proprio projeto; `$pdflatex = 'lualatex ...'` e
//!   `$pdf_mode = 4/5` escolhem o motor;
//! - comentario magico `% !TeX program = lualatex` no documento principal (TeXstudio);
//! - pastas `texmf`, `-texmf` ou `.texmf` na raiz, com uma subpasta `tex`, mesmo sem latexmkrc.

use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq)]
pub struct ConfigProjeto {
    /// Variaveis de ambiente para o motor, o bibtex, o biber e o makeindex.
    pub variaveis: Vec<(String, String)>,
    /// Motor pedido pelo projeto (pdflatex, xelatex ou lualatex).
    pub motor: Option<&'static str>,
    /// Frases para o comeco do log, explicando o que foi usado.
    pub notas: Vec<String>,
}

const SEPARADOR: &str = if cfg!(windows) { ";" } else { ":" };

fn motor_do_nome(nome: &str) -> Option<&'static str> {
    let nome = nome.trim().to_ascii_lowercase();
    let nome = nome.trim_end_matches(".exe");
    if nome.ends_with("lualatex") {
        Some("lualatex")
    } else if nome.ends_with("xelatex") {
        Some("xelatex")
    } else if nome.ends_with("pdflatex") {
        Some("pdflatex")
    } else {
        None
    }
}

/// Textos entre aspas simples ou duplas, na ordem.
fn entre_aspas(texto: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut chars = texto.chars();
    while let Some(c) = chars.next() {
        if c == '\'' || c == '"' {
            let valor: String = chars.by_ref().take_while(|&x| x != c).collect();
            saida.push(valor);
        }
    }
    saida
}

/// Caminho do latexmkrc relativo ao projeto. "/compile/" e a raiz do projeto no Overleaf.
fn resolver(raiz: &Path, caminho: &str) -> PathBuf {
    let limpo = caminho.trim();
    let sem_overleaf = limpo
        .strip_prefix("/compile/")
        .or_else(|| limpo.strip_prefix("/compile"))
        .unwrap_or(limpo);
    let sem_ponto = sem_overleaf.trim_start_matches("./");
    let p = Path::new(sem_ponto);
    if p.is_absolute() && sem_overleaf.len() == limpo.len() {
        p.to_path_buf()
    } else {
        raiz.join(sem_ponto.trim_start_matches('/'))
    }
}

/// Le o latexmkrc: arvores texmf, pastas de entrada e motor.
fn ler_latexmkrc(raiz: &Path, texto: &str, arvores: &mut Vec<PathBuf>, entradas: &mut Vec<String>) -> Option<&'static str> {
    let mut motor = None;
    let mut modo_pdf = None;
    for linha in texto.lines() {
        let linha = linha.split('#').next().unwrap_or("").trim();
        if linha.is_empty() {
            continue;
        }
        if let Some(pos) = linha.find("ensure_path") {
            let args = entre_aspas(&linha[pos..]);
            if args.len() >= 2 {
                for valor in &args[1..] {
                    // "pasta//" pede busca recursiva; a barra final some ao resolver
                    let recursivo = valor.ends_with("//");
                    let caminho = resolver(raiz, valor.trim_end_matches('/'));
                    match args[0].as_str() {
                        "TEXMFHOME" => arvores.push(caminho),
                        "TEXINPUTS" => entradas.push(format!(
                            "{}{}",
                            caminho.to_string_lossy().replace('\\', "/"),
                            if recursivo { "//" } else { "" }
                        )),
                        _ => {}
                    }
                }
            }
        } else if linha.starts_with("$pdflatex") {
            // o Overleaf troca o motor assim: $pdflatex = 'lualatex %O %S'
            if let Some(cmd) = entre_aspas(linha).first() {
                motor = motor_do_nome(cmd.split_whitespace().next().unwrap_or("")).or(motor);
            }
        } else if let Some(resto) = linha.strip_prefix("$pdf_mode") {
            modo_pdf = resto.trim_start_matches([' ', '=']).trim_end_matches(';').trim().parse::<u32>().ok();
        }
    }
    match modo_pdf {
        Some(4) => Some("lualatex"),
        Some(5) => Some("xelatex"),
        _ => motor,
    }
}

/// Comandos cujas linhas entram na impressao do documento: o que muda a escolha do motor.
const LINHAS_DA_IMPRESSAO: &[&str] = &[
    "\\usepackage", "\\RequirePackage", "\\documentclass", "\\LoadClass", "\\usetheme", "\\setmainfont",
    "\\setsansfont", "\\setmonofont", "\\setmathfont", "\\newfontfamily", "\\directlua", "\\luaexec",
];

/// Impressao do documento para o motor lembrado: hash do preambulo do principal (o texto antes de
/// `\begin{document}`, sem comentarios), das linhas de pacotes, classes e fontes dos arquivos de
/// `\input`, `\include` e `\subfile` (ate 80) e do conteudo das .cls e .sty do proprio projeto.
/// Se ela muda, o motor que funcionou antes pode nao valer mais e a deteccao decide de novo.
pub fn impressao_do_documento(raiz: &Path, principal: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // sem comentarios, sem espacos nas pontas e sem linhas vazias: so o codigo conta
    let sem_comentarios = |texto: &str| -> String {
        texto
            .lines()
            .map(|l| crate::arquivos::sem_comentario(l).trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let principal_texto = std::fs::read(principal).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let codigo = sem_comentarios(&principal_texto);
    codigo.split("\\begin{document}").next().unwrap_or("").hash(&mut h);
    // arquivos incluidos, a partir do principal
    let pasta_doc = principal.parent().unwrap_or(raiz).to_path_buf();
    let mut fila: Vec<(PathBuf, String)> = vec![(principal.to_path_buf(), codigo)];
    let mut vistos: Vec<PathBuf> = vec![principal.to_path_buf()];
    while let Some((arquivo, codigo)) = fila.pop() {
        let base = arquivo.parent().unwrap_or(&pasta_doc).to_path_buf();
        for comando in ["\\input", "\\include", "\\subfile"] {
            for nome in argumentos_de(&codigo, comando) {
                let nome = nome.trim_matches('"');
                for pasta in [&base, &pasta_doc] {
                    let p = pasta.join(nome);
                    let p = if p.extension().is_some() && p.is_file() { p } else { p.with_extension("tex") };
                    if !p.is_file() {
                        continue;
                    }
                    if !vistos.contains(&p) && vistos.len() < MAX_ARQUIVOS_DETECCAO {
                        vistos.push(p.clone());
                        let texto = std::fs::read(&p).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
                        let codigo = sem_comentarios(&texto);
                        for linha in codigo.lines().filter(|l| LINHAS_DA_IMPRESSAO.iter().any(|c| l.contains(c))) {
                            linha.trim().hash(&mut h);
                        }
                        fila.push((p, codigo));
                    }
                    break;
                }
            }
        }
    }
    // classes e pacotes do proprio projeto, em ordem de nome
    let mut locais: Vec<(String, PathBuf)> = indice_do_projeto(raiz).into_iter().collect();
    locais.sort();
    for (nome, caminho) in locais {
        nome.hash(&mut h);
        std::fs::read(&caminho).unwrap_or_default().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// `% !TeX program = lualatex` (ou `TS-program`) nas primeiras linhas do documento.
fn comentario_magico(texto: &str) -> Option<&'static str> {
    for linha in texto.lines().take(30) {
        let l = linha.trim_start();
        if !l.starts_with('%') {
            continue;
        }
        let minusculo = l.to_ascii_lowercase();
        let Some(pos) = minusculo.find("program") else { continue };
        if !minusculo.contains("!tex") {
            continue;
        }
        let depois = &l[pos + "program".len()..];
        if let Some(valor) = depois.split('=').nth(1) {
            return motor_do_nome(valor.split_whitespace().next().unwrap_or(""));
        }
    }
    None
}

/// Pacotes que so rodam no LuaLaTeX.
const PACOTES_LUA: &[&str] = &[
    "luacode", "luatexbase", "luaotfload", "lua-ul", "luacolor", "luatextra", "luamplib", "lua-visual-debug",
    "luapackageloader", "luaimageembed", "pyluatex", "luatexja", "luatexja-fontspec", "luatexja-preset",
];
/// Comandos que so existem no LuaTeX.
const COMANDOS_LUA: &[&str] = &["\\directlua", "\\luaexec", "\\luadirect", "\\latelua"];
/// Pacotes que pedem XeLaTeX ou LuaLaTeX (fontes do sistema, Unicode).
const PACOTES_UNICODE: &[&str] = &[
    "fontspec", "unicode-math", "polyglossia", "xecjk", "mathspec", "xltxtra", "xunicode", "realscripts",
    "xgreek", "bidi", "xepersian", "xetexko",
];
/// Comandos do fontspec e do unicode-math.
const COMANDOS_UNICODE: &[&str] = &[
    "\\setmainfont", "\\setsansfont", "\\setmonofont", "\\newfontfamily", "\\setmathfont", "\\setmathrm",
    "\\defaultfontfeatures",
];
/// Arquivos lidos no maximo por deteccao (principal, \input, .cls e .sty do projeto).
const MAX_ARQUIVOS_DETECCAO: usize = 80;

/// Motor escolhido sozinho e o motivo, para o log e a barra de status.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MotorAutomatico {
    pub motor: &'static str,
    pub motivo: String,
}

/// Argumentos entre chaves logo apos `comando` (ex.: `\usepackage[x]{a,b}` da `["a", "b"]`).
fn argumentos_de(codigo: &str, comando: &str) -> Vec<String> {
    let mut saida = Vec::new();
    for (i, _) in codigo.match_indices(comando) {
        let resto = &codigo[i + comando.len()..];
        if resto.chars().next().is_some_and(|c| c.is_alphabetic()) {
            continue;
        }
        let mut resto = resto.trim_start();
        // opcoes entre colchetes, antes ou depois do argumento (\setmainfont{X}[...] nao importa aqui)
        while let Some(r) = resto.strip_prefix('[') {
            resto = r.find(']').map(|f| r[f + 1..].trim_start()).unwrap_or("");
        }
        if let Some(r) = resto.strip_prefix('{')
            && let Some(f) = r.find('}')
        {
            saida.extend(r[..f].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        }
    }
    saida
}

/// Testes de motor: o que vem dentro deles (ate o `\fi` que fecha) vale so para um dos motores.
const TESTES_DE_MOTOR: &[&str] = &[
    "\\ifluatex", "\\ifxetex", "\\ifpdftex", "\\ifPDFTeX", "\\ifXeTeX", "\\ifLuaTeX", "\\ifpdf",
    "\\ifdefined\\directlua", "\\ifx\\directlua", "\\ifdefined\\XeTeXrevision", "\\ifx\\XeTeXrevision",
    "\\ifx\\pdftexversion", "\\ifdefined\\pdftexversion", "\\ifdefined\\luatexversion", "\\ifx\\luatexversion",
];

/// Tira do codigo cada bloco de teste de motor, do `\ifxetex` ate o `\fi` que o fecha (contando os
/// `\if...` de dentro). Um bloco sem `\fi` vai ate o fim.
fn sem_blocos_de_motor(codigo: &str) -> String {
    let mut saida = String::with_capacity(codigo.len());
    let mut resto = codigo;
    loop {
        let proximo = TESTES_DE_MOTOR
            .iter()
            .filter_map(|t| {
                resto.match_indices(t).find(|(i, a)| !resto[i + a.len()..].chars().next().is_some_and(|c| c.is_alphabetic()))
            })
            .min_by_key(|(i, _)| *i);
        let Some((ini, achado)) = proximo else {
            saida.push_str(resto);
            return saida;
        };
        saida.push_str(&resto[..ini]);
        let mut pos = ini + achado.len();
        let mut fundura = 1;
        while fundura > 0 {
            let Some(barra) = resto[pos..].find('\\') else {
                return saida;
            };
            let ini_nome = pos + barra + 1;
            let fim_nome = resto[ini_nome..]
                .char_indices()
                .find(|(_, c)| !c.is_ascii_alphabetic())
                .map(|(i, _)| ini_nome + i)
                .unwrap_or(resto.len());
            let nome = &resto[ini_nome..fim_nome];
            // \newif\ifalgo declara um condicional, nao abre um
            let declarado = resto[..pos + barra].ends_with("\\newif");
            if nome == "fi" {
                fundura -= 1;
            } else if nome.starts_with("if") && nome != "ifthenelse" && !declarado {
                fundura += 1;
            }
            pos = fim_nome.max(ini_nome);
        }
        resto = &resto[pos..];
    }
}

/// O comando aparece no codigo sem letra logo depois.
fn usa_comando(codigo: &str, comando: &str) -> bool {
    codigo
        .match_indices(comando)
        .any(|(i, c)| !codigo[i + c.len()..].chars().next().is_some_and(|x| x.is_alphabetic()))
}

/// Arquivos `.tex`, `.sty` e `.cls` do projeto pelo nome em minusculas (ate 6 niveis, sem pastas
/// ocultas), para achar o `.sty` de um `\usepackage` ou o `.cls` de um `\documentclass` locais.
fn indice_do_projeto(raiz: &Path) -> std::collections::HashMap<String, PathBuf> {
    fn juntar(pasta: &Path, nivel: u32, saida: &mut std::collections::HashMap<String, PathBuf>) {
        let Ok(entradas) = std::fs::read_dir(pasta) else { return };
        for e in entradas.flatten() {
            if saida.len() > 4000 {
                return;
            }
            let nome = e.file_name().to_string_lossy().to_lowercase();
            let Ok(tipo) = e.file_type() else { continue };
            if tipo.is_dir() {
                if nivel < 6 && !nome.starts_with('.') {
                    juntar(&e.path(), nivel + 1, saida);
                }
            } else if nome.ends_with(".sty") || nome.ends_with(".cls") {
                saida.entry(nome).or_insert_with(|| e.path());
            }
        }
    }
    let mut saida = std::collections::HashMap::new();
    juntar(raiz, 0, &mut saida);
    saida
}

/// Escolhe o motor pelo que o documento usa, lendo o principal, os arquivos de `\input`,
/// `\include` e `\subfile` e as classes e pacotes do proprio projeto. Lua (luacode,
/// \directlua...) da LuaLaTeX; fontspec, unicode-math, polyglossia ou \setmainfont dao XeLaTeX
/// (ou LuaLaTeX se tambem houver Lua). `None` quando nada pede outro motor (fica o pdfLaTeX).
pub fn motor_pelos_pacotes(raiz: &Path, principal: &Path) -> Option<MotorAutomatico> {
    let indice = indice_do_projeto(raiz);
    let pasta_doc = principal.parent().unwrap_or(raiz).to_path_buf();
    let mut fila: Vec<PathBuf> = vec![principal.to_path_buf()];
    let mut vistos: Vec<PathBuf> = Vec::new();
    let mut lua: Option<String> = None;
    let mut unicode: Option<String> = None;
    let nome_de = |p: &Path, raiz: &Path| {
        p.strip_prefix(raiz).unwrap_or(p).to_string_lossy().replace('\\', "/")
    };
    while let Some(arquivo) = fila.pop() {
        if vistos.contains(&arquivo) || vistos.len() >= MAX_ARQUIVOS_DETECCAO {
            continue;
        }
        vistos.push(arquivo.clone());
        let Ok(bytes) = std::fs::read(&arquivo) else { continue };
        let texto = String::from_utf8_lossy(&bytes);
        let codigo: String = texto
            .lines()
            .map(crate::arquivos::sem_comentario)
            .collect::<Vec<_>>()
            .join("\n");
        let onde = nome_de(&arquivo, raiz);
        // o que fica dentro de um teste de motor (\ifxetex ... \fi) nao decide: o arquivo funciona em mais de um
        let decisivo = sem_blocos_de_motor(&codigo);
        let pacotes_decisivos: Vec<String> = ["\\usepackage", "\\RequirePackage", "\\RequirePackageWithOptions"]
            .iter()
            .flat_map(|c| argumentos_de(&decisivo, c))
            .collect();
        let mut pacotes = argumentos_de(&codigo, "\\usepackage");
        pacotes.extend(argumentos_de(&codigo, "\\RequirePackage"));
        pacotes.extend(argumentos_de(&codigo, "\\RequirePackageWithOptions"));
        let temas = argumentos_de(&codigo, "\\usetheme");
        let classes: Vec<String> = argumentos_de(&codigo, "\\documentclass")
            .into_iter()
            .chain(argumentos_de(&codigo, "\\LoadClass"))
            .chain(argumentos_de(&codigo, "\\LoadClassWithOptions"))
            .collect();
        if lua.is_none() {
            if let Some(p) = pacotes_decisivos.iter().find(|p| PACOTES_LUA.iter().any(|l| l.eq_ignore_ascii_case(p))) {
                lua = Some(format!("o pacote {p} (em {onde}) só roda no LuaLaTeX"));
            } else if let Some(c) = COMANDOS_LUA.iter().find(|c| usa_comando(&decisivo, c)) {
                lua = Some(format!("o comando {c} (em {onde}) só existe no LuaLaTeX"));
            }
        }
        if unicode.is_none() {
            if let Some(p) = pacotes_decisivos.iter().find(|p| PACOTES_UNICODE.iter().any(|u| u.eq_ignore_ascii_case(p))) {
                unicode = Some(format!("o pacote {p} (em {onde}) precisa de XeLaTeX ou LuaLaTeX"));
            } else if let Some(c) = COMANDOS_UNICODE.iter().find(|c| usa_comando(&decisivo, c)) {
                unicode = Some(format!("o comando {c} (em {onde}) precisa de XeLaTeX ou LuaLaTeX"));
            }
        }
        if lua.is_some() {
            break;
        }
        // arquivos locais que o documento carrega
        let mut locais: Vec<PathBuf> = Vec::new();
        for p in &pacotes {
            locais.extend(indice.get(&format!("{}.sty", p.to_lowercase())).cloned());
        }
        for t in &temas {
            locais.extend(indice.get(&format!("beamertheme{}.sty", t.to_lowercase())).cloned());
        }
        for c in &classes {
            locais.extend(indice.get(&format!("{}.cls", c.to_lowercase())).cloned());
        }
        let base = arquivo.parent().unwrap_or(&pasta_doc).to_path_buf();
        for comando in ["\\input", "\\include", "\\subfile", "\\includeonly"] {
            for nome in argumentos_de(&codigo, comando) {
                let nome = nome.trim_matches('"');
                for pasta in [&base, &pasta_doc] {
                    let p = pasta.join(nome);
                    let com_tex = if p.extension().is_some() { p.clone() } else { p.with_extension("tex") };
                    if com_tex.is_file() {
                        locais.push(com_tex);
                        break;
                    } else if p.is_file() {
                        locais.push(p);
                        break;
                    }
                }
            }
        }
        // o primeiro da fila e lido primeiro (ordem do documento)
        for l in locais.into_iter().rev() {
            if !vistos.contains(&l) {
                fila.push(l);
            }
        }
    }
    match (lua, unicode) {
        (Some(motivo), _) => Some(MotorAutomatico { motor: "lualatex", motivo }),
        (None, Some(motivo)) => Some(MotorAutomatico { motor: "xelatex", motivo }),
        (None, None) => None,
    }
}

/// Motor da compilacao automatica, nesta ordem: comentario `% !TeX program` no principal,
/// latexmkrc do projeto, pacotes e comandos do documento (com os `\input` e as classes e
/// pacotes locais) e, sem nada disso, pdfLaTeX.
pub fn detectar_motor(raiz: &Path, principal: &Path) -> MotorAutomatico {
    let cfg = ler(raiz, principal);
    if let Some(m) = cfg.motor {
        let motivo = cfg
            .notas
            .iter()
            .find(|n| n.starts_with("Motor "))
            .map(|n| n.trim_start_matches(&format!("Motor {m}, ")).trim_end_matches('.').to_string())
            .unwrap_or_default();
        return MotorAutomatico { motor: m, motivo };
    }
    motor_pelos_pacotes(raiz, principal)
        .unwrap_or(MotorAutomatico { motor: "pdflatex", motivo: "nenhum pacote pede outro motor".to_string() })
}

/// Motor certo para repetir uma compilacao que falhou por motor errado, lido no log:
/// fontspec ou unicode-math no pdfLaTeX dao XeLaTeX; `\directlua` indefinido ou pacote que
/// exige LuaTeX dao LuaLaTeX. `None` se o log nao mostra erro de motor.
pub fn motor_pelo_log(log: &str, motor_atual: &str) -> Option<&'static str> {
    // o TeX quebra as linhas do log em 79 colunas ("Fatal Package f" + "ontspec Error")
    let log = &log.replace(['\r', '\n'], "");
    let pede_lua = log.contains("Undefined control sequence") && log.contains("\\directlua")
        || log.contains("requires LuaTeX")
        || log.contains("only works with LuaTeX")
        || log.contains("LuaTeX is required");
    if pede_lua && motor_atual != "lualatex" {
        return Some("lualatex");
    }
    let pede_unicode = log.contains("requires either XeTeX or")
        || log.contains("cannot-use-pdftex")
        || log.contains("requires XeTeX or LuaTeX")
        || log.contains("Fatal Package fontspec Error")
        || log.contains("unicode-math requires")
        || log.contains("Package polyglossia Error: The current engine");
    if pede_unicode && motor_atual == "pdflatex" {
        return Some("xelatex");
    }
    // mathspec e outros pacotes so do XeTeX (\RequireXeTeX) param o pdfLaTeX e o LuaLaTeX
    if log.contains("XeTeX is required") && motor_atual != "xelatex" {
        return Some("xelatex");
    }
    None
}

/// Monta a configuracao do projeto em `raiz` para o documento principal `principal`.
pub fn ler(raiz: &Path, principal: &Path) -> ConfigProjeto {
    let mut cfg = ConfigProjeto::default();
    let mut arvores: Vec<PathBuf> = Vec::new();
    let mut entradas: Vec<String> = Vec::new();

    let pasta_doc = principal.parent().unwrap_or(raiz);
    let mut lidos = Vec::new();
    for pasta in [raiz, pasta_doc] {
        for nome in ["latexmkrc", ".latexmkrc"] {
            let caminho = pasta.join(nome);
            if lidos.contains(&caminho) {
                continue;
            }
            if let Ok(texto) = std::fs::read_to_string(&caminho) {
                if let Some(m) = ler_latexmkrc(raiz, &texto, &mut arvores, &mut entradas) {
                    cfg.motor = Some(m);
                    cfg.notas.push(format!("Motor {m}, pedido pelo {nome} do projeto."));
                }
                lidos.push(caminho);
            }
        }
    }

    if let Ok(texto) = std::fs::read_to_string(principal)
        && let Some(m) = comentario_magico(&texto) {
            cfg.motor = Some(m);
            cfg.notas.retain(|n| !n.starts_with("Motor "));
            cfg.notas.push(format!("Motor {m}, pedido pelo comentário % !TeX program do documento."));
        }

    for nome in ["texmf", "-texmf", ".texmf"] {
        let p = raiz.join(nome);
        if p.join("tex").is_dir() && !arvores.contains(&p) {
            arvores.push(p);
        }
    }
    arvores.retain(|a| a.is_dir());

    let barra = |p: &Path| p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string();
    let mut tex = entradas;
    let mut bib = Vec::new();
    let mut bst = Vec::new();
    for a in &arvores {
        tex.push(format!("{}/tex//", barra(a)));
        bib.push(format!("{}/bibtex/bib//", barra(a)));
        bst.push(format!("{}/bibtex/bst//", barra(a)));
        cfg.notas.push(format!(
            "Classes e pacotes do projeto procurados também em {}.",
            a.strip_prefix(raiz).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| barra(a))
        ));
    }
    if let Some(a) = arvores.first() {
        cfg.variaveis.push(("TEXMFHOME".into(), barra(a)));
    }
    for (nome, lista) in [("TEXINPUTS", tex), ("BIBINPUTS", bib), ("BSTINPUTS", bst)] {
        if lista.is_empty() {
            continue;
        }
        // o separador no fim mantem as pastas padrao do TeX na busca
        let atual = std::env::var(nome).unwrap_or_default();
        let mut valor = format!(".{SEPARADOR}{}{SEPARADOR}", lista.join(SEPARADOR));
        if !atual.is_empty() {
            valor.push_str(atual.trim_start_matches(SEPARADOR));
            if !valor.ends_with(SEPARADOR) {
                valor.push_str(SEPARADOR);
            }
        }
        cfg.variaveis.push((nome.into(), valor));
    }
    cfg
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;

    struct PastaTemp(PathBuf);
    impl PastaTemp {
        fn nova(nome: &str) -> Self {
            let p = std::env::temp_dir().join(format!("eguatex-cfg-{nome}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            PastaTemp(p)
        }
    }
    impl Drop for PastaTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn latexmkrc_do_overleaf_da_arvore_e_motor() {
        let p = PastaTemp::nova("overleaf");
        fs::create_dir_all(p.0.join("-texmf/tex/latex/x")).unwrap();
        fs::write(
            p.0.join("latexmkrc"),
            "# comentario\nensure_path( 'TEXMFHOME', '/compile/-texmf/' );\n$pdf_mode = 1;\n\
             $pdflatex = 'lualatex --synctex=1 %O %S';\n",
        )
        .unwrap();
        fs::write(p.0.join("main.tex"), "\\documentclass{x}\n").unwrap();
        let cfg = ler(&p.0, &p.0.join("main.tex"));
        assert_eq!(cfg.motor, Some("lualatex"));
        let tex = &cfg.variaveis.iter().find(|(k, _)| k == "TEXINPUTS").unwrap().1;
        assert!(tex.contains("-texmf/tex//"), "{tex}");
        assert!(tex.ends_with(SEPARADOR), "{tex}");
        assert!(cfg.variaveis.iter().any(|(k, _)| k == "TEXMFHOME"));
    }

    #[test]
    fn comentario_magico_vence_e_pasta_texmf_vale_sem_latexmkrc() {
        let p = PastaTemp::nova("magico");
        fs::create_dir_all(p.0.join("texmf/tex")).unwrap();
        fs::write(p.0.join("main.tex"), "% !TeX program = XeLaTeX\n\\documentclass{article}\n").unwrap();
        let cfg = ler(&p.0, &p.0.join("main.tex"));
        assert_eq!(cfg.motor, Some("xelatex"));
        assert!(cfg.variaveis.iter().any(|(k, v)| k == "TEXINPUTS" && v.contains("texmf/tex//")));
    }

    #[test]
    fn projeto_comum_nao_muda_nada() {
        let p = PastaTemp::nova("comum");
        fs::write(p.0.join("main.tex"), "\\documentclass{article}\n").unwrap();
        assert_eq!(ler(&p.0, &p.0.join("main.tex")), ConfigProjeto::default());
    }

    fn motor_de(p: &PastaTemp, principal: &str) -> &'static str {
        detectar_motor(&p.0, &p.0.join(principal)).motor
    }

    /// A impressao muda com o preambulo, com os pacotes de um \input e com uma .sty local; o texto
    /// do documento e os comentarios nao mudam a impressao.
    #[test]
    fn impressao_do_documento_so_muda_com_preambulo_pacotes_e_estilos() {
        let p = PastaTemp::nova("impressao");
        let main = p.0.join("main.tex");
        fs::create_dir_all(p.0.join("cap")).unwrap();
        fs::write(&main, "\\documentclass{article}\n\\input{cap/pre}\n\\begin{document}\nOi\n\\end{document}\n").unwrap();
        fs::write(p.0.join("cap/pre.tex"), "\\usepackage{amsmath}\n").unwrap();
        fs::write(p.0.join("meu.sty"), "\\ProvidesPackage{meu}\n").unwrap();
        let base = impressao_do_documento(&p.0, &main);
        assert_eq!(base.len(), 16);
        fs::write(&main, "\\documentclass{article}\n\\input{cap/pre}\n% \\usepackage{fontspec}\n\\begin{document}\nOutro texto\n\\end{document}\n").unwrap();
        assert_eq!(impressao_do_documento(&p.0, &main), base);
        fs::write(&main, "\\documentclass{article}\n\\input{cap/pre}\n\\usepackage{fontspec}\n\\begin{document}\nOi\n\\end{document}\n").unwrap();
        let com_fontspec = impressao_do_documento(&p.0, &main);
        assert_ne!(com_fontspec, base);
        fs::write(p.0.join("cap/pre.tex"), "\\usepackage{amsmath}\n\\usepackage{luacode}\n").unwrap();
        let com_lua = impressao_do_documento(&p.0, &main);
        assert_ne!(com_lua, com_fontspec);
        fs::write(p.0.join("meu.sty"), "\\ProvidesPackage{meu}\n\\RequirePackage{fontspec}\n").unwrap();
        assert_ne!(impressao_do_documento(&p.0, &main), com_lua);
    }

    #[test]
    fn motor_automatico_pelos_pacotes() {
        let p = PastaTemp::nova("auto-pacotes");
        // fontspec e \setmainfont: XeLaTeX
        fs::write(p.0.join("fs.tex"), "\\documentclass{article}\n\\usepackage{fontspec}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "fs.tex"), "xelatex");
        fs::write(p.0.join("sm.tex"), "\\documentclass{article}\n\\setmainfont{Arial}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "sm.tex"), "xelatex");
        // luacode: LuaLaTeX, mesmo com fontspec antes
        fs::write(p.0.join("lua.tex"), "\\documentclass{article}\n\\usepackage{fontspec}\n\\usepackage{luacode}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "lua.tex"), "lualatex");
        fs::write(p.0.join("dl.tex"), "\\documentclass{article}\n\\begin{document}\\directlua{tex.print(1)}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "dl.tex"), "lualatex");
        // xeCJK (com maiusculas) e polyglossia: XeLaTeX; luatextra: LuaLaTeX
        fs::write(p.0.join("cjk.tex"), "\\documentclass{article}\n\\usepackage{xeCJK}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "cjk.tex"), "xelatex");
        fs::write(p.0.join("pg.tex"), "\\documentclass{article}\n\\usepackage{polyglossia}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "pg.tex"), "xelatex");
        fs::write(p.0.join("ltx.tex"), "\\documentclass{article}\n\\usepackage{luatextra}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "ltx.tex"), "lualatex");
        // comentado nao conta; documento comum fica no pdfLaTeX
        fs::write(p.0.join("com.tex"), "\\documentclass{article}\n%\\usepackage{fontspec}\n\\usepackage[T1]{fontenc}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "com.tex"), "pdflatex");
        // fontspec dentro de um \input
        fs::create_dir_all(p.0.join("cap")).unwrap();
        fs::write(p.0.join("cap/pre.tex"), "\\usepackage{unicode-math}\n").unwrap();
        fs::write(p.0.join("inp.tex"), "\\documentclass{article}\n\\input{cap/pre}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "inp.tex"), "xelatex");
    }

    #[test]
    fn motor_automatico_pela_classe_e_pelo_tema_locais() {
        let p = PastaTemp::nova("auto-classe");
        fs::write(p.0.join("minha.cls"), "\\LoadClass{article}\n\\RequirePackage{fontspec}\n").unwrap();
        fs::write(p.0.join("doc.tex"), "\\documentclass[a4]{minha}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "doc.tex"), "xelatex");
        // classe que testa o motor funciona em mais de um: nao decide
        fs::write(p.0.join("flex.cls"), "\\LoadClass{article}\n\\ifxetex\\RequirePackage{fontspec}\\fi\n").unwrap();
        fs::write(p.0.join("flex.tex"), "\\documentclass{flex}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "flex.tex"), "pdflatex");
        // caso real do ACM TechBrief e do MUL_poster: recusa o pdfTeX num teste e carrega o fontspec depois dele
        fs::write(
            p.0.join("techbrief.sty"),
            "\\RequirePackage{iftex}\n\\ifPDFTeX\n  \\PackageError{techbrief}{Please compile with LuaLaTeX or XeLaTeX}\n    {fontspec.}\n\\fi\n\n\\RequirePackage{fontspec}\n",
        )
        .unwrap();
        fs::write(p.0.join("acm.tex"), "\\documentclass[acmlarge]{acmart}\n\\usepackage{techbrief}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "acm.tex"), "xelatex");
        // \newif nao abre bloco; o \fi de dentro fecha so o teste interno
        assert_eq!(sem_blocos_de_motor("a\\ifxetex b\\ifnum1=1 c\\fi d\\fi e"), "a e");
        assert_eq!(sem_blocos_de_motor("\\newif\\ifcor x\\ifluatex\\newif\\ifpdfa y\\fi z"), "\\newif\\ifcor x z");
        // tema beamer local com luacode
        fs::create_dir_all(p.0.join("tema")).unwrap();
        fs::write(p.0.join("tema/beamerthemeX.sty"), "\\RequirePackage{luacode}\n").unwrap();
        fs::write(p.0.join("b.tex"), "\\documentclass{beamer}\n\\usetheme{X}\n\\begin{document}\\end{document}\n").unwrap();
        assert_eq!(motor_de(&p, "b.tex"), "lualatex");
    }

    /// Casos reais: o Inria usa mathspec e \setmainfont (README pede XeLaTeX); o TU Berlin usa
    /// baposter, arev e fontenc pelo \input do preambulo (pdfLaTeX).
    #[test]
    fn motor_automatico_inria_e_tu_berlin() {
        let p = PastaTemp::nova("auto-reais");
        fs::write(
            p.0.join("inria-poster.tex"),
            "\\documentclass[final,svgnames,english]{beamer}\n\\usepackage[english]{babel}\n\\usetheme{InriaPoster}\n\
             \\usepackage{mathspec}\n\\setmainfont{Inria Sans}[Scale=MatchLowercase]\n% Load transparent only with pdfTeX\n\
             \\ifx\\pdftexversion\\undefined\\else\n\\fi\n\\usepackage{fontawesome7}\n\\begin{document}\\end{document}\n",
        )
        .unwrap();
        fs::write(p.0.join("beamerthemeInriaPoster.sty"), "\\RequirePackage[most]{tcolorbox}\n\\RequirePackage{svg}\n").unwrap();
        assert_eq!(motor_de(&p, "inria-poster.tex"), "xelatex");

        let tu = PastaTemp::nova("auto-tu");
        fs::write(tu.0.join("00-preambel.tex"), "\\documentclass[a1paper,portrait]{baposter}\n\\usepackage{arev}\n\\usepackage[T1]{fontenc}\n").unwrap();
        fs::write(tu.0.join("baposter.cls"), "\\LoadClass{article}\n\\RequirePackage{tikz}\n").unwrap();
        fs::write(tu.0.join("poster.tex"), "\\input{00-preambel}\n\\begin{document}\n\\end{document}\n").unwrap();
        let m = detectar_motor(&tu.0, &tu.0.join("poster.tex"));
        assert_eq!(m.motor, "pdflatex", "{}", m.motivo);
    }

    #[test]
    fn motor_automatico_comentario_magico_e_latexmkrc_vencem_os_pacotes() {
        let p = PastaTemp::nova("auto-ordem");
        fs::write(p.0.join("a.tex"), "% !TeX program = lualatex\n\\documentclass{article}\n\\usepackage{fontspec}\n\\begin{document}\\end{document}\n").unwrap();
        let m = detectar_motor(&p.0, &p.0.join("a.tex"));
        assert_eq!(m.motor, "lualatex");
        assert!(m.motivo.contains("% !TeX program"), "{}", m.motivo);
        fs::write(p.0.join("latexmkrc"), "$pdf_mode = 5;\n").unwrap();
        fs::write(p.0.join("b.tex"), "\\documentclass{article}\n\\begin{document}\\end{document}\n").unwrap();
        let m = detectar_motor(&p.0, &p.0.join("b.tex"));
        assert_eq!(m.motor, "xelatex");
        assert!(m.motivo.contains("latexmkrc"), "{}", m.motivo);
        // o comentario vence o latexmkrc
        assert_eq!(motor_de(&p, "a.tex"), "lualatex");
    }

    #[test]
    fn motor_certo_lido_no_log_de_erro() {
        // log real do pdfLaTeX do TeX Live 2026, com a quebra de linha em 79 colunas
        let fontspec = "fontspec.sty:101: Fatal Package f\nontspec Error: The fontspec package requires either XeTeX or\n(fontspec)                      LuaTeX.";
        assert_eq!(motor_pelo_log(fontspec, "pdflatex"), Some("xelatex"));
        assert_eq!(motor_pelo_log(fontspec, "xelatex"), None);
        let lua = "./main.tex:3: Undefined control sequence.\nl.3 \\directlua\n               {tex.print(1)}";
        assert_eq!(motor_pelo_log(lua, "pdflatex"), Some("lualatex"));
        assert_eq!(motor_pelo_log(lua, "xelatex"), Some("lualatex"));
        assert_eq!(motor_pelo_log(lua, "lualatex"), None);
        assert_eq!(motor_pelo_log("! Undefined control sequence.\nl.5 \\foo", "pdflatex"), None);
        // mathspec no LuaLaTeX (log real do Inria)
        let mathspec = " ********************************************\n * XeTeX is required to compile this document.\n * Sorry!\n";
        assert_eq!(motor_pelo_log(mathspec, "lualatex"), Some("xelatex"));
        assert_eq!(motor_pelo_log(mathspec, "xelatex"), None);
    }

    #[test]
    fn pdf_mode_escolhe_o_motor() {
        let mut a = Vec::new();
        let mut e = Vec::new();
        assert_eq!(ler_latexmkrc(Path::new("."), "$pdf_mode = 5;", &mut a, &mut e), Some("xelatex"));
        assert_eq!(ler_latexmkrc(Path::new("."), "$pdf_mode=4;", &mut a, &mut e), Some("lualatex"));
        assert_eq!(ler_latexmkrc(Path::new("."), "$pdf_mode = 1;", &mut a, &mut e), None);
    }
}
