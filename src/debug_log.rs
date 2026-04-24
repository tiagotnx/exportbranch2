//! Opt-in debug log written to a file when `--debug` is passed.
//!
//! Each line is prefixed with the milliseconds elapsed since `DebugLog`
//! creation so the ordering and relative timing of events is clear. The
//! log is intended to capture enough information to diagnose bug reports
//! from the field without affecting the default output.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Result, WithPath};

/// Writable debug log. Wrap in an `Option` at call sites.
pub struct DebugLog {
    writer: Mutex<BufWriter<File>>,
    start: SystemTime,
}

impl DebugLog {
    /// Truncate or create `path` and return a log ready to be written to.
    pub fn create(path: &Path) -> Result<Self> {
        let file = File::create(path).with_path(path)?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            start: SystemTime::now(),
        })
    }

    /// Write a formatted line to the log. Errors are swallowed so a full
    /// disk never aborts an otherwise-healthy export.
    pub fn write(&self, args: std::fmt::Arguments) {
        if let Ok(mut w) = self.writer.lock() {
            let ms = self.start.elapsed().map_or(0, |d| d.as_millis());
            let _ = writeln!(w, "[{ms:>8}ms] {args}");
            let _ = w.flush();
        }
    }
}

/// Default path (`exportbranch-<unix_secs>.log`) used when `--debug` is
/// passed without an explicit value.
pub fn default_log_path() -> PathBuf {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    PathBuf::from(format!("exportbranch-{secs}.log"))
}

/// Log a formatted line iff `log` is `Some`. Accepts the same format
/// syntax as `println!` / `format!`.
#[macro_export]
macro_rules! debug_log {
    ($log:expr, $($arg:tt)*) => {
        if let Some(ref __dl) = $log {
            $crate::debug_log::DebugLog::write(__dl, format_args!($($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_escreve_linha_no_arquivo() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("debug.log");
        let log = DebugLog::create(&path).unwrap();
        log.write(format_args!("hello {}", "world"));
        drop(log);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("hello world"), "got: {contents:?}");
    }

    #[test]
    fn cada_linha_tem_prefixo_de_timestamp_em_ms() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("debug.log");
        let log = DebugLog::create(&path).unwrap();
        log.write(format_args!("primeira"));
        log.write(format_args!("segunda"));
        drop(log);
        let contents = std::fs::read_to_string(&path).unwrap();
        for line in contents.lines() {
            assert!(
                line.starts_with('[') && line.contains("ms]"),
                "linha sem prefixo: {line:?}"
            );
        }
    }

    #[test]
    fn create_em_path_invalido_retorna_erro_io() {
        let result = DebugLog::create(Path::new("/definitely/does/not/exist/xyz/debug.log"));
        assert!(matches!(result, Err(crate::error::ExportError::Io { .. })));
    }

    #[test]
    fn default_log_path_tem_prefixo_esperado() {
        let path = default_log_path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("exportbranch-"));
        assert_eq!(
            path.extension().and_then(|e| e.to_str()),
            Some("log"),
            "extensão esperada .log"
        );
    }

    #[test]
    fn macro_debug_log_no_op_quando_none() {
        // Garante que a macro compila e é no-op com `None`.
        let log: Option<DebugLog> = None;
        debug_log!(log, "não deve panicar — {}", 42);
    }
}
