//! Logging: in debug builds logs are kept in memory and printed to stderr when the editor
//! exits (until then the terminal belongs to the UI); in release builds they are appended to
//! `<YYYY-MM-DD>_revim.log` in the system temp directory.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    sync::{Arc, Mutex},
};

use tracing_subscriber::filter::LevelFilter;

/// Keep it alive until the end of `main`. In debug builds it prints the collected logs when
/// dropped, also when `main` unwinds after a panic.
pub struct LogGuard(Option<MemoryLog>);

impl Drop for LogGuard {
    fn drop(&mut self) {
        if let Some(memory) = &self.0 {
            let buf = memory.0.lock().unwrap_or_else(|e| e.into_inner());
            let _ = io::stderr().write_all(&buf);
        }
    }
}

/// Installs the global logger; `log::…!` calls are captured too. Call once, first thing in `main`.
pub fn init() -> LogGuard {
    if cfg!(debug_assertions) {
        let memory = MemoryLog::default();
        let writer = memory.clone();
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::DEBUG)
            .with_writer(move || writer.clone())
            .init();
        LogGuard(Some(memory))
    } else {
        match open_log_file() {
            Ok(file) => tracing_subscriber::fmt()
                .with_max_level(LevelFilter::INFO)
                .with_ansi(false)
                .with_writer(Mutex::new(file))
                .init(),
            Err(e) => eprintln!("revim: cannot open log file, logging is off: {e}"),
        }
        LogGuard(None)
    }
}

/// Opens today's log file for appending, so all runs of one day share it.
fn open_log_file() -> io::Result<File> {
    let date = chrono::Local::now().format("%Y-%m-%d");
    let path = std::env::temp_dir().join(format!("{date}_revim.log"));
    OpenOptions::new().create(true).append(true).open(path)
}

/// Log output kept in memory until the editor exits.
#[derive(Clone, Default)]
struct MemoryLog(Arc<Mutex<Vec<u8>>>);

impl Write for MemoryLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_of_memory_log_share_one_buffer() {
        let memory = MemoryLog::default();
        let (mut a, mut b) = (memory.clone(), memory.clone());
        write!(a, "one ").unwrap();
        write!(b, "two").unwrap();
        assert_eq!(*memory.0.lock().unwrap(), b"one two");
    }
}
