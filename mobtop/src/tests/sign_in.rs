use super::*;

#[test]
fn the_code_is_taken_from_the_redirect() {
    assert_eq!(
        result_of("GET /?code=abc-def HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
        Some(Ok("abc-def".to_string()))
    );
    assert_eq!(
        result_of("GET /?signInFailed HTTP/1.1\r\n\r\n"),
        Some(Err("Signing in failed. Try again.".to_string()))
    );
    assert_eq!(
        result_of("GET /?signInRefused HTTP/1.1\r\n\r\n"),
        Some(Err(
            "Only accounts the admin allowed can sign in to this Moblin Cloud. The admin can see your request."
                .to_string()
        ))
    );
    assert_eq!(result_of("GET /favicon.ico HTTP/1.1\r\n\r\n"), None);
    assert_eq!(result_of("GET / HTTP/1.1\r\n\r\n"), None);
    assert_eq!(result_of("GET /?session=a HTTP/1.1\r\n\r\n"), None);
    assert_eq!(result_of("POST /?code=a HTTP/1.1\r\n\r\n"), None);
    assert_eq!(result_of(""), None);
}

#[tokio::test]
async fn the_browser_is_answered_and_the_code_returned() {
    let listening = listen("http://backend", Platform::Kick).await.unwrap();
    let url = Url::parse(&listening.url).unwrap();
    assert_eq!(url.path(), "/auth/kick/login");
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["challenge"], challenge_of(&listening.verifier));
    assert_eq!(query["challenge"].len(), 43);
    assert_ne!(
        listen("http://backend", Platform::Kick)
            .await
            .unwrap()
            .verifier,
        listening.verifier
    );
    let port = &query["port"];
    let waiting = tokio::spawn(wait(listening.listener, |code| {
        Box::pin(async move { Ok(format!("session of {code}")) })
    }));
    let mut socket = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    socket
        .write_all(b"GET /?code=c HTTP/1.1\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains("Go back to mobtop."));
    assert_eq!(waiting.await.unwrap(), Ok("session of c".to_string()));
}
