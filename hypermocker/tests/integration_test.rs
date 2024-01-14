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
async fn request_then_expectation() {
    let _ = env_logger::try_init();

    let mock = Mock::bind().await.unwrap();
    let url = format!("http://localhost:{}/foo", mock.port);

    futures::future::join(
        async {
            let response = reqwest::get(url).await.unwrap();
            let bytes = response.bytes().await.unwrap();
            assert_eq!(&bytes[..], b"hello");
        },
        async {
            // Make sure we first do the request.
            tokio::time::sleep(Duration::from_secs(1)).await;

            let request = mock.expect("/foo".to_string()).await;
            request.respond(Bytes::from_static(b"hello")).await;
        },
    )
    .await;
}
