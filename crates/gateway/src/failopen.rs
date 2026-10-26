use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};

/// Hooks normally finish in ~0.1 s; the cap only stops a stuck step from holding git up.
pub const BUDGET: Duration = Duration::from_secs(5);

/// Runs `f` on a worker thread. A panic, an error or running past `timeout`
/// all come back as `Err`, so callers can fall back to leaving git alone.
pub fn guard<T, F>(timeout: Duration, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow!("timed out after {timeout:?}")),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow!("internal panic")),
    }
}

/// Silences the default panic message; failures are reported through the activity log instead.
pub fn quiet_panics() {
    std::panic::set_hook(Box::new(|_| {}));
}
