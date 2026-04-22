#![allow(missing_docs)]

use crate::configuration::Configuration;
use crate::error::{Result, WithPath};
use crate::export::{export, WalkContext};
use crate::export_branch_files::check_configuration_file;
use crate::file_checker::FileChecker;
use std::path::PathBuf;

pub struct ExportBranch<'a> {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub configuration: &'a Configuration,
    pub file_checker: &'a mut FileChecker,
}

impl<'a> ExportBranch<'a> {
    pub fn build(
        source: PathBuf,
        destination: PathBuf,
        configuration: &'a Configuration,
        file_checker: &'a mut FileChecker,
    ) -> ExportBranch<'a> {
        ExportBranch {
            source,
            destination,
            configuration,
            file_checker,
        }
    }

    pub fn perform_exporting(&mut self) -> Result<()> {
        let (file_filters, only_copy_files) = check_configuration_file(
            &self.source,
            self.configuration.file_filters(),
            self.configuration.only_copy_files(),
        )?;
        let destination = self.destination.clone();

        let updates = {
            let ctx = WalkContext {
                destination_root: &self.destination,
                configuration: self.configuration,
                file_checker: self.file_checker,
                file_filters: &file_filters,
                only_copy_files: &only_copy_files,
            };
            export(&ctx, &self.source, destination)?
        };

        for update in &updates {
            self.file_checker.apply(update);
        }

        let checker_dir = self.file_checker.directory().to_path_buf();
        self.file_checker.save().with_path(checker_dir)?;
        Ok(())
    }
}
