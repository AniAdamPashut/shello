use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Carriage return, then erase the whole line.
pub const CLEAR_LINE: &str = "\r\x1b[2K";
const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const INTERVAL: Duration = Duration::from_millis(80);

/// A one-line "waiting" animation drawn on a terminal until stopped.
/// Clones share the same animation, so any of them can stop it.
#[derive(Clone)]
pub struct Spinner {
    inner: Arc<Inner>,
}

struct Inner {
    stopped: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Spinner {
    pub fn start(out: Box<dyn Write + Send>, message: &str) -> Self {
        let inner = Arc::new(Inner {
            stopped: AtomicBool::new(false),
            thread: Mutex::new(None),
        });
        let drawing = Arc::clone(&inner);
        let message = message.to_string();
        let handle = thread::spawn(move || animate(out, &message, &drawing.stopped));
        *inner.thread.lock().unwrap() = Some(handle);

        Self { inner }
    }

    /// Stops the animation and clears its line. Safe to call more than
    /// once; every call returns only after the line is cleared, so output
    /// written afterwards never lands on the spinner's line.
    pub fn stop(&self) {
        self.inner.stopped.store(true, Ordering::SeqCst);
        let mut thread = self.inner.thread.lock().unwrap();
        if let Some(handle) = thread.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

/// Draws "⠋ message… 3s" frames until stopped, then clears the line.
fn animate(mut out: Box<dyn Write + Send>, message: &str, stopped: &AtomicBool) {
    let started = Instant::now();
    for frame in FRAMES.iter().cycle() {
        if stopped.load(Ordering::SeqCst) {
            break;
        }

        let seconds = started.elapsed().as_secs();
        let _ = write!(out, "\r{frame} {message}… {seconds}s");
        let _ = out.flush();
        thread::park_timeout(INTERVAL);
    }

    let _ = write!(out, "{CLEAR_LINE}");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::thread::sleep;
    use std::time::Duration;

    /// A writer whose bytes the test can read while the spinner holds it.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Shared {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    #[test]
    fn draws_frames_until_stopped_then_clears_line() {
        let out = Shared::default();
        let spinner = Spinner::start(Box::new(out.clone()), "Waiting for claude");
        sleep(Duration::from_millis(250));
        spinner.stop();
        let text = out.text();

        assert!(text.contains("Waiting for claude"), "{text:?}");
        assert!(
            text.matches('\r').count() >= 3,
            "expected several frames: {text:?}"
        );
        assert!(text.ends_with(CLEAR_LINE), "{text:?}");
    }

    #[test]
    fn nothing_is_drawn_after_stop() {
        let out = Shared::default();
        let spinner = Spinner::start(Box::new(out.clone()), "x");
        spinner.stop();
        let before = out.text();
        sleep(Duration::from_millis(200));

        assert_eq!(out.text(), before);
    }

    #[test]
    fn stop_is_idempotent_across_clones() {
        let out = Shared::default();
        let spinner = Spinner::start(Box::new(out.clone()), "x");
        let other = spinner.clone();
        spinner.stop();
        other.stop();

        assert_eq!(out.text().matches(CLEAR_LINE).count(), 1);
    }

    #[test]
    fn stops_promptly() {
        let spinner = Spinner::start(Box::new(Shared::default()), "x");
        let started = std::time::Instant::now();
        spinner.stop();

        assert!(started.elapsed() < Duration::from_millis(50));
    }
}
