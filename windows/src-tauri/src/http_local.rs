//! Leitura limitada de pedidos nos servidores locais (127.0.0.1) do login Google e do Picker.
//!
//! Qualquer programa do aparelho alcanca esses servidores. Sem limite, uma "linha" de 400 MB
//! sem quebra de linha vira 527 MB na memoria do app. A linha de pedido e os cabecalhos, juntos,
//! passam por um `Take` de 8 KB; o corpo de um POST, depois, tem o limite proprio de quem o le.

use std::io::{BufReader, Read, Take};
use std::net::TcpStream;

/// Bytes aceitos para a linha de pedido mais os cabecalhos.
pub(crate) const MAX_PEDIDO_BYTES: u64 = 8192;
/// Cabecalhos aceitos num pedido.
pub(crate) const MAX_CABECALHOS: usize = 64;

/// Leitor da conexao que entrega no maximo 8 KB (`BufReader::take`): o que passar disso nao e lido.
pub(crate) fn leitor_limitado(stream: &TcpStream) -> BufReader<Take<&TcpStream>> {
    BufReader::new(stream.take(MAX_PEDIDO_BYTES))
}

/// A linha veio inteira? Uma linha que bateu no teto de 8 KB sem chegar ao fim e recusada;
/// uma linha curta sem quebra no fim (cliente que fechou a conexao) continua aceita.
pub(crate) fn linha_completa(linha: &str) -> bool {
    linha.ends_with('\n') || (linha.len() as u64) < MAX_PEDIDO_BYTES
}

/// Resposta curta para o pedido grande demais.
pub(crate) const RESPOSTA_431: &[u8] =
    b"HTTP/1.1 431 Request Header Fields Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::{BufRead, Write};
    use std::net::TcpListener;

    #[test]
    fn linha_de_8kb_sem_quebra_e_recusada_e_curta_e_aceita() {
        assert!(!linha_completa(&"a".repeat(8192)));
        assert!(linha_completa(&format!("{}\n", "a".repeat(8191))));
        assert!(linha_completa("GET / HTTP/1.1"));
        assert!(linha_completa("GET / HTTP/1.1\r\n"));
    }

    #[test]
    fn leitor_limitado_nao_puxa_mais_que_8kb_da_conexao() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let cliente = std::thread::spawn(move || {
            let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
            let _ = s.write_all(&vec![b'a'; 100_000]);
        });
        let (s, _) = l.accept().unwrap();
        cliente.join().unwrap();
        let mut leitor = leitor_limitado(&s);
        let mut linha = String::new();
        leitor.read_line(&mut linha).unwrap();
        assert_eq!(linha.len(), 8192);
        assert!(!linha_completa(&linha));
    }
}
