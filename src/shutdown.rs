// Adapted from
// https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs

use tokio::sync::{broadcast, mpsc};

pub struct Sender {
    notify: broadcast::Sender<()>,
    complete_tx: mpsc::Sender<()>,
    complete_rx: mpsc::Receiver<()>,
}

impl Sender {
    pub fn new() -> Sender {
        let (notify, _) = broadcast::channel(1);
        let (complete_tx, complete_rx) = mpsc::channel(1);

        Sender {
            notify,
            complete_tx,
            complete_rx,
        }
    }

    pub fn subscribe(&self) -> Receiver {
        Receiver {
            shutdown: false,
            notify: self.notify.subscribe(),
            _complete_tx: self.complete_tx.clone(),
        }
    }

    pub async fn send(mut self) {
        // When `notify` is dropped, all tasks which have `subscribe`d will
        // receive the shutdown signal and can exit
        drop(self.notify);

        // Drop final `mpsc::Sender` so the `mpsc::Receiver` below can complete
        drop(self.complete_tx);

        // Wait for all active connections to finish processing. As the `mpsc::Sender`
        // handle held by the listener has been dropped above, the only remaining
        // `mpsc::Sender` instances are held by connection handler tasks. When those drop,
        // the `mpsc` channel will close and `recv()` will return `None`.
        let _ = self.complete_rx.recv().await;
    }
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Receiver {
    shutdown: bool,
    notify: broadcast::Receiver<()>,

    // Used when `Receiver` is dropped
    _complete_tx: mpsc::Sender<()>,
}

impl Receiver {
    /// Returns `true` if the shutdown signal has been received.
    #[allow(dead_code)]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    /// Receive the shutdown notice, waiting if necessary.
    pub async fn recv(&mut self) {
        // If the shutdown signal has already been received, then return
        // immediately.
        if self.shutdown {
            return;
        }

        // Cannot receive a "lag error" as only one value is ever sent.
        let _ = self.notify.recv().await;

        // Remember that the signal has been received.
        self.shutdown = true;
    }
}
