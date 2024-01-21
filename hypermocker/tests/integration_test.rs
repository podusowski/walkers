use hyper::body::Bytes;
use hypermocker::{Server, StatusCode};
use std::time::Duration;

#[tokio::test]
async fn anticipate_then_request() {
    let _ = env_logger::try_init();

    let mock = Server::bind().await;
    let url = format!("http://localhost:{}/foo", mock.port());
    let request = mock.anticipate("/foo").await;

    // Make sure that mock's internals kick in.
    tokio::time::sleep(Duration::from_secs(1)).await;

    futures::future::join(
        async {
            let response = reqwest::get(url).await.unwrap();
            let bytes = response.bytes().await.unwrap();
            assert_eq!(&bytes[..], b"hello");
        },
        async {
            request.respond(Bytes::from_static(b"hello")).await;
        },
    )
    .await;
}

#[tokio::test]
async fn anticipate_expect_then_request() {
    let _ = env_logger::try_init();

    let mock = Server::bind().await;
    let url = format!("http://localhost:{}/foo", mock.port());
    let mut request = mock.anticipate("/foo").await;

    // Make sure that mock's internals kick in.
    tokio::time::sleep(Duration::from_secs(1)).await;

    futures::future::join(
        async {
            let response = reqwest::get(url).await.unwrap();
            let bytes = response.bytes().await.unwrap();
            assert_eq!(&bytes[..], b"hello");
        },
        async {
            request.expect().await;
            request.respond(Bytes::from_static(b"hello")).await;
        },
    )
    .await;
}

#[tokio::test]
async fn respond_with_http_status() {
    let _ = env_logger::try_init();

    let mock = Server::bind().await;
    let url = format!("http://localhost:{}/foo", mock.port());
    let request = mock.anticipate("/foo").await;
    request.respond_with_status(StatusCode::NOT_FOUND).await;

    let response = reqwest::get(url).await.unwrap();
    assert_eq!(404, response.status());
}

#[tokio::test]
#[should_panic(expected = "there are unexpected requests")]
async fn unanticipated_request() {
    let _ = env_logger::try_init();

    let mock = Server::bind().await;
    let url = format!("http://localhost:{}/foo", mock.port());

    let response = reqwest::get(url).await.unwrap();
    let bytes = response.bytes().await.unwrap();
    assert_eq!(&bytes[..], b"unexpected");
}

#[tokio::test]
#[should_panic(expected = "already anticipating")]
async fn can_not_anticipate_twice() {
    let _ = env_logger::try_init();

    let mock = Server::bind().await;

    mock.anticipate("/foo").await;
    mock.anticipate("/foo").await;
}

#[tokio::test]
#[should_panic(expected = "this request was already expected")]
async fn can_not_expect_twice() {
    let _ = env_logger::try_init();

    let mock = Server::bind().await;
    let url = format!("http://localhost:{}/foo", mock.port());

    let mut request = mock.anticipate("/foo").await;

    futures::future::join(
        async {
            reqwest::get(url).await.unwrap();
        },
        async {
            request.expect().await;
            request.expect().await;
        },
    )
    .await;
}
