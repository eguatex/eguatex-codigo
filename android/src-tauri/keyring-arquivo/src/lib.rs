//! Cofre de credenciais do EguaTex Android.
//!
//! O Android nao tem o Gerenciador de Credenciais do Windows. Cada credencial vira um
//! arquivo na pasta privada do app (`$EGUATEX_DADOS/cofre`), que so o proprio app le.
//! O texto gravado e cifrado com AES-256-GCM por uma chave que mora no Android Keystore
//! (alias `eguatex-cofre`) e nunca sai dele: quem copia o arquivo (backup, root, disco) nao
//! consegue ler o token. Arquivos antigos, em texto claro, continuam legiveis e sao cifrados
//! na primeira leitura.
//! A interface copia a do crate `keyring` 3 para o resto do codigo nao mudar.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    NoEntry,
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoEntry => write!(f, "credencial não encontrada"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Entry {
    arquivo: PathBuf,
}

/// Marca de arquivo cifrado. Sem ela, o arquivo e do formato antigo (texto claro).
const MARCA: &str = "EGX1:";

fn para_hex(dados: &[u8]) -> String {
    dados.iter().map(|b| format!("{b:02x}")).collect()
}

fn de_hex(texto: &str) -> Option<Vec<u8>> {
    if !texto.is_ascii() || texto.len() % 2 != 0 {
        return None;
    }
    (0..texto.len()).step_by(2).map(|i| u8::from_str_radix(&texto[i..i + 2], 16).ok()).collect()
}

/// Cifra com a chave do Keystore. Devolve o IV (12 bytes) seguido do texto cifrado e da etiqueta.
#[cfg(target_os = "android")]
fn cifrar(claro: &[u8]) -> std::result::Result<Vec<u8>, String> {
    keystore::cifrar(claro)
}

#[cfg(target_os = "android")]
fn decifrar(cifrado: &[u8]) -> std::result::Result<Vec<u8>, String> {
    keystore::decifrar(cifrado)
}

/// So nos testes: troca de bits simples, so para exercitar o formato do arquivo. Fora do Android
/// e fora dos testes nao ha Keystore e a gravacao falha, em vez de cifrar de forma fraca.
/// A feature `cifra-de-teste` liga a mesma troca nos testes de quem usa o crate (o `cargo test`
/// do app no computador). No Android ela nao tem efeito: la vale sempre o Keystore.
#[cfg(all(not(target_os = "android"), any(test, feature = "cifra-de-teste")))]
fn cifrar(claro: &[u8]) -> std::result::Result<Vec<u8>, String> {
    Ok(claro.iter().map(|b| b ^ 0x5a).collect())
}

#[cfg(all(not(target_os = "android"), any(test, feature = "cifra-de-teste")))]
fn decifrar(cifrado: &[u8]) -> std::result::Result<Vec<u8>, String> {
    Ok(cifrado.iter().map(|b| b ^ 0x5a).collect())
}

#[cfg(all(not(target_os = "android"), not(test), not(feature = "cifra-de-teste")))]
fn cifrar(_claro: &[u8]) -> std::result::Result<Vec<u8>, String> {
    Err("sem Android Keystore neste sistema".into())
}

#[cfg(all(not(target_os = "android"), not(test), not(feature = "cifra-de-teste")))]
fn decifrar(_cifrado: &[u8]) -> std::result::Result<Vec<u8>, String> {
    Err("sem Android Keystore neste sistema".into())
}

#[cfg(target_os = "android")]
mod keystore {
    use jni::JNIEnv;
    use jni::objects::{JByteArray, JObject, JValue};

    const ALIAS: &str = "eguatex-cofre";
    /// KeyProperties.PURPOSE_ENCRYPT (1) mais PURPOSE_DECRYPT (2).
    const FINALIDADES: i32 = 3;
    const MODO_CIFRAR: i32 = 1;
    const MODO_DECIFRAR: i32 = 2;
    const TAMANHO_IV: usize = 12;
    const BITS_ETIQUETA: i32 = 128;
    const CONSTRUTOR: &str = "Landroid/security/keystore/KeyGenParameterSpec$Builder;";

    fn erro(e: impl std::fmt::Display) -> String {
        format!("Android Keystore: {e}")
    }

    /// Busca a chave AES no Keystore; cria se ainda nao existe.
    fn chave<'a>(env: &mut JNIEnv<'a>) -> jni::errors::Result<JObject<'a>> {
        let tipo = env.new_string("AndroidKeyStore")?;
        let loja = env
            .call_static_method(
                "java/security/KeyStore",
                "getInstance",
                "(Ljava/lang/String;)Ljava/security/KeyStore;",
                &[JValue::Object(&tipo)],
            )?
            .l()?;
        env.call_method(&loja, "load", "(Ljava/security/KeyStore$LoadStoreParameter;)V", &[JValue::Object(&JObject::null())])?;
        let alias = env.new_string(ALIAS)?;
        let existe = env.call_method(&loja, "containsAlias", "(Ljava/lang/String;)Z", &[JValue::Object(&alias)])?.z()?;
        if !existe {
            let construtor = env.new_object(
                "android/security/keystore/KeyGenParameterSpec$Builder",
                "(Ljava/lang/String;I)V",
                &[JValue::Object(&alias), JValue::Int(FINALIDADES)],
            )?;
            let gcm = env.new_string("GCM")?;
            let blocos = env.new_object_array(1, "java/lang/String", &gcm)?;
            let construtor = env
                .call_method(&construtor, "setBlockModes", &format!("([Ljava/lang/String;){CONSTRUTOR}"), &[JValue::Object(&blocos)])?
                .l()?;
            let sem = env.new_string("NoPadding")?;
            let preenchimentos = env.new_object_array(1, "java/lang/String", &sem)?;
            let construtor = env
                .call_method(
                    &construtor,
                    "setEncryptionPaddings",
                    &format!("([Ljava/lang/String;){CONSTRUTOR}"),
                    &[JValue::Object(&preenchimentos)],
                )?
                .l()?;
            let construtor = env.call_method(&construtor, "setKeySize", &format!("(I){CONSTRUTOR}"), &[JValue::Int(256)])?.l()?;
            let especificacao = env
                .call_method(&construtor, "build", "()Landroid/security/keystore/KeyGenParameterSpec;", &[])?
                .l()?;
            let algoritmo = env.new_string("AES")?;
            let provedor = env.new_string("AndroidKeyStore")?;
            let gerador = env
                .call_static_method(
                    "javax/crypto/KeyGenerator",
                    "getInstance",
                    "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;",
                    &[JValue::Object(&algoritmo), JValue::Object(&provedor)],
                )?
                .l()?;
            env.call_method(&gerador, "init", "(Ljava/security/spec/AlgorithmParameterSpec;)V", &[JValue::Object(&especificacao)])?;
            env.call_method(&gerador, "generateKey", "()Ljavax/crypto/SecretKey;", &[])?;
        }
        env.call_method(
            &loja,
            "getKey",
            "(Ljava/lang/String;[C)Ljava/security/Key;",
            &[JValue::Object(&alias), JValue::Object(&JObject::null())],
        )?
        .l()
    }

    fn nova_cifra<'a>(env: &mut JNIEnv<'a>) -> jni::errors::Result<JObject<'a>> {
        let nome = env.new_string("AES/GCM/NoPadding")?;
        env.call_static_method("javax/crypto/Cipher", "getInstance", "(Ljava/lang/String;)Ljavax/crypto/Cipher;", &[JValue::Object(&nome)])?
            .l()
    }

    /// Roda `f` com o ambiente JNI da thread atual e limpa qualquer excecao Java pendente.
    fn com_jni<T>(f: impl FnOnce(&mut JNIEnv) -> jni::errors::Result<T>) -> Result<T, String> {
        let ctx = ndk_context::android_context();
        // SAFETY: o tao inicializa o contexto Android antes de abrir a janela.
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(erro)?;
        let mut env = vm.attach_current_thread().map_err(erro)?;
        let resultado = f(&mut env);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        resultado.map_err(erro)
    }

    pub fn cifrar(claro: &[u8]) -> Result<Vec<u8>, String> {
        com_jni(|env| {
            let chave = chave(env)?;
            let cifra = nova_cifra(env)?;
            env.call_method(&cifra, "init", "(ILjava/security/Key;)V", &[JValue::Int(MODO_CIFRAR), JValue::Object(&chave)])?;
            let iv = env.call_method(&cifra, "getIV", "()[B", &[])?.l()?;
            let mut tudo = env.convert_byte_array(JByteArray::from(iv))?;
            let entrada = env.byte_array_from_slice(claro)?;
            let saida = env.call_method(&cifra, "doFinal", "([B)[B", &[JValue::Object(&entrada)])?.l()?;
            tudo.extend_from_slice(&env.convert_byte_array(JByteArray::from(saida))?);
            Ok(tudo)
        })
    }

    pub fn decifrar(cifrado: &[u8]) -> Result<Vec<u8>, String> {
        if cifrado.len() <= TAMANHO_IV {
            return Err(erro("arquivo do cofre truncado"));
        }
        let (iv, corpo) = cifrado.split_at(TAMANHO_IV);
        com_jni(|env| {
            let chave = chave(env)?;
            let cifra = nova_cifra(env)?;
            let iv = env.byte_array_from_slice(iv)?;
            let especificacao = env.new_object(
                "javax/crypto/spec/GCMParameterSpec",
                "(I[B)V",
                &[JValue::Int(BITS_ETIQUETA), JValue::Object(&iv)],
            )?;
            env.call_method(
                &cifra,
                "init",
                "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
                &[JValue::Int(MODO_DECIFRAR), JValue::Object(&chave), JValue::Object(&especificacao)],
            )?;
            let entrada = env.byte_array_from_slice(corpo)?;
            let saida = env.call_method(&cifra, "doFinal", "([B)[B", &[JValue::Object(&entrada)])?.l()?;
            env.convert_byte_array(JByteArray::from(saida))
        })
    }
}

fn pasta_cofre() -> PathBuf {
    let base = std::env::var_os("EGUATEX_DADOS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("cofre")
}

/// Troca qualquer caractere fora de [A-Za-z0-9._-] por '_' para virar nome de arquivo.
fn limpo(texto: &str) -> String {
    texto
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
        .collect()
}

impl Entry {
    pub fn new(servico: &str, usuario: &str) -> Result<Entry> {
        Ok(Entry { arquivo: pasta_cofre().join(format!("{}__{}", limpo(servico), limpo(usuario))) })
    }

    pub fn set_password(&self, senha: &str) -> Result<()> {
        if let Some(pai) = self.arquivo.parent() {
            std::fs::create_dir_all(pai).map_err(|e| Error::Io(e.to_string()))?;
        }
        // Sem Keystore a gravacao falha: o token nunca cai em texto claro no disco.
        let cifrado = cifrar(senha.as_bytes()).map_err(Error::Io)?;
        // Grava num temporario e troca de nome: uma queda no meio nao deixa o cofre truncado.
        let mut temporario = self.arquivo.clone().into_os_string();
        temporario.push(".tmp");
        let temporario = PathBuf::from(temporario);
        let gravado = std::fs::write(&temporario, format!("{MARCA}{}", para_hex(&cifrado)))
            .and_then(|()| std::fs::rename(&temporario, &self.arquivo));
        if let Err(e) = gravado {
            let _ = std::fs::remove_file(&temporario);
            return Err(Error::Io(e.to_string()));
        }
        Ok(())
    }

    pub fn get_password(&self) -> Result<String> {
        let texto = match std::fs::read_to_string(&self.arquivo) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::NoEntry),
            Err(e) => return Err(Error::Io(e.to_string())),
        };
        let Some(hex) = texto.strip_prefix(MARCA) else {
            // Formato antigo, em texto claro: cifra agora. Se o Keystore falhar, o login antigo
            // continua funcionando e a cifragem fica para a proxima leitura.
            let _ = self.set_password(&texto);
            return Ok(texto);
        };
        let cifrado = de_hex(hex.trim()).ok_or_else(|| Error::Io("arquivo do cofre danificado".into()))?;
        let claro = decifrar(&cifrado).map_err(Error::Io)?;
        String::from_utf8(claro).map_err(|_| Error::Io("arquivo do cofre danificado".into()))
    }

    pub fn delete_credential(&self) -> Result<()> {
        match std::fs::remove_file(&self.arquivo) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::NoEntry),
            Err(e) => Err(Error::Io(e.to_string())),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    // Os testes trocam a variavel de ambiente EGUATEX_DADOS, que e global: rodam um por vez.
    static TRAVA: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn grava_le_e_apaga() {
        let _vez = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
        let pasta = std::env::temp_dir().join("eguatex-cofre-teste");
        unsafe { std::env::set_var("EGUATEX_DADOS", &pasta) };
        let e = Entry::new("eguatex", "google oauth").unwrap();
        assert!(matches!(e.get_password(), Err(Error::NoEntry)));
        e.set_password("segredo").unwrap();
        assert_eq!(e.get_password().unwrap(), "segredo");
        e.delete_credential().unwrap();
        assert!(matches!(e.delete_credential(), Err(Error::NoEntry)));
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn arquivo_novo_nao_guarda_o_texto_claro() {
        let _vez = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
        let pasta = std::env::temp_dir().join("eguatex-cofre-teste-cifrado");
        unsafe { std::env::set_var("EGUATEX_DADOS", &pasta) };
        let e = Entry::new("eguatex", "token").unwrap();
        e.set_password("ya29.segredo-do-token").unwrap();
        let no_disco = std::fs::read_to_string(pasta.join("cofre").join("eguatex__token")).unwrap();
        assert!(no_disco.starts_with(MARCA));
        assert!(!no_disco.contains("segredo"));
        assert_eq!(e.get_password().unwrap(), "ya29.segredo-do-token");
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn arquivo_antigo_em_texto_claro_ainda_le_e_vira_cifrado() {
        let _vez = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
        let pasta = std::env::temp_dir().join("eguatex-cofre-teste-antigo");
        unsafe { std::env::set_var("EGUATEX_DADOS", &pasta) };
        let e = Entry::new("eguatex", "antigo").unwrap();
        std::fs::create_dir_all(pasta.join("cofre")).unwrap();
        let arquivo = pasta.join("cofre").join("eguatex__antigo");
        std::fs::write(&arquivo, r#"{"access_token":"a"}"#).unwrap();
        assert_eq!(e.get_password().unwrap(), r#"{"access_token":"a"}"#);
        let depois = std::fs::read_to_string(&arquivo).unwrap();
        assert!(depois.starts_with(MARCA) && !depois.contains("access_token"));
        assert_eq!(e.get_password().unwrap(), r#"{"access_token":"a"}"#);
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn arquivo_cifrado_danificado_da_erro() {
        let _vez = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
        let pasta = std::env::temp_dir().join("eguatex-cofre-teste-danificado");
        unsafe { std::env::set_var("EGUATEX_DADOS", &pasta) };
        let e = Entry::new("eguatex", "ruim").unwrap();
        std::fs::create_dir_all(pasta.join("cofre")).unwrap();
        std::fs::write(pasta.join("cofre").join("eguatex__ruim"), "EGX1:zz").unwrap();
        assert!(matches!(e.get_password(), Err(Error::Io(_))));
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn hex_vai_e_volta() {
        assert_eq!(de_hex(&para_hex(&[0, 15, 255])).unwrap(), vec![0, 15, 255]);
        assert!(de_hex("abc").is_none());
    }
}
