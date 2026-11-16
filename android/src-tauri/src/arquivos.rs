//! Comandos de arquivos e pastas do projeto, copia, exportacao em zip, limpeza de
//! arquivos auxiliares e busca direta e inversa do SyncTeX.
//!
//! Cada `caminho_relativo` e relativo a pasta do projeto e usa "/". As barras do
//! inicio sao removidas antes de validar, porque o frontend manda os caminhos no
//! formato da arvore ("/capitulos/intro.tex"). Letras de unidade (ex.: "C:") e qualquer
//! componente ".." sao recusados, para que nada saia da pasta do projeto. Os comandos
//! que devolvem caminho relativo usam o mesmo formato da arvore, com "/" no inicio.

use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::limites_zip;

/// Sufixos dos arquivos que o LaTeX gera durante a compilacao.
const SUFIXOS_AUXILIARES: &[&str] = &[
    ".aux",
    ".log",
    ".toc",
    ".out",
    ".bbl",
    ".blg",
    ".synctex.gz",
    ".fls",
    ".fdb_latexmk",
    ".lof",
    ".lot",
    ".nav",
    ".snm",
    ".vrb",
    ".xdv",
    ".bcf",
    ".run.xml",
    ".brf",
    ".idx",
    ".ilg",
    ".ind",
];

/// Limite de tentativas ao procurar um nome livre ("nome (1).ext", "nome (2).ext"...).
const MAX_TENTATIVAS_NOME: u32 = 9999;

const MSG_FORA_DO_PROJETO: &str = "Caminho inválido: não pode sair da pasta do projeto";

// ---------------------------------------------------------------------------
// Validacao de caminhos e nomes
// ---------------------------------------------------------------------------

/// Nomes de dispositivo do Windows (CON, NUL, COM1...), com ou sem extensao.
fn eh_nome_reservado(nome: &str) -> bool {
    let base = nome
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    base.len() == 4
        && (base.starts_with("COM") || base.starts_with("LPT"))
        && base.as_bytes()[3].is_ascii_digit()
}

/// Confere um nome NOVO (arquivo ou pasta) que vai ser criado.
pub(crate) fn validar_nome(nome: &str) -> Result<(), String> {
    if nome.is_empty() {
        return Err("O nome não pode ficar vazio".to_string());
    }
    if nome == "." || nome == ".." {
        return Err("Nome inválido".to_string());
    }
    // "--lua-filter=x.tex" viraria opcao do Pandoc (e "-x" de qualquer programa que receba o nome)
    if nome.starts_with('-') {
        return Err("O nome não pode começar com hífen (-)".to_string());
    }
    if nome
        .chars()
        .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err("O nome não pode ter / \\ : * ? \" < > |".to_string());
    }
    if nome.ends_with('.') || nome.ends_with(' ') {
        return Err("O nome não pode terminar com ponto ou espaço".to_string());
    }
    if eh_nome_reservado(nome) {
        return Err(format!("{} é um nome reservado do Windows", nome));
    }
    Ok(())
}

/// Separa um caminho relativo do projeto em componentes. Remove as barras do inicio
/// ANTES de validar e recusa ".." e qualquer componente com ":" (letra de unidade ou
/// fluxo alternativo do NTFS). Componentes vazios e "." sao ignorados.
fn dividir_caminho(caminho_relativo: &str) -> Result<Vec<String>, String> {
    let texto = caminho_relativo
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/");
    let mut partes = Vec::new();
    for parte in texto.split('/') {
        if parte.is_empty() || parte == "." {
            continue;
        }
        if parte == ".." {
            return Err(MSG_FORA_DO_PROJETO.to_string());
        }
        if parte.contains(':') || parte.contains('\0') {
            return Err("Caminho inválido: caminhos absolutos não sao permitidos".to_string());
        }
        partes.push(parte.to_string());
    }
    Ok(partes)
}

/// Componentes de um item que vai ser criado: cada um precisa ter nome valido.
fn partes_de_item_novo(caminho_relativo: &str) -> Result<Vec<String>, String> {
    let partes = dividir_caminho(caminho_relativo)?;
    if partes.is_empty() {
        return Err("Informe o nome do item".to_string());
    }
    for parte in &partes {
        validar_nome(parte)?;
    }
    Ok(partes)
}

/// Caminho relativo no formato da arvore: "/" no inicio e "/" como separador.
fn montar_relativo(partes: &[String]) -> String {
    format!("/{}", partes.join("/"))
}

fn pasta_do_projeto(caminho_projeto: &str) -> Result<PathBuf, String> {
    let base = PathBuf::from(caminho_projeto);
    if caminho_projeto.trim().is_empty() || !base.is_dir() {
        return Err(format!(
            "Pasta do projeto não encontrada: {}",
            caminho_projeto
        ));
    }
    Ok(base)
}

/// Junta componentes ja validados (um a um, nunca com um texto bruto).
fn juntar(base: &Path, partes: &[String]) -> PathBuf {
    let mut caminho = base.to_path_buf();
    for parte in partes {
        caminho.push(parte);
    }
    caminho
}

fn existe(caminho: &Path) -> bool {
    fs::symlink_metadata(caminho).is_ok()
}

/// Segunda barreira, alem da validacao do texto: resolve links simbolicos e juncoes
/// e confere que a pasta (ou o ancestral existente mais proximo dela) continua dentro
/// do projeto. Se o sistema de arquivos nao conseguir resolver, confia na validacao
/// do texto.
pub(crate) fn garantir_dentro(base: &Path, pasta: &Path) -> Result<(), String> {
    let Ok(base_real) = fs::canonicalize(base) else {
        return Ok(());
    };
    let mut atual = Some(pasta);
    while let Some(candidata) = atual {
        if let Ok(real) = fs::canonicalize(candidata) {
            return if real.starts_with(&base_real) {
                Ok(())
            } else {
                Err(MSG_FORA_DO_PROJETO.to_string())
            };
        }
        atual = candidata.parent();
    }
    Ok(())
}

/// Verdadeiro se os dois caminhos apontam para o mesmo item no disco (inclui trocar
/// so maiusculas e minusculas no Windows).
fn mesmo_item(a: &Path, b: &Path) -> bool {
    matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// Separa "nome.ext" em ("nome", ".ext"); sem extensao devolve ("nome", "").
fn dividir_nome_arquivo(nome: &str) -> (String, String) {
    let caminho = Path::new(nome);
    let radical = caminho
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| nome.to_string());
    let extensao = caminho
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (radical, extensao)
}

/// Apaga um arquivo; se estiver marcado como somente leitura, libera e tenta de novo.
fn apagar_arquivo(caminho: &Path) -> io::Result<()> {
    match fs::remove_file(caminho) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            let mut permissoes = fs::metadata(caminho)?.permissions();
            if !permissoes.readonly() {
                return Err(e);
            }
            #[allow(clippy::permissions_set_readonly_false)]
            permissoes.set_readonly(false);
            fs::set_permissions(caminho, permissoes)?;
            fs::remove_file(caminho)
        }
        outro => outro,
    }
}

// ---------------------------------------------------------------------------
// Criar, importar, renomear, excluir e copiar itens do projeto
// ---------------------------------------------------------------------------

/// Cria um arquivo vazio (e as pastas-mae que faltarem). Erro se ja existir.
#[tauri::command]
pub fn cmd_criar_arquivo(caminho_projeto: String, caminho_relativo: String) -> Result<(), String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    let partes = partes_de_item_novo(&caminho_relativo)?;
    let nome = partes.last().cloned().unwrap_or_default();
    let destino = juntar(&base, &partes);
    let pai = destino.parent().unwrap_or(&base).to_path_buf();
    garantir_dentro(&base, &pai)?;
    if existe(&destino) {
        return Err(format!("Já existe um item chamado {}", nome));
    }
    fs::create_dir_all(&pai)
        .map_err(|e| format!("Não foi possível criar as pastas de {}: {}", nome, e))?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destino)
        .map(|_| ())
        .map_err(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                format!("Já existe um item chamado {}", nome)
            } else {
                format!("Não foi possível criar {}: {}", nome, e)
            }
        })
}

/// Cria uma pasta (e as pastas-mae que faltarem). Erro se ja existir.
#[tauri::command]
pub fn cmd_criar_pasta(caminho_projeto: String, caminho_relativo: String) -> Result<(), String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    let partes = partes_de_item_novo(&caminho_relativo)?;
    let nome = partes.last().cloned().unwrap_or_default();
    let destino = juntar(&base, &partes);
    let pai = destino.parent().unwrap_or(&base).to_path_buf();
    garantir_dentro(&base, &pai)?;
    if existe(&destino) {
        return Err(format!("Já existe um item chamado {}", nome));
    }
    fs::create_dir_all(&pai)
        .map_err(|e| format!("Não foi possível criar as pastas de {}: {}", nome, e))?;
    fs::create_dir(&destino).map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            format!("Já existe um item chamado {}", nome)
        } else {
            format!("Não foi possível criar a pasta {}: {}", nome, e)
        }
    })
}

/// Copia o arquivo absoluto `origem` para `pasta_relativa` do projeto ("" = raiz),
/// mantendo o nome. Se o nome ja existir, usa "nome (1).ext", "nome (2).ext"...
/// Devolve o caminho novo no formato da arvore (ex.: "/figuras/foto (1).png").
// No Android o comando registrado e ponte_uri::cmd_importar_arquivo.
pub fn cmd_importar_arquivo(
    caminho_projeto: String,
    origem: String,
    pasta_relativa: String,
) -> Result<String, String> {
    importar_numerado(&caminho_projeto, &origem, &pasta_relativa, 1)
}

/// Extensoes de figura que o ÉguaTex insere com \includegraphics.
pub const EXTENSOES_FIGURA: &[&str] = &["png", "jpg", "jpeg", "pdf", "eps"];
/// Pasta do projeto onde as figuras importadas ficam.
pub const PASTA_FIGURAS: &str = "figuras";

/// O nome termina numa extensao de figura aceita?
pub fn e_figura(nome: &str) -> bool {
    let (_, ext) = dividir_nome_arquivo(nome);
    EXTENSOES_FIGURA.contains(&ext.trim_start_matches('.').to_lowercase().as_str())
}

/// Copia uma figura do computador para `figuras/` do projeto (a pasta e criada se faltar).
/// Nome repetido vira "nome (2).ext", "nome (3).ext"... Devolve o caminho relativo sem barra
/// inicial, pronto para o \includegraphics (ex.: "figuras/foto (2).png").
// No Android o comando registrado e ponte_uri::cmd_importar_figura.
pub fn cmd_importar_figura(caminho_projeto: String, origem: String) -> Result<String, String> {
    let nome = Path::new(&origem).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !e_figura(&nome) {
        return Err("Use .png, .jpg, .pdf ou .eps. O .svg não entra no pdflatex: exporte como PDF ou PNG".into());
    }
    let r = importar_numerado(&caminho_projeto, &origem, PASTA_FIGURAS, 2)?;
    Ok(r.trim_start_matches('/').to_string())
}

/// Corpo de `cmd_importar_arquivo`. A numeracao de nomes repetidos comeca em `primeiro_sufixo`
/// ("nome (1).ext" no envio comum; "nome (2).ext" nas figuras).
fn importar_numerado(
    caminho_projeto: &str,
    origem: &str,
    pasta_relativa: &str,
    primeiro_sufixo: u32,
) -> Result<String, String> {
    let (caminho_projeto, origem, pasta_relativa) = (caminho_projeto.to_string(), origem.to_string(), pasta_relativa.to_string());
    let base = pasta_do_projeto(&caminho_projeto)?;
    let arquivo_origem = PathBuf::from(&origem);
    if !arquivo_origem.is_file() {
        return Err(format!("Arquivo de origem não encontrado: {}", origem));
    }
    let nome_original = arquivo_origem
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| "Arquivo de origem inválido".to_string())?;

    let partes_pasta = dividir_caminho(&pasta_relativa)?;
    let pasta = juntar(&base, &partes_pasta);
    garantir_dentro(&base, &pasta)?;
    fs::create_dir_all(&pasta)
        .map_err(|e| format!("Não foi possível preparar a pasta de destino: {}", e))?;

    let (radical, extensao) = dividir_nome_arquivo(&nome_original);
    for tentativa in 0..=MAX_TENTATIVAS_NOME {
        let nome = if tentativa == 0 {
            nome_original.clone()
        } else {
            format!("{} ({}){}", radical, primeiro_sufixo + tentativa - 1, extensao)
        };
        let destino = pasta.join(&nome);
        if existe(&destino) {
            continue;
        }
        match OpenOptions::new().write(true).create_new(true).open(&destino) {
            Ok(mut saida) => {
                let copiado = File::open(&arquivo_origem)
                    .and_then(|mut entrada| io::copy(&mut entrada, &mut saida));
                drop(saida);
                if let Err(e) = copiado {
                    let _ = fs::remove_file(&destino);
                    return Err(format!("Não foi possível copiar {}: {}", nome_original, e));
                }
                let mut partes = partes_pasta.clone();
                partes.push(nome);
                return Ok(montar_relativo(&partes));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Não foi possível criar {}: {}", nome, e)),
        }
    }
    Err(format!(
        "Não foi possível achar um nome livre para {}",
        nome_original
    ))
}

/// Renomeia um arquivo ou pasta, mantendo-o na mesma pasta-mae. Devolve o caminho
/// novo no formato da arvore.
#[tauri::command]
pub fn cmd_renomear_item(
    caminho_projeto: String,
    caminho_relativo: String,
    novo_nome: String,
) -> Result<String, String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    let mut partes = dividir_caminho(&caminho_relativo)?;
    if partes.is_empty() {
        return Err("Não é possível renomear a pasta do projeto por aqui".to_string());
    }
    let novo = novo_nome.trim();
    validar_nome(novo)?;

    let origem = juntar(&base, &partes);
    let pai = origem.parent().unwrap_or(&base).to_path_buf();
    garantir_dentro(&base, &pai)?;
    if !existe(&origem) {
        return Err(format!("Item não encontrado: {}", montar_relativo(&partes)));
    }
    if partes.last().map(String::as_str) == Some(novo) {
        return Ok(montar_relativo(&partes));
    }
    let destino = pai.join(novo);
    if existe(&destino) && !mesmo_item(&origem, &destino) {
        return Err(format!("Já existe um item chamado {} nesta pasta", novo));
    }
    fs::rename(&origem, &destino).map_err(|e| format!("Não foi possível renomear: {}", e))?;
    if let Some(ultimo) = partes.last_mut() {
        *ultimo = novo.to_string();
    }
    Ok(montar_relativo(&partes))
}

/// Apaga um arquivo ou uma pasta (com tudo dentro). Recusa apagar a pasta do projeto.
#[tauri::command(async)]
pub fn cmd_excluir_item(caminho_projeto: String, caminho_relativo: String) -> Result<(), String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    let partes = dividir_caminho(&caminho_relativo)?;
    if partes.is_empty() {
        return Err("Não é possível excluir a pasta do projeto".to_string());
    }
    let alvo = juntar(&base, &partes);
    let pai = alvo.parent().unwrap_or(&base).to_path_buf();
    garantir_dentro(&base, &pai)?;
    let tipo = fs::symlink_metadata(&alvo)
        .map_err(|_| format!("Item não encontrado: {}", montar_relativo(&partes)))?
        .file_type();
    let resultado = if tipo.is_dir() {
        fs::remove_dir_all(&alvo)
    } else if tipo.is_symlink() {
        // remove so o link (arquivo ou pasta), nunca o que ele aponta
        fs::remove_file(&alvo).or_else(|_| fs::remove_dir(&alvo))
    } else {
        apagar_arquivo(&alvo)
    };
    resultado.map_err(|e| {
        format!(
            "Não foi possível excluir {}: {}",
            partes.last().map(String::as_str).unwrap_or(""),
            e
        )
    })
}

/// Copia um arquivo do projeto para o caminho absoluto `destino` (arquivo, nao pasta),
/// escolhido pelo usuario numa janela de salvar. Se `destino` ja existir, e
/// sobrescrito, porque a janela de salvar ja pediu a confirmacao.
// No Android o comando registrado e ponte_uri::cmd_copiar_para.
pub fn cmd_copiar_para(
    caminho_projeto: String,
    caminho_relativo: String,
    destino: String,
) -> Result<(), String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    let partes = dividir_caminho(&caminho_relativo)?;
    if partes.is_empty() {
        return Err("Escolha um arquivo do projeto para copiar".to_string());
    }
    let origem = juntar(&base, &partes);
    let pai = origem.parent().unwrap_or(&base).to_path_buf();
    garantir_dentro(&base, &pai)?;
    if !origem.is_file() {
        return Err(format!(
            "Arquivo não encontrado no projeto: {}",
            montar_relativo(&partes)
        ));
    }

    let destino = PathBuf::from(destino);
    if !destino.is_absolute() {
        return Err("O destino precisa ser um caminho absoluto".to_string());
    }
    if destino.is_dir() {
        return Err("O destino e uma pasta; informe o nome do arquivo".to_string());
    }
    if !destino.parent().is_some_and(Path::is_dir) {
        return Err("A pasta de destino não existe".to_string());
    }
    if existe(&destino) && mesmo_item(&origem, &destino) {
        return Ok(());
    }
    fs::copy(&origem, &destino)
        .map(|_| ())
        .map_err(|e| format!("Não foi possível copiar o arquivo: {}", e))
}

// ---------------------------------------------------------------------------
// Copiar o projeto inteiro
// ---------------------------------------------------------------------------

/// Copia a pasta de forma recursiva. Links simbolicos e juncoes nao sao seguidos
/// nem copiados (evita lacos e vazamento para fora do projeto).
fn copiar_pasta(origem: &Path, destino: &Path) -> io::Result<()> {
    fs::create_dir(destino)?;
    for entrada in fs::read_dir(origem)? {
        let entrada = entrada?;
        let tipo = entrada.file_type()?;
        let alvo = destino.join(entrada.file_name());
        if tipo.is_dir() {
            copiar_pasta(&entrada.path(), &alvo)?;
        } else if tipo.is_file() {
            fs::copy(entrada.path(), &alvo)?;
        }
    }
    Ok(())
}

/// Copia o projeto inteiro para uma pasta irma "<nome> (copia)" (se existir,
/// "<nome> (copia 2)", "(copia 3)"...). Devolve o caminho absoluto da copia.
#[tauri::command(async)]
pub fn cmd_copiar_projeto(caminho: String) -> Result<String, String> {
    let origem = pasta_do_projeto(&caminho)?;
    let nome = origem
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| "Caminho de projeto inválido".to_string())?;
    let pai = origem
        .parent()
        .ok_or_else(|| "Projeto sem pasta-mae".to_string())?;

    let mut destino = None;
    for numero in 1..=MAX_TENTATIVAS_NOME {
        let rotulo = if numero == 1 {
            "(copia)".to_string()
        } else {
            format!("(copia {})", numero)
        };
        let candidato = pai.join(format!("{} {}", nome, rotulo));
        if !existe(&candidato) {
            destino = Some(candidato);
            break;
        }
    }
    let destino = destino.ok_or_else(|| "Não foi possível achar um nome livre para a copia".to_string())?;

    if let Err(e) = copiar_pasta(&origem, &destino) {
        let _ = fs::remove_dir_all(&destino);
        return Err(format!("Não foi possível copiar o projeto: {}", e));
    }
    Ok(destino.display().to_string())
}

// ---------------------------------------------------------------------------
// Auxiliares do LaTeX: limpar e exportar
// ---------------------------------------------------------------------------

/// Verdadeiro para os arquivos que o LaTeX gera (.aux, .log, .synctex.gz...) a partir
/// de um .tex da mesma pasta. `tex_da_pasta` tem os radicais (em minusculas) dos .tex
/// dessa pasta: um `dados.log` ou `scf.out` sem `dados.tex` ao lado e do usuario, nao
/// do LaTeX, e nunca e apagado nem tirado do .zip.
fn eh_auxiliar(nome: &str, tex_da_pasta: &HashSet<String>) -> bool {
    let minusculo = nome.to_lowercase();
    SUFIXOS_AUXILIARES.iter().any(|sufixo| {
        minusculo
            .strip_suffix(sufixo)
            .is_some_and(|radical| !radical.is_empty() && tex_da_pasta.contains(radical))
    })
}

/// Radicais (em minusculas) dos arquivos .tex de uma pasta.
fn radicais_tex(entradas: &[fs::DirEntry]) -> HashSet<String> {
    entradas
        .iter()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let nome = e.file_name().to_string_lossy().to_lowercase();
            nome.strip_suffix(".tex").map(str::to_string)
        })
        .collect()
}

fn apagar_auxiliares_em(pasta: &Path, apagados: &mut u32, falhas: &mut Vec<String>) {
    let Ok(lidas) = fs::read_dir(pasta) else {
        return;
    };
    let entradas: Vec<fs::DirEntry> = lidas.flatten().collect();
    let tex_da_pasta = radicais_tex(&entradas);
    for entrada in entradas {
        let Ok(tipo) = entrada.file_type() else {
            continue;
        };
        let nome = entrada.file_name().to_string_lossy().into_owned();
        if tipo.is_dir() {
            if !nome.starts_with('.') {
                apagar_auxiliares_em(&entrada.path(), apagados, falhas);
            }
        } else if tipo.is_file() && eh_auxiliar(&nome, &tex_da_pasta) {
            match apagar_arquivo(&entrada.path()) {
                Ok(()) => *apagados += 1,
                Err(e) => falhas.push(format!("{}: {}", nome, e)),
            }
        }
    }
}

/// Apaga os arquivos auxiliares do LaTeX no projeto e nas subpastas (sem entrar em
/// pastas ocultas). Devolve quantos foram apagados.
#[tauri::command(async)]
pub fn cmd_limpar_auxiliares(caminho_projeto: String) -> Result<u32, String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    let mut apagados = 0;
    let mut falhas = Vec::new();
    apagar_auxiliares_em(&base, &mut apagados, &mut falhas);
    if apagados == 0 {
        if let Some(primeira) = falhas.first() {
            return Err(format!("Não foi possível apagar {}", primeira));
        }
    }
    Ok(apagados)
}

pub(crate) enum ItemZip {
    Arquivo { origem: PathBuf, nome: String },
    Pasta { nome: String },
}

/// Verdadeiro se `caminho` e o proprio zip que esta sendo gravado (quando o usuario
/// exporta para dentro do projeto e ja existia um zip anterior com esse nome).
fn eh_o_destino(caminho: &Path, destino_real: Option<&Path>) -> bool {
    let Some(destino) = destino_real else {
        return false;
    };
    caminho.file_name() == destino.file_name()
        && fs::canonicalize(caminho).is_ok_and(|real| real == destino)
}

/// Lista o que entra no zip. Pula os auxiliares, o X.pdf de cada X.tex da mesma pasta
/// e as pastas ocultas (.git e semelhantes). Pastas que ficariam vazias entram como
/// entrada de pasta, para nao se perderem.
pub(crate) fn coletar_itens_zip(
    pasta: &Path,
    prefixo: &str,
    itens: &mut Vec<ItemZip>,
    destino_real: Option<&Path>,
) -> io::Result<()> {
    let mut entradas = fs::read_dir(pasta)?.collect::<io::Result<Vec<_>>>()?;
    entradas.sort_by_key(|e| e.file_name());

    let mut tex_da_pasta: HashSet<String> = HashSet::new();
    for entrada in &entradas {
        if entrada.file_type()?.is_file() {
            let nome = entrada.file_name().to_string_lossy().to_lowercase();
            if let Some(radical) = nome.strip_suffix(".tex") {
                tex_da_pasta.insert(radical.to_string());
            }
        }
    }

    let inicio = itens.len();
    for entrada in &entradas {
        let tipo = entrada.file_type()?;
        let nome = entrada.file_name().to_string_lossy().into_owned();
        let relativo = if prefixo.is_empty() {
            nome.clone()
        } else {
            format!("{}/{}", prefixo, nome)
        };
        if tipo.is_dir() {
            if !nome.starts_with('.') {
                coletar_itens_zip(&entrada.path(), &relativo, itens, destino_real)?;
            }
        } else if tipo.is_file() {
            let pdf_gerado = nome
                .to_lowercase()
                .strip_suffix(".pdf")
                .is_some_and(|radical| tex_da_pasta.contains(radical));
            if eh_auxiliar(&nome, &tex_da_pasta) || pdf_gerado || eh_o_destino(&entrada.path(), destino_real) {
                continue;
            }
            itens.push(ItemZip::Arquivo {
                origem: entrada.path(),
                nome: relativo,
            });
        }
    }
    if itens.len() == inicio && !prefixo.is_empty() {
        itens.push(ItemZip::Pasta {
            nome: prefixo.to_string(),
        });
    }
    Ok(())
}

/// Ritmo dos eventos `tarefa-progresso` de um laco: no maximo 10 por segundo, mais o ultimo.
pub(crate) struct RitmoProgresso {
    canal: &'static str,
    inicio: std::time::Instant,
    ultimo: Option<u128>,
}

impl RitmoProgresso {
    pub(crate) fn novo(canal: &'static str) -> Self {
        Self { canal, inicio: std::time::Instant::now(), ultimo: None }
    }

    /// `feito` de `total` itens; o ultimo item sempre emite.
    pub(crate) fn passo(&mut self, feito: u64, total: u64) {
        let agora = self.inicio.elapsed().as_millis();
        if crate::drive::deve_emitir_progresso(self.ultimo, agora, feito >= total) {
            self.ultimo = Some(agora);
            crate::drive::emitir_progresso_canal(self.canal, feito, Some(total));
        }
    }
}

fn gravar_zip(itens: &[ItemZip], saida: &Path) -> Result<(), String> {
    let mut ritmo = RitmoProgresso::novo("zip-exportar");
    let arquivo =
        File::create(saida).map_err(|e| format!("Não foi possível criar o zip: {}", e))?;
    let mut zip = ZipWriter::new(BufWriter::new(arquivo));
    let opcoes = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (n, item) in itens.iter().enumerate() {
        ritmo.passo(n as u64, itens.len() as u64);
        match item {
            ItemZip::Pasta { nome } => {
                zip.add_directory(nome.as_str(), opcoes)
                    .map_err(|e| format!("Erro ao gravar a pasta {}: {}", nome, e))?;
            }
            ItemZip::Arquivo { origem, nome } => {
                let mut entrada = File::open(origem)
                    .map_err(|e| format!("Não foi possível ler {}: {}", nome, e))?;
                let tamanho = entrada.metadata().map(|m| m.len()).unwrap_or(0);
                zip.start_file(
                    nome.as_str(),
                    opcoes.large_file(tamanho >= u64::from(u32::MAX)),
                )
                .map_err(|e| format!("Erro ao gravar {}: {}", nome, e))?;
                io::copy(&mut entrada, &mut zip)
                    .map_err(|e| format!("Erro ao gravar {}: {}", nome, e))?;
            }
        }
    }
    ritmo.passo(itens.len() as u64, itens.len() as u64);
    let mut escritor = zip
        .finish()
        .map_err(|e| format!("Erro ao finalizar o zip: {}", e))?;
    escritor
        .flush()
        .map_err(|e| format!("Erro ao finalizar o zip: {}", e))
}

/// Grava em `destino` um .zip com os arquivos do projeto (caminhos relativos, com "/").
/// Pula os auxiliares do LaTeX e, para cada X.tex, o X.pdf gerado.
// No Android o comando registrado e ponte_uri::cmd_exportar_zip.
pub fn cmd_exportar_zip(caminho_projeto: String, destino: String) -> Result<(), String> {
    let base = pasta_do_projeto(&caminho_projeto)?;
    if destino.trim().is_empty() {
        return Err("Informe onde salvar o arquivo zip".to_string());
    }
    let destino = PathBuf::from(destino);
    if destino.is_dir() {
        return Err("O destino e uma pasta; informe o nome do arquivo zip".to_string());
    }
    let nome_destino = destino
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| "Destino inválido".to_string())?;
    if let Some(pasta) = destino.parent() {
        if !pasta.as_os_str().is_empty() && !pasta.is_dir() {
            return Err(format!("A pasta de destino não existe: {}", pasta.display()));
        }
    }

    let destino_real = fs::canonicalize(&destino).ok();
    let mut itens = Vec::new();
    coletar_itens_zip(&base, "", &mut itens, destino_real.as_deref())
        .map_err(|e| format!("Não foi possível ler o projeto: {}", e))?;

    // grava num arquivo temporario ao lado e so troca no fim: se algo falhar, o zip
    // antigo (se houver) continua intacto
    let temporario = destino.with_file_name(format!("{}.parte", nome_destino));
    if let Err(e) = gravar_zip(&itens, &temporario) {
        let _ = fs::remove_file(&temporario);
        return Err(e);
    }
    fs::rename(&temporario, &destino).map_err(|e| {
        let _ = fs::remove_file(&temporario);
        format!("Não foi possível salvar o zip: {}", e)
    })
}

/// Caminhos de dentro do zip que viram arquivos do projeto. Pula as pastas que o
/// macOS acrescenta (__MACOSX, .DS_Store) e recusa caminhos que sairiam da pasta.
fn caminhos_do_zip<R: io::Read + io::Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<Vec<(usize, PathBuf, bool)>, String> {
    let mut lista = Vec::new();
    for i in 0..zip.len() {
        let item = zip
            .by_index(i)
            .map_err(|e| format!("O zip está corrompido: {}", e))?;
        let Some(caminho) = item.enclosed_name() else {
            continue;
        };
        let lixo = caminho.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s == "__MACOSX" || s == ".DS_Store"
        });
        // "..", raiz ou letra de unidade: o caminho pode sair da pasta, entao e descartado
        let perigoso = caminho.components().any(|c| {
            matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))
        });
        if !lixo && !perigoso && !caminho.as_os_str().is_empty() {
            lista.push((i, caminho, item.is_dir()));
        }
    }
    Ok(lista)
}

/// Prefixo de pastas que todos os caminhos tem em comum (o "embrulho" do zip: uma pasta, ou
/// varias aninhadas, que so contem o projeto). Considera so diretorios, nunca o nome do arquivo:
/// `["m/T/t/x.tex", "m/T/y.txt"]` da `"m/T"` e `["x.tex", "a/y"]` da `""`. Caminho que termina em
/// "/" e uma pasta inteira. Separador pode ser "/" ou "\\".
pub(crate) fn prefixo_embrulho<S: AsRef<str>>(caminhos: &[S]) -> String {
    let separar = |c: &S| -> (Vec<String>, bool) {
        let norm = c.as_ref().replace('\\', "/");
        let eh_pasta = norm.ends_with('/');
        let mut partes: Vec<String> = norm.split('/').filter(|p| !p.is_empty()).map(str::to_string).collect();
        if !eh_pasta {
            partes.pop();
        }
        (partes, eh_pasta)
    };
    let em_comum = |a: &[String], b: &[String]| a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let mut comum: Option<Vec<String>> = None;
    // arquivos primeiro: a pasta que so contem outras pastas ("a/") nao limita o prefixo
    for c in caminhos {
        let (partes, eh_pasta) = separar(c);
        if eh_pasta {
            continue;
        }
        let n = comum.as_ref().map_or(partes.len(), |atual| em_comum(atual, &partes));
        if n == 0 {
            return String::new();
        }
        comum = Some(partes[..n].to_vec());
    }
    for c in caminhos {
        let (partes, eh_pasta) = separar(c);
        if !eh_pasta {
            continue;
        }
        match &comum {
            None => comum = Some(partes),
            // pasta que nao e ancestral do prefixo (nem igual a ele) corta o prefixo
            Some(atual) if em_comum(atual, &partes) < partes.len() => {
                let n = em_comum(atual, &partes);
                if n == 0 {
                    return String::new();
                }
                comum = Some(atual[..n].to_vec());
            }
            Some(_) => {}
        }
    }
    comum.unwrap_or_default().join("/")
}

/// Parte da linha antes do comentario (`%` sem barra antes comenta o resto da linha).
pub(crate) fn sem_comentario(linha: &str) -> &str {
    let mut anterior = ' ';
    for (i, ch) in linha.char_indices() {
        if ch == '%' && anterior != '\\' {
            return &linha[..i];
        }
        anterior = if anterior == '\\' && ch == '\\' { ' ' } else { ch };
    }
    linha
}

/// O comando `nome` (ex.: `\documentclass`) aparece fora de comentario, sem letra logo depois.
fn tem_comando(texto: &str, nome: &str) -> bool {
    texto.lines().any(|linha| {
        let codigo = sem_comentario(linha);
        codigo.match_indices(nome).any(|(i, achado)| {
            !codigo[i + achado.len()..].chars().next().is_some_and(|c| c.is_alphabetic())
        })
    })
}

/// O texto tem `\documentclass` fora de comentario.
pub(crate) fn tem_documentclass(texto: &str) -> bool {
    tem_comando(texto, "\\documentclass")
}

/// O texto tem `\begin{document}` fora de comentario (aceita espacos: `\begin {document}`).
pub(crate) fn tem_begin_document(texto: &str) -> bool {
    texto.lines().any(|linha| {
        let codigo: String = sem_comentario(linha).chars().filter(|c| !c.is_whitespace()).collect();
        codigo.contains("\\begin{document}")
    })
}

/// O documento usa a classe `standalone` (figura avulsa, compilada a parte).
fn classe_standalone(texto: &str) -> bool {
    texto.lines().any(|linha| {
        let codigo: String = sem_comentario(linha).chars().filter(|c| !c.is_whitespace()).collect();
        codigo.find("\\documentclass").is_some_and(|i| codigo[i..].contains("{standalone}"))
    })
}

/// O documento principal do projeto, como no Overleaf: so um `.tex` com `\begin{document}` fora de
/// comentario pode ser principal (um preambulo com `\documentclass` e sem `\begin{document}` nunca e).
/// Empate: figura `standalone` por ultimo, depois quem tem `\documentclass` no proprio arquivo,
/// `main.tex`, o nome igual ao da pasta do projeto (`nome_pasta`), o maior, o mais raso e a ordem
/// alfabetica. `None` se nenhum serve.
pub(crate) fn principal_entre<B: AsRef<[u8]>>(arquivos: &[(String, B)], nome_pasta: &str) -> Option<String> {
    let pasta = nome_pasta.to_lowercase();
    arquivos
        .iter()
        .filter(|(rel, _)| rel.to_lowercase().ends_with(".tex"))
        .filter_map(|(rel, bytes)| {
            let texto = String::from_utf8_lossy(bytes.as_ref());
            tem_begin_document(&texto).then(|| {
                let rel = rel.replace('\\', "/");
                let nome = rel.rsplit('/').next().unwrap_or(&rel).to_lowercase();
                let raiz_do_nome = nome.trim_end_matches(".tex").to_string();
                let chave = (
                    classe_standalone(&texto),
                    !tem_documentclass(&texto),
                    nome != "main.tex",
                    pasta.is_empty() || raiz_do_nome != pasta,
                    std::cmp::Reverse(bytes.as_ref().len()),
                    rel.matches('/').count(),
                    rel.clone(),
                );
                (chave, rel)
            })
        })
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, rel)| rel)
}

/// Le os `.tex` do projeto em disco (ate 6 niveis, sem pastas ocultas, ate 400 arquivos de no
/// maximo 4 MB) e escolhe o principal com `principal_entre`. Caminho relativo com "/".
pub(crate) fn principal_do_projeto(raiz: &Path) -> Option<String> {
    fn juntar(raiz: &Path, pasta: &Path, nivel: u32, saida: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entradas) = fs::read_dir(pasta) else { return };
        let mut entradas: Vec<_> = entradas.flatten().collect();
        entradas.sort_by_key(|e| e.file_name());
        for e in entradas {
            if saida.len() >= 400 {
                return;
            }
            let nome = e.file_name().to_string_lossy().to_string();
            let Ok(tipo) = e.file_type() else { continue };
            if tipo.is_dir() {
                if nivel < 6 && !nome.starts_with('.') {
                    juntar(raiz, &e.path(), nivel + 1, saida);
                }
            } else if tipo.is_file()
                && nome.to_lowercase().ends_with(".tex")
                && e.metadata().is_ok_and(|m| m.len() <= 4 * 1024 * 1024)
                && let Ok(bytes) = fs::read(e.path())
                && let Ok(rel) = e.path().strip_prefix(raiz)
            {
                saida.push((rel.to_string_lossy().replace('\\', "/"), bytes));
            }
        }
    }
    let mut arquivos = Vec::new();
    juntar(raiz, raiz, 0, &mut arquivos);
    let nome_pasta = raiz.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    principal_entre(&arquivos, &nome_pasta)
}

/// Documento principal do projeto em `diretorio` (relativo, com "/"), ou `None` se nenhum `.tex`
/// tem `\begin{document}`.
#[tauri::command(async)]
pub fn cmd_detectar_principal(diretorio: String) -> Option<String> {
    principal_do_projeto(Path::new(&diretorio))
}

/// Cada arquivo do zip (sem o embrulho `raiz`) existe em `pasta` com o mesmo conteudo.
fn pasta_tem_o_zip<R: io::Read + io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    lista: &[(usize, PathBuf, bool)],
    raiz: Option<&Path>,
    pasta: &Path,
) -> bool {
    // dois itens com o mesmo nome a menos de maiusculas (o MUL_poster.zip tem mul_poster.cls e
    // MUL_poster.cls) viram um arquivo so no Windows: vale o ultimo, que a extracao gravou por cima
    let ultimo: std::collections::HashMap<String, usize> =
        lista.iter().map(|(i, c, _)| (c.to_string_lossy().to_lowercase(), *i)).collect();
    for (i, caminho, eh_pasta) in lista {
        if ultimo.get(&caminho.to_string_lossy().to_lowercase()) != Some(i) {
            continue;
        }
        let relativo = match raiz {
            Some(r) => match caminho.strip_prefix(r) {
                Ok(resto) if !resto.as_os_str().is_empty() => resto.to_path_buf(),
                _ => continue,
            },
            None => caminho.clone(),
        };
        // ".." ou caminho absoluto sairia da pasta: a entrada nao conta na comparacao
        let fora = relativo.is_absolute()
            || relativo.components().any(|c| {
                matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))
            });
        // nomes que a extracao pula (hifen no inicio) tambem nao estao na pasta
        if fora || limites_zip::motivo_de_bloqueio(&relativo.to_string_lossy()).is_some() {
            continue;
        }
        let alvo = pasta.join(&relativo);
        if *eh_pasta {
            if !alvo.is_dir() {
                return false;
            }
            continue;
        }
        let Ok(mut item) = zip.by_index(*i) else { return false };
        let Ok(meta) = fs::metadata(&alvo) else { return false };
        if !meta.is_file() || meta.len() != item.size() {
            return false;
        }
        let mut no_zip = Vec::with_capacity(item.size().min(64 * 1024 * 1024) as usize);
        if io::Read::read_to_end(&mut item, &mut no_zip).is_err() {
            return false;
        }
        if fs::read(&alvo).ok().as_deref() != Some(no_zip.as_slice()) {
            return false;
        }
    }
    true
}

/// O que a importacao de um zip devolve: a pasta criada e o que ficou de fora por seguranca.
#[derive(Debug, Clone, Serialize)]
pub struct ResultadoImportacao {
    pub caminho: String,
    /// Entradas do zip que nao foram gravadas (programas, atalhos e nomes com hifen no inicio).
    pub ignorados: Vec<String>,
}

/// Extrai um .zip (por exemplo, o "Download Source" do Overleaf) numa pasta nova
/// dentro de `pasta_pai`, com o nome do zip. Devolve o caminho da pasta criada e a lista do
/// que foi ignorado. Os limites de `limites_zip` valem antes e durante a extracao; se algo
/// passar deles, a pasta parcial e apagada.
// No Android o comando registrado e ponte_uri::cmd_importar_zip.
pub fn cmd_importar_zip(arquivo_zip: String, pasta_pai: String) -> Result<ResultadoImportacao, String> {
    importar_zip_com_limites(&arquivo_zip, &pasta_pai, &limites_zip::Limites::PADRAO)
}

pub(crate) fn importar_zip_com_limites(
    arquivo_zip: &str,
    pasta_pai: &str,
    limites: &limites_zip::Limites,
) -> Result<ResultadoImportacao, String> {
    let origem = Path::new(arquivo_zip);
    let arquivo =
        File::open(origem).map_err(|e| format!("Não foi possível abrir o zip: {}", e))?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(arquivo))
        .map_err(|e| format!("O arquivo não é um zip válido: {}", e))?;
    let declarado = limites_zip::conferir(&mut zip, limites)?;
    let lista = caminhos_do_zip(&mut zip)?;
    if lista.is_empty() {
        return Err("O zip está vazio".to_string());
    }
    // programas e atalhos nao entram; o resto da conta (embrulho, nomes) so olha o que entra
    let mut ignorados: Vec<String> = Vec::new();
    let (lista, barrados): (Vec<_>, Vec<_>) = lista.into_iter().partition(|(_, c, eh_pasta)| {
        *eh_pasta || limites_zip::motivo_de_bloqueio(&c.to_string_lossy()) != Some(limites_zip::Motivo::Executavel)
    });
    ignorados.extend(barrados.iter().map(|(_, c, _)| limites_zip::nome_para_aviso(&c.to_string_lossy())));
    if lista.is_empty() {
        return Err(format!("O zip não tem nada que o ÉguaTex possa abrir. Ignorados: {}.", ignorados.join(", ")));
    }
    let nomes_no_zip: Vec<String> = lista
        .iter()
        .map(|(_, c, eh_pasta)| {
            let texto = c.to_string_lossy().replace('\\', "/");
            if *eh_pasta { format!("{texto}/") } else { texto }
        })
        .collect();
    // um zip sem .tex (instalador, .rar, PDF) nao vira projeto: nenhuma pasta e criada
    if !nomes_no_zip.iter().any(|n| n.to_lowercase().ends_with(".tex")) {
        let mut exemplos: Vec<&str> = nomes_no_zip
            .iter()
            .filter(|n| !n.ends_with('/'))
            .map(|n| n.rsplit('/').next().unwrap_or(n))
            .filter(|n| !n.starts_with("._"))
            .take(3)
            .collect();
        exemplos.dedup();
        let conteudo = if exemplos.is_empty() { String::new() } else { format!(" Ele traz: {}.", exemplos.join(", ")) };
        return Err(format!("Este .zip não tem arquivos LaTeX (.tex) e não foi aberto como projeto.{conteudo}"));
    }
    let prefixo = prefixo_embrulho(&nomes_no_zip);
    let raiz = if prefixo.is_empty() { None } else { Some(PathBuf::from(prefixo)) };

    let nome: String = origem
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "projeto".to_string())
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let nome = if nome.trim().is_empty() { "projeto".to_string() } else { nome.trim().to_string() };
    let pai = PathBuf::from(pasta_pai);
    fs::create_dir_all(&pai).map_err(|e| format!("Não foi possível criar {}: {}", pai.display(), e))?;
    limites_zip::conferir_espaco(&pai, declarado)?;
    let mut destino = None;
    for numero in 1..=MAX_TENTATIVAS_NOME {
        let candidato = if numero == 1 {
            pai.join(&nome)
        } else {
            pai.join(format!("{} ({})", nome, numero))
        };
        if !existe(&candidato) {
            destino = Some(candidato);
            break;
        }
        // o mesmo zip importado de novo: reabre a pasta que ja tem todos os arquivos dele iguais
        // (os auxiliares e o PDF gerados depois nao contam), em vez de criar "nome (2)"
        if candidato.is_dir() && pasta_tem_o_zip(&mut zip, &lista, raiz.as_deref(), &candidato) {
            return Ok(ResultadoImportacao { caminho: candidato.display().to_string(), ignorados });
        }
    }
    let destino = destino.ok_or_else(|| "Não foi possível achar um nome livre para a pasta".to_string())?;

    // bytes reais gravados: o cabecalho do zip pode mentir sobre o tamanho
    let mut restante = limites.total;
    let mut extrair = || -> Result<(), String> {
        fs::create_dir_all(&destino).map_err(|e| e.to_string())?;
        let mut ritmo = RitmoProgresso::novo("zip-importar");
        for (n, (i, caminho, eh_pasta)) in lista.iter().enumerate() {
            ritmo.passo(n as u64, lista.len() as u64);
            let relativo = match &raiz {
                Some(r) => match caminho.strip_prefix(r) {
                    Ok(resto) if !resto.as_os_str().is_empty() => resto.to_path_buf(),
                    _ => continue,
                },
                None => caminho.clone(),
            };
            if relativo.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err(format!("{}: caminho inválido no zip", relativo.display()));
            }
            if limites_zip::motivo_de_bloqueio(&relativo.to_string_lossy()).is_some() {
                // nome com hifen no inicio: um programa o leria como opcao
                ignorados.push(limites_zip::nome_para_aviso(&relativo.to_string_lossy()));
                continue;
            }
            let alvo = destino.join(&relativo);
            if *eh_pasta {
                fs::create_dir_all(&alvo).map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(p) = alvo.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let item = zip.by_index(*i).map_err(|e| e.to_string())?;
            let mut saida = File::create(&alvo).map_err(|e| format!("{}: {}", relativo.display(), e))?;
            limites_zip::copiar_limitado(item, &mut saida, limites, &mut restante)
                .map_err(|e| format!("{}: {}", relativo.display(), e))?;
        }
        ritmo.passo(lista.len() as u64, lista.len() as u64);
        Ok(())
    };
    if let Err(e) = extrair() {
        let _ = fs::remove_dir_all(&destino);
        return Err(format!("Não foi possível extrair o zip: {}", e));
    }
    Ok(ResultadoImportacao { caminho: destino.display().to_string(), ignorados })
}

// ---------------------------------------------------------------------------
// SyncTeX (programa de linha de comando `synctex`, que vem com o TeX)
// ---------------------------------------------------------------------------

/// Posicao no PDF (pagina a partir de 1; x, y, largura e altura em pontos).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PosicaoPdf {
    pub pagina: u32,
    pub x: f64,
    pub y: f64,
    pub largura: f64,
    pub altura: f64,
}

/// Posicao no codigo-fonte. `coluna` e -1 quando o SyncTeX nao sabe a coluna.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PosicaoCodigo {
    pub arquivo: String,
    pub linha: u32,
    pub coluna: i32,
}

fn localizar_executavel(nome: &str) -> Result<PathBuf, String> {
    which::which(nome).map_err(|_| format!("{} não encontrado", nome))
}

/// Texto de uma saida de programa. Normalmente e UTF-8; no Windows o synctex escreve
/// os nomes de arquivo na pagina de codigo ANSI do sistema (ex.: "F\xedsica"), que
/// e convertida aqui para nao estragar os acentos.
fn decodificar_saida(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(texto) => texto.to_string(),
        Err(_) => decodificar_ansi(bytes),
    }
}

#[cfg(windows)]
fn decodificar_ansi(bytes: &[u8]) -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MultiByteToWideChar(
            pagina_de_codigo: u32,
            opcoes: u32,
            origem: *const u8,
            bytes_origem: i32,
            destino: *mut u16,
            unidades_destino: i32,
        ) -> i32;
    }
    const CP_ACP: u32 = 0;

    let Ok(tamanho) = i32::try_from(bytes.len()) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    if tamanho == 0 {
        return String::new();
    }
    // SAFETY: os ponteiros e os tamanhos informados descrevem buffers validos; a
    // primeira chamada (destino nulo) so mede o tamanho necessario.
    let necessario =
        unsafe { MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), tamanho, std::ptr::null_mut(), 0) };
    if necessario <= 0 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut buffer = vec![0u16; necessario as usize];
    // SAFETY: `buffer` tem exatamente `necessario` unidades de 16 bits.
    let escrito = unsafe {
        MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            tamanho,
            buffer.as_mut_ptr(),
            necessario,
        )
    };
    if escrito <= 0 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    buffer.truncate(escrito as usize);
    String::from_utf16_lossy(&buffer)
}

#[cfg(not(windows))]
fn decodificar_ansi(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn rodar_synctex(executavel: &Path, argumentos: &[String]) -> Result<String, String> {
    let mut comando = Command::new(executavel);
    comando.args(argumentos);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        comando.creation_flags(0x08000000);
    }
    let saida = comando
        .output()
        .map_err(|e| format!("Não foi possível executar o synctex: {}", e))?;
    if !saida.status.success() {
        return Err(format!("O synctex terminou com erro ({})", saida.status));
    }
    Ok(decodificar_saida(&saida.stdout))
}

/// O synctex precisa do .synctex.gz (ou .synctex) ao lado do PDF; ele so existe se o
/// documento foi compilado com `-synctex=1`.
fn conferir_dados_synctex(pdf: &str) -> Result<(), String> {
    let caminho = Path::new(pdf);
    if !caminho.is_file() {
        return Err(format!("PDF não encontrado: {}", pdf));
    }
    let sem_extensao = caminho.with_extension("").display().to_string();
    let tem_dados = ["synctex.gz", "synctex"]
        .iter()
        .any(|extensao| Path::new(&format!("{}.{}", sem_extensao, extensao)).is_file());
    if tem_dados {
        Ok(())
    } else {
        Err("Dados do SyncTeX não encontrados. Compile o projeto novamente.".to_string())
    }
}

/// Pares "chave:valor" do PRIMEIRO registro entre "SyncTeX result begin" e
/// "SyncTeX result end". Le por prefixo de linha (a chave vai ate o primeiro ":", e o
/// valor pode ter outros ":" e espacos, como em "C:/Minha pasta/main.tex"). O registro
/// termina quando uma chave se repete, o que marca o inicio do registro seguinte.
fn primeiro_registro(saida: &str) -> Vec<(&str, &str)> {
    let mut registro: Vec<(&str, &str)> = Vec::new();
    let mut dentro = false;
    for linha in saida.lines() {
        if linha.starts_with("SyncTeX result begin") {
            dentro = true;
        } else if linha.starts_with("SyncTeX result end") {
            break;
        } else if dentro {
            if let Some((chave, valor)) = linha.split_once(':') {
                if registro.iter().any(|(existente, _)| *existente == chave) {
                    break;
                }
                registro.push((chave, valor));
            }
        }
    }
    registro
}

fn campo<'a>(registro: &[(&'a str, &'a str)], chave: &str) -> Option<&'a str> {
    registro
        .iter()
        .find(|(existente, _)| *existente == chave)
        .map(|(_, valor)| valor.trim())
}

fn interpretar_view(saida: &str) -> Option<PosicaoPdf> {
    let registro = primeiro_registro(saida);
    let numero = |chave: &str| campo(&registro, chave).and_then(|v| v.parse::<f64>().ok());
    Some(PosicaoPdf {
        pagina: campo(&registro, "Page")?.parse().ok()?,
        // "h" e a esquerda da caixa de largura W; "x" e o ponto do no, ja dentro da linha
        x: numero("h").or_else(|| numero("x"))?,
        y: numero("y")?,
        largura: numero("W").unwrap_or(0.0),
        altura: numero("H").unwrap_or(0.0),
    })
}

/// Troca "\" por "/", tira os "." e resolve os ".." de um caminho, sem consultar o disco.
fn limpar_segmentos(caminho: &str) -> String {
    let barras = caminho.chars().take_while(|&c| c == '/').count().min(2);
    let mut saida: Vec<&str> = Vec::new();
    for segmento in caminho.split('/') {
        match segmento {
            "" | "." => {}
            ".." => match saida.last() {
                Some(&ultimo) if ultimo != ".." && !ultimo.ends_with(':') => {
                    saida.pop();
                }
                _ => saida.push(".."),
            },
            outro => saida.push(outro),
        }
    }
    format!("{}{}", "/".repeat(barras), saida.join("/"))
}

/// O synctex devolve caminhos como "C:/proj/./cap/intro.tex". Aqui viram
/// "C:/proj/cap/intro.tex"; se vierem relativos, ficam relativos a pasta do PDF.
fn normalizar_caminho_synctex(bruto: &str, pasta_pdf: Option<&Path>) -> String {
    let limpo = limpar_segmentos(&bruto.trim().replace('\\', "/"));
    let absoluto = limpo.starts_with('/') || limpo.as_bytes().get(1) == Some(&b':');
    match pasta_pdf {
        Some(pasta) if !absoluto && !pasta.as_os_str().is_empty() => {
            let pasta = pasta.display().to_string().replace('\\', "/");
            limpar_segmentos(&format!("{}/{}", pasta.trim_end_matches('/'), limpo))
        }
        _ => limpo,
    }
}

fn interpretar_edit(saida: &str, pasta_pdf: Option<&Path>) -> Option<PosicaoCodigo> {
    let registro = primeiro_registro(saida);
    let arquivo = campo(&registro, "Input").filter(|valor| !valor.is_empty())?;
    Some(PosicaoCodigo {
        arquivo: normalizar_caminho_synctex(arquivo, pasta_pdf),
        linha: campo(&registro, "Line")?.parse().ok()?,
        coluna: campo(&registro, "Column")
            .and_then(|valor| valor.parse().ok())
            .unwrap_or(-1),
    })
}

/// Verdadeiro se o programa `synctex` esta instalado e no PATH.
#[tauri::command]
pub fn cmd_synctex_disponivel() -> bool {
    localizar_executavel("synctex").is_ok()
}

/// Busca direta: posicao no PDF que corresponde a `linha`:`coluna` de `arquivo_tex`.
/// Devolve None se o SyncTeX nao achar nada.
#[tauri::command(async)]
pub fn cmd_synctex_ir_pdf(
    pdf: String,
    arquivo_tex: String,
    linha: u32,
    coluna: u32,
) -> Result<Option<PosicaoPdf>, String> {
    let executavel = localizar_executavel("synctex")?;
    conferir_dados_synctex(&pdf)?;
    let argumentos = [
        "view".to_string(),
        "-i".to_string(),
        format!("{}:{}:{}", linha, coluna, arquivo_tex),
        "-o".to_string(),
        pdf,
    ];
    let saida = rodar_synctex(&executavel, &argumentos)?;
    Ok(interpretar_view(&saida))
}

/// Busca inversa: arquivo, linha e coluna do codigo-fonte que gerou o ponto (`x`, `y`)
/// da `pagina` do PDF. Devolve None se o SyncTeX nao achar nada.
#[tauri::command(async)]
pub fn cmd_synctex_ir_codigo(
    pdf: String,
    pagina: u32,
    x: f64,
    y: f64,
) -> Result<Option<PosicaoCodigo>, String> {
    let executavel = localizar_executavel("synctex")?;
    if !x.is_finite() || !y.is_finite() {
        return Err("Posição inválida no PDF".to_string());
    }
    conferir_dados_synctex(&pdf)?;
    let argumentos = [
        "edit".to_string(),
        "-o".to_string(),
        format!("{}:{}:{}:{}", pagina, x, y, pdf),
    ];
    let saida = rodar_synctex(&executavel, &argumentos)?;
    Ok(interpretar_edit(&saida, Path::new(&pdf).parent()))
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::BTreeSet;

    /// Pasta temporaria que se apaga sozinha. `raiz/projeto` e o projeto: qualquer
    /// coisa criada em `raiz` fora de `projeto` seria um vazamento para fora dele.
    struct PastaTemp {
        raiz: PathBuf,
    }

    impl PastaTemp {
        fn nova(nome: &str) -> Self {
            let raiz = std::env::temp_dir().join(format!(
                "eguatex-teste-arquivos-{}-{}",
                std::process::id(),
                nome
            ));
            let _ = fs::remove_dir_all(&raiz);
            fs::create_dir_all(raiz.join("projeto")).expect("criar pasta de teste");
            PastaTemp { raiz }
        }

        fn projeto(&self) -> PathBuf {
            self.raiz.join("projeto")
        }

        fn projeto_txt(&self) -> String {
            self.projeto().display().to_string()
        }
    }

    impl Drop for PastaTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.raiz);
        }
    }

    fn escrever(caminho: &Path, conteudo: &str) {
        if let Some(pai) = caminho.parent() {
            fs::create_dir_all(pai).unwrap();
        }
        fs::write(caminho, conteudo).unwrap();
    }

    fn nomes_na_pasta(pasta: &Path) -> BTreeSet<String> {
        fs::read_dir(pasta)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn cria_arquivo_vazio_com_pastas_mae() {
        let t = PastaTemp::nova("criar-arquivo");
        let p = t.projeto_txt();

        cmd_criar_arquivo(p.clone(), "/main.tex".into()).unwrap();
        assert!(t.projeto().join("main.tex").is_file());

        // com barra inicial e pasta-mae que ainda nao existe
        cmd_criar_arquivo(p.clone(), "/sub/a.tex".into()).unwrap();
        assert_eq!(fs::metadata(t.projeto().join("sub").join("a.tex")).unwrap().len(), 0);

        // sem barra inicial e com separador do Windows
        cmd_criar_arquivo(p.clone(), "capitulos\\b\\c.tex".into()).unwrap();
        assert!(t.projeto().join("capitulos").join("b").join("c.tex").is_file());

        // ja existe: erro, e o conteudo nao e tocado
        escrever(&t.projeto().join("main.tex"), "conteudo");
        let erro = cmd_criar_arquivo(p.clone(), "/main.tex".into()).unwrap_err();
        assert!(erro.contains("Já existe"), "{erro}");
        assert_eq!(fs::read_to_string(t.projeto().join("main.tex")).unwrap(), "conteudo");

        // nomes que o Windows nao aceita
        for ruim in ["/a?.tex", "/con.tex", "/aux", "/termina.", "/sub/NUL.txt"] {
            assert!(cmd_criar_arquivo(p.clone(), ruim.into()).is_err(), "aceitou {ruim}");
        }
        assert!(cmd_criar_arquivo(p.clone(), "".into()).is_err());
        assert!(cmd_criar_arquivo("C:/pasta/que/nao/existe/eguatex".into(), "/a.tex".into()).is_err());
    }

    #[test]
    fn cria_pasta_e_recusa_repetida() {
        let t = PastaTemp::nova("criar-pasta");
        let p = t.projeto_txt();

        cmd_criar_pasta(p.clone(), "/figuras".into()).unwrap();
        assert!(t.projeto().join("figuras").is_dir());
        cmd_criar_pasta(p.clone(), "/a/b/c".into()).unwrap();
        assert!(t.projeto().join("a").join("b").join("c").is_dir());

        let erro = cmd_criar_pasta(p.clone(), "/figuras".into()).unwrap_err();
        assert!(erro.contains("Já existe"), "{erro}");
        // existe um arquivo com esse nome
        escrever(&t.projeto().join("main.tex"), "x");
        assert!(cmd_criar_pasta(p.clone(), "/main.tex".into()).is_err());
        assert!(cmd_criar_pasta(p, "/".into()).is_err());
    }

    #[test]
    fn recusa_caminhos_que_saem_do_projeto() {
        let t = PastaTemp::nova("fora");
        let p = t.projeto_txt();
        let externo = t.raiz.join("externo.txt");
        escrever(&externo, "não mexa");
        let ruins = [
            "../fora.tex",
            "/../fora.tex",
            "..\\fora.tex",
            "\\..\\fora.tex",
            "a/../../fora.tex",
            "sub/../../fora.tex",
            "C:/Windows/fora.tex",
            "C:\\fora.tex",
            "/C:/fora.tex",
            "C:fora.tex",
            "arquivo.tex:fluxo",
        ];
        for ruim in ruins {
            assert!(cmd_criar_arquivo(p.clone(), ruim.into()).is_err(), "criar_arquivo aceitou {ruim}");
            assert!(cmd_criar_pasta(p.clone(), ruim.into()).is_err(), "criar_pasta aceitou {ruim}");
            assert!(cmd_renomear_item(p.clone(), ruim.into(), "novo.tex".into()).is_err(), "renomear aceitou {ruim}");
            assert!(cmd_excluir_item(p.clone(), ruim.into()).is_err(), "excluir aceitou {ruim}");
            assert!(
                cmd_importar_arquivo(p.clone(), externo.display().to_string(), ruim.into()).is_err(),
                "importar aceitou {ruim}"
            );
            assert!(
                cmd_copiar_para(p.clone(), ruim.into(), t.raiz.join("saida.txt").display().to_string()).is_err(),
                "copiar_para aceitou {ruim}"
            );
        }
        // nada foi criado nem apagado fora do projeto
        assert_eq!(fs::read_to_string(&externo).unwrap(), "não mexa");
        assert_eq!(nomes_na_pasta(&t.raiz), BTreeSet::from(["externo.txt".to_string(), "projeto".to_string()]));
        assert_eq!(nomes_na_pasta(&t.projeto()).len(), 0);
        // o texto do erro explica o motivo
        let erro = cmd_criar_arquivo(p, "/../fora.tex".into()).unwrap_err();
        assert!(erro.contains("Caminho inválido"), "{erro}");
    }

    #[test]
    fn renomeia_e_devolve_caminho_no_formato_da_arvore() {
        let t = PastaTemp::nova("renomear");
        let p = t.projeto_txt();
        escrever(&t.projeto().join("sub").join("a.tex"), "ola");

        let novo = cmd_renomear_item(p.clone(), "/sub/a.tex".into(), "b.tex".into()).unwrap();
        assert_eq!(novo, "/sub/b.tex");
        assert!(!t.projeto().join("sub").join("a.tex").exists());
        assert_eq!(fs::read_to_string(t.projeto().join("sub").join("b.tex")).unwrap(), "ola");

        // pasta inteira, com espacos no nome novo (cortados nas pontas)
        let pasta = cmd_renomear_item(p.clone(), "/sub".into(), "  capitulos ".into()).unwrap();
        assert_eq!(pasta, "/capitulos");
        assert!(t.projeto().join("capitulos").join("b.tex").is_file());

        // so trocar maiusculas e minusculas e permitido
        let maiuscula = cmd_renomear_item(p.clone(), "/capitulos/b.tex".into(), "B.tex".into()).unwrap();
        assert_eq!(maiuscula, "/capitulos/B.tex");
        assert!(nomes_na_pasta(&t.projeto().join("capitulos")).contains("B.tex"));

        // mesmo nome: nada a fazer
        assert_eq!(
            cmd_renomear_item(p.clone(), "/capitulos/B.tex".into(), "B.tex".into()).unwrap(),
            "/capitulos/B.tex"
        );

        // destino ja existe
        escrever(&t.projeto().join("capitulos").join("c.tex"), "outro");
        let erro = cmd_renomear_item(p.clone(), "/capitulos/B.tex".into(), "c.tex".into()).unwrap_err();
        assert!(erro.contains("Já existe"), "{erro}");
        assert_eq!(fs::read_to_string(t.projeto().join("capitulos").join("c.tex")).unwrap(), "outro");

        // nomes invalidos
        for ruim in ["", "   ", "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", ".", "..", "nome.", "con"] {
            assert!(
                cmd_renomear_item(p.clone(), "/capitulos/c.tex".into(), ruim.into()).is_err(),
                "aceitou o nome {ruim:?}"
            );
        }
        // item que nao existe e a propria pasta do projeto
        assert!(cmd_renomear_item(p.clone(), "/nao-existe.tex".into(), "x.tex".into()).is_err());
        assert!(cmd_renomear_item(p.clone(), "/".into(), "x".into()).is_err());
        assert!(cmd_renomear_item(p, "".into(), "x".into()).is_err());
    }

    #[test]
    fn figura_do_computador_vai_para_figuras_e_repetido_vira_nome_2() {
        let t = PastaTemp::nova("figura");
        let p = t.projeto_txt();
        let foto = t.raiz.join("externo").join("foto.png");
        escrever(&foto, "png-1");
        let f = foto.display().to_string();

        // a pasta figuras/ nao existia: e criada
        assert!(!t.projeto().join("figuras").exists());
        assert_eq!(cmd_importar_figura(p.clone(), f.clone()).unwrap(), "figuras/foto.png");
        assert_eq!(cmd_importar_figura(p.clone(), f.clone()).unwrap(), "figuras/foto (2).png");
        assert_eq!(cmd_importar_figura(p.clone(), f).unwrap(), "figuras/foto (3).png");
        assert_eq!(fs::read_to_string(t.projeto().join("figuras").join("foto (2).png")).unwrap(), "png-1");

        // o envio comum de arquivos continua numerando a partir de (1)
        let o = t.raiz.join("externo").join("dado.txt");
        escrever(&o, "x");
        let o = o.display().to_string();
        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "".into()).unwrap(), "/dado.txt");
        assert_eq!(cmd_importar_arquivo(p, o, "".into()).unwrap(), "/dado (1).txt");
    }

    #[test]
    fn so_figuras_aceitas_entram_em_figuras() {
        for ok in ["a.png", "a.JPG", "a.jpeg", "a.pdf", "a.eps", "foto final.PNG"] {
            assert!(e_figura(ok), "{ok}");
        }
        for ruim in ["a.tex", "a.exe", "semextensao", "a.png.exe", "a.gif", "a.svg"] {
            assert!(!e_figura(ruim), "{ruim}");
        }
        let t = PastaTemp::nova("figura-ruim");
        let p = t.projeto_txt();
        let texto = t.raiz.join("externo").join("nota.txt");
        escrever(&texto, "x");
        assert!(cmd_importar_figura(p, texto.display().to_string()).is_err());
        assert!(!t.projeto().join("figuras").exists());
    }

    #[test]
    fn importa_com_nome_repetido_gera_numeracao() {
        let t = PastaTemp::nova("importar");
        let p = t.projeto_txt();
        let origem = t.raiz.join("externo").join("dado.txt");
        escrever(&origem, "conteudo");
        let o = origem.display().to_string();

        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "".into()).unwrap(), "/dado.txt");
        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "".into()).unwrap(), "/dado (1).txt");
        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "/".into()).unwrap(), "/dado (2).txt");
        assert_eq!(fs::read_to_string(t.projeto().join("dado (2).txt")).unwrap(), "conteudo");

        // numa subpasta (criada se faltar), com e sem barra inicial
        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "/figuras".into()).unwrap(), "/figuras/dado.txt");
        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "figuras/".into()).unwrap(), "/figuras/dado (1).txt");
        assert_eq!(cmd_importar_arquivo(p.clone(), o.clone(), "/a/b".into()).unwrap(), "/a/b/dado.txt");

        // sem extensao
        let sem_extensao = t.raiz.join("externo").join("LEIAME");
        escrever(&sem_extensao, "x");
        let s = sem_extensao.display().to_string();
        assert_eq!(cmd_importar_arquivo(p.clone(), s.clone(), "".into()).unwrap(), "/LEIAME");
        assert_eq!(cmd_importar_arquivo(p.clone(), s, "".into()).unwrap(), "/LEIAME (1)");

        // o arquivo de origem nao e tocado; origem inexistente ou pasta dao erro
        assert_eq!(fs::read_to_string(&origem).unwrap(), "conteudo");
        assert!(cmd_importar_arquivo(p.clone(), t.raiz.join("nao-existe.txt").display().to_string(), "".into()).is_err());
        assert!(cmd_importar_arquivo(p, t.raiz.join("externo").display().to_string(), "".into()).is_err());
    }

    #[test]
    fn exclui_arquivo_e_pasta_e_protege_a_raiz() {
        let t = PastaTemp::nova("excluir");
        let p = t.projeto_txt();
        escrever(&t.projeto().join("raiz.tex"), "x");
        escrever(&t.projeto().join("sub").join("x").join("a.tex"), "x");
        escrever(&t.projeto().join("sub").join("b.tex"), "x");

        cmd_excluir_item(p.clone(), "/sub/b.tex".into()).unwrap();
        assert!(!t.projeto().join("sub").join("b.tex").exists());
        cmd_excluir_item(p.clone(), "/sub".into()).unwrap();
        assert!(!t.projeto().join("sub").exists());

        // somente leitura tambem e apagado
        let leitura = t.projeto().join("leitura.tex");
        escrever(&leitura, "x");
        let mut permissoes = fs::metadata(&leitura).unwrap().permissions();
        permissoes.set_readonly(true);
        fs::set_permissions(&leitura, permissoes).unwrap();
        cmd_excluir_item(p.clone(), "/leitura.tex".into()).unwrap();
        assert!(!leitura.exists());

        // a pasta do projeto nunca e apagada
        for raiz in ["", "/", "\\", ".", "/.", "./", "//", "/./"] {
            assert!(cmd_excluir_item(p.clone(), raiz.into()).is_err(), "apagou a raiz com {raiz:?}");
        }
        assert!(t.projeto().join("raiz.tex").is_file());
        assert!(cmd_excluir_item(p, "/nao-existe.tex".into()).is_err());
    }

    #[test]
    fn copia_arquivo_do_projeto_para_destino_absoluto() {
        let t = PastaTemp::nova("copiar-para");
        let p = t.projeto_txt();
        escrever(&t.projeto().join("sub").join("a.tex"), "ola");
        fs::create_dir_all(t.raiz.join("externo")).unwrap();
        let destino = t.raiz.join("externo").join("copia.tex");

        cmd_copiar_para(p.clone(), "/sub/a.tex".into(), destino.display().to_string()).unwrap();
        assert_eq!(fs::read_to_string(&destino).unwrap(), "ola");
        assert_eq!(fs::read_to_string(t.projeto().join("sub").join("a.tex")).unwrap(), "ola");

        // destino que ja existe e sobrescrito
        escrever(&t.projeto().join("sub").join("a.tex"), "novo");
        cmd_copiar_para(p.clone(), "/sub/a.tex".into(), destino.display().to_string()).unwrap();
        assert_eq!(fs::read_to_string(&destino).unwrap(), "novo");

        // copiar o arquivo sobre ele mesmo nao estraga nada
        let proprio = t.projeto().join("sub").join("a.tex");
        cmd_copiar_para(p.clone(), "/sub/a.tex".into(), proprio.display().to_string()).unwrap();
        assert_eq!(fs::read_to_string(&proprio).unwrap(), "novo");

        // erros: origem inexistente, origem e pasta, destino relativo, destino e pasta,
        // pasta de destino inexistente
        let d = destino.display().to_string();
        assert!(cmd_copiar_para(p.clone(), "/nao-existe.tex".into(), d.clone()).is_err());
        assert!(cmd_copiar_para(p.clone(), "/sub".into(), d.clone()).is_err());
        assert!(cmd_copiar_para(p.clone(), "/".into(), d).is_err());
        assert!(cmd_copiar_para(p.clone(), "/sub/a.tex".into(), "copia.tex".into()).is_err());
        assert!(cmd_copiar_para(p.clone(), "/sub/a.tex".into(), t.raiz.join("externo").display().to_string()).is_err());
        let sem_pasta = t.raiz.join("nao").join("existe.tex").display().to_string();
        assert!(cmd_copiar_para(p, "/sub/a.tex".into(), sem_pasta).is_err());
    }

    #[test]
    fn copia_projeto_para_pasta_irma() {
        let t = PastaTemp::nova("copiar-projeto");
        let p = t.projeto_txt();
        escrever(&t.projeto().join("main.tex"), "principal");
        escrever(&t.projeto().join("sub").join("a.tex"), "sub");
        escrever(&t.projeto().join(".oculta").join("x.txt"), "oculta");
        fs::create_dir_all(t.projeto().join("vazia")).unwrap();

        let c1 = cmd_copiar_projeto(p.clone()).unwrap();
        assert_eq!(PathBuf::from(&c1), t.raiz.join("projeto (copia)"));
        let c2 = cmd_copiar_projeto(p.clone()).unwrap();
        assert_eq!(PathBuf::from(&c2), t.raiz.join("projeto (copia 2)"));
        let c3 = cmd_copiar_projeto(p.clone()).unwrap();
        assert_eq!(PathBuf::from(&c3), t.raiz.join("projeto (copia 3)"));

        let copia = PathBuf::from(c1);
        assert_eq!(fs::read_to_string(copia.join("main.tex")).unwrap(), "principal");
        assert_eq!(fs::read_to_string(copia.join("sub").join("a.tex")).unwrap(), "sub");
        assert_eq!(fs::read_to_string(copia.join(".oculta").join("x.txt")).unwrap(), "oculta");
        assert!(copia.join("vazia").is_dir());
        // o original continua igual
        assert_eq!(fs::read_to_string(t.projeto().join("main.tex")).unwrap(), "principal");
        assert!(cmd_copiar_projeto(t.raiz.join("nao-existe").display().to_string()).is_err());
    }

    #[test]
    fn limpa_auxiliares_sem_entrar_em_pastas_ocultas() {
        let t = PastaTemp::nova("limpar");
        let p = t.projeto_txt();
        let auxiliares = [
            "main.aux", "main.log", "main.toc", "main.out", "main.bbl", "main.blg",
            "main.synctex.gz", "main.fls", "main.fdb_latexmk", "main.lof", "main.lot",
            "main.nav", "main.snm", "main.vrb", "main.xdv", "sub/cap.aux", "sub/fundo/x.log",
            "MAIÚSCULA.AUX",
        ];
        let mantidos = [
            "main.tex", "main.pdf", "sub/cap.tex", "figs/a.png", "dados.csv",
            "main.synctex", "main.aux.bak", ".git/modulo.aux", ".oculta/x.log",
            // radicais dos .tex que geram os auxiliares das subpastas e da maiuscula
            "sub/fundo/x.tex", "MAIÚSCULA.tex",
            // .log e .out sem .tex de mesmo nome sao do usuario (saida de instrumento)
            "dados.log", "scf.out", "sub/espectro.log",
        ];
        for nome in auxiliares.iter().chain(mantidos.iter()) {
            escrever(&t.projeto().join(nome), "x");
        }
        let apagados = cmd_limpar_auxiliares(p.clone()).unwrap();
        assert_eq!(apagados as usize, auxiliares.len());
        for nome in auxiliares {
            assert!(!t.projeto().join(nome).exists(), "{nome} não foi apagado");
        }
        for nome in mantidos {
            assert!(t.projeto().join(nome).is_file(), "{nome} foi apagado sem querer");
        }
        // segunda rodada: nada a apagar
        assert_eq!(cmd_limpar_auxiliares(p).unwrap(), 0);
        assert!(cmd_limpar_auxiliares(t.raiz.join("nao-existe").display().to_string()).is_err());
    }

    fn nomes_no_zip(caminho: &Path) -> BTreeSet<String> {
        let arquivo = File::open(caminho).unwrap();
        let zip = zip::ZipArchive::new(arquivo).unwrap();
        zip.file_names().map(str::to_string).collect()
    }

    #[test]
    fn importa_zip_e_tira_a_pasta_unica_do_topo() {
        let t = PastaTemp::nova("importa");
        let p = t.projeto_txt();
        escrever(&t.projeto().join("modelo/main.tex"), "\\documentclass{article}");
        escrever(&t.projeto().join("modelo/estilo.sty"), "% sty");
        let zip_unico = t.raiz.join("Artigo SBC.zip");
        cmd_exportar_zip(p.clone(), zip_unico.display().to_string()).unwrap();

        let pai = t.raiz.join("destino");
        let pasta = PathBuf::from(cmd_importar_zip(zip_unico.display().to_string(), pai.display().to_string()).unwrap().caminho);
        assert_eq!(pasta, pai.join("Artigo SBC"));
        assert!(pasta.join("main.tex").is_file());
        assert!(pasta.join("estilo.sty").is_file());

        // o mesmo zip de novo reabre a pasta (o PDF e os auxiliares gerados depois nao contam)
        fs::write(pasta.join("main.pdf"), "pdf").unwrap();
        let mesma = PathBuf::from(cmd_importar_zip(zip_unico.display().to_string(), pai.display().to_string()).unwrap().caminho);
        assert_eq!(mesma, pai.join("Artigo SBC"));
        // com um arquivo do zip editado na pasta, ganha outro nome e nao apaga a primeira
        fs::write(pasta.join("main.tex"), "editado").unwrap();
        let outra = PathBuf::from(cmd_importar_zip(zip_unico.display().to_string(), pai.display().to_string()).unwrap().caminho);
        assert_eq!(outra, pai.join("Artigo SBC (2)"));
        assert_eq!(fs::read_to_string(pasta.join("main.tex")).unwrap(), "editado");

        // com arquivos na raiz do zip, a estrutura fica igual
        escrever(&t.projeto().join("raiz.tex"), "x");
        let zip_misto = t.raiz.join("misto.zip");
        cmd_exportar_zip(p, zip_misto.display().to_string()).unwrap();
        let misto = PathBuf::from(cmd_importar_zip(zip_misto.display().to_string(), pai.display().to_string()).unwrap().caminho);
        assert!(misto.join("raiz.tex").is_file());
        assert!(misto.join("modelo/main.tex").is_file());

        escrever(&t.raiz.join("falso.zip"), "nao sou zip");
        assert!(cmd_importar_zip(t.raiz.join("falso.zip").display().to_string(), pai.display().to_string()).is_err());
    }

    #[test]
    fn prefixo_do_embrulho_so_conta_pastas() {
        assert_eq!(prefixo_embrulho(&["m/T/t/x.tex", "m/T/y.txt"]), "m/T");
        assert_eq!(prefixo_embrulho(&["x.tex", "a/y"]), "");
        assert_eq!(prefixo_embrulho(&["a/x.tex"]), "a");
        assert_eq!(prefixo_embrulho(&["a/", "a/b/", "a/b/x.tex"]), "a/b");
        assert_eq!(prefixo_embrulho(&["a\\b\\x.tex", "a\\b\\c\\y.tex"]), "a/b");
        assert_eq!(prefixo_embrulho(&["a/x.tex", "b/y.tex"]), "");
        assert_eq!(prefixo_embrulho::<&str>(&[]), "");
    }

    #[test]
    fn principal_ignora_comentario_prefere_main_e_o_mais_raso() {
        let doc = "\\documentclass{article}\n\\begin{document}x\\end{document}";
        let so_comentario = "%\\documentclass{article}\n  % \\documentclass{book}\nTexto";
        let escapado = "100\\% \\documentclass{article}";
        assert!(tem_documentclass(doc));
        assert!(!tem_documentclass(so_comentario));
        assert!(tem_documentclass(escapado));
        assert!(!tem_documentclass("\\documentclassx{a}"));
        // ignora o comentado
        let a = vec![("a.tex".to_string(), so_comentario), ("b.tex".to_string(), doc)];
        assert_eq!(principal_entre(&a, "").as_deref(), Some("b.tex"));
        // prefere main.tex, mesmo mais fundo
        let b = vec![("raiz.tex".to_string(), doc), ("sub/fundo/main.tex".to_string(), doc)];
        assert_eq!(principal_entre(&b, "").as_deref(), Some("sub/fundo/main.tex"));
        // sem main.tex e do mesmo tamanho: o mais raso, depois a ordem alfabetica
        let c = vec![("x/y/z.tex".to_string(), doc), ("x/b.tex".to_string(), doc), ("x/a.tex".to_string(), doc)];
        assert_eq!(principal_entre(&c, "").as_deref(), Some("x/a.tex"));
        // so arquivos que nao sao .tex ou sem \begin{document}: nada
        let d = vec![("a.sty".to_string(), doc), ("b.tex".to_string(), "texto")];
        assert_eq!(principal_entre(&d, ""), None);
    }

    /// Caso real do TU Berlin: `poster.tex` faz `\input{00-preambel}` e tem o `\begin{document}`; o
    /// `\documentclass` fica no preambulo; `bilder/cgam-diagramm.tex` e uma figura standalone.
    #[test]
    fn principal_e_quem_tem_begin_document_como_no_overleaf() {
        let preambulo = "% !TeX encoding = UTF-8\n\\documentclass[a1paper,portrait]{baposter}\n\\usepackage{arev}\n";
        let poster = "\\input{00-preambel}\n\n\\begin{document}\n\\begin{poster}{}\n\\end{poster}\n\\end{document}\n";
        let figura = "\\documentclass[crop=true,border={0pt 0pt 0pt 0pt}]{standalone}\n\\begin{document}\nx\n\\end{document}\n";
        let tu = vec![
            ("00-preambel.tex".to_string(), preambulo),
            ("bilder/cgam-diagramm.tex".to_string(), figura),
            ("bilder/tikz-cgam-diagramm.tex".to_string(), "\\begin{tikzpicture}\\end{tikzpicture}"),
            ("poster.tex".to_string(), poster),
        ];
        assert_eq!(principal_entre(&tu, "TU_Berlin").as_deref(), Some("poster.tex"));
        // \begin{document} comentado nao conta
        assert!(!tem_begin_document("% \\begin{document}\n"));
        assert!(tem_begin_document("  \\begin {document}\n"));
        // com \documentclass no proprio arquivo vence quem so tem \begin{document}
        let completo = "\\documentclass{article}\n\\begin{document}x\\end{document}";
        let v = vec![("a.tex".to_string(), poster), ("b.tex".to_string(), completo)];
        assert_eq!(principal_entre(&v, "").as_deref(), Some("b.tex"));
        // empate: main.tex, depois o nome da pasta, depois o maior
        let maior = "\\documentclass{article}\n\\begin{document}texto bem maior que o outro\\end{document}";
        let v = vec![("tese.tex".to_string(), completo), ("z.tex".to_string(), maior)];
        assert_eq!(principal_entre(&v, "Tese").as_deref(), Some("tese.tex"));
        assert_eq!(principal_entre(&v, "outra").as_deref(), Some("z.tex"));
        let v = vec![("main.tex".to_string(), completo), ("tese.tex".to_string(), maior)];
        assert_eq!(principal_entre(&v, "tese").as_deref(), Some("main.tex"));
    }

    #[test]
    fn principal_do_projeto_le_o_disco() {
        let t = PastaTemp::nova("principal-disco");
        let raiz = t.raiz.join("TU_Berlin");
        fs::create_dir_all(raiz.join("bilder")).unwrap();
        fs::write(raiz.join("00-preambel.tex"), "\\documentclass{baposter}\n").unwrap();
        fs::write(raiz.join("poster.tex"), "\\input{00-preambel}\n\\begin{document}\n\\end{document}\n").unwrap();
        fs::write(raiz.join("bilder/cgam-diagramm.tex"), "\\documentclass{standalone}\n\\begin{document}\n\\end{document}\n").unwrap();
        assert_eq!(principal_do_projeto(&raiz).as_deref(), Some("poster.tex"));
        assert_eq!(cmd_detectar_principal(raiz.join("bilder").display().to_string()).as_deref(), Some("cgam-diagramm.tex"));
    }

    /// Uma entrada com ".." ou absoluta na lista nao e lida fora da pasta: so as outras contam.
    #[test]
    fn pasta_tem_o_zip_ignora_entradas_que_saem_da_pasta() {
        let t = PastaTemp::nova("zip-fora");
        let zip_caminho = t.raiz.join("a.zip");
        let mut z = zip::ZipWriter::new(File::create(&zip_caminho).unwrap());
        z.start_file("a.tex", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"oi").unwrap();
        z.finish().unwrap();
        let pasta = t.raiz.join("proj");
        fs::create_dir_all(&pasta).unwrap();
        fs::write(pasta.join("a.tex"), "oi").unwrap();
        let mut zip = zip::ZipArchive::new(File::open(&zip_caminho).unwrap()).unwrap();
        let absoluto = t.raiz.join("sumiu.tex");
        let lista = vec![
            (0, PathBuf::from("a.tex"), false),
            (0, PathBuf::from("../sumiu.tex"), false),
            (0, absoluto, false),
        ];
        assert!(pasta_tem_o_zip(&mut zip, &lista, None, &pasta));
        // a entrada normal continua valendo
        fs::write(pasta.join("a.tex"), "outro").unwrap();
        assert!(!pasta_tem_o_zip(&mut zip, &lista, None, &pasta));
    }

    #[test]
    fn importa_zip_tira_pastas_aninhadas() {
        let t = PastaTemp::nova("importa-aninhado");
        let zip_caminho = t.raiz.join("124.zip");
        let mut z = zip::ZipWriter::new(File::create(&zip_caminho).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        for (nome, txt) in [
            ("modelos/Template_SBC/template-latex/sbc-template.tex", "\\documentclass{article}"),
            ("modelos/Template_SBC/template-latex/sbc.sty", "% sty"),
            ("modelos/Template_SBC/sbc_template.pdf", "pdf"),
        ] {
            z.start_file(nome, o).unwrap();
            z.write_all(txt.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        let pai = t.raiz.join("destino");
        let pasta = PathBuf::from(cmd_importar_zip(zip_caminho.display().to_string(), pai.display().to_string()).unwrap().caminho);
        assert_eq!(pasta, pai.join("124"));
        assert!(pasta.join("template-latex/sbc-template.tex").is_file());
        assert!(pasta.join("template-latex/sbc.sty").is_file());
        assert!(pasta.join("sbc_template.pdf").is_file());
        assert!(!pasta.join("modelos").exists());
    }

    #[test]
    fn exporta_zip_sem_auxiliares_nem_pdf_gerado() {
        let t = PastaTemp::nova("zip");
        let p = t.projeto_txt();
        let arquivos = [
            ("main.tex", "principal"),
            ("main.pdf", "gerado"),
            ("main.aux", "aux"),
            ("main.log", "log"),
            ("main.synctex.gz", "sync"),
            ("solta.pdf", "pdf sem tex"),
            ("figs/a.png", "imagem"),
            ("sub/cap.tex", "capitulo"),
            ("sub/cap.pdf", "gerado"),
            ("sub/extra.pdf", "pdf de outra coisa"),
            ("sub/Tese.TEX", "maiuscula"),
            ("sub/tese.PDF", "gerado maiúsculo"),
            ("acentuacao/F\u{ed}sica.tex", "acento"),
            (".git/config", "oculto"),
        ];
        for (nome, conteudo) in arquivos {
            escrever(&t.projeto().join(nome), conteudo);
        }
        fs::create_dir_all(t.projeto().join("vazia")).unwrap();

        let destino = t.raiz.join("saida.zip");
        cmd_exportar_zip(p.clone(), destino.display().to_string()).unwrap();
        let esperado: BTreeSet<String> = [
            "main.tex",
            "solta.pdf",
            "figs/a.png",
            "sub/cap.tex",
            "sub/extra.pdf",
            "sub/Tese.TEX",
            "acentuacao/F\u{ed}sica.tex",
            "vazia/",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(nomes_no_zip(&destino), esperado);

        // o conteudo bate
        let mut zip = zip::ZipArchive::new(File::open(&destino).unwrap()).unwrap();
        let mut texto = String::new();
        io::Read::read_to_string(&mut zip.by_name("sub/cap.tex").unwrap(), &mut texto).unwrap();
        assert_eq!(texto, "capitulo");
        // nao sobra arquivo temporario
        assert!(!t.raiz.join("saida.zip.parte").exists());

        // exportar para dentro do projeto nao coloca o zip dentro dele mesmo
        let dentro = t.projeto().join("backup.zip");
        cmd_exportar_zip(p.clone(), dentro.display().to_string()).unwrap();
        cmd_exportar_zip(p.clone(), dentro.display().to_string()).unwrap();
        let nomes = nomes_no_zip(&dentro);
        assert!(nomes.contains("main.tex"));
        assert!(!nomes.contains("backup.zip"), "o zip entrou nele mesmo: {nomes:?}");

        // erros: pasta de destino inexistente, destino e pasta, projeto inexistente
        assert!(cmd_exportar_zip(p.clone(), t.raiz.join("nao").join("existe.zip").display().to_string()).is_err());
        assert!(cmd_exportar_zip(p.clone(), t.raiz.display().to_string()).is_err());
        assert!(cmd_exportar_zip(p, "".into()).is_err());
        assert!(cmd_exportar_zip(t.raiz.join("nao-existe").display().to_string(), destino.display().to_string()).is_err());
    }

    #[test]
    fn valida_nomes_do_windows() {
        for bom in ["main.tex", "capítulo 1.tex", ".gitignore", "a-b_c.tex", "F\u{ed}sica", "console.tex", "com10.txt", "lpt.txt"] {
            assert!(validar_nome(bom).is_ok(), "recusou {bom}");
        }
        for ruim in ["", ".", "..", "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "x.", "x ", "CON", "con.tex", "Aux", "NUL.txt", "COM1", "com9.log", "LPT3.tex", "a\u{7}b"] {
            assert!(validar_nome(ruim).is_err(), "aceitou {ruim:?}");
        }
    }

    #[test]
    fn validar_nome_recusa_hifen_inicial() {
        // "--lua-filter=x.tex" viraria opcao do Pandoc; "-x" vira opcao de qualquer programa
        for ruim in ["-x.tex", "--lua-filter=x.tex", "-", "--", "-rf"] {
            let erro = validar_nome(ruim).expect_err(ruim);
            assert!(erro.contains("hífen"), "a mensagem deve explicar o hifen: {erro}");
        }
        for bom in ["a-b.tex", "x-", "capitulo-1.tex", "a--b", "_-x", ".-x"] {
            assert!(validar_nome(bom).is_ok(), "recusou {bom}");
        }
        assert!(partes_de_item_novo("capitulos/-x.tex").is_err());
        assert!(partes_de_item_novo("-pasta/x.tex").is_err());
    }

    #[test]
    fn divide_caminhos_removendo_as_barras_do_inicio() {
        assert_eq!(dividir_caminho("/sub/a.tex").unwrap(), ["sub", "a.tex"]);
        assert_eq!(dividir_caminho("\\sub\\a.tex").unwrap(), ["sub", "a.tex"]);
        assert_eq!(dividir_caminho("//sub//./a.tex").unwrap(), ["sub", "a.tex"]);
        assert_eq!(dividir_caminho("a.tex").unwrap(), ["a.tex"]);
        assert!(dividir_caminho("").unwrap().is_empty());
        assert!(dividir_caminho("/").unwrap().is_empty());
        assert!(dividir_caminho("/./").unwrap().is_empty());
        for ruim in ["/../x", "..", "a/..", "a\\..\\b", "C:/x", "/C:/x", "//C:/x", "x:y"] {
            assert!(dividir_caminho(ruim).is_err(), "aceitou {ruim}");
        }
        assert_eq!(montar_relativo(&["sub".to_string(), "a.tex".to_string()]), "/sub/a.tex");
    }

    #[test]
    fn nao_segue_links_para_fora_do_projeto() {
        // `garantir_dentro` com um ancestral que resolve para fora do projeto
        let t = PastaTemp::nova("link");
        let fora = t.raiz.join("fora");
        fs::create_dir_all(&fora).unwrap();
        assert!(garantir_dentro(&t.projeto(), &t.projeto().join("a").join("b")).is_ok());
        assert!(garantir_dentro(&t.projeto(), &t.projeto()).is_ok());
        assert!(garantir_dentro(&t.projeto(), &fora).is_err());
        assert!(garantir_dentro(&t.projeto(), &fora.join("nova")).is_err());

        // juncao (Windows) ou link simbolico (outros sistemas) apontando para fora
        #[cfg(windows)]
        let criado = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(t.projeto().join("atalho"))
            .arg(&fora)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        #[cfg(unix)]
        let criado = std::os::unix::fs::symlink(&fora, t.projeto().join("atalho")).is_ok();
        #[cfg(not(any(windows, unix)))]
        let criado = false;
        if !criado {
            eprintln!("não foi possível criar o link de teste; parte do teste pulada");
            return;
        }
        let p = t.projeto_txt();
        assert!(cmd_criar_arquivo(p.clone(), "/atalho/vazou.tex".into()).is_err());
        assert!(cmd_criar_pasta(p.clone(), "/atalho/vazou".into()).is_err());
        assert!(cmd_importar_arquivo(p.clone(), t.raiz.join("fora").display().to_string(), "/atalho".into()).is_err());
        assert!(!fora.join("vazou.tex").exists());
        // dentro do link nada pode ser apagado nem renomeado
        escrever(&fora.join("importante.txt"), "fica");
        assert!(cmd_excluir_item(p.clone(), "/atalho/importante.txt".into()).is_err());
        assert!(cmd_renomear_item(p.clone(), "/atalho/importante.txt".into(), "x.txt".into()).is_err());
        assert!(fora.join("importante.txt").is_file());
        // o proprio link pode ser excluido, e o que ele aponta continua la
        cmd_excluir_item(p, "/atalho".into()).unwrap();
        assert!(!t.projeto().join("atalho").exists());
        assert!(fora.join("importante.txt").is_file());
    }

    #[test]
    fn interpreta_a_saida_do_synctex_view() {
        let saida = "This is SyncTeX command line utility, version 1.5\r\n\
SyncTeX result begin\r\n\
Output:C:\\Pasta com espaco\\main.pdf\r\n\
Page:2\r\nx:185.242096\r\ny:146.719788\r\nh:133.768356\r\nv:148.656967\r\nW:343.711060\r\nH:8.855677\r\n\
before:\r\noffset:-1\r\nmiddle:\r\nafter:\r\n\
Output:C:\\Pasta com espaco\\main.pdf\r\n\
Page:3\r\nx:1.0\r\ny:2.0\r\nW:3.0\r\nH:4.0\r\n\
SyncTeX result end\r\n";
        assert_eq!(
            interpretar_view(saida),
            Some(PosicaoPdf { pagina: 2, x: 133.768356, y: 146.719788, largura: 343.71106, altura: 8.855677 })
        );
        // so o primeiro bloco, mesmo sem "Output:" no meio
        let sem_output = "SyncTeX result begin\nPage:4\nx:1\ny:2\nW:3\nH:4\nPage:5\nx:9\ny:9\nW:9\nH:9\nSyncTeX result end\n";
        assert_eq!(
            interpretar_view(sem_output),
            Some(PosicaoPdf { pagina: 4, x: 1.0, y: 2.0, largura: 3.0, altura: 4.0 })
        );
        // sem resultado
        assert_eq!(interpretar_view("This is SyncTeX command line utility, version 1.5\r\n"), None);
        assert_eq!(interpretar_view("This is SyncTeX\nSyncTeX Warning: No tag for C:\\x\\a.tex\n"), None);
        assert_eq!(interpretar_view("SyncTeX result begin\nSyncTeX result end\n"), None);
        // texto fora do bloco e ignorado
        assert_eq!(interpretar_view("Page:1\nx:1\ny:2\n"), None);
    }

    #[test]
    fn interpreta_a_saida_do_synctex_edit() {
        let saida = "This is SyncTeX command line utility, version 1.5\r\n\
SyncTeX result begin\r\n\
Output:C:\\Users\\Usuario Teste\\proj\\main.pdf\r\n\
Input:C:/Users/Usuario Teste/proj/./cap/intro.tex\r\n\
Line:3\r\nColumn:-1\r\nOffset:0\r\nContext:\r\n\
SyncTeX result end\r\n";
        assert_eq!(
            interpretar_edit(saida, Some(Path::new("C:\\Users\\Usuario Teste\\proj"))),
            Some(PosicaoCodigo { arquivo: "C:/Users/Usuario Teste/proj/cap/intro.tex".to_string(), linha: 3, coluna: -1 })
        );
        // com coluna conhecida
        let com_coluna = "SyncTeX result begin\nOutput:x.pdf\nInput:/home/u/main.tex\nLine:12\nColumn:7\nSyncTeX result end\n";
        assert_eq!(
            interpretar_edit(com_coluna, None),
            Some(PosicaoCodigo { arquivo: "/home/u/main.tex".to_string(), linha: 12, coluna: 7 })
        );
        // relativo: fica relativo a pasta do PDF
        let relativo = "SyncTeX result begin\nOutput:x.pdf\nInput:./cap/../main.tex\nLine:1\nColumn:-1\nSyncTeX result end\n";
        assert_eq!(
            interpretar_edit(relativo, Some(Path::new("C:\\proj"))).map(|p| p.arquivo),
            Some("C:/proj/main.tex".to_string())
        );
        // sem resultado ou sem a linha
        assert_eq!(interpretar_edit("SyncTeX result begin\nSyncTeX result end\n", None), None);
        assert_eq!(interpretar_edit("SyncTeX result begin\nInput:a.tex\nSyncTeX result end\n", None), None);
    }

    #[test]
    fn normaliza_caminhos_do_synctex() {
        assert_eq!(normalizar_caminho_synctex("C:/a/./b/../c.tex", None), "C:/a/c.tex");
        assert_eq!(normalizar_caminho_synctex("C:\\a\\b.tex", None), "C:/a/b.tex");
        assert_eq!(normalizar_caminho_synctex("//srv/share/./f.tex", None), "//srv/share/f.tex");
        assert_eq!(normalizar_caminho_synctex("./x.tex", Some(Path::new("C:/p"))), "C:/p/x.tex");
        assert_eq!(normalizar_caminho_synctex("x.tex", Some(Path::new(""))), "x.tex");
        assert_eq!(normalizar_caminho_synctex("C:/a/x.tex", Some(Path::new("D:/p"))), "C:/a/x.tex");
    }

    #[test]
    fn decodifica_a_saida_em_utf8_e_em_ansi() {
        assert_eq!(decodificar_saida("F\u{ed}sica".as_bytes()), "F\u{ed}sica");
        assert_eq!(decodificar_saida(b""), "");
        // bytes que nao sao UTF-8 (nomes na pagina de codigo ANSI): sem perder letras
        let ansi = decodificar_saida(b"F\xedsica & Ci\xeancia");
        assert!(!ansi.contains('\u{fffd}'), "{ansi}");
        assert!(ansi.starts_with('F') && ansi.ends_with("ncia") && ansi.contains("sica & Ci"), "{ansi}");
    }

    #[test]
    fn programa_ausente_devolve_erro_combinado() {
        assert_eq!(
            localizar_executavel("programa-que-nao-existe-eguatex"),
            Err("programa-que-nao-existe-eguatex não encontrado".to_string())
        );
        assert_eq!(cmd_synctex_disponivel(), which::which("synctex").is_ok());
    }

    #[test]
    fn synctex_recusa_pdf_ou_dados_ausentes() {
        if !cmd_synctex_disponivel() {
            eprintln!("synctex não instalado: teste pulado");
            return;
        }
        let t = PastaTemp::nova("synctex-ausente");
        let pdf = t.projeto().join("main.pdf");
        let erro = cmd_synctex_ir_pdf(pdf.display().to_string(), "main.tex".into(), 1, 1).unwrap_err();
        assert!(erro.contains("PDF não encontrado"), "{erro}");
        escrever(&pdf, "%PDF-1.4");
        let erro = cmd_synctex_ir_codigo(pdf.display().to_string(), 1, 10.0, 10.0).unwrap_err();
        assert!(erro.contains("SyncTeX não encontrados"), "{erro}");
        assert!(cmd_synctex_ir_codigo(pdf.display().to_string(), 1, f64::NAN, 10.0).is_err());
    }

    /// Compila de verdade (pdflatex + `-synctex=1` do compilador.rs) numa pasta com
    /// acento no nome e confere a busca direta e a inversa. Pulado se o TeX nao estiver
    /// instalado.
    #[test]
    fn synctex_ponta_a_ponta_com_tex_instalado() {
        if !cmd_synctex_disponivel() || which::which("pdflatex").is_err() {
            eprintln!("TeX ou synctex não instalado: teste de integração pulado");
            return;
        }
        let t = PastaTemp::nova("synctex-ponta-a-ponta");
        let projeto = t.raiz.join("F\u{ed}sica & Ci\u{ea}ncia");
        escrever(
            &projeto.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\nPrimeira linha do texto principal.\n\n\
Segundo paragrafo com mais palavras para gerar caixas.\n\n\\input{cap/intro}\n\n\
Ultimo paragrafo.\n\\end{document}\n",
        );
        escrever(&projeto.join("cap").join("intro.tex"), "Texto do capítulo.\n\nMais um parágrafo do capítulo.\n");

        // segura a trava dos testes que compilam: outro teste pode interromper compilacoes
        let _vez = crate::compilador::travar_teste();
        let resultado =
            crate::compilador::compilar("main.tex", &projeto.display().to_string(), "pdflatex", true);
        assert!(resultado.sucesso, "{}", resultado.log);
        let pdf = resultado.pdf.expect("caminho do PDF");
        assert!(
            projeto.join("main.synctex.gz").is_file(),
            "o compilador não gerou o .synctex.gz (falta -synctex=1)"
        );

        let tex = projeto.join("main.tex").display().to_string();
        let posicao = cmd_synctex_ir_pdf(pdf.clone(), tex.clone(), 5, 1)
            .unwrap()
            .expect("o SyncTeX devia achar a linha 5");
        assert_eq!(posicao.pagina, 1);
        assert!(posicao.largura > 0.0 && posicao.y > 0.0, "{posicao:?}");

        // do PDF de volta para o codigo: mesma linha, no mesmo arquivo, sem "/./"
        let codigo = cmd_synctex_ir_codigo(pdf.clone(), posicao.pagina, posicao.x + 1.0, posicao.y)
            .unwrap()
            .expect("o SyncTeX devia achar o código");
        assert_eq!(codigo.linha, 5);
        assert!(!codigo.arquivo.contains("/./"), "{}", codigo.arquivo);
        // `mesmo_item` resolve os dois caminhos no disco: so da certo se os acentos
        // do caminho que veio do synctex foram decodificados direito
        let mesmo_arquivo = |a: &str, b: &str| mesmo_item(Path::new(a), Path::new(b));
        assert!(mesmo_arquivo(&codigo.arquivo, &tex), "caminho diferente: {}", codigo.arquivo);

        // arquivo incluido por \input
        let intro = projeto.join("cap").join("intro.tex").display().to_string();
        let posicao = cmd_synctex_ir_pdf(pdf.clone(), intro.clone(), 3, 1).unwrap().expect("linha do capítulo");
        let codigo = cmd_synctex_ir_codigo(pdf.clone(), posicao.pagina, posicao.x + 1.0, posicao.y)
            .unwrap()
            .expect("código do capítulo");
        assert!(mesmo_arquivo(&codigo.arquivo, &intro), "caminho diferente: {}", codigo.arquivo);
        assert_eq!(codigo.linha, 3);

        // o synctex tambem entende o caminho no formato da arvore ("/cap/intro.tex")
        let da_arvore = cmd_synctex_ir_pdf(pdf.clone(), "/cap/intro.tex".into(), 3, 1).unwrap();
        assert_eq!(da_arvore.map(|p| p.pagina), Some(1));

        // arquivo que nao faz parte do documento: sem resultado, sem erro
        let outro = projeto.join("nao-incluido.tex").display().to_string();
        assert_eq!(cmd_synctex_ir_pdf(pdf, outro, 1, 1).unwrap(), None);
    }
}

#[cfg(test)]
#[path = "arquivos_borda.rs"]
mod arquivos_borda;
