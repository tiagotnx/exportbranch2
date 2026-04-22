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

fn is_disregarded(ctx: &WalkContext, dir: &Path) -> bool {
    let disregarded = ctx.configuration.disregarded_directories();
    // Fast path: the source root is canonicalized once in `lib::source_path`,
    // so every entry yielded by `read_dir` is already canonical and a direct
    // hash lookup is enough — no per-entry `canonicalize` syscall.
    !disregarded.is_empty() && disregarded.contains(dir)
}

pub fn export(ctx: &WalkContext, source: &Path, destination: PathBuf) -> Result<Vec<FileUpdate>> {
    let destination = format_lower(destination, ctx.destination_root, ctx.configuration.lower());

    if !destination.exists() {
        fs::create_dir_all(&destination).with_path(&destination)?;
    }

    let entries: Vec<fs::DirEntry> = fs::read_dir(source)
        .with_path(source)?
        .map(|e| e.with_path(source))
        .collect::<Result<Vec<_>>>()?;

    let nested: Vec<Vec<FileUpdate>> = entries
        .into_par_iter()
        .map(|entry| -> Result<Vec<FileUpdate>> {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if is_disregarded(ctx, &entry_path) {
                    return Ok(Vec::new());
                }
                export_directory(ctx, &entry_path, &destination)
            } else if entry_path.is_file() {
                let target = destination.join(entry.file_name());
                Ok(export_file(ctx, entry_path, target)?.into_iter().collect())
            } else {
                Ok(Vec::new())
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(nested.into_iter().flatten().collect())
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

fn export_directory(
    ctx: &WalkContext,
    source: &Path,
    destination: &Path,
) -> Result<Vec<FileUpdate>> {
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
) -> Result<Option<FileUpdate>> {
    if !file_match(&source_file, ctx.file_filters) {
        return Ok(None);
    }

    let destination_file = format_lower(
        destination_file,
        ctx.destination_root,
        ctx.configuration.lower(),
    );

    let modified = match file_need_update(&source_file, ctx, &destination_file) {
        FileStatus::UpToDate => return Ok(None),
        FileStatus::Modified(system_time) => system_time,
    };

    let only_copy = file_match(&source_file, ctx.only_copy_files);

    print_file(only_copy, &source_file, &destination_file);

    if only_copy {
        fs::copy(&source_file, &destination_file).with_path(&source_file)?;
    } else {
        convert_file(&source_file, &destination_file).with_path(&source_file)?;
    }

    Ok(Some(FileUpdate {
        path: source_file,
        modified,
    }))
}

fn print_file(only_copy: bool, entry_path: &Path, dest_path: &Path) {
    let label = if only_copy {
        "copying..."
    } else {
        "converting..."
    };
    let entry_str = entry_path.to_string_lossy();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(
        handle,
        "{label}\r\nsource.....: {}\r\ndestination: {}\r",
        source_path_display(&entry_str),
        dest_path.to_string_lossy()
    );
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
}
