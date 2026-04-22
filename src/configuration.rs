#![allow(missing_docs)]

use crate::error::{ExportError, Result, WithPath};
use clap::{ArgAction, Parser};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_ONLY_COPY_FILES: [&str; 5] = ["*.a", "*.so", "*.h", "*.0", "*.18"];

const DEFAULT_FILTERS: [&str; 19] = [
    "*.prg", "*.mke", "*.mkp", "*.mks", "*.mkc", "*.hbp", "*.hbc", "*.hbm", "*.ch", "*.so*",
    "*.cpp", "*.a", "*.c", "*.h", "*.sh", "*.0", "*.18", "*.jar", "*.spec",
];

const DISREGARDED_DIRECTORIES: [&str; 5] = [
    "bin",
    "lib",
    "new/fivewin",
    "programas_externos/conversoes",
    "programas_externos/hbfunctions",
];

#[derive(Parser, Debug)]
#[command(
    name = "exportbranch",
    version,
    about = "Filter, copy and convert Harbour branch files for Compex.",
    disable_help_flag = false,
    arg_required_else_help = true
)]
struct Cli {
    /// Source path(s); use `;` to pass multiple roots.
    #[arg(short = 's', long = "source", value_delimiter = ';', required = true)]
    source: Vec<String>,

    /// Destination path(s); use `;` to pass multiple destinations.
    #[arg(
        short = 'd',
        long = "destination",
        value_delimiter = ';',
        required = true
    )]
    destination: Vec<String>,

    /// Glob patterns copied byte-for-byte (no CP850 conversion).
    #[arg(short = 'c', long = "only-copy", value_delimiter = ';')]
    only_copy_files: Vec<String>,

    /// Glob patterns selected for export.
    #[arg(short = 'f', long = "filters", value_delimiter = ';')]
    file_filters: Vec<String>,

    /// Print the parsed configuration before exporting.
    #[arg(long, action = ArgAction::SetTrue)]
    show: bool,

    /// Force re-export when the destination copy is missing.
    #[arg(long, action = ArgAction::SetTrue)]
    exists: bool,

    /// DEPRECATED alias of --exists; will be removed in a future release.
    #[arg(long, action = ArgAction::SetTrue)]
    md5: bool,

    /// Re-export all files, ignoring the modification tracker.
    #[arg(long, action = ArgAction::SetTrue)]
    reload: bool,

    /// Lowercase directory and file names under the destination.
    #[arg(long, action = ArgAction::SetTrue)]
    lower: bool,
}

pub struct Configuration {
    source: Vec<String>,
    destination: Vec<String>,
    only_copy_files: Vec<String>,
    file_filters: Vec<String>,
    show: bool,
    exists: bool,
    reload: bool,
    lower: bool,
    disregarded_directories: HashSet<PathBuf>,
}

impl Configuration {
    pub fn build(args: &mut impl Iterator<Item = String>) -> Result<Configuration> {
        let cli = Cli::try_parse_from(args).map_err(|e| ExportError::InvalidArgs(e.to_string()))?;

        if cli.md5 {
            eprintln!(
                "warning: --md5 is deprecated, use --exists instead (the flag will be removed in a future release)"
            );
        }

        let exists = cli.exists || cli.md5;

        let source: Vec<String> = cli.source.into_iter().filter(|s| !s.is_empty()).collect();
        let destination: Vec<String> = cli
            .destination
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();

        if source.is_empty() || destination.is_empty() {
            return Err(ExportError::InvalidArgs(
                "source and destination cannot be empty".into(),
            ));
        }

        let mut disregarded_directories: HashSet<PathBuf> = HashSet::new();
        for source_directory in &source {
            let source_path = Path::new(source_directory)
                .canonicalize()
                .with_path(source_directory)?;
            for disregarded_directory in DISREGARDED_DIRECTORIES {
                let joined = source_path.join(disregarded_directory);
                // Only insert if it actually exists — missing dirs can't be
                // visited during the walk, and `canonicalize` would fail on
                // them. This also normalizes to the same form as paths
                // yielded by `read_dir` from the canonical source.
                if let Ok(canon) = joined.canonicalize() {
                    disregarded_directories.insert(canon);
                }
            }
        }

        Ok(Configuration {
            source,
            destination,
            only_copy_files: if cli.only_copy_files.is_empty() {
                DEFAULT_ONLY_COPY_FILES.map(str::to_string).to_vec()
            } else {
                cli.only_copy_files
            },
            file_filters: if cli.file_filters.is_empty() {
                DEFAULT_FILTERS.map(str::to_string).to_vec()
            } else {
                cli.file_filters
            },
            exists,
            reload: cli.reload,
            lower: cli.lower,
            disregarded_directories,
            show: cli.show,
        })
    }

    pub fn print(&self) {
        if self.show {
            println!(
                "Export Branch\r\nsource.........: {:?}\r\ndestination....: {:?}\r\nonly_copy_files: {:?}\r\nfile_filters...: {:?}\r\nexists.........: {:?}\r\nreload.........: {:?}\r\nlower..........: {:?}\r\ndisregarded....: {:?}\r\n",
                self.source,
                self.destination,
                self.only_copy_files,
                self.file_filters,
                self.exists,
                self.reload,
                self.lower,
                self.disregarded_directories,
            );
        }
        println!("--------------------------\r\nExporting...\r\n");
    }

    pub fn source(&self) -> &Vec<String> {
        &self.source
    }

    pub fn destination(&self) -> &Vec<String> {
        &self.destination
    }

    pub fn only_copy_files(&self) -> &Vec<String> {
        &self.only_copy_files
    }

    pub fn file_filters(&self) -> &Vec<String> {
        &self.file_filters
    }

    pub fn exists(&self) -> bool {
        self.exists
    }

    pub fn reload(&self) -> bool {
        self.reload
    }

    pub fn lower(&self) -> bool {
        self.lower
    }

    pub fn disregarded_directories(&self) -> &HashSet<PathBuf> {
        &self.disregarded_directories
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_iter(args: &[&str]) -> std::vec::IntoIter<String> {
        std::iter::once("exportbranch")
            .chain(args.iter().copied())
            .map(String::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn build_sem_source_retorna_erro() {
        let result = Configuration::build(&mut args_iter(&["-d", "/tmp"]));
        assert!(matches!(result, Err(ExportError::InvalidArgs(_))));
    }

    #[test]
    fn build_sem_destination_retorna_erro() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let result = Configuration::build(&mut args_iter(&["-s", p]));
        assert!(matches!(result, Err(ExportError::InvalidArgs(_))));
    }

    #[test]
    fn build_aceita_paths_separados_por_ponto_virgula() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let arg = format!("{p};{p}");
        let cfg = Configuration::build(&mut args_iter(&["-s", &arg, "-d", "/tmp;/tmp2"])).unwrap();
        assert_eq!(cfg.source().len(), 2);
        assert_eq!(cfg.destination().len(), 2);
    }

    #[test]
    fn build_exists_ativa_flag() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg =
            Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "--exists"])).unwrap();
        assert!(cfg.exists());
    }

    #[test]
    fn build_md5_ainda_funciona_como_alias_depreciado() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "--md5"])).unwrap();
        assert!(cfg.exists());
    }

    #[test]
    fn build_arg_desconhecido_retorna_help() {
        let result = Configuration::build(&mut args_iter(&["-z", "foo"]));
        assert!(matches!(result, Err(ExportError::InvalidArgs(_))));
    }

    #[test]
    fn build_filters_default_quando_omitido() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp"])).unwrap();
        assert!(cfg.file_filters().contains(&"*.prg".to_string()));
        assert!(cfg.only_copy_files().contains(&"*.h".to_string()));
    }

    #[test]
    fn build_filters_customizados_substituem_default() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg =
            Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "-f", "*.txt;*.md"]))
                .unwrap();
        assert_eq!(
            cfg.file_filters(),
            &vec!["*.txt".to_string(), "*.md".to_string()]
        );
    }

    #[test]
    fn build_source_inexistente_retorna_erro_io() {
        let result = Configuration::build(&mut args_iter(&[
            "-s",
            "/non/existent/path/xyz123",
            "-d",
            "/tmp",
        ]));
        assert!(matches!(result, Err(ExportError::Io { .. })));
    }

    #[test]
    fn build_flag_duplicada_sem_valor_retorna_erro() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        // `-d -s p` — o -d fica sem valor
        let result = Configuration::build(&mut args_iter(&["-d", "-s", p]));
        assert!(matches!(result, Err(ExportError::InvalidArgs(_))));
    }

    #[test]
    fn build_flag_trailing_sem_valor_retorna_erro() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let result = Configuration::build(&mut args_iter(&["-s", p, "-d"]));
        assert!(matches!(result, Err(ExportError::InvalidArgs(_))));
    }

    #[test]
    fn build_aceita_long_options() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["--source", p, "--destination", "/tmp"]))
            .unwrap();
        assert_eq!(cfg.source(), &vec![p.to_string()]);
    }

    #[test]
    fn build_aceita_equals_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let arg_s = format!("--source={p}");
        let cfg = Configuration::build(&mut args_iter(&[&arg_s, "--destination=/tmp"])).unwrap();
        assert_eq!(cfg.source(), &vec![p.to_string()]);
    }
}
