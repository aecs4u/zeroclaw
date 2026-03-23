//! TLS/HTTPS support for the ZeroClaw gateway.

use anyhow::{Context, Result};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

pub fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<Arc<ServerConfig>> {
    let cert_pem = std::fs::read(cert_path)
        .with_context(|| format!("failed to read TLS certificate: {}", cert_path.display()))?;
    let key_pem = std::fs::read(key_path)
        .with_context(|| format!("failed to read TLS private key: {}", key_path.display()))?;
    tls_config_from_pem(&cert_pem, &key_pem)
}

fn tls_config_from_pem(cert_pem: &[u8], key_pem: &[u8]) -> Result<Arc<ServerConfig>> {
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut &*cert_pem)
        .collect::<Result<Vec<_>, _>>()
        .context("failed to parse TLS certificate chain")?;
    if certs.is_empty() {
        anyhow::bail!("no certificates found in PEM data");
    }
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut &*key_pem)
        .context("failed to parse TLS private key")?
        .ok_or_else(|| anyhow::anyhow!("no private key found in PEM data"))?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("failed to build TLS server configuration")?;
    Ok(Arc::new(config))
}

#[cfg(feature = "gateway-tls")]
pub fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    use rcgen::{CertificateParams, DistinguishedName, KeyPair};
    tracing::info!("generating self-signed TLS certificate for localhost");
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "ZeroClaw Gateway");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "ZeroClaw");
    params.subject_alt_names = vec![
        rcgen::SanType::DnsName(rcgen::Ia5String::try_from("localhost").unwrap()),
        rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        rcgen::SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
    ];
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(365);
    let key_pair = KeyPair::generate().context("failed to generate TLS key pair")?;
    let cert = params
        .self_signed(&key_pair)
        .context("failed to generate self-signed certificate")?;
    Ok((
        cert.pem().into_bytes(),
        key_pair.serialize_pem().into_bytes(),
    ))
}

pub fn create_tls_acceptor(
    cert_path: Option<&str>,
    key_path: Option<&str>,
    #[allow(unused_variables)] self_signed: bool,
) -> Result<TlsAcceptor> {
    #[cfg(feature = "gateway-tls")]
    if self_signed {
        let (cert_pem, key_pem) = generate_self_signed_cert()?;
        let config = tls_config_from_pem(&cert_pem, &key_pem)?;
        return Ok(TlsAcceptor::from(config));
    }
    let cert_path = cert_path
        .ok_or_else(|| anyhow::anyhow!("TLS enabled but gateway.tls.cert_path not set"))?;
    let key_path =
        key_path.ok_or_else(|| anyhow::anyhow!("TLS enabled but gateway.tls.key_path not set"))?;
    let config = load_tls_config(Path::new(cert_path), Path::new(key_path))?;
    Ok(TlsAcceptor::from(config))
}

pub async fn accept_tls(
    acceptor: &TlsAcceptor,
    stream: TcpStream,
) -> Result<tokio_rustls::server::TlsStream<TcpStream>> {
    acceptor
        .accept(stream)
        .await
        .context("TLS handshake failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_cert() {
        assert!(tls_config_from_pem(b"", b"").is_err());
    }

    #[test]
    fn rejects_invalid_pem() {
        assert!(tls_config_from_pem(b"not a cert", b"not a key").is_err());
    }

    #[test]
    fn requires_paths_when_not_self_signed() {
        let r = create_tls_acceptor(None, None, false);
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("cert_path"));
    }

    #[cfg(feature = "gateway-tls")]
    #[test]
    fn self_signed_cert_is_valid_pem() {
        let (cert, key) = generate_self_signed_cert().unwrap();
        assert!(cert.starts_with(b"-----BEGIN CERTIFICATE-----"));
        assert!(key.starts_with(b"-----BEGIN PRIVATE KEY-----"));
    }

    #[cfg(feature = "gateway-tls")]
    #[test]
    fn self_signed_acceptor_works() {
        assert!(create_tls_acceptor(None, None, true).is_ok());
    }
}
