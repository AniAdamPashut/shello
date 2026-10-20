use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty directory unique to this call.
pub fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("shello-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes contents to a file named name in a fresh temp directory.
pub fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp_dir().join(name);
    std::fs::write(&path, contents).unwrap();
    path
}
