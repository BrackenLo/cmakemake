use std::{error::Error, path::PathBuf};

use colored::Colorize;

use crate::CONFIG_NAME;

#[derive(Debug)]
pub enum ProjectError {
    MissingName,
    UnknownArgument(String),
    InvalidProjectDirectory,

    FailedToCreateFolder(PathBuf, String),
    FailedToInitGit(String),
    FailedToCreateFile(PathBuf, String),
    CannotOpenFile(PathBuf, String),

    FailedToRunProcess(String, Option<i32>),
}

impl Error for ProjectError {}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::MissingName => write!(
                f,
                "{} {}",
                "error:".red(),
                "please provide a suitable project name",
            ),

            ProjectError::UnknownArgument(argument) => write!(
                f,
                "{} {} '{}'",
                "error:".red(),
                "unknown argument",
                argument.bold(),
            ),

            ProjectError::InvalidProjectDirectory => write!(
                f,
                "{} {} {}",
                "error:".red(),
                "current directory doesn't contain a",
                CONFIG_NAME,
            ),

            ProjectError::FailedToCreateFolder(name, error) => write!(
                f,
                "{} {} '{}' {} {}",
                "error:".red(),
                "failed to create folder",
                name.display(),
                "with error:",
                error.red(),
            ),

            ProjectError::FailedToInitGit(error) => write!(
                f,
                "{} {} {}",
                "error:".red(),
                "failed to init git repo with error:",
                error.red(),
            ),

            ProjectError::FailedToCreateFile(file, error) => write!(
                f,
                "{} {} '{}' {} {}",
                "error:".red(),
                "failed to create file",
                file.display(),
                "with error:",
                error.red(),
            ),

            ProjectError::CannotOpenFile(file, error) => write!(
                f,
                "{} {} '{}' {} {}",
                "error:".red(),
                "failed to open file",
                file.display(),
                "with error:",
                error.red(),
            ),

            ProjectError::FailedToRunProcess(process, code) => {
                let error_code = match code {
                    Some(code) => format!("exit code {}", code),
                    None => String::from("an unknown error code"),
                };

                write!(
                    f,
                    "{} {} '{}' {} {}",
                    "error:".red(),
                    "run process",
                    process,
                    "exited with",
                    error_code,
                )
            }
        }
    }
}

pub trait DisplayError {
    fn display_error(self);
}

impl<T> DisplayError for Result<T, ProjectError> {
    fn display_error(self) {
        if let Err(e) = self {
            eprintln!("{}", e)
        }
    }
}
