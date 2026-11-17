//! Testes contra o Drive real, com os tokens drive.file de duas contas de teste.
//! Nao rodam no `cargo test` normal. Ordem:
//!   cargo test --lib drive_real_fase1 -- --ignored --nocapture --test-threads=1
//!   (B escolhe o zip no Picker)
//!   cargo test --lib drive_real_fase2 -- --ignored --nocapture --test-threads=1
//!   cargo test --lib drive_real_limpar -- --ignored --nocapture --test-threads=1
//!   cargo test --lib drive_real_fase3 -- --ignored --nocapture --test-threads=1   (Abrir do Google Drive, so conta A)
//! Nunca imprime token.

use super::*;
use crate::ia::google::CONTA_TESTE;

/// Pasta de trabalho destes testes: variavel EGUATEX_DRIVE_REAL_BASE ou, sem ela, a pasta temporaria do sistema.
fn base() -> PathBuf {
    std::env::var_os("EGUATEX_DRIVE_REAL_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("eguatex-drive-real"))
}

/// JSON do cliente OAuth: variavel EGUATEX_GOOGLE_OAUTH_JSON ou, sem ela, src-tauri/google-oauth.json.
fn oauth_json() -> PathBuf {
    std::env::var_os("EGUATEX_GOOGLE_OAUTH_JSON")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("google-oauth.json"))
}

fn estado_json() -> PathBuf {
    base().join("drive-real").join("estado.json")
}

struct Tabela(Vec<(String, String, String)>);

impl Tabela {
    fn linha(&mut self, passo: &str, http: &str, resultado: &str) {
        println!("  {passo} -> {http}: {resultado}");
        self.0.push((passo.into(), http.into(), resultado.into()));
    }

    fn imprimir(&self) {
        println!("\n| passo | HTTP | resultado |\n|---|---|---|");
        for (p, h, r) in &self.0 {
            println!("| {p} | {h} | {r} |");
        }
    }
}

/// "HTTP" do resultado: 200 se deu certo; senao o trecho do erro (o app ja traduz o status).
fn http<T>(r: &Result<T, String>) -> String {
    match r {
        Ok(_) => "200".into(),
        Err(e) if e.starts_with(ACESSO_PERDIDO) => "404".into(),
        Err(e) => e.chars().take(80).collect(),
    }
}

/// Renova o access token da conta (A ou B) e faz o app usa-lo.
async fn usar(conta: &str) -> String {
    let tk: Value = serde_json::from_str(&fs::read_to_string(base().join("teste-drive-file").join(format!("tokens_{conta}.json"))).unwrap()).unwrap();
    let cfg: Value = serde_json::from_str(&fs::read_to_string(oauth_json()).unwrap()).unwrap();
    let id = cfg["installed"]["client_id"].as_str().unwrap().to_string();
    let seg = cfg["installed"]["client_secret"].as_str().unwrap_or("").to_string();
    let refresh = tk["refresh_token"].as_str().unwrap().to_string();
    let resp: Value = google::cliente()
        .post("https://oauth2.googleapis.com/token")
        .form(&[("grant_type", "refresh_token"), ("refresh_token", refresh.as_str()), ("client_id", id.as_str()), ("client_secret", seg.as_str())])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let acesso = resp["access_token"].as_str().unwrap_or_else(|| panic!("nao renovou o token da conta {conta}: {} / {}", resp["error"], resp["error_description"])).to_string();
    let email = tk["email"].as_str().unwrap().to_string();
    *CONTA_TESTE.lock().unwrap() = Some((email.clone(), acesso));
    println!("conta {conta} ativa: {email}");
    email
}

fn projeto(raiz: &Path, nome: &str, arquivos: &[(&str, &str)]) -> PathBuf {
    let dir = raiz.join(nome);
    let _ = fs::remove_dir_all(&dir);
    for (rel, txt) in arquivos {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, txt).unwrap();
    }
    dir
}

/// Zips do app com esse nome na conta ativa (so os que ela criou).
async fn zips_com_nome(nome: &str) -> Vec<String> {
    let mut url = reqwest::Url::parse(&format!("{API}/files")).unwrap();
    url.query_pairs_mut()
        .append_pair("q", &format!("name='{nome}' and trashed=false and 'me' in owners"))
        .append_pair("fields", "files(id)")
        .append_pair("pageSize", "20");
    let j = api(Method::GET, url.as_str(), None).await.unwrap();
    j["files"].as_array().into_iter().flatten().filter_map(|f| f["id"].as_str().map(String::from)).collect()
}

#[tokio::test]
#[ignore]
async fn drive_real_fase1() {
    let mut t = Tabela(vec![]);
    usar("A").await;
    let raiz = base().join("drive-real").join("A");
    let _ = fs::create_dir_all(&raiz);
    let sufixo = google::agora();
    let nome1 = format!("TesteDriveP1-{sufixo}");
    let nome2 = format!("TesteDriveP2-{sufixo}");

    // o app ja tinha a pasta "ÉguaTeX" na conta A?
    let pasta_antes = {
        let mut url = reqwest::Url::parse(&format!("{API}/files")).unwrap();
        url.query_pairs_mut()
            .append_pair("q", &format!("mimeType='{MIME_PASTA}' and trashed=false and 'me' in owners and appProperties has {{ key='eguatex' and value='app' }}"))
            .append_pair("fields", "files(id)");
        api(Method::GET, url.as_str(), None).await.unwrap()["files"].as_array().map(|a| a.len()).unwrap_or(0) > 0
    };

    // 1. A publica
    let p1 = projeto(&raiz, &nome1, &[("main.tex", "titulo A"), ("cap/a.tex", "capitulo original")]);
    let r = publicar(&p1).await;
    t.linha("1 A publica (cria o zip)", &http(&r), &format!("{:?}", r.as_ref().map(|id| format!("{}...", &id[..id.len().min(8)]))));
    let id1 = r.unwrap();
    let n = zips_com_nome(&nome_do_zip(&nome1)).await.len();
    t.linha("1b zips com o nome no Drive", "200", &format!("{n} (esperado 1)"));
    assert_eq!(n, 1);

    // 2. A sincroniza de novo
    let r = sincronizar(&p1).await;
    let (env, bx) = r.as_ref().map(|x| (x.enviados, x.baixados)).unwrap_or((99, 99));
    t.linha("2 A sincroniza de novo", &http(&r), &format!("enviados={env} baixados={bx} (esperado 0 e 0)"));
    let n = zips_com_nome(&nome_do_zip(&nome1)).await.len();
    t.linha("2b zips com o nome no Drive", "200", &format!("{n} (esperado 1, sem arquivo novo)"));
    assert_eq!((n, env, bx), (1, 0, 0));

    // 3. duas publicacoes simultaneas de A, projeto sem estado
    let p2 = projeto(&raiz, &nome2, &[("main.tex", "outro projeto")]);
    let (r1, r2) = tokio::join!(publicar(&p2), publicar(&p2));
    let mesmos = matches!((&r1, &r2), (Ok(a), Ok(b)) if a == b);
    t.linha("3 duas publicacoes simultaneas", &format!("{}/{}", http(&r1), http(&r2)), &format!("mesmo id nas duas: {mesmos}"));
    let n = zips_com_nome(&nome_do_zip(&nome2)).await.len();
    t.linha("3b zips com o nome no Drive", "200", &format!("{n} (esperado 1)"));
    assert!(mesmos);
    assert_eq!(n, 1);
    let id2 = r1.unwrap();

    fs::write(
        estado_json(),
        serde_json::to_string_pretty(&json!({
            "nome1": nome1, "nome2": nome2, "id1": id1, "id2": id2, "pasta_existia_antes": pasta_antes,
            "zip1": nome_do_zip(&nome1), "zip2": nome_do_zip(&nome2), "dir_a": p1.to_string_lossy(),
        }))
        .unwrap(),
    )
    .unwrap();
    t.imprimir();
    println!("\nID do zip para a conta B escolher no Picker: {id1}\nNome do arquivo: {}", nome_do_zip(&nome1));
}

#[tokio::test]
#[ignore]
async fn drive_real_fase2() {
    let mut t = Tabela(vec![]);
    let e: Value = serde_json::from_str(&fs::read_to_string(estado_json()).expect("rode a fase 1 antes")).unwrap();
    let id1 = e["id1"].as_str().unwrap().to_string();
    let raiz = base().join("drive-real");

    // 4. B abre o zip escolhido no Picker e sincroniza (baixa)
    let email_b = usar("B").await;
    let pb = projeto(&raiz.join("B"), "ProjetoB", &[]);
    fs::create_dir_all(&pb).unwrap();
    gravar_estado(&pb, &Estado { arquivo_id: id1.clone(), ..Default::default() }).unwrap();
    let r = sincronizar(&pb).await;
    t.linha("4 B sincroniza (baixa o zip)", &http(&r), &format!("{:?}", r.as_ref().map(|x| format!("baixados={}", x.baixados))));
    if r.is_err() {
        t.imprimir();
        panic!("B sem acesso ao zip: falta escolher o arquivo no Picker com a conta B");
    }
    assert!(pb.join("main.tex").exists());

    // 5. B edita main.tex e sincroniza (PATCH no zip existente)
    fs::write(pb.join("main.tex"), "titulo A editado por B").unwrap();
    let r = sincronizar(&pb).await;
    t.linha("5 B edita main.tex e sincroniza", &http(&r), &format!("{:?}", r.as_ref().map(|x| format!("enviados={}", x.enviados))));
    assert_eq!(r.unwrap().enviados, 1);

    // 6. A sincroniza e recebe a mudanca de B
    usar("A").await;
    let pa = PathBuf::from(e["dir_a"].as_str().unwrap());
    let r = sincronizar(&pa).await;
    let recebido = fs::read_to_string(pa.join("main.tex")).unwrap_or_default();
    t.linha("6 A sincroniza e recebe a mudanca", &http(&r), &format!("main.tex = {recebido:?}"));
    assert_eq!(recebido, "titulo A editado por B");

    // 7. conflito: os dois editam cap/a.tex; B sobe primeiro, depois A sincroniza
    fs::write(pa.join("cap/a.tex"), "capitulo editado por A").unwrap();
    usar("B").await;
    fs::write(pb.join("cap/a.tex"), "capitulo editado por B").unwrap();
    let r = sincronizar(&pb).await;
    t.linha("7a B edita cap/a.tex e sobe primeiro", &http(&r), &format!("{:?}", r.as_ref().map(|x| format!("enviados={}", x.enviados))));
    usar("A").await;
    let r = sincronizar(&pa).await;
    let conflitos = r.as_ref().map(|x| x.conflitos.clone()).unwrap_or_default();
    let final_a = fs::read_to_string(pa.join("cap/a.tex")).unwrap_or_default();
    let copia = conflitos.first().and_then(|c| fs::read_to_string(pa.join(c)).ok()).unwrap_or_default();
    t.linha("7b A sincroniza com conflito", &http(&r), &format!("conflitos={conflitos:?}; cap/a.tex={final_a:?}; copia={copia:?}"));
    assert!(conflitos.iter().any(|c| c.contains("(conflito de")));
    assert_eq!(final_a, "capitulo editado por B");
    assert_eq!(copia, "capitulo editado por A");
    println!("(conta B: {email_b})");
    t.imprimir();
}

#[tokio::test]
#[ignore]
async fn drive_real_limpar() {
    let mut t = Tabela(vec![]);
    let e: Value = serde_json::from_str(&fs::read_to_string(estado_json()).expect("sem estado.json")).unwrap();
    usar("A").await;
    for chave in ["zip1", "zip2"] {
        let nome = e[chave].as_str().unwrap();
        for id in zips_com_nome(nome).await {
            let r = api(Method::DELETE, &format!("{API}/files/{id}"), None).await;
            t.linha(&format!("apaga {nome}"), &http(&r), "excluido de vez");
        }
        t.linha(&format!("confere {nome}"), "200", &format!("restam {}", zips_com_nome(nome).await.len()));
    }
    if !e["pasta_existia_antes"].as_bool().unwrap_or(true) {
        // a pasta "ÉguaTeX" foi criada pelo teste: sai se estiver vazia
        if let Ok(id) = pasta_do_app().await {
            let mut url = reqwest::Url::parse(&format!("{API}/files")).unwrap();
            url.query_pairs_mut().append_pair("q", &format!("'{id}' in parents and trashed=false")).append_pair("fields", "files(id)");
            let filhos = api(Method::GET, url.as_str(), None).await.unwrap()["files"].as_array().map(|a| a.len()).unwrap_or(1);
            if filhos == 0 {
                let r = api(Method::DELETE, &format!("{API}/files/{id}"), None).await;
                t.linha("apaga a pasta ÉguaTex criada pelo teste", &http(&r), "vazia");
            }
        }
    }
    t.imprimir();
}

/// Fluxo "Abrir do Google Drive" com a conta A. O Picker nao se automatiza, entao o teste cria
/// os arquivos no Drive pela API (o app tem acesso a eles) e usa o mesmo caminho que o comando
/// usa depois da escolha: trazer para o projeto, gravar vinculos e sincronizar.
///   cargo test --lib drive_real_fase3 -- --ignored --nocapture --test-threads=1
#[tokio::test]
#[ignore]
async fn drive_real_fase3() {
    let mut t = Tabela(vec![]);
    usar("A").await;
    let raiz = base().join("drive-real").join("A");
    let _ = fs::create_dir_all(&raiz);
    let sufixo = google::agora();
    let nome_arq = format!("vinculo-{sufixo}.tex");
    let dir = projeto(&raiz, &format!("TesteVinculos-{sufixo}"), &[("main.tex", "meu main local")]);

    // arquivo no Drive (como se a pessoa o tivesse escolhido no Picker)
    let criado = api(Method::POST, &format!("{API}/files?fields=id"), Some(json!({ "name": nome_arq, "mimeType": "text/plain" }))).await.unwrap();
    let id = criado["id"].as_str().unwrap().to_string();
    let r = subir_conteudo_tipo(&id, b"versao 1 do Drive".to_vec(), "text/plain").await;
    t.linha("1 cria o arquivo de teste no Drive", &http(&r), "text/plain");
    r.unwrap();

    async {
        // 2. traz para o projeto: grava o arquivo e o vinculo
        let m = meta(&id).await.unwrap();
        let bytes = baixar(&id).await.unwrap();
        let mut res = ResultadoAbrir::default();
        trazer_para_o_projeto(&dir, vec![Baixado::solto(m.clone(), bytes.clone())], None, &mut res).await.unwrap();
        let vinc = ler_vinculos(&dir);
        t.linha("2 traz para o projeto", "200", &format!("adicionados={:?} vinculados={} vinculo={}", res.adicionados, res.vinculados, vinc.contains_key(&nome_arq)));
        assert_eq!(res.vinculados, 1);

        // 3. nome repetido: nao sobrescreve
        let mut res2 = ResultadoAbrir::default();
        trazer_para_o_projeto(&dir, vec![Baixado::solto(m, bytes)], None, &mut res2).await.unwrap();
        let esperado = nome_arq.replace(".tex", " (2).tex");
        t.linha("3 mesmo arquivo de novo", "200", &format!("adicionados={:?} (esperado {esperado}) vinculados={} (esperado 0)", res2.adicionados, res2.vinculados));
        assert_eq!(res2.adicionados, vec![esperado]);
        assert_eq!(res2.vinculados, 0);

        // 4. sem mudanca: nada acontece
        let r = sincronizar(&dir).await;
        let (e, b) = r.as_ref().map(|x| (x.enviados, x.baixados)).unwrap_or((99, 99));
        t.linha("4 sincroniza sem mudar nada", &http(&r), &format!("enviados={e} baixados={b} (esperado 0 e 0)"));
        assert_eq!((e, b), (0, 0));

        // 5. edita no disco: envia (PATCH)
        fs::write(dir.join(&nome_arq), "editado no disco").unwrap();
        let r = sincronizar(&dir).await;
        let no_drive = String::from_utf8(baixar(&id).await.unwrap()).unwrap();
        t.linha("5 edita no disco e sincroniza", &http(&r), &format!("enviados={} drive={no_drive:?}", r.as_ref().map(|x| x.enviados).unwrap_or(99)));
        assert_eq!(no_drive, "editado no disco");

        // 6. muda so no Drive: traz
        subir_conteudo_tipo(&id, b"mudou la no Drive".to_vec(), "text/plain").await.unwrap();
        let r = sincronizar(&dir).await;
        let local_agora = fs::read_to_string(dir.join(&nome_arq)).unwrap();
        t.linha("6 muda so no Drive e sincroniza", &http(&r), &format!("baixados={} (esperado 1) local={local_agora:?}", r.as_ref().map(|x| x.baixados).unwrap_or(99)));
        assert_eq!(r.unwrap().baixados, 1);
        assert_eq!(local_agora, "mudou la no Drive");

        // 7. muda dos dois lados: copia de conflito, nada sobrescrito
        fs::write(dir.join(&nome_arq), "minha versao").unwrap();
        subir_conteudo_tipo(&id, b"versao da outra pessoa".to_vec(), "text/plain").await.unwrap();
        let r = sincronizar(&dir).await.unwrap();
        let copia = r.copias_do_drive.first().cloned().unwrap_or_default();
        let conteudo_copia = fs::read_to_string(local(&dir, &copia)).unwrap_or_default();
        let no_drive = String::from_utf8(baixar(&id).await.unwrap()).unwrap();
        t.linha(
            "7 muda dos dois lados",
            "200",
            &format!("copia={copia:?} conteudo_copia={conteudo_copia:?} local={:?} drive={no_drive:?}", fs::read_to_string(dir.join(&nome_arq)).unwrap()),
        );
        assert!(copia.contains("(conflito de"));
        assert_eq!(conteudo_copia, "versao da outra pessoa");
        assert_eq!(fs::read_to_string(dir.join(&nome_arq)).unwrap(), "minha versao");
        assert_eq!(no_drive, "versao da outra pessoa");

        // 8. de novo sem a pessoa mexer: nao cria outra copia nem sobrescreve
        let r = sincronizar(&dir).await.unwrap();
        t.linha("8 sincroniza de novo em conflito", "200", &format!("copias={} enviados={}", r.copias_do_drive.len(), r.enviados));
        assert!(r.copias_do_drive.is_empty() && r.enviados == 0);

        // 9. a pessoa junta as versoes e salva: envia
        fs::write(dir.join(&nome_arq), "versoes juntadas").unwrap();
        let r = sincronizar(&dir).await.unwrap();
        let no_drive = String::from_utf8(baixar(&id).await.unwrap()).unwrap();
        t.linha("9 pessoa resolve e sincroniza", "200", &format!("enviados={} drive={no_drive:?}", r.enviados));
        assert_eq!(no_drive, "versoes juntadas");

        // 10. arquivo novo criado no projeto nao sobe
        fs::write(dir.join("so-local.tex"), "novo").unwrap();
        let r = sincronizar(&dir).await.unwrap();
        t.linha("10 arquivo novo local", "200", &format!("enviados={} (esperado 0)", r.enviados));
        assert_eq!(r.enviados, 0);
    }
    .await;

    // limpeza: apaga de vez o arquivo de teste
    let r = api(Method::DELETE, &format!("{API}/files/{id}"), None).await;
    t.linha("11 apaga o arquivo de teste do Drive", &http(&r), "excluido de vez");
    t.imprimir();
}

// ---------------------------------------------------------------------------
// v26: zip comum, pasta e figuras, com a conta A. Rodar com:
//   cargo test --lib drive_real_fase4 -- --ignored --nocapture --test-threads=1   (zip comum)
//   cargo test --lib drive_real_fase5 -- --ignored --nocapture --test-threads=1   (pasta com subpasta)
//   cargo test --lib drive_real_fase6 -- --ignored --nocapture --test-threads=1   (figuras)
// O Picker nao se automatiza: os testes criam os arquivos no Drive pela API e seguem o mesmo
// caminho que o app segue depois da escolha (meta, decisao, download, gravacao).
// ---------------------------------------------------------------------------

/// Cria um arquivo no Drive da conta ativa, com o tipo MIME pedido, e devolve o id.
async fn criar_arquivo(nome: &str, mime: &str, bytes: &[u8], pai: Option<&str>) -> String {
    let mut metadados = json!({ "name": nome, "mimeType": mime });
    if let Some(p) = pai {
        metadados["parents"] = json!([p]);
    }
    let fronteira = format!("teste{}", google::agora());
    let mut corpo = Vec::new();
    corpo.extend_from_slice(format!("--{fronteira}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadados}\r\n").as_bytes());
    corpo.extend_from_slice(format!("--{fronteira}\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
    corpo.extend_from_slice(bytes);
    corpo.extend_from_slice(format!("\r\n--{fronteira}--").as_bytes());
    let req = google::cliente()
        .post(format!("{UPLOAD}/files?uploadType=multipart&fields=id"))
        .header("Content-Type", format!("multipart/related; boundary={fronteira}"))
        .body(corpo);
    let j: Value = enviar(req).await.unwrap().json().await.unwrap();
    j["id"].as_str().unwrap().to_string()
}

async fn criar_pasta(nome: &str, pai: Option<&str>) -> String {
    let mut m = json!({ "name": nome, "mimeType": MIME_PASTA });
    if let Some(p) = pai {
        m["parents"] = json!([p]);
    }
    api(Method::POST, &format!("{API}/files?fields=id"), Some(m)).await.unwrap()["id"].as_str().unwrap().to_string()
}

async fn apagar_tudo(ids: &[String]) {
    for id in ids {
        let _ = api(Method::DELETE, &format!("{API}/files/{id}"), None).await;
    }
}

/// Um zip parecido com o exportado pelo Overleaf.
fn zip_overleaf() -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    z.add_directory("sections/", o).unwrap();
    for (nome, conteudo) in [
        ("main.tex", "\\documentclass{article}\\begin{document}\\input{sections/intro}\\end{document}"),
        ("references.bib", "@book{a,title={T}}"),
        ("sections/intro.tex", "Introdução com acentuação."),
        ("figures/fig 1.png", "png-falso"),
    ] {
        z.start_file(nome, o).unwrap();
        z.write_all(conteudo.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

#[tokio::test]
#[ignore]
async fn drive_real_fase4_zip_comum() {
    let mut t = Tabela(vec![]);
    usar("A").await;
    let raiz = base().join("drive-real").join("zip");
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(&raiz).unwrap();
    let sufixo = google::agora();
    let mut ids = Vec::new();
    let bytes = zip_overleaf();
    let mut falhas = Vec::new();
    for (i, mime) in ["application/zip", "application/x-zip-compressed", "application/x-zip", "application/octet-stream"].iter().enumerate() {
        let nome = format!("124-{sufixo}-{i}.zip");
        let id = criar_arquivo(&nome, mime, &bytes, None).await;
        ids.push(id.clone());
        let m = meta(&id).await.unwrap();
        let decisao = decidir_escolha_projeto(&m.nome, &m.mime);
        let r = baixar_projeto_de(m.clone(), &raiz.display().to_string()).await;
        let resumo = match &r {
            Ok(Some(dir)) => format!(
                "pasta={} main.tex={} intro={} figura={}",
                Path::new(dir).file_name().unwrap().to_string_lossy(),
                Path::new(dir).join("main.tex").exists(),
                Path::new(dir).join("sections").join("intro.tex").exists(),
                Path::new(dir).join("figures").join("fig 1.png").exists()
            ),
            Ok(None) => "cancelado".into(),
            Err(e) => format!("ERRO {e}"),
        };
        t.linha(&format!("zip {mime}"), &http(&r), &format!("mime no Drive={:?} decisao={decisao:?}; {resumo}", m.mime));
        if !matches!(&r, Ok(Some(d)) if Path::new(d).join("main.tex").exists()) {
            falhas.push(mime.to_string());
        }
    }
    apagar_tudo(&ids).await;
    t.imprimir();
    assert!(falhas.is_empty(), "zip nao abriu para os tipos: {falhas:?}");
}

#[tokio::test]
#[ignore]
async fn drive_real_fase5_pasta_com_subpasta() {
    let mut t = Tabela(vec![]);
    usar("A").await;
    let raiz = base().join("drive-real").join("pasta");
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(&raiz).unwrap();
    let sufixo = google::agora();
    let nome_pasta = format!("TestePasta-{sufixo}");

    let pasta = criar_pasta(&nome_pasta, None).await;
    let sub = criar_pasta("figs", Some(&pasta)).await;
    let a = criar_arquivo("main.tex", "text/plain", b"main", Some(&pasta)).await;
    let b = criar_arquivo("refs.bib", "application/octet-stream", b"@book{}", Some(&pasta)).await;
    let c = criar_arquivo("grafico.png", "image/png", b"png", Some(&sub)).await;
    t.linha("1 cria pasta com 2 arquivos e 1 subpasta (com 1 arquivo)", "200", &nome_pasta.to_string());

    // o que o Picker devolve no segundo passo: arquivos e a subpasta marcados, com o pai de cada um
    let dentro = vec![
        drive_picker::ArquivoEscolhido { id: a.clone(), nome: "main.tex".into(), mime_type: "text/plain".into(), parents: vec![pasta.clone()] },
        drive_picker::ArquivoEscolhido { id: b.clone(), nome: "refs.bib".into(), mime_type: "application/octet-stream".into(), parents: vec![pasta.clone()] },
        drive_picker::ArquivoEscolhido { id: sub.clone(), nome: "figs".into(), mime_type: MIME_PASTA.into(), parents: vec![pasta.clone()] },
        drive_picker::ArquivoEscolhido { id: c.clone(), nome: "grafico.png".into(), mime_type: "image/png".into(), parents: vec![sub.clone()] },
    ];
    let escolhida = drive_picker::ArquivoEscolhido { id: pasta.clone(), nome: nome_pasta.clone(), mime_type: MIME_PASTA.into(), parents: vec![] };

    // uma pasta so, projeto novo: o projeto leva o nome da pasta, arquivos na raiz, subpasta mantida
    let r = abrir_selecao(None, &raiz.display().to_string(), None, vec![], vec![(escolhida.clone(), dentro.clone())]).await;
    let (dir, resumo) = match &r {
        Ok(Some(res)) => (res.caminho.clone(), format!("adicionados={:?} vinculados={} avisos={:?}", res.adicionados, res.vinculados, res.avisos)),
        other => (String::new(), format!("{other:?}")),
    };
    t.linha("2 abre a pasta como projeto novo", &http(&r), &resumo);
    let dir = PathBuf::from(dir);
    assert_eq!(dir.file_name().unwrap().to_string_lossy(), nome_pasta);
    assert_eq!(fs::read_to_string(dir.join("main.tex")).unwrap(), "main");
    assert_eq!(fs::read_to_string(dir.join("refs.bib")).unwrap(), "@book{}");
    assert_eq!(fs::read(dir.join("figs").join("grafico.png")).unwrap(), b"png");
    let v = ler_vinculos(&dir);
    t.linha("3 vinculos gravados com o caminho da subpasta", "200", &format!("{:?}", v.keys().collect::<Vec<_>>()));
    assert!(v.contains_key("figs/grafico.png") && v.contains_key("main.tex") && v.len() == 3);

    // editar na subpasta e sincronizar sobe so esse arquivo
    fs::write(dir.join("figs").join("grafico.png"), "png editado").unwrap();
    let s = sincronizar(&dir).await;
    t.linha("4 edita figs/grafico.png e sincroniza", &http(&s), &format!("enviados={}", s.as_ref().map(|x| x.enviados).unwrap_or(99)));
    assert_eq!(s.unwrap().enviados, 1);
    assert_eq!(baixar(&c).await.unwrap(), b"png editado");

    // pasta junto com outro arquivo avulso, em projeto aberto: a pasta vira subpasta com o nome dela
    let projeto_aberto = projeto(&raiz, "ProjetoAberto", &[("existente.tex", "ja estava")]);
    let avulso = criar_arquivo("extra.tex", "text/plain", b"extra", None).await;
    let r2 = abrir_selecao(
        Some(projeto_aberto.clone()),
        &raiz.display().to_string(),
        None,
        vec![drive_picker::ArquivoEscolhido { id: avulso.clone(), nome: "extra.tex".into(), mime_type: "text/plain".into(), parents: vec![] }],
        vec![(escolhida, dentro)],
    )
    .await;
    t.linha("5 pasta + avulso num projeto aberto", &http(&r2), &format!("{:?}", r2.as_ref().ok().and_then(|x| x.as_ref()).map(|x| x.adicionados.clone())));
    assert!(projeto_aberto.join(&nome_pasta).join("main.tex").exists());
    assert!(projeto_aberto.join(&nome_pasta).join("figs").join("grafico.png").exists());
    assert!(projeto_aberto.join("extra.tex").exists() && projeto_aberto.join("existente.tex").exists());

    apagar_tudo(&[pasta, avulso]).await;
    t.imprimir();
}

#[tokio::test]
#[ignore]
async fn drive_real_fase6_figuras() {
    let mut t = Tabela(vec![]);
    usar("A").await;
    let raiz = base().join("drive-real").join("figuras");
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(&raiz).unwrap();
    let sufixo = google::agora();
    let projeto_dir = projeto(&raiz, "ProjetoFig", &[("main.tex", "x")]);

    // imagens de verdade para a conversao
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 120, 200, 255])));
    let em = |f: image::ImageFormat| {
        let mut c = Cursor::new(Vec::new());
        img.write_to(&mut c, f).unwrap();
        c.into_inner()
    };
    let casos: Vec<(String, &str, Vec<u8>)> = vec![
        (format!("fig-{sufixo}-png.png"), "image/png", em(image::ImageFormat::Png)),
        (format!("fig-{sufixo}-jpg.jpg"), "image/jpeg", em(image::ImageFormat::Jpeg)),
        (format!("fig-{sufixo}-pdf.pdf"), "application/pdf", b"%PDF-1.4 falso".to_vec()),
        (format!("fig-{sufixo}-eps.eps"), "application/postscript", b"%!PS-Adobe falso".to_vec()),
        (format!("fig-{sufixo}-svg.svg"), "image/svg+xml", b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec()),
        (format!("fig-{sufixo}-gif.gif"), "image/gif", em(image::ImageFormat::Gif)),
        (format!("fig-{sufixo}-bmp.bmp"), "image/bmp", em(image::ImageFormat::Bmp)),
        (format!("fig-{sufixo}-tiff.tiff"), "image/tiff", em(image::ImageFormat::Tiff)),
        (format!("fig-{sufixo}-webp.webp"), "image/webp", em(image::ImageFormat::WebP)),
        (format!("doc-{sufixo}-docx.docx"), "application/octet-stream", b"nao e figura".to_vec()),
    ];
    let mut ids = Vec::new();
    let mut erros = Vec::new();
    for (nome, mime, bytes) in &casos {
        let id = criar_arquivo(nome, mime, bytes, None).await;
        ids.push(id.clone());
        let r = figura_do_drive(&projeto_dir, &id).await;
        let esperado_ok = !nome.ends_with(".docx");
        t.linha(&format!("figura {}", nome.rsplit('.').next().unwrap()), &http(&r), &format!("{r:?}"));
        match (&r, esperado_ok) {
            (Ok(rel), true) => {
                if !projeto_dir.join(rel).exists() {
                    erros.push(format!("{nome}: {rel} nao foi gravado"));
                }
            }
            (Err(e), false) if e.contains("Use .png, .jpg, .pdf ou .eps") => {}
            _ => erros.push(format!("{nome}: resultado inesperado {r:?}")),
        }
    }
    // convertidas viram png de verdade e nao ficam ligadas ao arquivo original
    let figuras: Vec<String> = fs::read_dir(projeto_dir.join("figuras")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    t.linha("pasta figuras/", "200", &format!("{figuras:?}"));
    let v = ler_vinculos(&projeto_dir);
    t.linha("vinculos (so arquivos que nao foram convertidos)", "200", &format!("{:?}", v.keys().collect::<Vec<_>>()));
    for conv in ["gif", "bmp", "tiff", "webp"] {
        let esperado = format!("fig-{sufixo}-{conv}.{conv}");
        let como_png = nome_como_png(&esperado);
        let caminho = projeto_dir.join("figuras").join(&como_png);
        let existe = caminho.exists();
        if !existe {
            erros.push(format!("{conv}: nao virou png em figuras/"));
        } else if let Ok(b) = fs::read(&caminho)
            && !b.starts_with(b"\x89PNG") {
                erros.push(format!("{conv}: arquivo gravado nao e png"));
            }
        assert!(!v.keys().any(|k| k == &format!("figuras/{como_png}")), "{conv}: figura convertida nao pode ser ligada");
    }
    apagar_tudo(&ids).await;
    t.imprimir();
    assert!(erros.is_empty(), "{erros:?}");
}

/// Procura o "124.zip" do Paulo entre os arquivos que o app enxerga (drive.file: os que ele
/// escolheu no seletor) e segue o caminho real do app com ele. Mostra o erro exato.
///   cargo test --lib drive_real_124 -- --ignored --nocapture --test-threads=1
#[tokio::test]
#[ignore]
async fn drive_real_124() {
    usar("A").await;
    let mut url = reqwest::Url::parse(&format!("{API}/files")).unwrap();
    url.query_pairs_mut()
        .append_pair("q", "name contains '124' and trashed=false")
        .append_pair("fields", "files(id,name,mimeType,size,md5Checksum,ownedByMe,parents,createdTime,modifiedTime)")
        .append_pair("pageSize", "50");
    let j = api(Method::GET, url.as_str(), None).await.unwrap();
    let arquivos = j["files"].as_array().cloned().unwrap_or_default();
    println!("achados com '124' no nome: {}", arquivos.len());
    for f in &arquivos {
        println!("  {} | {} | {} | {} bytes | dono={}", f["id"], f["name"], f["mimeType"], f["size"], f["ownedByMe"]);
    }
    let Some(f) = arquivos.iter().find(|f| f["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case("124.zip"))) else {
        panic!("124.zip nao esta entre os arquivos que o app enxerga");
    };
    let id = f["id"].as_str().unwrap().to_string();
    let m = meta(&id).await.unwrap();
    println!("meta: nome={:?} mime={:?} md5={} versao={}", m.nome, m.mime, m.md5, m.versao);
    println!("decisao projeto: {:?}", decidir_escolha_projeto(&m.nome, &m.mime));
    let bytes = baixar(&id).await.unwrap();
    println!("baixou {} bytes; assinatura zip={}", bytes.len(), bytes_sao_zip(&bytes));
    match zip::ZipArchive::new(Cursor::new(&bytes[..])) {
        Ok(mut z) => {
            println!("entradas no zip: {}", z.len());
            for i in 0..z.len().min(60) {
                let e = z.by_index(i).unwrap();
                println!("  [{i}] {:?} dir={} tamanho={} (bruto {:?})", e.name(), e.is_dir(), e.size(), String::from_utf8_lossy(e.name_raw()));
            }
        }
        Err(e) => println!("ZipArchive::new falhou: {e}"),
    }
    match desempacotar_tudo(&bytes) {
        Ok((a, p)) => println!("desempacotar_tudo: {} arquivos, {} preservados", a.len(), p.len()),
        Err(e) => println!("desempacotar_tudo ERRO: {e}"),
    }

    // o caminho do botao "Do Google Drive" (abrir_selecao) e o do modo projeto (baixar_projeto_de)
    let raiz = base().join("drive-real").join("124");
    let _ = fs::remove_dir_all(&raiz);
    fs::create_dir_all(&raiz).unwrap();
    let escolhido = drive_picker::ArquivoEscolhido { id: id.clone(), nome: "124.zip".into(), mime_type: m.mime.clone(), parents: vec![] };
    let r = abrir_selecao(None, &raiz.display().to_string(), None, vec![escolhido], vec![]).await;
    println!("abrir_selecao -> {r:?}");
    let r2 = baixar_projeto_de(m.clone(), &raiz.display().to_string()).await;
    println!("baixar_projeto_de -> {r2:?}");

    // documento em branco aberto + Arquivo > Adicionar arquivos do Drive (destino = projeto aberto)
    let branco = raiz.join("Documento em branco");
    fs::create_dir_all(&branco).unwrap();
    fs::write(branco.join("main.tex"), "\\documentclass{article}\n\\begin{document}\nOi\n\\end{document}\n").unwrap();
    let escolhido = drive_picker::ArquivoEscolhido { id: id.clone(), nome: "124.zip".into(), mime_type: m.mime.clone(), parents: vec![] };
    let r3 = abrir_selecao(Some(branco.clone()), &raiz.display().to_string(), None, vec![escolhido], vec![]).await;
    println!("projeto aberto -> {r3:?}");

    // o principal sugerido compila na pasta dele e nao deve faltar nenhum arquivo ("not found")
    let sugerido = r3.as_ref().ok().and_then(|x| x.as_ref()).and_then(|x| x.principal_sugerido.clone());
    println!("principal_sugerido -> {sugerido:?}");
    if let Some(principal) = sugerido {
        let c = crate::compilador::compilar_com_bibtex(&principal, &branco.display().to_string(), "pdflatex", false);
        let faltando: Vec<&str> = c.log.lines().filter(|l| l.contains("not found")).collect();
        println!("compilou {principal}: sucesso={} pdf={:?} erros={}", c.sucesso, c.pdf, c.erros.len());
        println!("log tem \"not found\": {} {:?}", !faltando.is_empty(), faltando);
    }
}

/// Seletor do Google servido numa porta fixa (47123), com o token da conta A, para depurar o
/// Picker num navegador controlado por fora. Espera 10 minutos pela escolha.
///   cargo test --lib drive_real_picker_manual -- --ignored --nocapture --test-threads=1
#[tokio::test]
#[ignore]
async fn drive_real_picker_manual() {
    usar("A").await;
    let token = google::token_acesso().await.unwrap();
    let (chave, projeto) = drive_picker::configuracao().unwrap();
    let html = drive_picker::pagina_com_pai(drive_picker::Modo::Arquivos, &token, chave, projeto, "seg", false, None);
    let listener = std::net::TcpListener::bind("127.0.0.1:47123").unwrap();
    println!("PRONTO http://127.0.0.1:47123/?t=seg");
    let r = tokio::task::spawn_blocking(move || drive_picker::servir(listener, "seg", &html, drive_picker::Modo::Arquivos, std::time::Duration::from_secs(600))).await.unwrap();
    println!("ESCOLHA {r:?}");
}
