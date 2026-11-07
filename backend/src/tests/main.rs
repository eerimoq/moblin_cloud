use axum::routing::get;
use reqwest::StatusCode;
use tokio::sync::oneshot;

use super::*;

#[tokio::test]
async fn requests_in_flight_finish_before_stopping() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            "done"
        }),
    );
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve(listener, router, async {
        stopped.await.unwrap();
    }));
    let request = tokio::spawn(async move {
        reqwest::Client::new()
            .get(format!("http://{address}/slow"))
            .send()
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    stop.send(()).unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "done");
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        reqwest::Client::new()
            .get(format!("http://{address}/slow"))
            .send()
            .await
            .is_err()
    );
}
