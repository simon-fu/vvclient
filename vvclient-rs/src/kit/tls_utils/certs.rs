
use std::path::Path;

use anyhow::{Result, Context};
use rustls::pki_types::CertificateDer;


#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CertBytes {
    pub key_bytes: Vec<u8>,
    pub key_ext: Option<String>,    // file extension
    pub cert_bytes: Vec<u8>,
    pub cert_ext: Option<String>,   // file extension
}

impl CertBytes {
    pub async fn load_fils(key_path: &Path, cert_path: &Path) -> Result<Self> {
        Ok(Self {
            key_bytes: tokio::fs::read(key_path).await.with_context(||format!("failed to load key file [{key_path:?}]"))?,
            key_ext: Self::path_ext_string(key_path),

            cert_bytes: tokio::fs::read(cert_path).await.with_context(||format!("failed to load cert file [{cert_path:?}]"))?,
            cert_ext: Self::path_ext_string(cert_path),
        })
    }

    #[inline]
    fn path_ext_string(path: &Path) -> Option<String> {
        path.extension()
        .map(|x|x.to_str().to_owned()).unwrap_or(None)
        .map(|x|x.to_owned())
    }

    pub fn into_rust_server_config(self) -> Result<rustls::ServerConfig> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    
        let key = if self.key_ext.as_deref().map_or(false, |x| x == "der") {
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_bytes))
            // rustls::PrivateKey(key)
        } else {
            rustls_pemfile::private_key(&mut &*self.key_bytes)
            .context("malformed PKCS #1 private key")?
            .ok_or_else(|| anyhow::Error::msg("no private keys found"))?
    
        };
    
        
        let cert_chain = if self.cert_ext.as_deref().map_or(false, |x| x == "der") {
            vec![CertificateDer::from(self.cert_bytes)]
        } else {
            rustls_pemfile::certs(&mut &*self.cert_bytes)
                .collect::<Result<_, _>>()
                .context("invalid PEM-encoded certificate")?
        };
    
    
        let server_crypto = rustls::ServerConfig::builder()
        // .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)?;
    
        return Ok(server_crypto)
    }
}


#[inline]
pub fn pem_to_der<'a>(mut src: &'a [u8]) -> Result<Vec<CertificateDer<'static>>> {
    rustls_pemfile::certs(&mut src)
    .collect::<Result<_, _>>()
    .context("pem to der failed")
}



