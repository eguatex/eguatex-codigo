//! Pecas do assistente com a chave do Gemini: codigos de resultado do teste da chave, textos de
//! erro em portugues, eventos para a tela e o servidor falso dos testes.

use std::time::Duration;

use serde_json::Value;

/// Resultado do teste da chave e classificacao dos erros HTTP do Gemini.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codigo {
    Ok,
    Invalida,
    SemInternet,
    Limite,
    Tempo,
    Ocupado,
    Bloqueada,
    /// Modelo que a conta nao pode usar (404).
    Indisponivel,
    Erro,
}

impl Codigo {
    /// Codigo que a frente recebe; o texto em portugues vem do i18n (`ia.chave.resultado.*`).
    pub fn texto(self) -> &'static str {
        match self {
            Codigo::Ok => "ok",
            Codigo::Invalida => "invalida",
            Codigo::SemInternet => "sem-internet",
            Codigo::Limite => "limite",
            Codigo::Tempo => "tempo",
            Codigo::Ocupado => "ocupado",
            Codigo::Bloqueada => "bloqueada",
            Codigo::Indisponivel | Codigo::Erro => "erro",
        }
    }
}

/// Erro de envio do reqwest: tempo esgotado ou sem conexao.
pub fn codigo_de_envio(e: &reqwest::Error) -> Codigo {
    if e.is_timeout() {
        Codigo::Tempo
    } else if e.is_connect() {
        Codigo::SemInternet
    } else {
        Codigo::Erro
    }
}

/// Texto do chat para um erro do Gemini. `detalhe` e a mensagem original da API (ja sem a chave).
pub fn mensagem(codigo: Codigo, status: u16, detalhe: &str) -> String {
    match codigo {
        Codigo::Ok => String::new(),
        Codigo::Invalida => "A chave do Gemini foi recusada. Abra o assistente de IA, remova a chave e cole uma chave nova.".into(),
        Codigo::Bloqueada => "A chave do Gemini não tem permissão para este pedido (chave bloqueada ou país sem acesso).".into(),
        Codigo::Limite => "Limite de uso do Gemini atingido. Tente de novo mais tarde.".into(),
        Codigo::Ocupado => "O Gemini está sobrecarregado agora. Tente de novo em alguns minutos.".into(),
        Codigo::Indisponivel => "O modelo escolhido não está disponível na sua conta do Gemini. Escolha outro modelo no assistente.".into(),
        Codigo::SemInternet => MSG_SEM_REDE.into(),
        Codigo::Tempo => "O Gemini demorou demais para responder. Tente de novo em alguns minutos.".into(),
        Codigo::Erro if status == 400 => {
            format!("O Gemini não aceitou o pedido (erro 400). Limpe a conversa e tente de novo. Detalhe: {detalhe}")
        }
        // sem status HTTP (erro dentro do stream): nada de "erro 0"
        Codigo::Erro if status == 0 => msg_erro_no_stream(detalhe),
        Codigo::Erro if detalhe.is_empty() => format!("O Gemini devolveu o erro {status}."),
        Codigo::Erro => format!("O Gemini devolveu o erro {status}: {detalhe}"),
    }
}

/// Erro que o Gemini mandou dentro do stream, depois do 200, sem numero de status.
pub fn msg_erro_no_stream(detalhe: &str) -> String {
    if detalhe.is_empty() {
        "O Gemini parou a resposta com um erro. Tente de novo.".into()
    } else {
        format!("O Gemini parou a resposta com um erro. Tente de novo. Detalhe: {detalhe}")
    }
}

pub const MSG_SEM_REDE: &str = "Sem conexão com a internet. Confira a rede e tente de novo.";

/// Stream acabou sem nenhuma resposta aproveitavel.
pub const MSG_SEM_RESPOSTA: &str = "A IA não respondeu; tente de novo.";

/// Vai ao fim do texto quando o stream acaba sem o `finishReason` do Gemini: o texto saiu pela
/// metade.
pub const AVISO_INTERROMPIDA: &str = "\n\n(A resposta foi interrompida antes do fim. Peça para continuar ou tente de novo.)";

/// Sem chave guardada.
pub const MSG_SEM_CHAVE: &str = "Falta a chave do Gemini. Abra o assistente de IA, clique em Criar minha chave e cole a chave no campo.";

/// Quem recebe os eventos do stream (`ia-pedaco`, `ia-ocupado`): a janela do app ou, nos testes,
/// um gravador.
pub trait Emissor: Send + Sync {
    fn emit(&self, evento: &str, dados: Value);
}

impl<R: tauri::Runtime> Emissor for tauri::AppHandle<R> {
    fn emit(&self, evento: &str, dados: Value) {
        let _ = tauri::Emitter::emit(self, evento, dados);
    }
}

/// Cliente HTTP com limite total (teste da chave).
pub fn cliente_com_limite(limite: Duration) -> reqwest::Client {
    reqwest::Client::builder().timeout(limite).build().unwrap_or_default()
}

/// Cliente do stream: sem limite total (uma resposta longa passa de 2 min), so de conexao.
pub fn cliente_stream() -> reqwest::Client {
    reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).build().unwrap_or_default()
}

// ---------- servidor falso dos testes ----------

#[cfg(test)]
pub(crate) mod teste {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::sync::mpsc::{Receiver, channel};

    use serde_json::Value;

    /// Resposta pronta do servidor falso: status, tipo e corpo.
    pub struct Resposta {
        pub status: u16,
        pub tipo: &'static str,
        pub corpo: String,
    }

    pub fn json(status: u16, corpo: &str) -> Resposta {
        Resposta { status, tipo: "application/json", corpo: corpo.into() }
    }

    pub fn sse(corpo: &str) -> Resposta {
        Resposta { status: 200, tipo: "text/event-stream", corpo: corpo.into() }
    }

    /// Pedido recebido: linha do pedido, cabecalhos (nome em minusculas) e corpo.
    #[derive(Debug, Clone)]
    pub struct Pedido {
        pub linha: String,
        pub cabecalhos: Vec<(String, String)>,
        pub corpo: String,
    }

    impl Pedido {
        pub fn cabecalho(&self, nome: &str) -> Option<&str> {
            self.cabecalhos.iter().find(|(n, _)| n == nome).map(|(_, v)| v.as_str())
        }
        pub fn json(&self) -> Value {
            serde_json::from_str(&self.corpo).unwrap_or(Value::Null)
        }
        /// O pedido inteiro como chegou, para procurar a chave em qualquer lugar.
        pub fn tudo(&self) -> String {
            let cab: Vec<String> = self.cabecalhos.iter().map(|(n, v)| format!("{n}: {v}")).collect();
            format!("{}\n{}\n\n{}", self.linha, cab.join("\n"), self.corpo)
        }
    }

    /// Servidor HTTP/1.1 em 127.0.0.1 que responde `respostas` na ordem, uma por conexao.
    /// Devolve o endereco base e o canal com os pedidos recebidos.
    pub fn servidor(respostas: Vec<Resposta>) -> (String, Receiver<Pedido>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for r in respostas {
                let Ok((s, _)) = l.accept() else { return };
                let mut leitor = BufReader::new(s.try_clone().unwrap());
                let mut linha = String::new();
                leitor.read_line(&mut linha).ok();
                let mut cabecalhos = Vec::new();
                let mut tamanho = 0usize;
                loop {
                    let mut l2 = String::new();
                    if leitor.read_line(&mut l2).unwrap_or(0) == 0 || l2.trim().is_empty() {
                        break;
                    }
                    if let Some((n, v)) = l2.trim_end().split_once(':') {
                        let n = n.trim().to_lowercase();
                        if n == "content-length" {
                            tamanho = v.trim().parse().unwrap_or(0);
                        }
                        cabecalhos.push((n, v.trim().to_string()));
                    }
                }
                let mut corpo = vec![0u8; tamanho];
                leitor.read_exact(&mut corpo).ok();
                let _ = tx.send(Pedido {
                    linha: linha.trim_end().to_string(),
                    cabecalhos,
                    corpo: String::from_utf8_lossy(&corpo).into_owned(),
                });
                let mut s = s;
                let _ = write!(
                    s,
                    "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    r.status,
                    r.tipo,
                    r.corpo.len(),
                    r.corpo
                );
                let _ = s.flush();
            }
        });
        (base, rx)
    }

    /// Endereco de uma porta fechada (nada escuta): o envio falha com erro de conexao.
    pub fn porta_fechada() -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        drop(l);
        format!("http://127.0.0.1:{porta}")
    }

    /// Servidor que aceita a conexao e nunca responde.
    pub fn servidor_mudo() -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        std::thread::spawn(move || {
            let mut abertas = Vec::new();
            for s in l.incoming().take(4).flatten() {
                abertas.push(s);
            }
            std::thread::sleep(std::time::Duration::from_secs(30));
        });
        base
    }

    /// Emissor que guarda os eventos.
    #[derive(Default)]
    pub struct Gravador(pub Mutex<Vec<(String, Value)>>);

    impl super::Emissor for Gravador {
        fn emit(&self, evento: &str, dados: Value) {
            self.0.lock().unwrap().push((evento.to_string(), dados));
        }
    }

    impl Gravador {
        pub fn texto(&self) -> String {
            self.0
                .lock()
                .unwrap()
                .iter()
                .filter(|(e, _)| e == "ia-pedaco")
                .filter_map(|(_, d)| d["texto"].as_str().map(String::from))
                .collect()
        }
    }

    /// Canal de cancelamento que nunca cancela.
    pub fn sem_cancelar() -> tokio::sync::watch::Receiver<bool> {
        let (tx, rx) = tokio::sync::watch::channel(false);
        std::mem::forget(tx);
        rx
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codigos_para_a_frente() {
        let todos = [
            (Codigo::Ok, "ok"),
            (Codigo::Invalida, "invalida"),
            (Codigo::SemInternet, "sem-internet"),
            (Codigo::Limite, "limite"),
            (Codigo::Tempo, "tempo"),
            (Codigo::Ocupado, "ocupado"),
            (Codigo::Bloqueada, "bloqueada"),
            (Codigo::Indisponivel, "erro"),
            (Codigo::Erro, "erro"),
        ];
        for (c, t) in todos {
            assert_eq!(c.texto(), t);
        }
    }

    #[test]
    fn mensagens_em_portugues_sem_travessao() {
        for c in [Codigo::Invalida, Codigo::Bloqueada, Codigo::Limite, Codigo::Ocupado, Codigo::Indisponivel, Codigo::Tempo, Codigo::Erro] {
            let m = mensagem(c, 400, "x");
            assert!(!m.is_empty());
            assert!(m.contains("Gemini") || m == MSG_SEM_REDE);
            assert!(!m.contains('\u{2014}') && !m.contains('\u{2013}'));
        }
        assert!(MSG_SEM_CHAVE.contains("cole a chave"));
        assert!(mensagem(Codigo::Erro, 400, "x").contains("erro 400"));
        // sem status: nada de "erro 0"
        assert!(!mensagem(Codigo::Erro, 0, "").contains("erro 0"));
    }
}
