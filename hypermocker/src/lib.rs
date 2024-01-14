use std::{
    collections::HashMap,
    convert::Infallible,
    future::Future,
    net::{SocketAddr, SocketAddrV4},
    pin::Pin,
    sync::Arc,
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

pub struct Mock {
    pub port: u16,
    state: Arc<Mutex<State>>,
}

impl Mock {
    pub async fn bind() -> Result<Mock, Box<dyn std::error::Error + Send + Sync>> {
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

    pub async fn expect(&self, url: String) -> Expectation {
        log::info!("Expecting '{}'.", url);

        let request = self.state.lock().await.requests.remove(&url);

        if let Some(tx) = request {
            log::debug!("Found matching request.");
            Expectation { tx }
        } else {
            log::debug!("Waiting for request.");
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.state.lock().await.expectations.insert(url, rx);
            Expectation { tx }
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
                .await
                .expectations
                .remove(&request.uri().path().to_string());

            if let Some(rx) = expectation {
                log::debug!("Already expecting, responding.");
                let payload = rx.await.unwrap();
                Ok(Response::new(Full::new(payload)))
            } else {
                log::debug!("Not yet expected, waiting.");
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
