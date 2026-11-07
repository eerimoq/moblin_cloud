use anyhow::{Context, Result, ensure};
use ciborium::Value;
use clap::ValueEnum;
use ring::digest::SHA256;
use ring::signature::{ECDSA_P256_SHA256_ASN1, UnparsedPublicKey};
use x509_parser::certificate::X509Certificate;
use x509_parser::oid_registry::asn1_rs::oid;
use x509_parser::pem::parse_x509_pem;
use x509_parser::prelude::FromDer;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::Deserialize;

const ROOT_CA_PEM: &[u8] = include_bytes!("apple_app_attestation_root_ca.pem");
const NONCE_EXTENSION_PREFIX: &[u8] = &[0x30, 0x24, 0xA1, 0x22, 0x04, 0x20];

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Environment {
    Production,
    Development,
}

impl Environment {
    fn aaguid(self) -> &'static [u8; 16] {
        match self {
            Environment::Production => b"appattest\0\0\0\0\0\0\0",
            Environment::Development => b"appattestdevelop",
        }
    }
}

pub type PublicKey = Vec<u8>;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attestation {
    pub key_id: Option<String>,
    pub attestation: Option<String>,
    pub attestation_challenge: Option<String>,
    pub assertion: Option<String>,
}

pub struct AppAttest {
    app_id_hash: [u8; 32],
    environment: Environment,
}

impl AppAttest {
    pub fn new(app_id: &str, environment: Environment) -> Self {
        Self {
            app_id_hash: sha256(&[app_id.as_bytes()]),
            environment,
        }
    }

    pub fn verify(&self, attestation: &Attestation, client_data: &[u8]) -> Result<()> {
        let key_id =
            base64(attestation.key_id.as_deref().context("no key id")?).context("key id")?;
        let attested = base64(
            attestation
                .attestation
                .as_deref()
                .context("no attestation")?,
        )
        .context("attestation")?;
        let challenge = base64(
            attestation
                .attestation_challenge
                .as_deref()
                .context("no attestation challenge")?,
        )
        .context("attestation challenge")?;
        let assertion = base64(attestation.assertion.as_deref().context("no assertion")?)
            .context("assertion")?;
        let public_key = self
            .verify_attestation(&attested, &key_id, &challenge)
            .context("bad attestation")?;
        self.verify_assertion(&assertion, client_data, &public_key)
            .context("bad assertion")?;
        Ok(())
    }

    pub fn verify_attestation(
        &self,
        attestation: &[u8],
        key_id: &[u8],
        challenge: &[u8],
    ) -> Result<PublicKey> {
        let attestation: Value =
            ciborium::from_reader(attestation).context("attestation is not CBOR")?;
        let format = field(&attestation, "fmt")?
            .as_text()
            .context("fmt is not a string")?;
        ensure!(format == "apple-appattest", "unexpected format {format:?}");
        let statement = field(&attestation, "attStmt")?;
        let certificates = field(statement, "x5c")?
            .as_array()
            .context("x5c is not an array")?
            .iter()
            .map(|value| {
                value
                    .as_bytes()
                    .map(Vec::as_slice)
                    .context("x5c entry is not bytes")
            })
            .collect::<Result<Vec<_>>>()?;
        let auth_data = field(&attestation, "authData")?
            .as_bytes()
            .context("authData is not bytes")?;
        let leaf = self.verify_certificate_chain(&certificates)?;
        let nonce = sha256(&[auth_data, &sha256(&[challenge])]);
        ensure!(
            certificate_nonce(&leaf)? == nonce,
            "the certificate nonce does not match the authenticator data"
        );
        let public_key = leaf.public_key().subject_public_key.data.to_vec();
        ensure!(
            public_key.len() == 65 && public_key[0] == 4,
            "the certificate public key is not an uncompressed P-256 point"
        );
        ensure!(
            sha256(&[&public_key]) == key_id,
            "the key id is not the hash of the certificate public key"
        );
        let auth_data = AuthData::parse(auth_data)?;
        ensure!(
            auth_data.rp_id_hash == self.app_id_hash,
            "the attestation is for another app"
        );
        ensure!(
            auth_data.counter == 0,
            "the attestation counter is not zero"
        );
        let credential = auth_data.credential()?;
        ensure!(
            credential.aaguid == self.environment.aaguid(),
            "the attestation is for another environment: {}",
            String::from_utf8_lossy(credential.aaguid).trim_end_matches('\0')
        );
        ensure!(
            credential.id == key_id,
            "the credential id is not the key id"
        );
        Ok(public_key)
    }

    pub fn verify_assertion(
        &self,
        assertion: &[u8],
        client_data: &[u8],
        public_key: &[u8],
    ) -> Result<u32> {
        let assertion: Value = ciborium::from_reader(assertion).context("assertion is not CBOR")?;
        let signature = field(&assertion, "signature")?
            .as_bytes()
            .context("signature is not bytes")?;
        let auth_data = field(&assertion, "authenticatorData")?
            .as_bytes()
            .context("authenticatorData is not bytes")?;
        let nonce = sha256(&[auth_data, &sha256(&[client_data])]);
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, public_key)
            .verify(&nonce, signature)
            .ok()
            .context("bad signature")?;
        let auth_data = AuthData::parse(auth_data)?;
        ensure!(
            auth_data.rp_id_hash == self.app_id_hash,
            "the assertion is for another app"
        );
        Ok(auth_data.counter)
    }

    fn verify_certificate_chain<'a>(
        &self,
        certificates: &[&'a [u8]],
    ) -> Result<X509Certificate<'a>> {
        let certificates = certificates
            .iter()
            .map(|der| {
                X509Certificate::from_der(der)
                    .map(|(_, certificate)| certificate)
                    .context("x5c entry is not a certificate")
            })
            .collect::<Result<Vec<_>>>()?;
        let (_, root) = parse_x509_pem(ROOT_CA_PEM).expect("the root certificate is valid PEM");
        let root = root.parse_x509().expect("the root certificate is valid");
        let mut certificates = certificates.into_iter();
        let leaf = certificates.next().context("x5c is empty")?;
        let mut subject = leaf.clone();
        for issuer in certificates {
            ensure!(issuer.is_ca(), "an intermediate certificate is not a CA");
            subject
                .verify_signature(Some(issuer.public_key()))
                .ok()
                .context("a certificate is not signed by the next one in x5c")?;
            subject = issuer;
        }
        subject
            .verify_signature(Some(root.public_key()))
            .ok()
            .context("the certificate chain does not end at Apple's App Attest root")?;
        Ok(leaf)
    }
}

fn certificate_nonce(certificate: &X509Certificate) -> Result<[u8; 32]> {
    let extension = certificate
        .get_extension_unique(&oid!(1.2.840.113635.100.8.2))
        .ok()
        .flatten()
        .context("no nonce extension in the certificate")?;
    extension
        .value
        .strip_prefix(NONCE_EXTENSION_PREFIX)
        .and_then(|nonce| nonce.try_into().ok())
        .context("malformed nonce extension")
}

struct AuthData<'a> {
    rp_id_hash: &'a [u8],
    counter: u32,
    rest: &'a [u8],
}

struct Credential<'a> {
    aaguid: &'a [u8],
    id: &'a [u8],
}

impl<'a> AuthData<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        ensure!(data.len() >= 37, "authenticator data is too short");
        Ok(AuthData {
            rp_id_hash: &data[..32],
            counter: u32::from_be_bytes(data[33..37].try_into().unwrap()),
            rest: &data[37..],
        })
    }

    fn credential(&self) -> Result<Credential<'a>> {
        let credential = self.rest;
        ensure!(
            credential.len() >= 18,
            "attested credential data is too short"
        );
        let length = u16::from_be_bytes(credential[16..18].try_into().unwrap()) as usize;
        ensure!(
            credential.len() >= 18 + length,
            "credential id is truncated"
        );
        Ok(Credential {
            aaguid: &credential[..16],
            id: &credential[18..18 + length],
        })
    }
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value
        .as_map()
        .context("not a CBOR map")?
        .iter()
        .find(|(key, _)| key.as_text() == Some(name))
        .map(|(_, value)| value)
        .with_context(|| format!("no {name} field"))
}

pub fn base64(text: &str) -> Result<Vec<u8>> {
    BASE64_STANDARD.decode(text).context("not base64")
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut context = ring::digest::Context::new(&SHA256);
    for part in parts {
        context.update(part);
    }
    context.finish().as_ref().try_into().unwrap()
}

#[cfg(test)]
#[path = "tests/app_attest.rs"]
mod tests;
