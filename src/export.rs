#![allow(missing_docs)]

use crate::configuration::Configuration;
use crate::convert_file::convert_file;
use crate::error::{ExportError, Result, WithPath};
use crate::file_checker::{FileChecker, FileStatus, FileUpdate};
use rayon::prelude::*;
use regex::RegexSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct WalkContext<'a> {
    pub destination_root: &'a Path,
    pub configuration: &'a Configuration,
    pub file_checker: &'a FileChecker,
    pub file_filters: &'a RegexSet,
    pub only_copy_files: &'a RegexSet,
}

/// Counts emitted by an export run. `converted + copied + skipped` equals
/// the number of files that matched the filter; files that didn't match
/// the filter are omitted entirely.
#[derive(Default, Debug, Clone, Copy)]
pub struct ExportStats {
    pub converted: usize,
    pub copied: usize,
    pub skipped: usize,
}

impl ExportStats {
    #[must_use]
    pub fn merge(mut self, other: ExportStats) -> ExportStats {
        self.converted += other.converted;
        self.copied += other.copied;
        self.skipped += other.skipped;
        self
    }

    #[must_use]
    pub fn total(self) -> usize {
        self.converted + self.copied + self.skipped
    }
}

/// Result of walking a single directory subtree: every modification that
/// needs to be persisted plus the aggregate counts for reporting.
#[derive(Default)]
pub struct ExportOutcome {
    pub updates: Vec<FileUpdate>,
    pub stats: ExportStats,
}

impl ExportOutcome {
    fn merge(mut self, mut other: ExportOutcome) -> ExportOutcome {
        self.updates.append(&mut other.updates);
        self.stats = self.stats.merge(other.stats);
        self
    }
}

fn is_disregarded(ctx: &WalkContext, dir: &Path) -> bool {
    let disregarded = ctx.configuration.disregarded_directories();
    // Fast path: the source root is canonicalized once in `lib::source_path`,
    // so every entry yielded by `read_dir` is already canonical and a direct
    // hash lookup is enough — no per-entry `canonicalize` syscall.
    !disregarded.is_empty() && disregarded.contains(dir)
}

pub fn export(ctx: &WalkContext, source: &Path, destination: PathBuf) -> Result<ExportOutcome> {
    let destination = format_lower(destination, ctx.destination_root, ctx.configuration.lower());

    // `create_dir_all` is idempotent — calling it directly saves the extra
    // `exists()` stat syscall on the (common) case where the dir is there.
    fs::create_dir_all(&destination).with_path(&destination)?;

    let entries: Vec<fs::DirEntry> = fs::read_dir(source)
        .with_path(source)?
        .map(|e| e.with_path(source))
        .collect::<Result<Vec<_>>>()?;

    entries
        .into_par_iter()
        .map(|entry| -> Result<ExportOutcome> {
            // `file_type()` reuses the value `readdir` already returned for
            // most filesystems — no extra `stat` syscall per entry.
            let file_type = entry.file_type().with_path(entry.path())?;
            let entry_path = entry.path();
            if file_type.is_dir() {
                if is_disregarded(ctx, &entry_path) {
                    return Ok(ExportOutcome::default());
                }
                export_directory(ctx, &entry_path, &destination)
            } else if file_type.is_file() {
                let target = destination.join(entry.file_name());
                Ok(file_outcome_to_export_outcome(export_file(
                    ctx, entry_path, target,
                )?))
            } else {
                Ok(ExportOutcome::default())
            }
        })
        .try_reduce(ExportOutcome::default, |a, b| Ok(a.merge(b)))
}

fn file_outcome_to_export_outcome(outcome: FileOutcome) -> ExportOutcome {
    let mut stats = ExportStats::default();
    let updates = match outcome {
        FileOutcome::Updated(update) => {
            if update.only_copy {
                stats.copied = 1;
            } else {
                stats.converted = 1;
            }
            vec![update]
        }
        FileOutcome::Skipped => {
            stats.skipped = 1;
            Vec::new()
        }
        FileOutcome::Filtered => Vec::new(),
    };
    ExportOutcome { updates, stats }
}

/// Outcome of considering a single file for export.
enum FileOutcome {
    /// File was converted or copied; carries the metadata to persist.
    Updated(FileUpdate),
    /// File matched the filter but its mtime hadn't changed since the last run.
    Skipped,
    /// File was excluded by `--filters` — not counted in the stats.
    Filtered,
}

fn format_lower(destination: PathBuf, raw_destination: &Path, lower: bool) -> PathBuf {
    if !lower {
        return destination;
    }

    let suffix = destination
        .strip_prefix(raw_destination)
        .unwrap_or(&destination);
    let lowered = suffix.to_string_lossy().to_lowercase();
    raw_destination.join(lowered)
}

fn export_directory(ctx: &WalkContext, source: &Path, destination: &Path) -> Result<ExportOutcome> {
    let entry_file_name = source
        .file_name()
        .ok_or_else(|| ExportError::MissingFileName(source.to_path_buf()))?;
    let dest_path = destination.join(entry_file_name);
    export(ctx, source, dest_path)
}

fn export_file(
    ctx: &WalkContext,
    source_file: PathBuf,
    destination_file: PathBuf,
) -> Result<FileOutcome> {
    if !file_match(&source_file, ctx.file_filters) {
        return Ok(FileOutcome::Filtered);
    }

    let destination_file = format_lower(
        destination_file,
        ctx.destination_root,
        ctx.configuration.lower(),
    );

    let modified = match file_need_update(&source_file, ctx, &destination_file) {
        FileStatus::UpToDate => return Ok(FileOutcome::Skipped),
        FileStatus::Modified(system_time) => system_time,
    };

    let only_copy = file_match(&source_file, ctx.only_copy_files);

    if !ctx.configuration.quiet() {
        print_file(only_copy, &source_file, &destination_file);
    }

    if only_copy {
        fs::copy(&source_file, &destination_file).with_path(&source_file)?;
    } else {
        convert_file(&source_file, &destination_file).with_path(&source_file)?;
    }

    Ok(FileOutcome::Updated(FileUpdate {
        path: source_file,
        modified,
        only_copy,
    }))
}

fn print_file(only_copy: bool, entry_path: &Path, dest_path: &Path) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = writeln!(
        handle,
        "{}",
        format_file_line(only_copy, entry_path, dest_path)
    );
}

fn format_file_line(only_copy: bool, source: &Path, destination: &Path) -> String {
    let verb = if only_copy { "copy" } else { "convert" };
    let source_str = source.to_string_lossy();
    let source_display = source_path_display(&source_str);
    let destination_str = destination.to_string_lossy();
    format!("{verb:<7} {source_display} → {destination_str}")
}

#[cfg(target_os = "windows")]
fn source_path_display(entry_path: &str) -> &str {
    entry_path.strip_prefix(r"\\?\").unwrap_or(entry_path)
}

#[cfg(not(target_os = "windows"))]
fn source_path_display(entry_path: &str) -> &str {
    entry_path
}

fn file_match(file: &Path, filters: &RegexSet) -> bool {
    let Some(file_name) = file.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    filters.is_match(file_name)
}

fn file_need_update(file: &Path, ctx: &WalkContext, destination_file: &Path) -> FileStatus {
    let configuration = ctx.configuration;

    if configuration.reload() || (configuration.exists() && !destination_file.exists()) {
        return ctx.file_checker.force_update(file);
    }

    ctx.file_checker.check(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export_branch_files::checked_to_regex_set;

    #[test]
    fn file_match_casa_extensao_prg() {
        let set = checked_to_regex_set(&["*.prg".to_string()]).unwrap();
        let path = PathBuf::from("foo.prg");
        assert!(file_match(&path, &set));
    }

    #[test]
    fn file_match_nao_casa_extensao_diferente() {
        let set = checked_to_regex_set(&["*.prg".to_string()]).unwrap();
        let path = PathBuf::from("foo.txt");
        assert!(!file_match(&path, &set));
    }

    #[test]
    fn file_match_aceita_qualquer_um_dos_filtros() {
        let set = checked_to_regex_set(&["*.prg".to_string(), "*.ch".to_string()]).unwrap();
        assert!(file_match(&PathBuf::from("a.prg"), &set));
        assert!(file_match(&PathBuf::from("b.ch"), &set));
        assert!(!file_match(&PathBuf::from("c.txt"), &set));
    }

    #[test]
    fn file_match_aceita_path_sem_nome_retorna_false() {
        let set = checked_to_regex_set(&["*.prg".to_string()]).unwrap();
        assert!(!file_match(Path::new(""), &set));
    }

    #[test]
    fn format_file_line_convert_usa_verbo_convert() {
        let line = format_file_line(false, Path::new("/src/foo.prg"), Path::new("/dst/foo.prg"));
        assert_eq!(line, "convert /src/foo.prg → /dst/foo.prg");
    }

    #[test]
    fn format_file_line_copy_usa_verbo_copy_alinhado_em_sete() {
        let line = format_file_line(true, Path::new("/src/lib.a"), Path::new("/dst/lib.a"));
        assert_eq!(line, "copy    /src/lib.a → /dst/lib.a");
    }
}
