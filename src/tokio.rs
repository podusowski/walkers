//! Managed thread for Tokio runtime.
use std::{future::Future, sync::Arc};

pub struct TokioRuntimeThread {
    join_handle: Option<std::thread::JoinHandle<()>>,
    quit_tx: tokio::sync::mpsc::UnboundedSender<()>,
    pub runtime: Arc<tokio::runtime::Runtime>,
}

impl TokioRuntimeThread {
    pub fn new<F>(f: F) -> Self
    where
        F: Future + Send + 'static,
        F::Output: Send,
    {
        let (quit_tx, mut quit_rx) = tokio::sync::mpsc::unbounded_channel();
        let (rt_tx, mut rt_rx) = tokio::sync::mpsc::unbounded_channel();

        let join_handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("could not create the Tokio runtime, downloads will not work");

            let rt = Arc::new(runtime);
            rt_tx
                .send(rt.clone())
                .expect("could not return the Tokio runtime to the main thread");
            rt.block_on(quit_rx.recv());
        });

        let runtime = rt_rx
            .blocking_recv()
            .expect("Tokio thread died before returning the Tokio runtime");

        runtime.spawn(f);

        Self {
            join_handle: Some(join_handle),
            quit_tx,
            runtime,
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
