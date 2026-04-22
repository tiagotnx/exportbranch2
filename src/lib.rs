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
    let destination_path_buffer = destination_path(&source_path_buffer, destination);
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

/// On Windows, mirror the canonical source path under `destination` so a
/// source like `L:\trunk\include` lands at `<destination>\trunk\include`.
/// We strip both `Prefix` (drive / `\\?\` UNC) and `RootDir` (the leading
/// `\`) — without removing `RootDir` the result becomes drive-relative and
/// `PathBuf::push` would silently discard `destination`.
///
/// On other platforms `destination` is used as-is.
#[cfg(windows)]
fn destination_path(canonical_source: &Path, destination: &str) -> PathBuf {
    use std::path::Component;

    let relative: PathBuf = canonical_source
        .components()
        .filter(|c| !matches!(c, Component::Prefix(_) | Component::RootDir))
        .map(|c| c.as_os_str())
        .collect();
    Path::new(destination).join(relative)
}

#[cfg(not(windows))]
fn destination_path(_canonical_source: &Path, destination: &str) -> PathBuf {
    Path::new(destination).to_path_buf()
}

fn print_time_elapsed(timer: Instant) {
    println!(
        "\r\n--------------------------\r\nTime elapsed: {:?} secs",
        timer.elapsed().as_secs()
    );
}
