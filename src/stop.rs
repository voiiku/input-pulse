use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub(crate) struct StopToken {
    state: Arc<StopState>,
}

#[derive(Debug, Default)]
struct StopState {
    requested: Mutex<bool>,
    wake: Condvar,
}

impl StopToken {
    /// Creates a token with no stop request.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests a stop and wakes any inter-cycle waits using this token.
    ///
    /// This does not wait for the worker to finish. Join its thread to confirm
    /// termination and retrieve its result.
    pub fn request_stop(&self) {
        let mut requested = self.state.requested.lock().expect("stop state poisoned");
        *requested = true;
        self.state.wake.notify_all();
    }

    /// Returns whether a stop has been requested, not whether a run has exited.
    pub fn is_stop_requested(&self) -> bool {
        *self.state.requested.lock().expect("stop state poisoned")
    }

    pub(crate) fn wait(&self, duration: Duration) -> bool {
        let requested = self.state.requested.lock().expect("stop state poisoned");
        let (requested, _) = self
            .state
            .wake
            .wait_timeout_while(requested, duration, |requested| !*requested)
            .expect("stop state poisoned");
        *requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread, time::Instant};

    #[test]
    fn requests_are_shared_permanent_and_idempotent() {
        let token = StopToken::new();
        let clone = token.clone();
        assert!(!token.is_stop_requested());
        clone.request_stop();
        token.request_stop();
        assert!(clone.is_stop_requested());
        assert!(token.wait(Duration::from_secs(60)));
        assert!(!StopToken::new().is_stop_requested());
    }

    #[test]
    fn stop_wakes_waiters_on_other_threads() {
        let token = StopToken::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let token = token.clone();
                let started = started_tx.clone();
                let done = done_tx.clone();
                thread::spawn(move || {
                    started.send(()).unwrap();
                    done.send(token.wait(Duration::from_secs(60))).unwrap();
                })
            })
            .collect();
        for _ in &workers {
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        token.request_stop();
        for _ in &workers {
            assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        }
        for worker in workers {
            worker.join().unwrap();
        }
    }

    #[test]
    fn wait_times_out_without_requesting_stop() {
        let token = StopToken::new();
        let started = Instant::now();
        assert!(!token.wait(Duration::from_millis(10)));
        assert!(started.elapsed() >= Duration::from_millis(10));
        assert!(!token.is_stop_requested());
    }
}
