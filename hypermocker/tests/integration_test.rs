use hyper::body::Bytes;
use hypermocker::Mock;
use std::time::Duration;

#[tokio::test]
async fn expectation_then_request() {
    let _ = env_logger::try_init();

    let mock = Mock::bind().await.unwrap();
    let url = format!("http://localhost:{}/foo", mock.port);
    let request = mock.expect("/foo".to_string()).await;

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
async fn unexpected_request() {
    let _ = env_logger::try_init();

    let mock = Mock::bind().await.unwrap();
    let url = format!("http://localhost:{}/foo", mock.port);

    let response = reqwest::get(url).await.unwrap();
    let bytes = response.bytes().await.unwrap();
    assert_eq!(&bytes[..], b"unexpected");
}
