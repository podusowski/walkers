use std::{
    collections::HashMap, convert::Infallible, future::Future, net::SocketAddr, pin::Pin, sync::Arc,
};

use http_body_util::Full;
use hyper::{
    body::Bytes,
    server::conn::http1,
    service::{service_fn, Service},
    Request, Response,
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, sync::Mutex};

#[derive(Default)]
struct State {
    /// Expectations [`Mock::except`], made before incoming HTTP request.
    expectations: HashMap<String, tokio::sync::oneshot::Receiver<Bytes>>,
}

struct Mock {
    state: Arc<Mutex<State>>,
}

impl Mock {
    pub async fn expect(&self, url: String) -> Expectation {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.state.lock().await.expectations.insert(url, rx);
        Expectation { tx }
    }
}

struct Expectation {
    tx: tokio::sync::oneshot::Sender<Bytes>,
}

impl Expectation {
    pub async fn respond(self, payload: Bytes) {
        self.tx.send(payload).unwrap();
    }
}

struct MockRequest {
    state: Arc<Mutex<State>>,
}

impl Service<Request<hyper::body::Incoming>> for MockRequest {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, request: Request<hyper::body::Incoming>) -> Self::Future {
        let state = self.state.clone();
        Box::pin(async move {
            let expectation = state
                .lock()
                .await
                .expectations
                .remove(&request.uri().path().to_string())
                .unwrap();
            let payload = expectation.await.unwrap();
            Ok(Response::new(Full::new(payload)))
        })
    }
}

async fn start_mock() -> Result<Mock, Box<dyn std::error::Error + Send + Sync>> {
    let state = Arc::new(Mutex::new(State::default()));

    let state_clone = state.clone();
    tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let listener = TcpListener::bind(addr).await.unwrap();

        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);

            let state = state_clone.clone();
            tokio::task::spawn(async move {
                // Finally, we bind the incoming connection to our `hello` service
                if let Err(err) = http1::Builder::new()
                    // `service_fn` converts our function in a `Service`
                    .serve_connection(io, MockRequest { state })
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    });

    Ok(Mock { state })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hyper::body::Bytes;

    use crate::start_mock;

    #[tokio::test]
    async fn expectation_then_request() {
        let mock = start_mock().await.unwrap();
        let request = mock.expect("/foo".to_string()).await;

        // Make sure that mock's internals kick in.
        tokio::time::sleep(Duration::from_secs(1)).await;

        futures::future::join(
            async {
                let response = reqwest::get("http://localhost:3000/foo").await.unwrap();
                let bytes = response.bytes().await.unwrap();
                assert_eq!(&bytes[..], b"hello");
            },
            async {
                request.respond(Bytes::from_static(b"hello")).await;
            },
        )
        .await;
    }
}
