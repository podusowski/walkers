use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::Service, Request, Response};
use hyper_util::rt::TokioIo;
use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;

#[derive(Default)]
struct State {
    /// Expectations [`Mock::except`], made before incoming HTTP request.
    expectations: HashMap<String, tokio::sync::oneshot::Receiver<Bytes>>,

    unexpected: Vec<String>,
}

pub struct Mock {
    pub port: u16,
    state: Arc<Mutex<State>>,
}

impl Mock {
    /// Create new [`Mock`], and bind it to a random port.
    pub async fn bind() -> Mock {
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
                    http1::Builder::new()
                        .serve_connection(io, MockRequest { state })
                        .await
                        .unwrap();
                });
            }
        });

        Mock { port, state }
    }

    pub async fn expect(&self, url: String) -> Expectation {
        log::info!("Expecting '{}'.", url);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.state.lock().unwrap().expectations.insert(url, rx);
        Expectation { tx }
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        if !self.state.lock().unwrap().unexpected.is_empty() {
            panic!("there are unexpected requests");
        }
    }
}

pub struct Expectation {
    tx: tokio::sync::oneshot::Sender<Bytes>,
}

impl Expectation {
    pub async fn respond(self, payload: Bytes) {
        log::info!("Responding.");
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
        log::info!("Incoming request '{}'.", request.uri());
        let state = self.state.clone();
        Box::pin(async move {
            let expectation = state
                .lock()
                .unwrap()
                .expectations
                .remove(&request.uri().path().to_string());

            if let Some(rx) = expectation {
                log::debug!("Responding.");
                let payload = rx.await.unwrap();
                Ok(Response::new(Full::new(payload)))
            } else {
                log::warn!("Unexpected '{}'.", request.uri());
                state
                    .lock()
                    .unwrap()
                    .unexpected
                    .push(request.uri().to_string());
                Ok(Response::builder()
                    .status(418)
                    .body(Full::new(Bytes::from_static(b"unexpected")))
                    .unwrap())
            }
        })
    }
}
