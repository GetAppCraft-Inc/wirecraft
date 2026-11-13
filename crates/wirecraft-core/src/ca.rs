//! Root certificate authority: generated once per machine, stored under the data dir, and used to mint
//! leaf certificates for intercepted hosts on the fly (cached per host).

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType, SerialNumber,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use serde::Serialize;
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

const CERT_FILE: &str = "wirecraft-root.pem";
const KEY_FILE: &str = "wirecraft-root.key";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaInfo {
    pub common_name: String,
    pub key_algorithm: String,
    pub not_before: i64,
    pub not_after: i64,
    pub sha256: String,
    pub dir: String,
    pub cert_path: String,
}

pub struct CertAuthority {
    issuer: Issuer<'static, KeyPair>,
    ca_der: CertificateDer<'static>,
    ca_pem: String,
    leaf_key: KeyPair,
    leaf_signer: Arc<dyn rustls::sign::SigningKey>,
    cache: Mutex<HashMap<String, Arc<CertifiedKey>>>,
    info: CaInfo,
}

impl std::fmt::Debug for CertAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertAuthority").field("info", &self.info).finish()
    }
}

pub fn default_ca_dir() -> PathBuf {
    crate::paths::data_dir().join("ca")
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "localhost".into())
}

impl CertAuthority {
    /// Load the CA from `dir`, generating (and persisting) a new one when absent.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);
        // CAs made before the rename use xwire-root.* file names: keep using that CA under the new names
        if !cert_path.exists() && dir.join("xwire-root.pem").exists() && dir.join("xwire-root.key").exists() {
            for ext in ["pem", "key", "crt", "der"] {
                let _ = std::fs::rename(dir.join(format!("xwire-root.{ext}")), dir.join(format!("wirecraft-root.{ext}")));
            }
        }
        if cert_path.exists() && key_path.exists() {
            let pem = std::fs::read_to_string(&cert_path)?;
            let key = KeyPair::from_pem(&std::fs::read_to_string(&key_path)?)?;
            return Self::from_parts(dir, pem, key);
        }
        Self::generate(dir)
    }

    /// Generate a fresh CA, overwriting any existing one in `dir`.
    pub fn generate(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let key = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("WireCraft Root CA · {}", hostname()));
        dn.push(DnType::OrganizationName, "WireCraft");
        dn.push(DnType::OrganizationalUnitName, "Generated on this machine — never shared");
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(1);
        params.not_after = now + Duration::days(365);
        params.serial_number = Some(random_serial());
        let cert = params.self_signed(&key)?;
        let pem = cert.pem();
        std::fs::write(dir.join(CERT_FILE), &pem)?;
        std::fs::write(dir.join("wirecraft-root.crt"), &pem)?;
        std::fs::write(dir.join("wirecraft-root.der"), cert.der().as_ref())?;
        write_private(&dir.join(KEY_FILE), key.serialize_pem().as_bytes())?;
        Self::from_parts(dir, pem, key)
    }

    fn from_parts(dir: &Path, pem: String, key: KeyPair) -> Result<Self> {
        let der: CertificateDer<'static> = rustls_pemfile_der(&pem)?;
        let (_, x509) = x509_parser::parse_x509_certificate(der.as_ref()).context("parsing CA certificate")?;
        let cn = x509
            .subject()
            .iter_common_name()
            .next()
            .and_then(|c| c.as_str().ok())
            .unwrap_or("WireCraft Root CA")
            .to_string();
        let info = CaInfo {
            common_name: cn,
            key_algorithm: "ECDSA P-256".into(),
            not_before: x509.validity().not_before.timestamp(),
            not_after: x509.validity().not_after.timestamp(),
            sha256: Sha256::digest(der.as_ref()).iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":"),
            dir: dir.display().to_string(),
            cert_path: dir.join(CERT_FILE).display().to_string(),
        };
        // the .crt / .der copies may be missing for CAs from older builds
        if !dir.join("wirecraft-root.der").exists() {
            let _ = std::fs::write(dir.join("wirecraft-root.der"), der.as_ref());
            let _ = std::fs::write(dir.join("wirecraft-root.crt"), &pem);
        }
        let issuer = Issuer::from_ca_cert_pem(&pem, key)?;
        let leaf_key = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
        let leaf_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));
        let leaf_signer = rustls::crypto::ring::sign::any_supported_type(&leaf_der)?;
        Ok(Self { issuer, ca_der: der, ca_pem: pem, leaf_key, leaf_signer, cache: Mutex::new(HashMap::new()), info })
    }

    pub fn info(&self) -> &CaInfo {
        &self.info
    }

    pub fn pem(&self) -> &str {
        &self.ca_pem
    }

    pub fn der(&self) -> &[u8] {
        self.ca_der.as_ref()
    }

    /// A leaf certificate (chain: leaf, CA) for `host`, minted on first use.
    pub fn cert_for(&self, host: &str) -> Result<Arc<CertifiedKey>> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if let Some(c) = self.cache.lock().get(&host) {
            return Ok(c.clone());
        }
        let mut params = CertificateParams::default();
        let san = match host.parse::<IpAddr>() {
            Ok(ip) => SanType::IpAddress(ip),
            Err(_) => SanType::DnsName(host.clone().try_into()?),
        };
        params.subject_alt_names = vec![san];
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, host.clone());
        dn.push(DnType::OrganizationName, "WireCraft");
        params.distinguished_name = dn;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(1);
        params.not_after = now + Duration::days(300);
        params.serial_number = Some(random_serial());
        let cert = params.signed_by(&self.leaf_key, &self.issuer)?;
        let ck = Arc::new(CertifiedKey::new(vec![cert.der().clone(), self.ca_der.clone()], self.leaf_signer.clone()));
        let mut cache = self.cache.lock();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(host, ck.clone());
        Ok(ck)
    }

    /// rustls server config that serves a minted certificate for the SNI name, or `fallback_host` without SNI.
    pub fn server_config(self: &Arc<Self>, fallback_host: &str, alpn: &[&[u8]]) -> Arc<rustls::ServerConfig> {
        let resolver = Arc::new(HostResolver { ca: self.clone(), fallback: fallback_host.to_string() });
        let mut cfg = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_no_client_auth()
            .with_cert_resolver(resolver);
        cfg.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
        Arc::new(cfg)
    }
}

#[derive(Debug)]
struct HostResolver {
    ca: Arc<CertAuthority>,
    fallback: String,
}

impl ResolvesServerCert for HostResolver {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let host = hello.server_name().unwrap_or(&self.fallback);
        match self.ca.cert_for(host) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!("minting certificate for {host}: {e:#}");
                None
            }
        }
    }
}

fn random_serial() -> SerialNumber {
    let mut b = [0u8; 16];
    // ring's SystemRandom is already linked through rustls
    use rustls::crypto::ring::default_provider;
    let _ = default_provider().secure_random.fill(&mut b);
    b[0] &= 0x7f;
    SerialNumber::from_slice(&b)
}

fn rustls_pemfile_der(pem: &str) -> Result<CertificateDer<'static>> {
    use rustls::pki_types::pem::PemObject;
    Ok(CertificateDer::from_pem_slice(pem.as_bytes())?.into_owned())
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).mode(0o600).open(path)?;
        f.write_all(data)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, data)?;
        Ok(())
    }
}
