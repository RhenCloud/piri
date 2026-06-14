use std::time::{Duration, Instant};

pub fn send_notification(summary: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .arg("-a")
        .arg("piri")
        .arg("-i")
        .arg("dialog-error")
        .arg(summary)
        .arg(body)
        .spawn();
}

/// A simple throttle helper to limit the frequency of executions.
#[derive(Debug, Default)]
pub struct Throttle {
    last_execution: Option<Instant>,
}

impl Throttle {
    pub fn new() -> Self {
        Self {
            last_execution: None,
        }
    }

    /// Checks if enough time has passed since the last execution.
    /// If so, updates the last execution time and returns true.
    /// Does NOT update the timestamp when throttled, so the next allowed
    /// execution is not pushed further back by repeated throttled calls.
    pub fn check_and_update(&mut self, duration: Duration) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_execution {
            if now.duration_since(last) < duration {
                return false;
            }
        }
        self.last_execution = Some(now);
        true
    }

    /// Similar to check_and_update, but does NOT update the timestamp if throttled.
    /// This matches the behavior of schedule_autofill/schedule_apply_widths.
    pub fn check_and_update_no_reset(&mut self, duration: Duration) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_execution {
            if now.duration_since(last) < duration {
                return false;
            }
        }
        self.last_execution = Some(now);
        true
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.last_execution = None;
    }

    /// Run a function only if the throttle period has passed.
    #[allow(dead_code)]
    pub fn run<F, R>(&mut self, duration: Duration, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        if self.check_and_update(duration) {
            Some(f())
        } else {
            None
        }
    }

    /// Run a function only if the throttle period has passed (no reset on throttle).
    #[allow(dead_code)]
    pub fn run_no_reset<F, R>(&mut self, duration: Duration, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        if self.check_and_update_no_reset(duration) {
            Some(f())
        } else {
            None
        }
    }
}

/// A simple debounce helper.
#[derive(Debug, Default)]
pub struct Debounce {
    timer: Option<tokio::task::JoinHandle<()>>,
}

impl Debounce {
    pub fn new() -> Self {
        Self { timer: None }
    }

    /// Debounces an async action.
    /// If a new action is triggered before the duration expires, the previous one is cancelled.
    pub fn debounce<F, Fut>(&mut self, duration: Duration, action: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // Cancel existing timer if any
        if let Some(handle) = self.timer.take() {
            handle.abort();
        }

        // Spawn new timer
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            action().await;
        });

        self.timer = Some(handle);
    }

    #[allow(dead_code)]
    pub fn cancel(&mut self) {
        if let Some(handle) = self.timer.take() {
            handle.abort();
        }
    }
}
