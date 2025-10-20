//! Managed thread for an IO runtime. Concrete implementation depends on the target.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::*;

#[cfg(target_arch = "wasm32")]
pub(crate) use web::*;

#[cfg(target_arch = "wasm32")]
mod web {
    use crate::HttpOptions;
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};

    pub struct Runtime;

    impl Runtime {
        pub fn new<F>(f: F) -> Self
        where
            F: std::future::Future<Output = ()> + 'static,
        {
            wasm_bindgen_futures::spawn_local(f);
            Self {}
        }
    }

    pub fn http_client(http_options: HttpOptions) -> ClientWithMiddleware {
        if http_options.cache.is_some() {
            log::warn!(
                "HTTP cache directory set, but ignored because, in WASM, caching is handled by the browser."
            );
        }
        ClientBuilder::new(reqwest::Client::new()).build()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::HttpOptions;
    use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};

    pub struct Runtime {
        join_handle: Option<std::thread::JoinHandle<()>>,
        quit_tx: tokio::sync::mpsc::UnboundedSender<()>,
    }

    impl Runtime {
        pub fn new<F>(f: F) -> Self
        where
            F: std::future::Future + Send + 'static,
            F::Output: Send,
        {
            let (quit_tx, mut quit_rx) = tokio::sync::mpsc::unbounded_channel();

            let join_handle = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("could not create the Tokio runtime, downloads will not work");

                runtime.spawn(f);
                runtime.block_on(quit_rx.recv());
            });

            Self {
                join_handle: Some(join_handle),
                quit_tx,
            }
        }
    }

    impl Drop for Runtime {
        fn drop(&mut self) {
            // Tokio thread might be dead, nothing to do in this case.
            let _ = self.quit_tx.send(());

            if let Some(join_handle) = self.join_handle.take() {
                log::debug!("Waiting for the Tokio thread to exit.");
                // Again, Tokio thread might be already dead, nothing to do in this case.
                let _ = join_handle.join();
            }

            log::debug!("Tokio thread is down.");
        }
    }

    pub fn http_client(http_options: HttpOptions) -> ClientWithMiddleware {
        let builder = ClientBuilder::new(reqwest::Client::new());

        if let Some(cache) = http_options.cache {
            builder.with(Cache(HttpCache {
                mode: CacheMode::Default,
                manager: CACacheManager {
                    path: cache,
                    remove_opts: Default::default(),
                },
                options: HttpCacheOptions::default(),
            }))
        } else {
            builder
        }
        .build()
    }
}
