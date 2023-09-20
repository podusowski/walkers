//! Managed thread for Tokio runtime.
use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
pub use native::TokioRuntimeThread;

#[cfg(target_arch = "wasm32")]
pub use web::WasmBindgenFutures as TokioRuntimeThread;

mod web {
    use super::*;

    pub struct WasmBindgenFutures;

    impl WasmBindgenFutures {
        pub fn new<F>(f: F) -> Self
        where
            F: Future<Output=()> + 'static,
        {
            wasm_bindgen_futures::spawn_local(f);
            Self {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;

    pub struct TokioRuntimeThread {
        join_handle: Option<std::thread::JoinHandle<()>>,
        quit_tx: tokio::sync::mpsc::UnboundedSender<()>,
    }

    impl TokioRuntimeThread {
        pub fn new<F>(f: F) -> Self
        where
            F: Future + Send + 'static,
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

    impl Drop for TokioRuntimeThread {
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
}
