use anyhow::Result;
use rustls::pki_types::CertificateDer;

use super::certs::{pem_to_der, CertBytes};

pub struct EmbedCert {
    pub key_bytes: &'static [u8],
    pub cert_bytes: &'static [u8],
}

impl EmbedCert {
    pub fn get() -> Self {
        // CA 证书： openssl req -newkey rsa:2048 -new -nodes -x509 -days 3650 -keyout key.pem -out cert.pem
        // 终端证书： openssl req -newkey rsa:2048 -new -nodes -x509 -days 3650 -keyout key.pem -out cert.pem -subj "/CN=localhost" -addext "basicConstraints=CA:FALSE" -addext "subjectAltName=DNS:localhost"

        let cert_bytes = include_bytes!("self_signed_certs/cert.pem");
        let key_bytes =  include_bytes!("self_signed_certs/key.pem");

        Self {
            key_bytes,
            cert_bytes,
        }
    }

    #[inline]
    pub fn cert_der(&self) -> Result<Vec<CertificateDer<'static>>> {
        pem_to_der(&self.cert_bytes)
    }

    pub fn to_bytes(&self) -> CertBytes {
        CertBytes {
            key_bytes: self.key_bytes.into(),
            key_ext: None,
            cert_bytes: self.cert_bytes.into(),
            cert_ext: None,
        }
    }
}



