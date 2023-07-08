//! Managed thread for Tokio runtime.
use std::sync::Arc;

pub struct TokioRuntimeThread {
    join_handle: Option<std::thread::JoinHandle<()>>,
    quit_tx: tokio::sync::mpsc::UnboundedSender<()>,
    pub runtime: Arc<tokio::runtime::Runtime>,
}

impl TokioRuntimeThread {
    pub fn new() -> Self {
        let (quit_tx, mut quit_rx) = tokio::sync::mpsc::unbounded_channel();
        let (rt_tx, mut rt_rx) = tokio::sync::mpsc::unbounded_channel();

        let join_handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let rt = Arc::new(runtime);
            rt_tx.send(rt.clone()).unwrap();
            rt.block_on(quit_rx.recv());
        });

        Self {
            join_handle: Some(join_handle),
            quit_tx,
            runtime: rt_rx.blocking_recv().unwrap(),
        }
    }
}

impl Drop for TokioRuntimeThread {
    fn drop(&mut self) {
        self.quit_tx.send(()).unwrap();

        if let Some(join_handle) = self.join_handle.take() {
            log::debug!("Waiting for the Tokio thread to exit.");
            // Not much to do if it's an error.
            _ = join_handle.join();
        }

        log::debug!("Tokio thread is down.");
    }
}
