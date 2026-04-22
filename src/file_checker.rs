#![allow(missing_docs)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Result;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const EXPORT_BRANCH_FILES_METADATA: &str = "export_branch_files_metadata.txt";

pub enum FileStatus {
    UpToDate,
    Modified(SystemTime),
}

pub struct FileUpdate {
    pub path: PathBuf,
    pub modified: SystemTime,
}

pub struct FileChecker {
    directory: PathBuf,
    files: HashMap<String, u128>,
}

impl FileChecker {
    pub fn new(directory: PathBuf) -> FileChecker {
        match FileChecker::read_file(&directory) {
            Ok(contents) => FileChecker::build(directory, &contents),
            _ => FileChecker::default(directory),
        }
    }

    pub fn check(&self, file: &Path) -> FileStatus {
        match FileChecker::get_modified(file) {
            Ok(system_time) => {
                let current = to_nanos(system_time);
                let key = file.to_string_lossy();
                match self.files.get(key.as_ref()) {
                    Some(&stored) if stored == current => FileStatus::UpToDate,
                    _ => FileStatus::Modified(system_time),
                }
            }
            _ => FileStatus::Modified(SystemTime::now()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let mut file = FileChecker::get_file(&self.directory)?;
        let mut contents = String::new();

        for (file_name, nanos) in &self.files {
            let _ = writeln!(contents, "{file_name};{nanos}");
        }
        file.write_all(contents.as_bytes())
    }

    pub fn add_file(&mut self, file: &Path, system_time: SystemTime) {
        self.files
            .insert(file.to_string_lossy().into_owned(), to_nanos(system_time));
    }

    pub fn force_update(&self, file: &Path) -> FileStatus {
        match FileChecker::get_modified(file) {
            Ok(system_time) => FileStatus::Modified(system_time),
            _ => FileStatus::Modified(SystemTime::now()),
        }
    }

    pub fn apply(&mut self, update: &FileUpdate) {
        self.add_file(&update.path, update.modified);
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl FileChecker {
    fn default(directory: PathBuf) -> FileChecker {
        FileChecker {
            directory,
            files: HashMap::new(),
        }
    }

    fn read_file(directory: &Path) -> Result<String> {
        let mut file = FileChecker::get_file(directory)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Ok(contents)
    }

    fn get_file(directory: &Path) -> Result<File> {
        let file = directory.join(EXPORT_BRANCH_FILES_METADATA);

        if !file.exists() {
            return File::create(file);
        }

        File::options().read(true).write(true).open(file)
    }

    fn build(directory: PathBuf, contents: &str) -> FileChecker {
        let mut files = HashMap::new();

        for line in contents.lines() {
            let mut parts = line.splitn(2, ';');
            let Some(file_name) = parts.next() else {
                continue;
            };
            let Some(file_metadata) = parts.next() else {
                continue;
            };
            // Nanos (new format). Legacy Debug-format entries fail to parse
            // and are skipped — the file will be re-exported once on next
            // run and then re-saved in the new format.
            if let Ok(nanos) = file_metadata.parse::<u128>() {
                files.insert(file_name.to_string(), nanos);
            }
        }
        FileChecker { directory, files }
    }

    fn get_modified(file: &Path) -> Result<SystemTime> {
        let metadata = file.metadata()?;
        let modified = metadata.modified()?;
        Ok(modified)
    }
}

// Pre-epoch timestamps collapse to 0; in practice any FS we read from sets
// mtime ≥ 1970, so the only realistic path to 0 is a clock skew bug. A
// collision would force one extra re-export per affected file — acceptable.
fn to_nanos(t: SystemTime) -> u128 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn build_ignora_metadata_em_formato_legado() {
        // Linha 1: formato antigo (Debug SystemTime). Linha 2: novo formato.
        let dir = tempfile::tempdir().unwrap();
        let legacy = "/some/path;SystemTime { tv_sec: 100, tv_nsec: 0 }\n\
                      /other/path;123456789\n";
        let checker = FileChecker::build(dir.path().to_path_buf(), legacy);
        assert_eq!(checker.files.get("/other/path"), Some(&123_456_789u128));
        assert!(!checker.files.contains_key("/some/path"));
    }

    #[test]
    fn save_e_reload_preserva_entrada() {
        let dir = tempfile::tempdir().unwrap();
        let mut checker = FileChecker::new(dir.path().to_path_buf());

        let fake_file = dir.path().join("foo.prg");
        std::fs::write(&fake_file, b"x").unwrap();
        // Use whole seconds — `SystemTime` resolution is 100 ns on Windows,
        // so sub-microsecond offsets get rounded and the round-trip would
        // not match.
        let t = UNIX_EPOCH + Duration::from_secs(42);
        checker.add_file(&fake_file, t);
        checker.save().unwrap();

        let reloaded = FileChecker::new(dir.path().to_path_buf());
        let key = fake_file.to_string_lossy().into_owned();
        assert_eq!(reloaded.files.get(&key), Some(&42_000_000_000u128));
    }
}
