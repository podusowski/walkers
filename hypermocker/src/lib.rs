use http_body_util::Full;
use hyper::{server::conn::http1, service::Service, Response};
use hyper_util::rt::TokioIo;
use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub use hyper::body::Bytes;

struct Expectation {
    payload_rx: oneshot::Receiver<Bytes>,
    happened_tx: oneshot::Sender<()>,
}

#[derive(Default)]
struct State {
    /// Anticipations made by [`Mock::anticipate`].
    expectations: HashMap<String, Expectation>,

    /// Requests that were unexpected.
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

    /// Anticipate a HTTP request, but do not respond to it yet.
    pub async fn anticipate(&self, url: String) -> AnticipatedRequest {
        log::info!("Expecting '{}'.", url);
        let (payload_tx, payload_rx) = oneshot::channel();
        let (happened_tx, happened_rx) = oneshot::channel();
        self.state.lock().unwrap().expectations.insert(
            url,
            Expectation {
                payload_rx,
                happened_tx,
            },
        );
        AnticipatedRequest {
            payload_tx,
            happened_rx: Some(happened_rx),
        }
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        if !self.state.lock().unwrap().unexpected.is_empty() {
            panic!("there are unexpected requests");
        }
    }
}

/// HTTP request that was anticipated to arrive.
pub struct AnticipatedRequest {
    payload_tx: tokio::sync::oneshot::Sender<Bytes>,
    happened_rx: Option<oneshot::Receiver<()>>,
}

impl AnticipatedRequest {
    pub async fn respond(self, payload: Bytes) {
        log::info!("Responding.");
        self.payload_tx.send(payload).unwrap();
    }

    /// Expect the request to come, but still do not respond to it yet.
    pub async fn expect(&mut self) {
        if let Some(happened_tx) = self.happened_rx.take() {
            happened_tx.await.unwrap();
        } else {
            panic!("this request was already expected");
        }
    }
}

struct MockRequest {
    state: Arc<Mutex<State>>,
}

impl Service<hyper::Request<hyper::body::Incoming>> for MockRequest {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, request: hyper::Request<hyper::body::Incoming>) -> Self::Future {
        log::info!("Incoming request '{}'.", request.uri());
        let state = self.state.clone();
        Box::pin(async move {
            let expectation = state
                .lock()
                .unwrap()
                .expectations
                .remove(&request.uri().path().to_string());

            if let Some(expectation) = expectation {
                log::debug!("Responding.");

                // [`AnticipatedRequest`] might be dropped by now, and there is no one to receive it,
                // but that is OK.
                let _ = expectation.happened_tx.send(());

                let payload = expectation.payload_rx.await.unwrap();
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
