use std::sync::{Arc, LazyLock};
use anyhow::Result;
// use rustls::pki_types::ServerName; // WebPkiVerifier
use rustls::{client::WebPkiServerVerifier, crypto::CryptoProvider, pki_types::CertificateDer};

use super::embed::EmbedCert;

#[derive(Debug)]
pub struct CustomCertVerifier {
    webpki_verifier: Option<Arc<WebPkiServerVerifier>>,
    extra_certs: Vec<CertificateDer<'static>>,
}

impl CustomCertVerifier {

    pub fn new() -> Arc<Self> {
        let r = EmbedCert::get().cert_der();
        match r {
            Ok(v) => Self::new_extra_certs(v),
            Err(e) => unreachable!("{e:?}"),
        }
    }


    pub fn new_extra_certs(
        extra_certs: Vec<CertificateDer<'static>>,
    ) -> Arc<Self> {
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.into(),
        }.into();

        let r = WebPkiServerVerifier::builder(root_store).build();

        match r {
            Ok(v) => Self {
                webpki_verifier: Some(v),
                extra_certs,
            }.into(),
            Err(e) => unreachable!("{e:?}"),
        }
    }

    pub fn new_certs_only(
        extra_certs: Vec<CertificateDer<'static>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            webpki_verifier: None,
            extra_certs,
        })
    }

    pub fn new_dummy() -> Arc<Self> {
        Arc::new(Self {
            webpki_verifier: None,
            extra_certs: vec![],
        })
    }
}

impl rustls::client::danger::ServerCertVerifier for CustomCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {

        for cert in self.extra_certs.iter() {
            if end_entity == cert {
                return Ok(rustls::client::danger::ServerCertVerified::assertion())
            }
        }

        match &self.webpki_verifier {
            Some(verifier) => verifier.verify_server_cert(end_entity, intermediates, server_name, ocsp, now),
            None => Ok(rustls::client::danger::ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {

        match &self.webpki_verifier {
            Some(verifier) => verifier.verify_tls12_signature(message, cert, dss),
            None => rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &DEF_PROVIDER.signature_verification_algorithms,
            ),
        }
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {

        match &self.webpki_verifier {
            Some(verifier) => verifier.verify_tls13_signature(message, cert, dss),
            None => rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &DEF_PROVIDER.signature_verification_algorithms,
            ),
        }
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        match &self.webpki_verifier {
            Some(verifier) => verifier.supported_verify_schemes(),
            None => DEF_PROVIDER.signature_verification_algorithms.supported_schemes(),
        }
    }
}

const DEF_PROVIDER: LazyLock<CryptoProvider> = LazyLock::new(|| rustls::crypto::ring::default_provider());
