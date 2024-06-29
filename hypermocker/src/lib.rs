#![doc = include_str!("../README.md")]

use http_body_util::Full;
use hyper::server::conn::http1;
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

pub use hyper;
pub use hyper::body::Bytes;

type Response = hyper::Response<Full<Bytes>>;
pub type StatusCode = hyper::StatusCode;

/// Request that already came.
type HyperRequest = hyper::Request<hyper::body::Incoming>;

struct Expectation {
    payload_rx: oneshot::Receiver<Response>,
    request_tx: oneshot::Sender<HyperRequest>,
}

#[derive(Default)]
struct State {
    /// Anticipations made by [`Server::anticipate`].
    expectations: HashMap<String, Expectation>,

    /// Requests that were unexpected.
    unexpected: Vec<String>,
}

/// Central part of the library. All HTTP requests need to be anticipated, otherwise it will panic
/// when dropped.
pub struct Server {
    port: u16,
    state: Arc<Mutex<State>>,
}

impl Server {
    /// Create new [`Server`], and bind it to a random port.
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

    /// Returns the port, which this server listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Anticipate a HTTP request, but do not respond to it yet, nor wait for it to happen.
    pub async fn anticipate(&self, url: impl Into<String>) -> AnticipatedRequest {
        let url = url.into();
        log::info!("Anticipating '{}'.", url);
        let (payload_tx, payload_rx) = oneshot::channel();
        let (request_tx, happened_rx) = oneshot::channel();
        if self
            .state
            .lock()
            .unwrap()
            .expectations
            .insert(
                url.to_owned(),
                Expectation {
                    payload_rx,
                    request_tx,
                },
            )
            .is_some()
        {
            panic!("already anticipating");
        };
        AnticipatedRequest {
            url,
            payload_tx,
            request_rx: Some(happened_rx),
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

    /// Used to send the response.
    payload_tx: tokio::sync::oneshot::Sender<Response>,

    /// Notifies when the request is actually received by the server.
    request_rx: Option<oneshot::Receiver<HyperRequest>>,
}

impl AnticipatedRequest {
    /// Respond to this request immediately if active, or save it for later.
    pub async fn respond(self, payload: impl AsRef<[u8]>) {
        log::info!("Saving response for '{}'.", self.url);
        let payload: hyper::body::Bytes = payload.as_ref().to_owned().into();
        let response = hyper::Response::new(Full::new(payload));
        self.payload_tx.send(response).unwrap();
    }

    /// Similar to [AnticipatedRequest], but with status and empty body.
    pub async fn respond_with_status(self, status: hyper::StatusCode) {
        log::info!(
            "Saving response (with status: {}) for '{}'.",
            status,
            self.url
        );
        let response = hyper::Response::builder()
            .status(status)
            .body(Full::new(Bytes::default()))
            .unwrap();
        self.payload_tx.send(response).unwrap();
    }

    /// Expect the request to come, but do not respond to it yet.
    pub async fn expect(&mut self) -> HyperRequest {
        log::info!("Expecting '{}'.", self.url);
        if let Some(request_tx) = self.request_rx.take() {
            request_tx.await.unwrap()
        } else {
            panic!("this request was already expected");
        }
    }
}

struct Service {
    state: Arc<Mutex<State>>,
}

impl hyper::service::Service<hyper::Request<hyper::body::Incoming>> for Service {
    type Response = hyper::Response<Full<Bytes>>;
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
                .remove(request.uri().path());

            if let Some(expectation) = expectation {
                let uri = request.uri().to_owned();

                // [`AnticipatedRequest`] might be dropped by now, and there is no one to receive it,
                // but that is OK.
                let _ = expectation.request_tx.send(request);

                match expectation.payload_rx.await {
                    Ok(payload) => {
                        log::info!("Responding to '{}' with {:?}.", uri, payload);
                        Ok(payload)
                    }
                    Err(_) => {
                        log::error!(
                            "AnticipatedRequest for '{}' was dropped before responding.",
                            uri
                        );
                        // TODO: This panic will be ignored by hyper/tokio stack.
                        panic!(
                            "AnticipatedRequest for '{}' was dropped before responding.",
                            uri
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
                Ok(hyper::Response::builder()
                    .status(418)
                    .body(Full::new(Bytes::from_static(b"unexpected")))
                    .unwrap())
            }
        })
    }
}
