#![allow(missing_docs)]

use crate::debug_log::{default_log_path, DebugLog};
use crate::error::{ExportError, Result, WithPath};
use clap::{ArgAction, CommandFactory, Parser};
use clap_complete::Shell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Long version string emitted by `--version`. Includes the cargo version,
/// the short git SHA captured at build time, and the commit date — handy
/// when triaging "which build is this?" support questions.
pub const VERSION_LONG: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_SHA"),
    ", ",
    env!("GIT_DATE"),
    ")"
);

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
    version = VERSION_LONG,
    about = "Filter, copy and convert Harbour branch files for Compex.",
    disable_help_flag = false,
    arg_required_else_help = true
)]
struct Cli {
    /// Source path(s); use `;` to pass multiple roots.
    #[arg(short = 's', long = "source", value_delimiter = ';')]
    source: Vec<String>,

    /// Destination path(s); use `;` to pass multiple destinations.
    #[arg(short = 'd', long = "destination", value_delimiter = ';')]
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

    /// Suppress all non-error output; only the final summary stays.
    #[arg(short = 'q', long, action = ArgAction::SetTrue, conflicts_with = "show")]
    quiet: bool,

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

    /// Write a debug log to PATH. If the flag is given without a value,
    /// writes to `exportbranch-<unix_secs>.log` in the current directory.
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "")]
    debug: Option<String>,

    /// Generate a shell completion script and exit. Skips -s/-d validation.
    /// Example: `exportbranch --completions bash > exportbranch.bash`.
    #[arg(long, value_name = "SHELL", value_enum, exclusive = true)]
    completions: Option<Shell>,
}

pub struct Configuration {
    source: Vec<String>,
    destination: Vec<String>,
    only_copy_files: Vec<String>,
    file_filters: Vec<String>,
    show: bool,
    quiet: bool,
    exists: bool,
    reload: bool,
    lower: bool,
    disregarded_directories: HashSet<PathBuf>,
    debug_log: Option<DebugLog>,
    completions: Option<Shell>,
}

/// On Windows, rewrites items of `-s` that are drive-relative (leading
/// `\` or `/`) so they inherit the drive `Prefix` of the first item.
/// Lets users write `T:\new;\src;\include` as shorthand for
/// `T:\new;T:\src;T:\include`. Items with their own drive and items
/// without a leading separator (`src`) pass through unchanged. If a
/// drive-relative item needs to inherit and the first item has no drive
/// to inherit from, returns `MissingDrivePrefix`.
#[cfg(windows)]
fn normalize_source_drives(sources: Vec<String>) -> Result<Vec<String>> {
    use std::path::Component;

    let mut iter = sources.into_iter();
    let Some(first) = iter.next() else {
        return Ok(Vec::new());
    };

    let prefix = match Path::new(&first).components().next() {
        Some(Component::Prefix(p)) => Some(p.as_os_str().to_os_string()),
        _ => None,
    };

    let mut out = vec![first];
    for item in iter {
        let has_prefix = matches!(
            Path::new(&item).components().next(),
            Some(Component::Prefix(_))
        );
        let needs_inherit = !has_prefix && (item.starts_with('\\') || item.starts_with('/'));
        if needs_inherit {
            match prefix.as_ref() {
                Some(p) => out.push(format!("{}{}", p.to_string_lossy(), item)),
                None => return Err(ExportError::MissingDrivePrefix(PathBuf::from(&item))),
            }
        } else {
            out.push(item);
        }
    }
    Ok(out)
}

#[cfg(not(windows))]
#[allow(clippy::unnecessary_wraps)]
fn normalize_source_drives(sources: Vec<String>) -> Result<Vec<String>> {
    Ok(sources)
}

impl Configuration {
    pub fn build(args: &mut impl Iterator<Item = String>) -> Result<Configuration> {
        let cli = Cli::try_parse_from(args).map_err(|e| ExportError::InvalidArgs(e.to_string()))?;

        // `--completions` short-circuits everything: no -s/-d validation, no
        // disregarded-dir canonicalization, no debug log file. The caller
        // (`lib::run`) inspects `completions()` and emits the script.
        if let Some(shell) = cli.completions {
            return Ok(Configuration::completions_only(shell));
        }

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

        let source = normalize_source_drives(source)?;

        let debug_log = match cli.debug {
            None => None,
            Some(ref p) if p.is_empty() => Some(DebugLog::create(&default_log_path())?),
            Some(ref p) => Some(DebugLog::create(Path::new(p))?),
        };

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
            quiet: cli.quiet,
            debug_log,
            completions: None,
        })
    }

    fn completions_only(shell: Shell) -> Configuration {
        Configuration {
            source: Vec::new(),
            destination: Vec::new(),
            only_copy_files: Vec::new(),
            file_filters: Vec::new(),
            show: false,
            quiet: false,
            exists: false,
            reload: false,
            lower: false,
            disregarded_directories: HashSet::new(),
            debug_log: None,
            completions: Some(shell),
        }
    }

    /// Writes a generated completion script for `shell` to `out`. Used by
    /// `lib::run` when `--completions` was passed.
    pub fn emit_completions<W: std::io::Write>(shell: Shell, out: &mut W) {
        clap_complete::generate(shell, &mut Cli::command(), "exportbranch", out);
    }

    pub fn print(&self) {
        if self.quiet {
            return;
        }
        eprint!("{}", self.format_summary());
    }

    fn format_summary(&self) -> String {
        use std::fmt::Write;

        let join = |v: &[String]| v.join(";");
        let mut out = String::new();
        let _ = writeln!(out, "source       = {}", join(&self.source));
        let _ = writeln!(out, "destination  = {}", join(&self.destination));
        if self.show {
            let default_filters: Vec<String> =
                DEFAULT_FILTERS.iter().map(|s| (*s).to_string()).collect();
            if self.file_filters != default_filters {
                let _ = writeln!(out, "filters      = {}", join(&self.file_filters));
            }
            let default_only_copy: Vec<String> = DEFAULT_ONLY_COPY_FILES
                .iter()
                .map(|s| (*s).to_string())
                .collect();
            if self.only_copy_files != default_only_copy {
                let _ = writeln!(out, "only-copy    = {}", join(&self.only_copy_files));
            }
            if self.exists || self.reload || self.lower {
                let _ = writeln!(
                    out,
                    "flags        = exists:{} reload:{} lower:{}",
                    self.exists, self.reload, self.lower
                );
            }
        }
        out.push_str("Exporting...\n");
        out
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

    pub fn quiet(&self) -> bool {
        self.quiet
    }

    pub fn completions(&self) -> Option<Shell> {
        self.completions
    }

    pub fn disregarded_directories(&self) -> &HashSet<PathBuf> {
        &self.disregarded_directories
    }

    pub fn debug_log(&self) -> Option<&DebugLog> {
        self.debug_log.as_ref()
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
    fn build_completions_dispensa_source_e_destination() {
        let cfg = Configuration::build(&mut args_iter(&["--completions", "bash"])).unwrap();
        assert_eq!(cfg.completions(), Some(Shell::Bash));
        assert!(cfg.source().is_empty());
        assert!(cfg.destination().is_empty());
    }

    #[test]
    fn emit_completions_gera_script_nao_vazio_para_bash() {
        let mut buf: Vec<u8> = Vec::new();
        Configuration::emit_completions(Shell::Bash, &mut buf);
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("exportbranch"), "script de completions vazio");
    }

    #[test]
    fn build_quiet_ativa_flag() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "-q"])).unwrap();
        assert!(cfg.quiet());
    }

    #[test]
    fn build_quiet_e_show_sao_mutuamente_exclusivos() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let result = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "-q", "--show"]));
        assert!(matches!(result, Err(ExportError::InvalidArgs(_))));
    }

    #[test]
    fn format_summary_quiet_e_no_op_via_print() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "-q"])).unwrap();
        // print() não tem return value testável, mas se quiet=true, deve
        // sair antes do format_summary. O smoke test aqui é só garantir que
        // a flag está corretamente lida.
        assert!(cfg.quiet());
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

    #[test]
    #[cfg(windows)]
    fn normalize_source_drives_item_sem_drive_herda_drive_do_primeiro() {
        let out =
            normalize_source_drives(vec![r"T:\new".into(), r"\src".into(), r"\include".into()])
                .unwrap();
        assert_eq!(
            out,
            vec![
                r"T:\new".to_string(),
                r"T:\src".to_string(),
                r"T:\include".to_string(),
            ]
        );
    }

    #[test]
    #[cfg(windows)]
    fn normalize_source_drives_item_com_forward_slash_herda_drive() {
        let out = normalize_source_drives(vec![r"T:\new".into(), r"/src".into()]).unwrap();
        assert_eq!(out, vec![r"T:\new".to_string(), r"T:/src".to_string()]);
    }

    #[test]
    #[cfg(windows)]
    fn normalize_source_drives_primeiro_sem_drive_com_herdeiro_retorna_erro() {
        // Primeiro item é drive-relative e o segundo também tenta herdar →
        // não há drive de onde herdar, erro imediato.
        let err = normalize_source_drives(vec![r"\src".into(), r"\include".into()]).unwrap_err();
        assert!(matches!(err, ExportError::MissingDrivePrefix(_)));
    }

    #[test]
    #[cfg(windows)]
    fn normalize_source_drives_unico_item_sem_drive_passa_como_esta() {
        // Ninguém para herdar: deixa passar e o canonicalize lida com o erro.
        let out = normalize_source_drives(vec![r"\src".into()]).unwrap();
        assert_eq!(out, vec![r"\src".to_string()]);
    }

    #[test]
    #[cfg(windows)]
    fn normalize_source_drives_preserva_items_com_drive_proprio() {
        let out = normalize_source_drives(vec![r"T:\new".into(), r"U:\other".into()]).unwrap();
        assert_eq!(out, vec![r"T:\new".to_string(), r"U:\other".to_string()]);
    }

    #[test]
    #[cfg(windows)]
    fn normalize_source_drives_item_relativo_sem_leading_sep_passa_como_esta() {
        let out = normalize_source_drives(vec![r"T:\new".into(), r"src".into()]).unwrap();
        assert_eq!(out, vec![r"T:\new".to_string(), r"src".to_string()]);
    }

    #[test]
    #[cfg(not(windows))]
    fn normalize_source_drives_em_linux_e_no_op() {
        let input = vec!["/tmp/a".to_string(), "/tmp/b".to_string()];
        let out = normalize_source_drives(input.clone()).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn build_sem_debug_flag_nao_cria_log() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp"])).unwrap();
        assert!(cfg.debug_log().is_none());
    }

    #[test]
    fn build_debug_flag_com_path_cria_arquivo_de_log() {
        let src = tempfile::tempdir().unwrap();
        let src_p = src.path().to_str().unwrap();
        let log_dir = tempfile::tempdir().unwrap();
        let log_path = log_dir.path().join("exp.log");
        let log_p_str = log_path.to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&[
            "-s", src_p, "-d", "/tmp", "--debug", log_p_str,
        ]))
        .unwrap();
        assert!(cfg.debug_log().is_some());
        assert!(log_path.exists(), "arquivo de log deveria ter sido criado");
    }

    #[test]
    fn format_summary_padrao_imprime_apenas_source_destination_e_exporting() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp"])).unwrap();
        let out = cfg.format_summary();
        assert!(out.contains(&format!("source       = {p}")));
        assert!(out.contains("destination  = /tmp"));
        assert!(out.ends_with("Exporting...\n"));
        assert!(!out.contains("filters"));
        assert!(!out.contains("only-copy"));
        assert!(!out.contains("flags"));
    }

    #[test]
    fn format_summary_show_omite_chaves_iguais_aos_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&["-s", p, "-d", "/tmp", "--show"])).unwrap();
        let out = cfg.format_summary();
        assert!(!out.contains("filters"));
        assert!(!out.contains("only-copy"));
        assert!(!out.contains("flags"));
    }

    #[test]
    fn format_summary_show_inclui_filters_quando_customizados() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&[
            "-s",
            p,
            "-d",
            "/tmp",
            "--show",
            "-f",
            "*.txt;*.md",
        ]))
        .unwrap();
        let out = cfg.format_summary();
        assert!(out.contains("filters      = *.txt;*.md"));
    }

    #[test]
    fn format_summary_show_inclui_flags_quando_alguma_ativa() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap();
        let cfg = Configuration::build(&mut args_iter(&[
            "-s", p, "-d", "/tmp", "--show", "--reload",
        ]))
        .unwrap();
        let out = cfg.format_summary();
        assert!(out.contains("flags        = exists:false reload:true lower:false"));
    }

    #[test]
    fn build_debug_flag_com_path_invalido_retorna_erro_io() {
        let src = tempfile::tempdir().unwrap();
        let src_p = src.path().to_str().unwrap();
        let result = Configuration::build(&mut args_iter(&[
            "-s",
            src_p,
            "-d",
            "/tmp",
            "--debug",
            "/definitely/not/a/path/xyz/log.log",
        ]));
        assert!(matches!(result, Err(ExportError::Io { .. })));
    }
}
