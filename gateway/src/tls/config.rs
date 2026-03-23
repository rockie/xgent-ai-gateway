use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rustls::ServerConfig;
use tonic::transport::{Certificate, Identity, ServerTlsConfig};

/// Build a rustls `ServerConfig` for HTTP/HTTPS with standard TLS (no client auth).
///
/// Sets ALPN protocols to `h2` and `http/1.1` for HTTP/2 negotiation.
pub fn build_http_tls_config(
    tls: &crate::config::TlsConfig,
) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let cert_file = File::open(&tls.cert_path)?;
    let key_file = File::open(&tls.key_path)?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))?
        .ok_or("no private key found in key file")?;

    let mut config = ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(certs, key)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(config)
}

/// Build a tonic `ServerTlsConfig` for gRPC with mTLS (requires client certificate).
pub fn build_grpc_tls_config(
    tls: &crate::config::GrpcTlsConfig,
) -> Result<ServerTlsConfig, Box<dyn std::error::Error>> {
    let cert = std::fs::read_to_string(&tls.server.cert_path)?;
    let key = std::fs::read_to_string(&tls.server.key_path)?;
    let client_ca = std::fs::read_to_string(&tls.client_ca_path)?;

    let config = ServerTlsConfig::new()
        .identity(Identity::from_pem(&cert, &key))
        .client_ca_root(Certificate::from_pem(&client_ca));

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_test_cert_files() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let key_pair = rcgen::KeyPair::generate().unwrap();
        let params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let cert = params.self_signed(&key_pair).unwrap();

        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&cert_path, cert.pem()).unwrap();
        std::fs::write(&key_path, key_pair.serialize_pem()).unwrap();

        (dir, cert_path, key_path)
    }

    #[test]
    fn test_build_http_tls_config_valid() {
        let (_dir, cert_path, key_path) = generate_test_cert_files();
        let tls = crate::config::TlsConfig {
            cert_path: cert_path.to_str().unwrap().to_string(),
            key_path: key_path.to_str().unwrap().to_string(),
        };
        let config = build_http_tls_config(&tls).expect("should build TLS config");
        assert!(
            config.alpn_protocols.contains(&b"h2".to_vec()),
            "must have h2 ALPN"
        );
        assert!(
            config.alpn_protocols.contains(&b"http/1.1".to_vec()),
            "must have http/1.1 ALPN"
        );
    }

    #[test]
    fn test_build_http_tls_config_missing_cert() {
        let tls = crate::config::TlsConfig {
            cert_path: "/nonexistent/cert.pem".to_string(),
            key_path: "/nonexistent/key.pem".to_string(),
        };
        assert!(build_http_tls_config(&tls).is_err());
    }

    #[test]
    fn test_build_grpc_tls_config_valid() {
        let key_pair = rcgen::KeyPair::generate().unwrap();
        let mut ca_params =
            rcgen::CertificateParams::new(vec!["Test CA".to_string()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_cert = ca_params.self_signed(&key_pair).unwrap();

        let server_key = rcgen::KeyPair::generate().unwrap();
        let server_params =
            rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &key_pair)
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("server.pem");
        let key_path = dir.path().join("server-key.pem");
        let ca_path = dir.path().join("ca.pem");
        std::fs::write(&cert_path, server_cert.pem()).unwrap();
        std::fs::write(&key_path, server_key.serialize_pem()).unwrap();
        std::fs::write(&ca_path, ca_cert.pem()).unwrap();

        let tls = crate::config::GrpcTlsConfig {
            server: crate::config::TlsConfig {
                cert_path: cert_path.to_str().unwrap().to_string(),
                key_path: key_path.to_str().unwrap().to_string(),
            },
            client_ca_path: ca_path.to_str().unwrap().to_string(),
        };
        assert!(build_grpc_tls_config(&tls).is_ok());
    }

    #[test]
    fn test_admin_config_default() {
        let admin = crate::config::AdminConfig::default();
        assert!(admin.username.is_none(), "default admin username should be None");
    }

    #[test]
    fn test_config_backward_compatible_no_tls() {
        // GatewayConfig should deserialize without TLS fields
        let toml_str = r#"
[grpc]
enabled = true
listen_addr = "0.0.0.0:50051"

[http]
enabled = true
listen_addr = "0.0.0.0:8080"

[redis]
url = "redis://127.0.0.1:6379"
result_ttl_secs = 86400

[queue]
stream_maxlen = 10000
block_timeout_ms = 5000
"#;
        let cfg: crate::config::GatewayConfig =
            toml::from_str(toml_str).expect("should deserialize without TLS fields");
        assert!(cfg.grpc.tls.is_none());
        assert!(cfg.http.tls.is_none());
        assert!(cfg.admin.username.is_none());
    }
}
