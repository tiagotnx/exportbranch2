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
/// Opt-in debug log (`--debug`) for post-mortem diagnostics.
pub mod debug_log;
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
    let argv_snapshot = args.clone();
    let configuration = Configuration::build(&mut args.into_iter())?;

    let log = configuration.debug_log();
    debug_log!(log, "exportbranch {}", env!("CARGO_PKG_VERSION"));
    debug_log!(log, "argv: {argv_snapshot:?}");
    debug_log!(log, "source: {:?}", configuration.source());
    debug_log!(log, "destination: {:?}", configuration.destination());
    debug_log!(log, "file_filters: {:?}", configuration.file_filters());
    debug_log!(
        log,
        "only_copy_files: {:?}",
        configuration.only_copy_files()
    );
    debug_log!(
        log,
        "flags: exists={} reload={} lower={}",
        configuration.exists(),
        configuration.reload(),
        configuration.lower()
    );
    debug_log!(
        log,
        "disregarded_directories: {:?}",
        configuration.disregarded_directories()
    );

    configuration.print();

    for source in configuration.source() {
        for destination in configuration.destination() {
            export(source, destination, &configuration)?;
        }
    }

    debug_log!(log, "finished in {} secs", timer.elapsed().as_secs());
    print_time_elapsed(timer);
    Ok(())
}

fn export(source: &str, destination: &str, configuration: &Configuration) -> Result<()> {
    let log = configuration.debug_log();
    debug_log!(log, "export start: -s {source:?} -d {destination:?}");
    let source_path_buffer = source_path(source)?;
    let destination_path_buffer = destination_path(source, destination)?;
    debug_log!(log, "  source canonical: {source_path_buffer:?}");
    debug_log!(log, "  destination resolved: {destination_path_buffer:?}");
    let mut file_checker = FileChecker::new(destination_path_buffer.clone());
    let mut export = ExportBranch::build(
        source_path_buffer,
        destination_path_buffer,
        configuration,
        &mut file_checker,
    );

    let result = export.perform_exporting();
    match &result {
        Ok(()) => debug_log!(log, "export ok: -s {source:?} -d {destination:?}"),
        Err(e) => debug_log!(log, "export failed: -s {source:?} -d {destination:?}: {e}"),
    }
    result
}

fn source_path(source: &str) -> Result<PathBuf> {
    Path::new(source).canonicalize().with_path(source)
}

/// Build the destination root for an export.
///
/// On Windows `destination` is used as a root and the source layout (minus
/// its drive `Prefix`) is mirrored underneath it. The drive of the source
/// is stripped, the leading root component (`\`) is dropped so that
/// `Path::join` treats the remainder as *relative*, and the result is
/// joined onto `destination`. Examples:
///
/// - `-s L:\trunk\frente -d R:\` → `R:\trunk\frente`
/// - `-s L:\trunk\frente -d R:\Trunk2` → `R:\Trunk2\trunk\frente`
/// - `-s T:\new -d L:\trunk2\` → `L:\trunk2\new`
/// - `-s C:\ProdutosSG\Trunk -d E:\` → `E:\ProdutosSG\Trunk`
///
/// Dropping the root component matters because Windows `Path::join` would
/// otherwise keep only the `Prefix` of the left-hand side when the
/// right-hand side has a root — which would collapse `R:\Trunk2` back to
/// `R:\`, the v0.1.6 behaviour this release replaces.
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
    let relative: PathBuf = stripped
        .components()
        .filter(|c| !matches!(c, Component::RootDir))
        .collect();
    Ok(Path::new(destination).join(relative))
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
    fn destination_path_em_windows_preserva_path_apos_drive_no_destination() {
        // `-s L:\trunk\frente -d R:\Trunk2` → `R:\Trunk2\trunk\frente`.
        // Diferente do v0.1.6 (que descartava `\Trunk2`): agora o destination
        // é usado como raiz e o source é espelhado (sem drive) abaixo dele.
        let out = destination_path(r"L:\trunk\frente", r"R:\Trunk2").unwrap();
        assert_eq!(out, PathBuf::from(r"R:\Trunk2\trunk\frente"));
    }

    #[test]
    #[cfg(windows)]
    fn destination_path_caso_do_bug_reportado() {
        // `-s T:\new -d L:\trunk2\` → `L:\trunk2\new`.
        // Regressão: o v0.1.6 colapsava para `L:\new` e o usuário esperava
        // a forma com o `trunk2` preservado.
        let out = destination_path(r"T:\new", r"L:\trunk2\").unwrap();
        assert_eq!(out, PathBuf::from(r"L:\trunk2\new"));
    }

    #[test]
    #[cfg(windows)]
    fn destination_path_em_windows_exige_drive_prefix_no_source() {
        let err = destination_path(r"trunk\frente", r"R:\").unwrap_err();
        assert!(matches!(err, ExportError::MissingDrivePrefix(_)));
    }
}
