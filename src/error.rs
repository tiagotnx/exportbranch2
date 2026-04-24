use std::fmt;
use std::io;
use std::path::PathBuf;

/// All recoverable errors raised by `exportbranch`.
///
/// `main` matches on the variant to choose a process exit code; see
/// [`ExportError::exit_code`].
#[derive(Debug)]
#[allow(missing_docs)]
pub enum ExportError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    InvalidArgs(String),
    InvalidGlob {
        pattern: String,
        source: regex::Error,
    },
    MissingFileName(PathBuf),
    Metadata {
        path: PathBuf,
        source: io::Error,
    },
    /// Source path lacks a Windows drive `Prefix` component, so the
    /// destination drive cannot be derived. Only emitted on Windows.
    MissingDrivePrefix(PathBuf),
}

/// `Result` alias defaulting to [`ExportError`].
pub type Result<T> = std::result::Result<T, ExportError>;

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Io { path, source } => {
                write!(f, "I/O error on {}: {}", path.display(), source)
            }
            ExportError::InvalidArgs(msg) => write!(f, "{msg}"),
            ExportError::InvalidGlob { pattern, source } => {
                write!(f, "invalid glob pattern `{pattern}`: {source}")
            }
            ExportError::MissingFileName(path) => {
                write!(f, "path has no file name component: {}", path.display())
            }
            ExportError::Metadata { path, source } => {
                write!(
                    f,
                    "failed to read metadata for {}: {}",
                    path.display(),
                    source
                )
            }
            ExportError::MissingDrivePrefix(path) => {
                write!(
                    f,
                    "source path has no drive prefix (e.g. `C:\\`): {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io { source, .. } | ExportError::Metadata { source, .. } => Some(source),
            ExportError::InvalidGlob { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl ExportError {
    /// Process exit code conventionally associated with this error variant.
    pub fn exit_code(&self) -> i32 {
        match self {
            ExportError::InvalidArgs(_) => 2,
            ExportError::Metadata { .. } => 3,
            _ => 1,
        }
    }
}

/// Extension trait that turns an [`io::Error`] into an [`ExportError::Io`]
/// with the file path responsible for the failure attached.
pub trait WithPath<T> {
    /// Attach `path` to a failed [`io::Result`], producing an [`ExportError`].
    fn with_path<P: Into<PathBuf>>(self, path: P) -> Result<T>;
}

impl<T> WithPath<T> for std::result::Result<T, io::Error> {
    fn with_path<P: Into<PathBuf>>(self, path: P) -> Result<T> {
        self.map_err(|source| ExportError::Io {
            path: path.into(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io_inclui_path() {
        let err = ExportError::Io {
            path: PathBuf::from("/tmp/missing"),
            source: io::Error::new(io::ErrorKind::NotFound, "file not found"),
        };
        let msg = format!("{err}");
        assert!(msg.contains("/tmp/missing"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn display_invalid_args_retorna_mensagem() {
        let err = ExportError::InvalidArgs("use -s and -d".into());
        assert_eq!(format!("{err}"), "use -s and -d");
    }

    #[test]
    fn display_invalid_glob_inclui_pattern() {
        let err = ExportError::InvalidGlob {
            pattern: "*.[".into(),
            source: regex::Error::Syntax("unterminated".into()),
        };
        let msg = format!("{err}");
        assert!(msg.contains("*.["));
    }

    #[test]
    fn exit_code_invalid_args_e_2() {
        let err = ExportError::InvalidArgs("x".into());
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn exit_code_metadata_e_3() {
        let err = ExportError::Metadata {
            path: PathBuf::from("x"),
            source: io::Error::other("y"),
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn with_path_anexa_caminho_no_io_error() {
        let result: std::result::Result<(), io::Error> =
            Err(io::Error::new(io::ErrorKind::NotFound, "nope"));
        let err = result.with_path("/some/path").unwrap_err();
        match err {
            ExportError::Io { path, .. } => assert_eq!(path, PathBuf::from("/some/path")),
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
