use http_body_util::Full;
use hyper::{server::conn::http1, Response};
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

pub struct Server {
    port: u16,
    state: Arc<Mutex<State>>,
}

impl Server {
    /// Create new [`Mock`], and bind it to a random port.
    pub async fn bind() -> Server {
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
                        .serve_connection(io, Service { state })
                        .await
                        .unwrap();
                });
            }
        });

        Server { port, state }
    }

    /// Port, which this server listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Anticipate a HTTP request, but do not respond to it yet.
    pub async fn anticipate(&self, url: impl Into<String>) -> AnticipatedRequest {
        let url = url.into();
        log::info!("Anticipating '{}'.", url);
        let (payload_tx, payload_rx) = oneshot::channel();
        let (happened_tx, happened_rx) = oneshot::channel();
        if self
            .state
            .lock()
            .unwrap()
            .expectations
            .insert(
                url.to_owned(),
                Expectation {
                    payload_rx,
                    happened_tx,
                },
            )
            .is_some()
        {
            panic!("already anticipating");
        };
        AnticipatedRequest {
            url,
            payload_tx,
            happened_rx: Some(happened_rx),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if !self.state.lock().unwrap().unexpected.is_empty() {
            panic!("there are unexpected requests");
        }
    }
}

/// HTTP request that was anticipated to arrive.
pub struct AnticipatedRequest {
    url: String,
    payload_tx: tokio::sync::oneshot::Sender<Bytes>,
    happened_rx: Option<oneshot::Receiver<()>>,
}

impl AnticipatedRequest {
    /// Respond to this request with the given body.
    pub async fn respond(self, payload: Bytes) {
        log::info!("Responding to '{}'.", self.url);
        self.payload_tx.send(payload).unwrap();
    }

    /// Expect the request to come, but still do not respond to it yet.
    pub async fn expect(&mut self) {
        log::info!("Expecting '{}'.", self.url);
        if let Some(happened_tx) = self.happened_rx.take() {
            happened_tx.await.unwrap();
        } else {
            panic!("this request was already expected");
        }
    }
}

struct Service {
    state: Arc<Mutex<State>>,
}

impl hyper::service::Service<hyper::Request<hyper::body::Incoming>> for Service {
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
                // [`AnticipatedRequest`] might be dropped by now, and there is no one to receive it,
                // but that is OK.
                let _ = expectation.happened_tx.send(());

                match expectation.payload_rx.await {
                    Ok(payload) => {
                        log::debug!(
                            "Proper responding to '{}' with {} bytes.",
                            request.uri(),
                            payload.len()
                        );
                        Ok(Response::new(Full::new(payload)))
                    }
                    Err(_) => {
                        log::error!(
                            "AnticipatedRequest for '{}' was dropped before responding.",
                            request.uri()
                        );
                        // TODO: This panic will be ignored by hyper/tokio stack.
                        panic!(
                            "AnticipatedRequest for '{}' was dropped before responding.",
                            request.uri()
                        );
                    }
                }
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
