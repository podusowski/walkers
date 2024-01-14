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

    /// Incoming requests that came before expectation was made.
    requests: HashMap<String, tokio::sync::oneshot::Sender<Bytes>>,
}

struct Mock {
    port: u16,
    state: Arc<Mutex<State>>,
}

impl Mock {
    pub async fn expect(&self, url: String) -> Expectation {
        println!("Expecting");
        if let Some(tx) = self.state.lock().await.requests.remove(&url) {
            Expectation { tx }
        } else {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.state.lock().await.expectations.insert(url, rx);
            Expectation { tx }
        }
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
            if let Some(rx) = state
                .lock()
                .await
                .expectations
                .remove(&request.uri().path().to_string())
            {
                let payload = rx.await.unwrap();
                Ok(Response::new(Full::new(payload)))
            } else {
                let (tx, rx) = tokio::sync::oneshot::channel();
                state
                    .lock()
                    .await
                    .requests
                    .insert(request.uri().to_string(), tx);
                let payload = rx.await.unwrap();
                Ok(Response::new(Full::new(payload)))
            }
        })
    }
}

async fn start_mock() -> Result<Mock, Box<dyn std::error::Error + Send + Sync>> {
    let state = Arc::new(Mutex::new(State::default()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let state_clone = state.clone();
    tokio::spawn(async move {
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

    Ok(Mock { port, state })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hyper::body::Bytes;

    use crate::start_mock;

    #[tokio::test]
    async fn expectation_then_request() {
        let mock = start_mock().await.unwrap();
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
        let mock = start_mock().await.unwrap();
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
}
