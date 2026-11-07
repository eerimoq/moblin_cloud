use super::*;

#[test]
fn auth_assertions_name_the_seller() {
    let paypal = PayPal::new(
        Client::new(),
        SANDBOX_URL.to_string(),
        "client".to_string(),
        "secret".to_string(),
        "PARTNER000001".to_string(),
        None,
        None,
    );
    let assertion = paypal.auth_assertion("SELLER0000001");
    let parts: Vec<&str> = assertion.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "eyJhbGciOiJub25lIn0=");
    let payload: Value =
        serde_json::from_slice(&BASE64_STANDARD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(
        payload,
        json!({"iss": "client", "payer_id": "SELLER0000001"})
    );
    assert_eq!(parts[2], "");
}
