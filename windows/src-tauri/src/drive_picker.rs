//! Google Picker: a pessoa escolhe no navegador o arquivo do Drive que o ÉguaTex pode usar.
//!
//! Com o escopo `drive.file` o app so toca arquivos que ele mesmo criou ou que a pessoa
//! escolheu no Picker. O Picker libera exatamente o item escolhido (escolher uma pasta nao
//! libera os arquivos dentro dela), por isso o projeto compartilhado e um unico arquivo
//! `<nome>.eguatex.zip`.
//!
//! O Picker so roda dentro de uma pagina web. O app abre no navegador do sistema, como no
//! login (ia/google.rs), uma pagina servida por um servidor local em 127.0.0.1 numa porta
//! livre. A pagina carrega o Picker com o token da pessoa e devolve o id e o nome do arquivo
//! escolhido por `GET /picked`. Tempo limite: 5 minutos.

use std::io::{BufRead, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Emitter;

use crate::ia::google;

const CHAVE: Option<&str> = option_env!("EGUATEX_PICKER_API_KEY");
const NUMERO_PROJETO: Option<&str> = option_env!("EGUATEX_GOOGLE_PROJECT_NUMBER");
/// Sufixo dos arquivos de projeto no Drive.
pub const SUFIXO_PROJETO: &str = ".eguatex.zip";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Projeto compartilhado: um arquivo `*.eguatex.zip`.
    Projeto,
    Planilha,
    Documento,
    /// Varios arquivos do projeto (.tex, .bib, imagens, .zip...), com navegacao por pastas.
    Arquivos,
    /// Uma figura (png, jpg, jpeg, pdf, eps), com navegacao por pastas.
    Imagem,
}

/// Extensoes que o modo `Arquivos` aceita (sem o ponto, minusculas).
pub const EXTENSOES_ARQUIVOS: &[&str] = &[
    "tex", "bib", "sty", "cls", "bst", "txt", "md", "png", "jpg", "jpeg", "pdf", "eps", "svg", "zip",
];

/// Os modos de arquivos e de projeto NAO filtram por tipo MIME: o Drive da tipos variados ao mesmo
/// arquivo (um .zip pode vir como application/zip, x-zip-compressed, x-zip ou octet-stream; um
/// .tex como text/plain ou x-tex) e qualquer filtro esconderia arquivos da pessoa. O app confere
/// depois da escolha, pela extensao (`EXTENSOES_ARQUIVOS`) e pelo conteudo, e explica o que recusar.
/// Tipos MIME do modo `Imagem` (mais a pasta, para navegar). A extensao e conferida depois.
const MIMES_IMAGEM: &[&str] = &[
    "application/vnd.google-apps.folder",
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/x-ms-bmp",
    "image/tiff",
    "image/jpg",
    "image/pjpeg",
    "image/x-png",
    "image/eps",
    "image/x-eps",
    "application/eps",
    "application/x-eps",
    "application/pdf",
    "application/x-pdf",
    "application/postscript",
    "application/octet-stream",
];

/// Maximo de arquivos aceitos em uma escolha.
const MAX_ARQUIVOS: usize = 200;

impl Modo {
    fn chave(self) -> &'static str {
        match self {
            Modo::Projeto => "projeto",
            Modo::Planilha => "planilha",
            Modo::Documento => "documento",
            Modo::Arquivos => "arquivos",
            Modo::Imagem => "imagem",
        }
    }

    fn titulo(self) -> &'static str {
        texto(match self {
            Modo::Projeto => "titulo_projeto",
            Modo::Planilha => "titulo_planilha",
            Modo::Documento => "titulo_documento",
            Modo::Arquivos => "titulo_arquivos",
            Modo::Imagem => "titulo_imagem",
        })
    }

    pub fn de_texto(texto: &str) -> Option<Modo> {
        match texto {
            "projeto" => Some(Modo::Projeto),
            "planilha" => Some(Modo::Planilha),
            "documento" => Some(Modo::Documento),
            "arquivos" => Some(Modo::Arquivos),
            "imagem" => Some(Modo::Imagem),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Escolhido {
    pub id: String,
    pub nome: String,
}

/// Um arquivo escolhido no modo `Arquivos`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArquivoEscolhido {
    pub id: String,
    pub nome: String,
    #[serde(default, rename = "mimeType")]
    pub mime_type: String,
    #[serde(default)]
    pub parents: Vec<String>,
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Le a lista de arquivos que a pagina manda (JSON). Descarta ids invalidos, nomes vazios e
/// repetidos; erro se nada servir ou se passar de `MAX_ARQUIVOS`.
pub fn ler_itens(json: &str) -> Result<Vec<ArquivoEscolhido>, String> {
    let lista: Vec<ArquivoEscolhido> = serde_json::from_str(json).map_err(|e| format!("lista invalida: {e}"))?;
    if lista.len() > MAX_ARQUIVOS {
        return Err(format!("arquivos demais (maximo {MAX_ARQUIVOS})"));
    }
    let mut vistos = std::collections::BTreeSet::new();
    let itens: Vec<ArquivoEscolhido> = lista
        .into_iter()
        .filter(|a| id_ok(&a.id) && !a.nome.trim().is_empty() && vistos.insert(a.id.clone()))
        .map(|mut a| {
            a.parents.retain(|p| id_ok(p));
            a
        })
        .collect();
    if itens.is_empty() {
        return Err("nenhum arquivo valido".into());
    }
    Ok(itens)
}

/// Verdadeiro se a chave do Picker e o numero do projeto foram embutidos na compilacao.
pub fn configurado() -> bool {
    CHAVE.is_some_and(|c| !c.is_empty()) && NUMERO_PROJETO.is_some_and(|n| !n.is_empty())
}

pub(crate) fn configuracao() -> Result<(&'static str, &'static str), String> {
    match (CHAVE, NUMERO_PROJETO) {
        (Some(c), Some(n)) if !c.is_empty() && !n.is_empty() => Ok((c, n)),
        _ => Err("O seletor de arquivos do Google ainda não foi configurado nesta versão do ÉguaTex.".into()),
    }
}

/// Todos os textos da pagina do Picker (pt-BR), num lugar so. A pagina recebe este mapa como
/// `TXT` e nao tem texto proprio; os titulos por modo tambem saem daqui (`Modo::titulo`).
/// `{n}` e `{nome}` sao trocados pela pagina.
const TEXTOS: &[(&str, &str)] = &[
    ("pagina_titulo", "ÉguaTex: Google Drive"),
    ("carregando", "Carregando o seletor do Google..."),
    ("voltar", "Voltar ao ÉguaTex"),
    ("falha_carregar", "Não foi possível carregar o seletor do Google. Confira a internet e tente de novo no ÉguaTex."),
    ("escolher_de_novo", "Escolher de novo"),
    ("recebidos", "Recebidos {n} arquivo(s). Pode voltar ao ÉguaTex."),
    ("recebido", "Recebido: {nome}. Pode voltar ao ÉguaTex."),
    ("escolha_recusada", "Não foi possível usar essa escolha. Escolha de novo."),
    ("sem_resposta", "O ÉguaTex não respondeu. Volte ao app e tente de novo."),
    ("cancelado", "Cancelado. Pode voltar ao ÉguaTex."),
    ("titulo_projeto", "Escolha o projeto (.eguatex.zip) ou um .zip ou arquivo do seu Drive"),
    ("titulo_planilha", "Escolha a planilha do Google"),
    ("titulo_documento", "Escolha o documento do Google"),
    ("titulo_arquivos", "Escolha os arquivos do Google Drive"),
    ("titulo_imagem", "Escolha a figura no Google Drive"),
    ("titulo_dentro", "Escolha os arquivos da pasta"),
    ("dica_pasta", "Marque todos os arquivos e escolha Selecionar."),
    ("dica_cookies", "Se o seletor pedir login ou não responder, permita os cookies de docs.google.com no navegador e recarregue a página."),
    ("seletor_mudo", "O seletor do Google não abriu neste navegador. Entre no navegador com a mesma conta do Google do ÉguaTex, permita os cookies de terceiros e desligue VPN ou bloqueador de anúncios. Depois toque em Escolher de novo."),
];

/// Texto da pagina pela chave (vazio se a chave nao existir; um teste garante que existem).
fn texto(chave: &str) -> &'static str {
    TEXTOS.iter().find(|(k, _)| *k == chave).map(|(_, v)| *v).unwrap_or("")
}

/// O mapa de textos como objeto JSON seguro para o `<script>`.
fn textos_json() -> String {
    let mapa: serde_json::Map<String, Value> = TEXTOS.iter().map(|(k, v)| (k.to_string(), Value::String(v.to_string()))).collect();
    Value::Object(mapa).to_string().replace('<', "\\u003c")
}

/// Valor JSON seguro para ficar dentro de um `<script>` (sem fechar a tag por engano).
fn js(valor: &str) -> String {
    Value::String(valor.to_string()).to_string().replace('<', "\\u003c")
}

const PAGINA: &str = r##"<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" id="vp" content="width=600"><title></title>
<script>const LARG=Math.max(600,screen.width);document.getElementById('vp').content='width='+LARG;if(LARG<=700)document.documentElement.className='estreita';</script>
<style>.estreita body{margin:8px!important}.estreita h2{font-size:20px;margin:6px 0}.estreita p{font-size:14px;margin:4px 0}
.estreita .picker-dialog{position:fixed!important;top:96px!important;left:50%!important;transform:translateX(-50%)!important;margin:0!important}</style></head>
<body style="font-family:sans-serif;text-align:center;margin-top:12vh">
<h2 id="t"></h2><p id="m"></p><p id="b"></p>
<script src="https://apis.google.com/js/api.js" onerror="falhou()"></script>
<script>
const TOKEN=@TOKEN@, KEY=@KEY@, APP=@APP@, MODO=@MODO@, T=@T@, TITULO=@TITULO@, VOLTAR=@VOLTAR@, MIMES=@MIMES@, TXT=@TXT@, PAI=@PAI@;
const msg=document.getElementById('m'), bot=document.getElementById('b');
document.title=TXT.pagina_titulo;msg.textContent=TXT.carregando;
document.getElementById('t').textContent=PAI?TXT.titulo_dentro:TITULO;
function chamar(rota,extra){return fetch(rota+'?t='+encodeURIComponent(T)+(extra||''));}
function voltar(){bot.innerHTML=VOLTAR?'<a href="eguatex://login" style="display:inline-block;padding:14px 28px;background:#5f1a5c;color:#fff;border-radius:8px;text-decoration:none;font-size:18px">'+TXT.voltar+'</a>':'';}
function falhou(){msg.textContent=TXT.falha_carregar;chamar('/cancelado','&erro=1').catch(()=>{});}
let carregou=false,relogio=0;
function estado(){const q=document.querySelector('.picker-dialog');if(!q)return 'A sem quadro';const r=q.getBoundingClientRect();const f=q.querySelector('iframe');return (r.width>0&&r.height>0?'C quadro '+Math.round(r.width)+'x'+Math.round(r.height):'B quadro oculto')+(f?' com iframe':' sem iframe');}
function vigiar(){clearTimeout(relogio);carregou=false;relogio=setTimeout(()=>{if(carregou)return;const e=estado();denovo(TXT.seletor_mudo+' ['+e.charAt(0)+']');chamar('/diag','&d='+encodeURIComponent('seletor mudo: '+e+'; '+navigator.userAgent)).catch(()=>{});},25000);}
function denovo(texto){msg.textContent=texto;bot.innerHTML='';const b=document.createElement('button');b.textContent=TXT.escolher_de_novo;b.onclick=abrir;bot.appendChild(b);}
function abrir(){
 bot.innerHTML='';msg.textContent=PAI?TXT.dica_pasta+' '+TXT.dica_cookies:TXT.dica_cookies;
 const g=google.picker;
 const vistas=[];
 if(MODO==='projeto'||MODO==='arquivos'||MODO==='imagem'){
  // no modo arquivos a pasta tambem se marca (o Picker libera so a pasta, nao o que tem dentro)
  const pasta=MODO==='arquivos';
  const vista=()=>{const v=new g.DocsView(g.ViewId.DOCS).setIncludeFolders(true).setSelectFolderEnabled(pasta);return MIMES?v.setMimeTypes(MIMES):v;};
  if(PAI){
   vistas.push(vista().setParent(PAI));
  }else{
   vistas.push(vista().setOwnedByMe(true));
   vistas.push(vista().setOwnedByMe(false));
  }
 }else{
  const id=MODO==='planilha'?g.ViewId.SPREADSHEETS:g.ViewId.DOCUMENTS;
  vistas.push(new g.DocsView(id));
  vistas.push(new g.DocsView(id).setOwnedByMe(false));
 }
 const b=new g.PickerBuilder();
 vistas.forEach(v=>b.addView(v));
 if(MODO==='arquivos')b.enableFeature(g.Feature.MULTISELECT_ENABLED);
 // celular: o Picker ocupa a tela (o minimo dele e 566x350, o maximo 1051x650)
 if(LARG<=700)b.setSize(566,Math.max(350,Math.min(650,Math.max(innerHeight,Math.round(screen.height*LARG/screen.width)-240)-110)));
 b.setOAuthToken(TOKEN).setDeveloperKey(KEY).setAppId(APP).setLocale('pt-BR')
  .setCallback(d=>{
   if(d.action==='loaded'){carregou=true;return;}
   clearTimeout(relogio);
   if(d.action===g.Action.PICKED&&MODO==='arquivos'){
    const itens=d.docs.map(x=>({id:x.id,nome:x.name,mimeType:x.mimeType||'',parents:x.parentId?[x.parentId]:[]}));
    fetch('/picked?t='+encodeURIComponent(T),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(itens)}).then(r=>{
     if(r.ok){msg.textContent=TXT.recebidos.replace('{n}',itens.length);voltar();}
     else{denovo(TXT.escolha_recusada);}
    }).catch(()=>{msg.textContent=TXT.sem_resposta;});
   }else if(d.action===g.Action.PICKED){
    const x=d.docs[0];
    chamar('/picked','&id='+encodeURIComponent(x.id)+'&nome='+encodeURIComponent(x.name)).then(r=>{
     if(r.ok){msg.textContent=TXT.recebido.replace('{nome}',x.name);voltar();}
     else{denovo(TXT.escolha_recusada);}
    }).catch(()=>{msg.textContent=TXT.sem_resposta;});
   }else if(d.action===g.Action.CANCEL){
    msg.textContent=TXT.cancelado;chamar('/cancelado').catch(()=>{});voltar();
   }
  }).build().setVisible(true);
 vigiar();
}
window.addEventListener('load',()=>{
 if(typeof gapi==='undefined'){falhou();return;}
 gapi.load('picker',abrir);
});
</script></body></html>"##;

/// Pagina do Picker com os dados da pessoa e do projeto do Google Cloud (sem pasta de entrada).
#[cfg(test)]
fn pagina(modo: Modo, token: &str, chave: &str, projeto: &str, segredo: &str, voltar_ao_app: bool) -> String {
    pagina_com_pai(modo, token, chave, projeto, segredo, voltar_ao_app, None)
}

/// Igual a `pagina`; com `pai` o seletor abre ja dentro dessa pasta (`setParent`).
pub(crate) fn pagina_com_pai(modo: Modo, token: &str, chave: &str, projeto: &str, segredo: &str, voltar_ao_app: bool, pai: Option<&str>) -> String {
    PAGINA
        .replace("@PAI@", &js(pai.unwrap_or("")))
        .replace("@TOKEN@", &js(token))
        .replace("@KEY@", &js(chave))
        .replace("@APP@", &js(projeto))
        .replace("@MODO@", &js(modo.chave()))
        .replace("@T@", &js(segredo))
        .replace("@TITULO@", &js(modo.titulo()))
        .replace("@VOLTAR@", if voltar_ao_app { "true" } else { "false" })
        .replace("@TXT@", &textos_json())
        .replace("@MIMES@", &js(&(if modo == Modo::Imagem { MIMES_IMAGEM.join(",") } else { String::new() })))
}

fn responder(stream: &mut std::net::TcpStream, status: &str, tipo: &str, corpo: &str) {
    let texto = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {tipo}; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let _ = stream.write_all(texto.as_bytes());
}

/// Linha de pedido, cabecalhos e corpo de um pedido HTTP simples (o corpo so existe em POST e vai
/// ate 1 MB). A linha e os cabecalhos passam por `http_local::leitor_limitado` (8 KB, no maximo
/// `MAX_CABECALHOS` cabecalhos); passou disso, devolve `Err` e quem chama responde 400 e fecha.
fn ler_pedido(stream: &std::net::TcpStream) -> Result<(String, String, String), ()> {
    use crate::http_local::{MAX_CABECALHOS, leitor_limitado, linha_completa};
    let mut leitor = leitor_limitado(stream);
    let mut linha = String::new();
    leitor.read_line(&mut linha).ok();
    if !linha_completa(&linha) {
        return Err(());
    }
    let metodo = linha.split_whitespace().next().unwrap_or("GET").to_string();
    let caminho = linha.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut tamanho = 0usize;
    let mut cabecalhos = 0usize;
    loop {
        let mut cab = String::new();
        if leitor.read_line(&mut cab).unwrap_or(0) == 0 {
            // acabou a conexao ou o teto de 8 KB: sem a linha em branco, o pedido nao terminou
            if leitor.get_ref().limit() == 0 {
                return Err(());
            }
            break;
        }
        if !linha_completa(&cab) {
            return Err(());
        }
        if cab.trim().is_empty() {
            break;
        }
        cabecalhos += 1;
        if cabecalhos > MAX_CABECALHOS {
            return Err(());
        }
        if let Some((nome, valor)) = cab.split_once(':')
            && nome.trim().eq_ignore_ascii_case("content-length") {
                tamanho = valor.trim().parse().unwrap_or(0);
            }
    }
    let mut corpo = String::new();
    if metodo == "POST" && tamanho > 0 && tamanho <= 1 << 20 {
        // o teto de 8 KB vale so para a linha e os cabecalhos: o que o leitor ja puxou do
        // corpo fica no buffer dele, e o resto vem direto da conexao
        let ja_lido = leitor.buffer();
        let mut bytes = ja_lido[..ja_lido.len().min(tamanho)].to_vec();
        let falta = tamanho - bytes.len();
        if falta > 0 {
            let mut resto = vec![0u8; falta];
            if (&*stream).read_exact(&mut resto).is_err() {
                return Ok((metodo, caminho, corpo));
            }
            bytes.extend(resto);
        }
        corpo = String::from_utf8_lossy(&bytes).into_owned();
    }
    Ok((metodo, caminho, corpo))
}

/// Atende o navegador ate o Picker devolver o(s) arquivo(s), ser cancelado ou o tempo acabar.
/// `Ok(None)` quando a pessoa cancelou. Pedidos sem o segredo da sessao levam 404.
/// Os modos de um arquivo devolvem uma lista com um item; o modo `Arquivos`, varios.
pub(crate) fn servir(
    listener: TcpListener,
    segredo: &str,
    html: &str,
    modo: Modo,
    limite: Duration,
) -> Result<Option<Vec<ArquivoEscolhido>>, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let fim = Instant::now() + limite;
    // Cada conexao tem a sua thread. O Chrome do Android abre conexoes de antemao e nao manda nada
    // nelas; atendidas uma por vez, cada uma segurava o servidor ate o tempo de leitura acabar.
    let (tx, rx) = std::sync::mpsc::channel();
    let segredo: std::sync::Arc<str> = segredo.into();
    let html: std::sync::Arc<str> = html.into();
    loop {
        if let Ok(r) = rx.try_recv() {
            return r;
        }
        if Instant::now() > fim {
            return Err("Tempo esgotado esperando a escolha do arquivo no navegador.".into());
        }
        let (stream, _) = match listener.accept() {
            Ok(par) => par,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let (tx, segredo, html) = (tx.clone(), segredo.clone(), html.clone());
        let espera = fim.saturating_duration_since(Instant::now()).max(Duration::from_secs(1));
        std::thread::spawn(move || {
            if let Some(r) = atender(stream, &segredo, &html, modo, espera) {
                let _ = tx.send(r);
            }
        });
    }
}

/// Atende uma conexao do servidor do Picker. `Some` quando o pedido encerra a escolha (arquivos
/// recebidos, cancelado ou falha ao carregar); `None` para os demais (pagina, 404, pedido recusado).
fn atender(mut stream: std::net::TcpStream, segredo: &str, html: &str, modo: Modo, espera: Duration) -> Option<Result<Option<Vec<ArquivoEscolhido>>, String>> {
    stream.set_nonblocking(false).ok();
    // O Chrome abre conexoes de antemao, sem mandar nada, e usa uma delas depois para enviar a
    // escolha. Ate o primeiro byte chegar o servidor nao responde: um 404 escrito na conexao muda
    // ficava guardado e o Chrome o lia como resposta da escolha ("Nao foi possivel usar essa escolha").
    stream.set_read_timeout(Some(espera)).ok();
    if !matches!(stream.peek(&mut [0u8; 1]), Ok(n) if n > 0) {
        return None;
    }
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();

    let Ok((metodo, caminho, corpo)) = ler_pedido(&stream) else {
        crate::registro::drive("picker", "pedido recusado: linha ou cabecalhos grandes demais");
        let _ = stream.write_all(crate::http_local::RESPOSTA_431);
        return None;
    };
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{caminho}")) else {
        crate::registro::drive("picker", "pedido recusado: caminho invalido");
        responder(&mut stream, "400 Bad Request", "text/plain", "pedido invalido");
        return None;
    };
    let mut t = String::new();
    let mut id = String::new();
    let mut nome = String::new();
    let mut itens = String::new();
    let mut diag = String::new();
    let mut erro = false;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "t" => t = v.into_owned(),
            "d" => diag = v.chars().filter(|c| c.is_ascii_alphanumeric() || " -_.,:;=()/".contains(*c)).take(300).collect(),
            "id" => id = v.into_owned(),
            "nome" => nome = v.into_owned(),
            "itens" => itens = v.into_owned(),
            "erro" => erro = true,
            _ => {}
        }
    }
    crate::registro::drive("picker", &format!("{metodo} {} corpo={} bytes segredo_ok={}", url.path(), corpo.len(), t == segredo));
    if t != segredo {
        responder(&mut stream, "404 Not Found", "text/plain", "nao encontrado");
        return None;
    }
    match url.path() {
        "/" => {
            responder(&mut stream, "200 OK", "text/html", html);
            None
        }
        "/picked" if modo == Modo::Arquivos => {
            // a lista vem no corpo (POST) ou, para listas curtas, em ?itens=<json>
            let json = if corpo.trim().is_empty() { itens } else { corpo };
            match ler_itens(&json) {
                Ok(lista) => {
                    crate::registro::drive("picker", &format!("recebidos {} item(ns): {}", lista.len(), lista.iter().map(|a| format!("{} ({})", a.nome, a.mime_type)).collect::<Vec<_>>().join(", ")));
                    responder(&mut stream, "200 OK", "text/plain", "ok");
                    Some(Ok(Some(lista)))
                }
                Err(e) => {
                    crate::registro::drive("picker", &format!("lista recusada: {e}"));
                    responder(&mut stream, "400 Bad Request", "text/plain", "arquivos invalidos");
                    None
                }
            }
        }
        "/picked" => {
            if !id_ok(&id) || nome.trim().is_empty() {
                crate::registro::drive("picker", "escolha recusada: id ou nome invalido");
                responder(&mut stream, "400 Bad Request", "text/plain", "arquivo invalido");
                return None;
            }
            crate::registro::drive("picker", &format!("recebido 1 item: {nome}"));
            responder(&mut stream, "200 OK", "text/plain", "ok");
            Some(Ok(Some(vec![ArquivoEscolhido { id, nome, mime_type: String::new(), parents: vec![] }])))
        }
        // a pagina avisa quando o quadro do Google nao abriu (so vai para o log; a espera continua)
        "/diag" => {
            crate::registro::drive("picker", &format!("diagnostico da pagina: {diag}"));
            responder(&mut stream, "200 OK", "text/plain", "ok");
            None
        }
        "/cancelado" => {
            responder(&mut stream, "200 OK", "text/plain", "ok");
            if erro {
                return Some(Err("O seletor do Google não carregou. Confira a conexão com a internet.".into()));
            }
            Some(Ok(None))
        }
        _ => {
            responder(&mut stream, "404 Not Found", "text/plain", "nao encontrado");
            None
        }
    }
}

/// No Android 15 o app fica sem internet enquanto o navegador esta na frente. Depois da
/// escolha, espera a pessoa voltar ao app antes de seguir (ate 5 min).
async fn aguardar_rede_no_android(app: &tauri::AppHandle) {
    if !cfg!(target_os = "android") {
        return;
    }
    let limite = Instant::now() + Duration::from_secs(300);
    while Instant::now() < limite {
        match google::cliente().head("https://www.googleapis.com/").timeout(Duration::from_secs(5)).send().await {
            Ok(_) => return,
            Err(_) => {
                let _ = app.emit("ia-google-progresso", "Volte ao ÉguaTex para continuar");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Abre o Picker no navegador do sistema e devolve o que a pessoa escolheu (um item, ou varios no
/// modo `Arquivos`). `Ok(None)` se a pessoa cancelou.
async fn abrir_picker(app: &tauri::AppHandle, modo: Modo, pai: Option<&str>) -> Result<Option<Vec<ArquivoEscolhido>>, String> {
    let (chave, projeto) = configuracao()?;
    let token = google::token_acesso().await?;
    let segredo = google::texto_aleatorio(24);
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("Erro ao abrir porta local: {e}"))?;
    let porta = listener.local_addr().map_err(|e| e.to_string())?.port();
    let html = pagina_com_pai(modo, &token, chave, projeto, &segredo, cfg!(target_os = "android"), pai);
    let url = format!("http://127.0.0.1:{porta}/?t={segredo}");
    crate::registro::drive("picker", &format!("abrindo modo {} na porta {porta}", modo.chave()));

    // Android: servico em primeiro plano enquanto o navegador esta na frente. Sem ele o sistema
    // congela o app e o seletor nunca carrega nem entrega a escolha ao 127.0.0.1 (mesma causa do
    // login no celular real). A guarda vale ate a rede voltar e para o servico em qualquer saida.
    let _acordado = google::manter_acordado::Guarda::nova();

    // O Picker roda no navegador: ele usa a sessao do Google de la (uma janela do proprio app nao
    // tem sessao e o Google recusa). Navegador que bloqueia cookies de terceiros (Brave) ganha a dica na pagina.
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| format!("Erro ao abrir o navegador: {e}"))?;
    let _ = app.emit("ia-google-progresso", "Aguardando a escolha no navegador");

    let escolhido = tokio::task::spawn_blocking(move || servir(listener, &segredo, &html, modo, Duration::from_secs(300)))
        .await
        .map_err(|e| e.to_string())?
        .inspect_err(|e| crate::registro::drive("picker", &format!("erro: {e}")))?;
    if escolhido.is_none() {
        crate::registro::drive("picker", "cancelado");
    }
    if escolhido.is_some() {
        aguardar_rede_no_android(app).await;
    }
    Ok(escolhido)
}

/// Abre o Picker para um arquivo. `Ok(None)` se a pessoa cancelou.
pub async fn escolher(app: &tauri::AppHandle, modo: Modo) -> Result<Option<Escolhido>, String> {
    Ok(abrir_picker(app, modo, None).await?.and_then(|l| l.into_iter().next()).map(|a| Escolhido { id: a.id, nome: a.nome }))
}

/// Abre o Picker para uma figura (png, jpg, jpeg, pdf, eps). `Ok(None)` se cancelou.
pub async fn escolher_imagem(app: &tauri::AppHandle) -> Result<Option<Escolhido>, String> {
    escolher(app, Modo::Imagem).await
}

/// Abre o Picker com varios arquivos do Drive (modo `Arquivos`). `Ok(None)` se cancelou.
pub async fn escolher_arquivos(app: &tauri::AppHandle) -> Result<Option<Vec<ArquivoEscolhido>>, String> {
    abrir_picker(app, Modo::Arquivos, None).await
}

/// Segundo passo para uma pasta: o seletor abre ja dentro dela, com varios arquivos e subpastas
/// marcaveis. O Picker libera so a pasta escolhida, nao o que ha dentro; por isso a pessoa marca
/// os arquivos aqui. `Ok(None)` se cancelou.
pub async fn escolher_dentro_da_pasta(app: &tauri::AppHandle, pasta_id: &str) -> Result<Option<Vec<ArquivoEscolhido>>, String> {
    if !id_ok(pasta_id) {
        return Err("Pasta do Drive inválida.".into());
    }
    abrir_picker(app, Modo::Arquivos, Some(pasta_id)).await
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Read;
    use std::net::TcpStream;

    fn pedir(porta: u16, caminho: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        write!(s, "GET {caminho} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).ok();
        r
    }

    fn enviar_json(porta: u16, caminho: &str, corpo: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        write!(
            s,
            "POST {caminho} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{corpo}",
            corpo.len()
        )
        .unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).ok();
        r
    }

    /// Manda `bytes` numa conexao e devolve o que `ler_pedido` respondeu no servidor.
    fn pedido_lido(bytes: Vec<u8>) -> Result<(String, String, String), ()> {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let cliente = std::thread::spawn(move || {
            let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
            let _ = s.write_all(&bytes);
            // fica aberta ate o servidor fechar
            let mut resto = Vec::new();
            let _ = s.read_to_end(&mut resto);
        });
        let (s, _) = l.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let r = ler_pedido(&s);
        drop(s);
        cliente.join().unwrap();
        r
    }

    #[test]
    fn pedido_com_linha_de_1mb_sem_quebra_e_recusado() {
        assert!(pedido_lido(vec![b'a'; 1 << 20]).is_err());
    }

    #[test]
    fn pedido_com_cabecalhos_demais_e_recusado() {
        let mut p = String::from("GET /picked HTTP/1.1\r\n");
        for i in 0..=crate::http_local::MAX_CABECALHOS {
            p.push_str(&format!("X-{i}: a\r\n"));
        }
        p.push_str("\r\n");
        assert!(pedido_lido(p.into_bytes()).is_err());
    }

    #[test]
    fn pedido_normal_passa_com_e_sem_corpo() {
        let (m, c, corpo) = pedido_lido(b"GET /picked?t=1 HTTP/1.1\r\nHost: x\r\n\r\n".to_vec()).unwrap();
        assert_eq!((m.as_str(), c.as_str(), corpo.as_str()), ("GET", "/picked?t=1", ""));
        let grande = "a".repeat(100_000);
        let p = format!("POST /r HTTP/1.1\r\nContent-Length: {}\r\n\r\n{grande}", grande.len());
        let (m, _, corpo) = pedido_lido(p.into_bytes()).unwrap();
        assert_eq!((m.as_str(), corpo.len()), ("POST", 100_000));
    }

    type Servidor = std::thread::JoinHandle<Result<Option<Vec<ArquivoEscolhido>>, String>>;

    fn iniciar(modo: Modo, limite: Duration) -> (u16, Servidor) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || servir(l, "seg", "<html>PAGINA</html>", modo, limite));
        (porta, h)
    }

    #[test]
    fn pagina_leva_os_dados_e_escapa_o_script() {
        let p = pagina(Modo::Projeto, "tk</script>", "AIzaX", "123", "seg", false);
        assert!(p.contains("\"AIzaX\"") && p.contains("\"123\"") && p.contains("\"projeto\""));
        assert!(p.contains("\\u003c/script>") && !p.contains("tk</script>"));
        assert!(p.contains("VOLTAR=false"));
        assert!(!p.contains('@'), "sobrou marcador sem trocar");
        assert!(pagina(Modo::Planilha, "t", "k", "1", "s", true).contains("VOLTAR=true"));
    }

    #[test]
    fn pagina_cabe_na_tela_do_celular() {
        let p = pagina(Modo::Arquivos, "t", "k", "1", "s", true);
        assert!(p.contains("name=\"viewport\""));
        assert!(p.contains("b.setSize(566,"));
        // o Google as vezes centraliza o quadro abaixo da dobra: em tela estreita ele fica preso no topo
        assert!(p.contains(".estreita .picker-dialog{position:fixed!important;top:96px!important"));
        // quadro do Google que nao abre em 25 s vira aviso na pagina e linha no log
        assert!(p.contains("vigiar();") && p.contains("TXT.seletor_mudo") && p.contains("'/diag'"));
    }

    #[test]
    fn pagina_do_modo_arquivos_tem_multiselecao_pastas_e_tipos() {
        let p = pagina(Modo::Arquivos, "tk", "AIzaX", "123", "seg", false);
        assert!(p.contains("\"arquivos\"") && p.contains("MULTISELECT_ENABLED"));
        assert!(p.contains("setIncludeFolders(true)") && p.contains("setSelectFolderEnabled(pasta)"));
        assert!(p.contains("const pasta=MODO==='arquivos'"), "pasta marcavel so no modo arquivos");
        assert!(p.contains("setOwnedByMe(true)") && p.contains("setOwnedByMe(false)"));
        // sem filtro de tipo: nenhum arquivo some do seletor por causa do tipo MIME
        assert!(p.contains("MIMES=\"\"") && p.contains("MIMES?v.setMimeTypes(MIMES):v"));
        assert!(!p.contains("application/zip") || p.contains("MIMES=\"\""));
        assert!(!p.contains('@'), "sobrou marcador sem trocar");
    }

    #[test]
    fn pagina_do_modo_imagem_e_de_uma_figura_so_sem_multiselecao_ligada() {
        let p = pagina(Modo::Imagem, "tk", "AIzaX", "123", "seg", false);
        assert!(p.contains("MODO=\"imagem\"") || p.contains("\"imagem\""));
        assert!(p.contains("image/png") && p.contains("image/jpeg") && !p.contains("image/svg+xml"));
        assert!(p.contains("application/pdf") && p.contains("application/postscript"));
        // os tipos de texto do modo Arquivos nao entram aqui
        assert!(!p.contains("text/x-tex") && !p.contains("text/markdown"));
        // a multiselecao so liga no modo "arquivos"
        assert!(p.contains("if(MODO==='arquivos')b.enableFeature"));
        assert_eq!(Modo::de_texto("imagem"), Some(Modo::Imagem));
        assert!(!p.contains('@'), "sobrou marcador sem trocar");
    }

    #[test]
    fn modo_projeto_abre_sem_busca_preenchida_e_aceita_zip_comum() {
        for modo in [Modo::Projeto, Modo::Arquivos, Modo::Imagem, Modo::Planilha, Modo::Documento] {
            let p = pagina(modo, "tk", "AIzaX", "123", "seg", false);
            assert!(!p.contains("setQuery"), "{modo:?} abre com a busca preenchida");
        }
        let p = pagina(Modo::Projeto, "tk", "AIzaX", "123", "seg", false);
        assert!(p.contains("MIMES=\"\""), "o modo projeto nao filtra por tipo: o .zip aparece qualquer que seja o tipo");
        assert!(p.contains("setIncludeFolders(true)"), "o modo projeto navega por pastas");
        // um .zip comum (ex.: exportado do Overleaf) chega ao app em vez de levar 400
        let (porta, h) = iniciar(Modo::Projeto, Duration::from_secs(20));
        assert!(pedir(porta, "/picked?t=seg&id=1Zip&nome=124.zip").contains("200"));
        let l = h.join().unwrap().unwrap().unwrap();
        assert_eq!((l[0].id.as_str(), l[0].nome.as_str()), ("1Zip", "124.zip"));
        // e um arquivo que nao e zip tambem
        let (porta, h) = iniciar(Modo::Projeto, Duration::from_secs(20));
        assert!(pedir(porta, "/picked?t=seg&id=1Tex&nome=main.tex").contains("200"));
        assert_eq!(h.join().unwrap().unwrap().unwrap()[0].nome, "main.tex");
    }

    #[test]
    fn imagem_escolhida_chega_como_um_item_so() {
        let (porta, h) = iniciar(Modo::Imagem, Duration::from_secs(20));
        assert!(pedir(porta, "/picked?t=seg&id=1AbC&nome=fig%201.png").contains("200"));
        let l = h.join().unwrap().unwrap().unwrap();
        assert_eq!((l.len(), l[0].id.as_str(), l[0].nome.as_str()), (1, "1AbC", "fig 1.png"));
    }

    #[test]
    fn conexao_muda_do_navegador_nao_segura_a_escolha() {
        let (porta, h) = iniciar(Modo::Arquivos, Duration::from_secs(20));
        // o Chrome do Android abre conexoes de antemao e nao manda nada nelas
        let mut mudas: Vec<TcpStream> = (0..3).map(|_| TcpStream::connect(("127.0.0.1", porta)).unwrap()).collect();
        std::thread::sleep(Duration::from_millis(300));
        assert!(pedir(porta, "/diag?t=seg&d=seletor%20mudo%3A%20A%20sem%20quadro").contains("200"));
        // a escolha vai por uma das conexoes mudas, como faz o Chrome: nela nao pode haver resposta velha
        let inicio = Instant::now();
        let corpo = r#"[{"id":"1AbC","nome":"fig.png","mimeType":"image/png"}]"#;
        let mut s = mudas.remove(0);
        write!(s, "POST /picked?t=seg HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{corpo}", corpo.len()).unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).ok();
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        assert!(inicio.elapsed() < Duration::from_secs(2), "esperou as conexoes mudas: {:?}", inicio.elapsed());
        let e = h.join().unwrap().unwrap().unwrap();
        assert_eq!(e[0].nome, "fig.png");
    }

    #[test]
    fn servidor_entrega_a_pagina_e_o_arquivo_escolhido() {
        let (porta, h) = iniciar(Modo::Projeto, Duration::from_secs(20));
        assert!(pedir(porta, "/favicon.ico").contains("404"));
        assert!(pedir(porta, "/?t=errado").contains("404"));
        assert!(pedir(porta, "/?t=seg").contains("PAGINA"));
        // sem id valido nao vale; qualquer nome vale (zip comum, .tex...): o app decide o que fazer
        assert!(pedir(porta, "/picked?t=seg&id=&nome=notas.zip").contains("400"));
        assert!(pedir(porta, "/picked?t=seg&id=../x&nome=notas.zip").contains("400"));
        let ok = pedir(porta, "/picked?t=seg&id=1AbC&nome=Tese%20final.eguatex.zip");
        assert!(ok.contains("200"));
        let e = h.join().unwrap().unwrap().unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].id.as_str(), e[0].nome.as_str()), ("1AbC", "Tese final.eguatex.zip"));
    }

    #[test]
    fn planilha_aceita_qualquer_nome_e_cancelar_devolve_none() {
        let (porta, h) = iniciar(Modo::Planilha, Duration::from_secs(20));
        pedir(porta, "/picked?t=seg&id=X1&nome=Dados");
        assert_eq!(h.join().unwrap().unwrap().unwrap()[0].nome, "Dados");
        let (porta, h) = iniciar(Modo::Documento, Duration::from_secs(20));
        pedir(porta, "/cancelado?t=seg");
        assert_eq!(h.join().unwrap().unwrap(), None);
    }

    #[test]
    fn falha_ao_carregar_e_tempo_esgotado_viram_erro() {
        let (porta, h) = iniciar(Modo::Projeto, Duration::from_secs(20));
        pedir(porta, "/cancelado?t=seg&erro=1");
        assert!(h.join().unwrap().unwrap_err().contains("não carregou"));
        let (_, h) = iniciar(Modo::Projeto, Duration::from_millis(300));
        assert!(h.join().unwrap().unwrap_err().contains("Tempo esgotado"));
    }

    #[test]
    fn a_pagina_nao_tem_texto_proprio_e_usa_todos_os_textos_do_mapa() {
        for solto in ["Não foi possível", "Pode voltar", "Escolher de novo", "Carregando", "Cancelado.", "Voltar ao ÉguaTex", "não respondeu"] {
            assert!(!PAGINA.contains(solto), "texto fora do mapa: {solto}");
        }
        for (chave, valor) in TEXTOS {
            assert!(!valor.is_empty());
            // os titulos por modo saem de `Modo::titulo`, os demais sao lidos pela pagina (TXT.chave)
            assert!(chave.starts_with("titulo_") || PAGINA.contains(&format!("TXT.{chave}")), "pagina nao usa {chave}");
        }
        let p = pagina(Modo::Arquivos, "tk", "AIzaX", "123", "seg", true);
        assert!(p.contains("\"carregando\":\"Carregando o seletor do Google...\""));
        assert!(p.contains("TXT=") && !p.contains("@TXT@"));
        for modo in [Modo::Projeto, Modo::Planilha, Modo::Documento, Modo::Arquivos, Modo::Imagem] {
            assert!(!modo.titulo().is_empty(), "{modo:?}");
        }
        assert_eq!(texto("chave que nao existe"), "");
    }

    #[test]
    fn seletor_dentro_da_pasta_abre_no_pai_com_a_dica_e_aceita_pastas() {
        let p = pagina_com_pai(Modo::Arquivos, "tk", "AIzaX", "123", "seg", false, Some("PASTA123"));
        assert!(p.contains("PAI=\"PASTA123\""));
        assert!(p.contains("setParent(PAI)") && p.contains("MULTISELECT_ENABLED"));
        // a dica do Ctrl+A vem do mapa de textos
        assert!(p.contains("Marque todos os arquivos e escolha Selecionar."));
        assert!(p.contains("const pasta=MODO==='arquivos'"), "subpastas marcaveis so no modo arquivos");
        // sem pasta: PAI vazio
        assert!(pagina(Modo::Arquivos, "tk", "AIzaX", "123", "seg", false).contains("PAI=\"\""));
        assert!(!p.contains('@'), "sobrou marcador sem trocar");
    }

    #[test]
    fn tipos_de_zip_e_de_imagem_do_seletor() {
        // arquivos e projeto: sem lista de tipos (o .zip nunca some por causa do tipo MIME)
        for modo in [Modo::Arquivos, Modo::Projeto] {
            assert!(pagina(modo, "tk", "k", "1", "s", false).contains("MIMES=\"\""), "{modo:?}");
        }
        let imagem = pagina(Modo::Imagem, "tk", "k", "1", "s", false);
        for m in ["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp", "image/tiff", "application/pdf", "application/postscript"] {
            assert!(imagem.contains(m), "imagem sem {m}");
        }
        assert!(!imagem.contains("image/svg+xml"), "svg nao e figura");
    }

    #[test]
    fn modos_e_nomes() {
        assert_eq!(Modo::de_texto("planilha"), Some(Modo::Planilha));
        assert_eq!(Modo::de_texto("arquivos"), Some(Modo::Arquivos));
        assert_eq!(Modo::de_texto("outro"), None);
        assert!(Modo::Projeto.titulo().contains(".eguatex.zip"));
    }

    const TRES: &str = r#"[
        {"id":"1AAA","nome":"main.tex","mimeType":"text/plain","parents":["PAI1"]},
        {"id":"2BBB","nome":"refs.bib","mimeType":"application/octet-stream","parents":["PAI1"]},
        {"id":"3CCC","nome":"fig 1.png","mimeType":"image/png"}
    ]"#;

    #[test]
    fn picked_aceita_varios_arquivos_no_corpo() {
        let (porta, h) = iniciar(Modo::Arquivos, Duration::from_secs(20));
        let corpo: String = TRES.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(enviar_json(porta, "/picked?t=seg", &corpo).contains("200"));
        let lista = h.join().unwrap().unwrap().unwrap();
        assert_eq!(lista.len(), 3);
        assert_eq!(lista[0], ArquivoEscolhido { id: "1AAA".into(), nome: "main.tex".into(), mime_type: "text/plain".into(), parents: vec!["PAI1".into()] });
        assert_eq!(lista[2].nome, "fig 1.png");
        assert!(lista[2].parents.is_empty());
    }

    #[test]
    fn picked_aceita_lista_curta_na_query() {
        let (porta, h) = iniciar(Modo::Arquivos, Duration::from_secs(20));
        let itens = r#"[{"id":"1AAA","nome":"a b.tex"},{"id":"2BBB","nome":"c.bib"}]"#;
        let q = Url::parse_with_params("http://x/", &[("itens", itens)]).unwrap();
        let resp = pedir(porta, &format!("/picked?t=seg&{}", q.query().unwrap()));
        assert!(resp.contains("200"));
        let lista = h.join().unwrap().unwrap().unwrap();
        assert_eq!(lista.iter().map(|a| a.nome.as_str()).collect::<Vec<_>>(), vec!["a b.tex", "c.bib"]);
    }

    #[test]
    fn picked_sem_o_segredo_da_sessao_leva_404_e_nao_vale() {
        let (porta, h) = iniciar(Modo::Arquivos, Duration::from_secs(20));
        let corpo = r#"[{"id":"1AAA","nome":"main.tex"}]"#;
        assert!(enviar_json(porta, "/picked", corpo).contains("404"));
        assert!(enviar_json(porta, "/picked?t=errado", corpo).contains("404"));
        // lista invalida (id com caracteres perigosos) e vazia: 400, o servidor segue esperando
        assert!(enviar_json(porta, "/picked?t=seg", r#"[{"id":"../x","nome":"a.tex"}]"#).contains("400"));
        assert!(enviar_json(porta, "/picked?t=seg", "[]").contains("400"));
        assert!(enviar_json(porta, "/picked?t=seg", "isto nao e json").contains("400"));
        pedir(porta, "/cancelado?t=seg");
        assert_eq!(h.join().unwrap().unwrap(), None);
    }

    #[test]
    fn ler_itens_descarta_repetidos_e_limita_o_tamanho() {
        let r = ler_itens(r#"[{"id":"A1","nome":"a.tex"},{"id":"A1","nome":"a.tex"},{"id":"B2","nome":" "}]"#).unwrap();
        assert_eq!(r.len(), 1);
        let muitos: Vec<Value> = (0..201).map(|i| serde_json::json!({"id": format!("I{i}"), "nome": "x.tex"})).collect();
        assert!(ler_itens(&Value::Array(muitos).to_string()).unwrap_err().contains("demais"));
    }
}
