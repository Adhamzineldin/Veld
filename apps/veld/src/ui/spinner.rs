//! A spinner on stderr. It animates only when stderr is a terminal; otherwise it is silent,
//! so piped output stays plain.

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use super::theme::{Style, Theme};

pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// A spinner that does nothing.
    #[must_use]
    pub fn off() -> Self {
        Spinner {
            stop: Arc::new(AtomicBool::new(true)),
            handle: None,
        }
    }

    #[must_use]
    pub fn start(theme: Theme, message: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let message = message.to_string();
        let handle = std::thread::spawn(move || {
            let frames = theme.sym().frames;
            let mut i = 0usize;
            while !flag.load(Ordering::Relaxed) {
                let frame = theme.paint(Style::Cyan, frames[i % frames.len()]);
                let mut err = std::io::stderr().lock();
                let _ = write!(err, "\r\x1b[2K{frame} {message}");
                let _ = err.flush();
                drop(err);
                i += 1;
                std::thread::sleep(Duration::from_millis(80));
            }
        });
        Spinner {
            stop,
            handle: Some(handle),
        }
    }

    /// Stops the animation and erases its line.
    pub fn finish(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
            let mut err = std::io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.halt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_off_spinner_finishes_without_a_thread() {
        Spinner::off().finish();
    }
}
