//! `exportbranch` filters, copies and converts files from a Harbour source
//! tree before they are handed to the `Compex` compiler.
//!
//! The crate is split into:
//! - [`configuration`]: command-line parsing and per-source overrides;
//! - [`export`] / [`export_branch`]: the parallel directory walk;
//! - [`convert_file`] / [`convertions`]: the CP850 → ASCII conversion engine;
//! - [`file_checker`]: per-file modification-time tracker;
//! - [`error`]: typed errors with attached file paths.
#![warn(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::struct_excessive_bools
)]

/// Command-line parsing and the [`configuration::Configuration`] type.
pub mod configuration;
/// CP850 → ASCII conversion (`convert_buffer`, `convert_stream`, `convert_file`).
pub mod convert_file;
/// Static lookup tables driving the conversion engine.
pub mod convertions;
/// Typed error enum and the `WithPath` extension trait.
pub mod error;
/// Parallel directory walk and per-file dispatch.
pub mod export;
/// High-level orchestration of a single source → destination export.
pub mod export_branch;
/// Per-source override files and glob → regex compilation.
pub mod export_branch_files;
/// Modification-time tracker persisted alongside the destination tree.
pub mod file_checker;

pub use error::{ExportError, Result, WithPath};

use configuration::Configuration;
use export_branch::ExportBranch;
use file_checker::FileChecker;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

/// CLI entry point. `args` is the full process argv (including the binary name).
pub fn run(args: Vec<String>) -> Result<()> {
    let timer = Instant::now();
    let configuration = Configuration::build(&mut args.into_iter())?;

    configuration.print();

    for source in configuration.source() {
        for destination in configuration.destination() {
            export(source, destination, &configuration)?;
        }
    }

    print_time_elapsed(timer);
    Ok(())
}

fn export(source: &str, destination: &str, configuration: &Configuration) -> Result<()> {
    let source_path_buffer = source_path(source)?;
    let destination_path_buffer = destination_path(source, destination)?;
    let mut file_checker = FileChecker::new(destination_path_buffer.clone());
    let mut export = ExportBranch::build(
        source_path_buffer,
        destination_path_buffer,
        configuration,
        &mut file_checker,
    );

    export.perform_exporting()
}

fn source_path(source: &str) -> Result<PathBuf> {
    Path::new(source).canonicalize().with_path(source)
}

/// Build the destination root for an export.
///
/// On Windows we mirror the *raw* source path (the string the user passed
/// to `-s`, before canonicalisation) under the destination drive: the
/// drive `Prefix` of the source is stripped, and the remainder — which
/// still carries the leading `\` — is `join`ed onto `destination`.
/// Because Windows path semantics make `PathBuf::join` keep only the
/// `Prefix` of the left-hand side when the right-hand side has a root,
/// the destination's drive letter is preserved and everything after it is
/// replaced by the source layout. Examples:
///
/// - `-s L:\trunk\frente -d R:\` → `R:\trunk\frente`
/// - `-s L:\trunk\include -d R:\anything` → `R:\trunk\include`
///
/// This matches the historical `MateusZanchoNeto/exportbranch` behaviour
/// production scripts depend on: a single `-d <drive>:\` invoked once per
/// source root sends each source to its own subtree on the destination
/// drive instead of colliding at the root.
///
/// On other platforms the destination is used as-is.
#[cfg(windows)]
fn destination_path(raw_source: &str, destination: &str) -> Result<PathBuf> {
    use std::path::Component;

    let source_path = Path::new(raw_source);
    let Some(Component::Prefix(prefix)) = source_path.components().next() else {
        return Err(ExportError::MissingDrivePrefix(source_path.to_path_buf()));
    };
    let stripped = source_path
        .strip_prefix(prefix.as_os_str())
        .map_err(|_| ExportError::MissingDrivePrefix(source_path.to_path_buf()))?;
    Ok(Path::new(destination).join(stripped))
}

#[cfg(not(windows))]
#[allow(clippy::unnecessary_wraps)] // keeps signature aligned with Windows arm
fn destination_path(_raw_source: &str, destination: &str) -> Result<PathBuf> {
    Ok(Path::new(destination).to_path_buf())
}

fn print_time_elapsed(timer: Instant) {
    println!(
        "\r\n--------------------------\r\nTime elapsed: {:?} secs",
        timer.elapsed().as_secs()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn destination_path_em_linux_usa_destination_como_e() {
        let out = destination_path("/mnt/c/src", "/mnt/e/Trunk").unwrap();
        assert_eq!(out, PathBuf::from("/mnt/e/Trunk"));
    }

    #[test]
    #[cfg(windows)]
    fn destination_path_em_windows_espelha_source_sob_drive_do_destination() {
        // `-s L:\trunk\frente -d R:\` → `R:\trunk\frente`.
        let out = destination_path(r"L:\trunk\frente", r"R:\").unwrap();
        assert_eq!(out, PathBuf::from(r"R:\trunk\frente"));
    }

    #[test]
    #[cfg(windows)]
    fn destination_path_em_windows_descarta_path_apos_drive_no_destination() {
        // Quando o destination tem path além do drive, o `join` com um
        // source que começa com `\` (root sem prefix) preserva apenas o
        // drive do destination. Documenta a semântica — é o que a
        // referência `MateusZanchoNeto/exportbranch` sempre fez.
        let out = destination_path(r"L:\trunk\frente", r"R:\anything\else").unwrap();
        assert_eq!(out, PathBuf::from(r"R:\trunk\frente"));
    }

    #[test]
    #[cfg(windows)]
    fn destination_path_em_windows_exige_drive_prefix_no_source() {
        let err = destination_path(r"trunk\frente", r"R:\").unwrap_err();
        assert!(matches!(err, ExportError::MissingDrivePrefix(_)));
    }
}
