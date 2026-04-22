#![allow(missing_docs)]

use crate::error::{ExportError, Result, WithPath};
use regex::RegexSet;
use std::fs;
use std::io::Read;
use std::path::Path;

pub fn check_configuration_file(
    directory: &Path,
    default_file_filters: &[String],
    default_only_copy: &[String],
) -> Result<(RegexSet, RegexSet)> {
    let config_file_filters = read_config_file(directory, "filtrosarquivos.exb")?;
    let config_only_copy = read_config_file(directory, "extecoesapenascopiar.exb")?;
    let config_do_not_convert = read_config_file(directory, "naoconverteacentos.exb")?;

    let file_filters = config_file_filters
        .as_deref()
        .unwrap_or(default_file_filters);
    let mut only_copy: Vec<String> = Vec::new();
    if let Some(files) = config_only_copy {
        only_copy.extend(files);
    }
    if let Some(files) = config_do_not_convert {
        only_copy.extend(files);
    }
    let only_copy_slice: &[String] = if only_copy.is_empty() {
        default_only_copy
    } else {
        &only_copy
    };

    Ok((
        checked_to_regex_set(file_filters)?,
        checked_to_regex_set(only_copy_slice)?,
    ))
}

pub fn checked_to_regex_set(checked: &[String]) -> Result<RegexSet> {
    let patterns: Vec<String> = checked
        .iter()
        .map(|file| {
            let body = file.replace('.', "\\.").replace('*', ".*");
            format!("^{body}$")
        })
        .collect();

    RegexSet::new(&patterns).map_err(|source| ExportError::InvalidGlob {
        pattern: checked.join(","),
        source,
    })
}

fn read_config_file(directory: &Path, config_file: &str) -> Result<Option<Vec<String>>> {
    let file_name = directory.join(config_file);

    if !file_name.exists() {
        return Ok(None);
    }

    let mut file = fs::File::open(&file_name).with_path(&file_name)?;
    let mut config_file_buffer = String::new();

    file.read_to_string(&mut config_file_buffer)
        .with_path(&file_name)?;

    let mut config: Vec<String> = vec![];

    for file in config_file_buffer.split(';') {
        let file_filter = file.replace(['\n', '\r'], "");

        if !file_filter.is_empty() {
            config.push(file_filter);
        }
    }

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_to_regex_set_aceita_glob_simples() {
        let set = checked_to_regex_set(&["*.prg".to_string()]).unwrap();
        assert_eq!(set.len(), 1);
        assert!(set.is_match("foo.prg"));
        assert!(set.is_match("bar.prg"));
    }

    #[test]
    fn checked_to_regex_set_escapa_ponto() {
        let set = checked_to_regex_set(&["*.h".to_string()]).unwrap();
        assert!(set.is_match("foo.h"));
    }

    #[test]
    fn checked_to_regex_set_h_nao_casa_html() {
        let set = checked_to_regex_set(&["*.h".to_string()]).unwrap();
        assert!(set.is_match("foo.h"));
        assert!(
            !set.is_match("foo.html"),
            "âncora `^...$` impede *.h de casar foo.html"
        );
    }

    #[test]
    fn checked_to_regex_set_prg_nao_casa_prefixo_programa() {
        let set = checked_to_regex_set(&["*.prg".to_string()]).unwrap();
        assert!(!set.is_match("foo.prg.bak"));
    }

    #[test]
    fn checked_to_regex_set_propaga_erro_em_glob_invalido() {
        let result = checked_to_regex_set(&["[".to_string()]);
        assert!(matches!(result, Err(ExportError::InvalidGlob { .. })));
    }

    #[test]
    fn check_configuration_file_sem_arquivos_retorna_filtros_originais() {
        let dir = tempfile::tempdir().unwrap();
        let defaults_filters = vec!["*.prg".to_string()];
        let defaults_copy = vec!["*.h".to_string()];

        let (filters, copy) =
            check_configuration_file(dir.path(), &defaults_filters, &defaults_copy).unwrap();

        assert!(filters.is_match("foo.prg"));
        assert!(copy.is_match("bar.h"));
    }

    #[test]
    fn check_configuration_file_com_filtrosarquivos_exb_substitui_filtros() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("filtrosarquivos.exb"), "*.txt;*.md").unwrap();

        let defaults_filters = vec!["*.prg".to_string()];
        let defaults_copy = vec!["*.h".to_string()];
        let (filters, _) =
            check_configuration_file(dir.path(), &defaults_filters, &defaults_copy).unwrap();

        assert!(filters.is_match("foo.txt"));
        assert!(filters.is_match("bar.md"));
        assert!(!filters.is_match("foo.prg"));
    }

    #[test]
    fn check_configuration_file_com_extecoesapenascopiar_nao_altera_file_filters() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("extecoesapenascopiar.exb"), "*.a;*.so").unwrap();

        let defaults_filters = vec!["*.prg".to_string()];
        let defaults_copy = vec!["*.h".to_string()];
        let (filters, copy) =
            check_configuration_file(dir.path(), &defaults_filters, &defaults_copy).unwrap();

        assert!(filters.is_match("foo.prg"));
        assert!(copy.is_match("libx.a"));
        assert!(copy.is_match("libx.so"));
    }
}
