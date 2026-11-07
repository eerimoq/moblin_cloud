use super::*;
use ring::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair};

const APP_ID: &str = "35MFYY2JY5.co.chiff.attestation-test";
const KEY_ID: &str = "AcP/pnpoNVPIJYZOvmIvWzDvmxkFoQCE4Uu7Nk6WiAA=";
const ATTESTATION_CHALLENGE: &[u8] = b"attestation-test";
const ASSERTION_CLIENT_DATA: &[u8] = br#"{"challenge":"assertion-test"}"#;
const PUBLIC_KEY: &str = "0437c404fa2bbf8fbcf4ee7080573d5fa80c4f6cc3a22f7db43af92c394e7cd1c880c95ab422972625e8e673af1bda2b096654e9b602895601f925bb5941c53082";

fn base64(text: &str) -> Vec<u8> {
    BASE64_STANDARD.decode(text.trim()).unwrap()
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn attestation() -> Vec<u8> {
    base64(include_str!("testdata/attestation.base64"))
}

fn assertion() -> Vec<u8> {
    base64(include_str!("testdata/assertion.base64"))
}

fn app_attest() -> AppAttest {
    AppAttest::new(APP_ID, Environment::Development)
}

fn error(result: Result<impl std::fmt::Debug>) -> String {
    format!("{:#}", result.unwrap_err())
}

#[test]
fn verifies_attestation() {
    let public_key = app_attest()
        .verify_attestation(&attestation(), &base64(KEY_ID), ATTESTATION_CHALLENGE)
        .unwrap();
    assert_eq!(public_key, hex(PUBLIC_KEY));
}

fn fields() -> Attestation {
    Attestation {
        key_id: Some(KEY_ID.to_string()),
        attestation: Some(
            include_str!("testdata/attestation.base64")
                .trim()
                .to_string(),
        ),
        attestation_challenge: Some(BASE64_STANDARD.encode(ATTESTATION_CHALLENGE)),
        assertion: Some(include_str!("testdata/assertion.base64").trim().to_string()),
    }
}

#[test]
fn verifies_attestation_and_assertion_together() {
    app_attest()
        .verify(&fields(), ASSERTION_CLIENT_DATA)
        .unwrap();
    let result = app_attest().verify(&fields(), b"other client data");
    assert_eq!(error(result), "bad assertion: bad signature");
    let mut fields_without_assertion = fields();
    fields_without_assertion.assertion = None;
    let result = app_attest().verify(&fields_without_assertion, ASSERTION_CLIENT_DATA);
    assert_eq!(error(result), "no assertion");
    let mut fields_with_bad_key_id = fields();
    fields_with_bad_key_id.key_id = Some("not base64!".to_string());
    let result = app_attest().verify(&fields_with_bad_key_id, ASSERTION_CLIENT_DATA);
    assert!(error(result).starts_with("key id: not base64"));
    let mut fields_with_other_key_id = fields();
    fields_with_other_key_id.key_id = Some(BASE64_STANDARD.encode([0; 32]));
    let result = app_attest().verify(&fields_with_other_key_id, ASSERTION_CLIENT_DATA);
    assert!(error(result).starts_with("bad attestation: "));
}

#[test]
fn rejects_attestation_for_another_app() {
    let app_attest = AppAttest::new("35MFYY2JY5.co.chiff.other", Environment::Development);
    let result =
        app_attest.verify_attestation(&attestation(), &base64(KEY_ID), ATTESTATION_CHALLENGE);
    assert_eq!(error(result), "the attestation is for another app");
}

#[test]
fn rejects_attestation_for_another_environment() {
    let app_attest = AppAttest::new(APP_ID, Environment::Production);
    let result =
        app_attest.verify_attestation(&attestation(), &base64(KEY_ID), ATTESTATION_CHALLENGE);
    assert_eq!(
        error(result),
        "the attestation is for another environment: appattestdevelop"
    );
}

#[test]
fn rejects_attestation_with_another_challenge() {
    let result = app_attest().verify_attestation(&attestation(), &base64(KEY_ID), b"other");
    assert_eq!(
        error(result),
        "the certificate nonce does not match the authenticator data"
    );
}

#[test]
fn rejects_attestation_with_another_key_id() {
    let mut key_id = base64(KEY_ID);
    key_id[0] ^= 1;
    let result = app_attest().verify_attestation(&attestation(), &key_id, ATTESTATION_CHALLENGE);
    assert_eq!(
        error(result),
        "the key id is not the hash of the certificate public key"
    );
}

#[test]
fn rejects_tampered_attestation() {
    let mut attestation = attestation();
    let position = attestation.len() - 40;
    attestation[position] ^= 1;
    let result =
        app_attest().verify_attestation(&attestation, &base64(KEY_ID), ATTESTATION_CHALLENGE);
    assert_eq!(
        error(result),
        "the certificate nonce does not match the authenticator data"
    );
}

fn flip_bit_in(attestation: &mut [u8], occurrence: usize, text: &[u8]) {
    let position = attestation
        .windows(text.len())
        .enumerate()
        .filter(|(_, window)| *window == text)
        .nth(occurrence)
        .unwrap()
        .0;
    attestation[position] ^= 1;
}

#[test]
fn rejects_attestation_with_tampered_certificate() {
    let mut attestation = attestation();
    flip_bit_in(&mut attestation, 0, b"Apple App Attestation CA 1");
    let result =
        app_attest().verify_attestation(&attestation, &base64(KEY_ID), ATTESTATION_CHALLENGE);
    assert_eq!(
        error(result),
        "a certificate is not signed by the next one in x5c"
    );
}

#[test]
fn rejects_attestation_with_tampered_intermediate_certificate() {
    let mut attestation = attestation();
    flip_bit_in(&mut attestation, 1, b"Apple App Attestation CA 1");
    let result =
        app_attest().verify_attestation(&attestation, &base64(KEY_ID), ATTESTATION_CHALLENGE);
    assert_eq!(
        error(result),
        "the certificate chain does not end at Apple's App Attest root"
    );
}

#[test]
fn rejects_garbage_attestation() {
    let result =
        app_attest().verify_attestation(b"garbage", &base64(KEY_ID), ATTESTATION_CHALLENGE);
    assert!(error(result).starts_with("attestation is not CBOR"));
}

#[test]
fn verifies_assertion() {
    let counter = app_attest()
        .verify_assertion(&assertion(), ASSERTION_CLIENT_DATA, &hex(PUBLIC_KEY))
        .unwrap();
    assert_eq!(counter, 3);
}

#[test]
fn ignores_what_follows_the_assertion_counter() {
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
    let key_pair =
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap();
    let mut auth_data = sha256(&[APP_ID.as_bytes()]).to_vec();
    auth_data.extend_from_slice(&[0xc0, 0, 0, 0, 7]);
    auth_data.extend_from_slice(b"\xa1\x00\x01extension data of any length");
    let nonce = sha256(&[&auth_data, &sha256(&[ASSERTION_CLIENT_DATA])]);
    let signature = key_pair.sign(&rng, &nonce).unwrap();
    let mut assertion = Vec::new();
    ciborium::into_writer(
        &Value::Map(vec![
            (
                Value::Text("signature".into()),
                Value::Bytes(signature.as_ref().to_vec()),
            ),
            (
                Value::Text("authenticatorData".into()),
                Value::Bytes(auth_data),
            ),
        ]),
        &mut assertion,
    )
    .unwrap();
    let counter = app_attest()
        .verify_assertion(
            &assertion,
            ASSERTION_CLIENT_DATA,
            key_pair.public_key().as_ref(),
        )
        .unwrap();
    assert_eq!(counter, 7);
}

#[test]
fn rejects_assertion_with_other_client_data() {
    let result = app_attest().verify_assertion(
        &assertion(),
        br#"{"challenge":"assertion-other"}"#,
        &hex(PUBLIC_KEY),
    );
    assert_eq!(error(result), "bad signature");
}

#[test]
fn rejects_assertion_with_another_key() {
    let mut public_key = hex(PUBLIC_KEY);
    public_key[10] ^= 1;
    let result = app_attest().verify_assertion(&assertion(), ASSERTION_CLIENT_DATA, &public_key);
    assert_eq!(error(result), "bad signature");
}

#[test]
fn rejects_assertion_for_another_app() {
    let app_attest = AppAttest::new("35MFYY2JY5.co.chiff.other", Environment::Development);
    let result = app_attest.verify_assertion(&assertion(), ASSERTION_CLIENT_DATA, &hex(PUBLIC_KEY));
    assert_eq!(error(result), "the assertion is for another app");
}

#[test]
fn rejects_garbage_assertion() {
    let result = app_attest().verify_assertion(b"garbage", ASSERTION_CLIENT_DATA, &hex(PUBLIC_KEY));
    assert!(error(result).starts_with("assertion is not CBOR"));
}
