use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use inquire::validator::{ErrorMessage, Validation};

use crate::{error::ProjectError, ConfigFile, CONFIG_NAME};

pub fn create_dir(path: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir(path)
        .map_err(|err| ProjectError::FailedToCreateFolder(path.to_owned(), err.to_string()))
}

pub fn init_file(path: &Path, data: &[u8]) -> Result<File, ProjectError> {
    let mut file = create_file(path)?;
    write_file(path, &mut file, data)?;
    Ok(file)
}

pub fn create_file(path: &Path) -> Result<File, ProjectError> {
    std::fs::File::create(path)
        .map_err(|err| ProjectError::FailedToCreateFile(path.to_owned(), err.to_string()))
}

pub fn write_file(path: &Path, file: &mut File, data: &[u8]) -> Result<usize, ProjectError> {
    file.write(data)
        .map_err(|err| ProjectError::FailedToCreateFile(path.to_owned(), err.to_string()))
}

pub fn open_file(path: &Path) -> Result<File, ProjectError> {
    File::open(path).map_err(|err| ProjectError::CannotOpenFile(path.to_owned(), err.to_string()))
}

pub fn get_config() -> Result<ConfigFile, ProjectError> {
    if !Path::new(CONFIG_NAME).exists() {
        return Err(ProjectError::InvalidProjectDirectory);
    }

    let mut config_file = open_file(&Path::new(CONFIG_NAME))?;

    let mut buffer = String::new();
    config_file
        .read_to_string(&mut buffer)
        .map_err(|err| ProjectError::CannotOpenFile(PathBuf::from(CONFIG_NAME), err.to_string()))?;

    let config: ConfigFile = toml::from_str(&buffer)
        .map_err(|err| ProjectError::CannotOpenFile(PathBuf::from(CONFIG_NAME), err.to_string()))?;

    Ok(config)
}

pub fn write_config(config: ConfigFile) -> Result<(), ProjectError> {
    let path = PathBuf::from(CONFIG_NAME);

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&path)
        .map_err(|err| ProjectError::CannotOpenFile(path.clone(), err.to_string()))?;

    file.write(toml::to_string(&config).unwrap().as_bytes())
        .map_err(|err| ProjectError::FailedToCreateFile(path.to_owned(), err.to_string()))?;

    Ok(())
}

#[derive(Clone)]
pub struct FolderAutocomplete(pub PathBuf);

impl inquire::Autocomplete for FolderAutocomplete {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        let mut input_splits = input.split(|char| char == '/');
        let last_input = input_splits.next_back().unwrap_or_default();

        let path_str = input_splits
            .map(|val| format!("{}/", val))
            .collect::<String>();

        let path = self.0.join(&path_str);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let dir = std::fs::read_dir(&path).unwrap();

        let input_len = last_input.len();

        let folders = dir
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                if entry.path().is_dir() {
                    let path = entry.path();

                    let folder_name = path.file_name()?.to_str().unwrap();

                    if folder_name.split_at_checked(input_len)?.0 == last_input {
                        return Some(format!("{}{}", path_str, folder_name));
                    }
                    return None;
                }

                None
            })
            .collect();

        Ok(folders)
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        let val = match highlighted_suggestion {
            Some(val) => Some(format!("{}/", val)),
            None => {
                let val = self.get_suggestions(input)?;

                match val.into_iter().next() {
                    Some(val) => Some(format!("{}/", val)),
                    None => None,
                }
            }
        };

        Ok(val)
    }
}

pub fn dep_flag_validation(input: &str) -> Result<Validation, inquire::CustomUserError> {
    if input.is_empty() {
        return Ok(Validation::Valid);
    }

    let valid = match input.split_whitespace().count() > 1 {
        true => Validation::Valid,
        false => Validation::Invalid(ErrorMessage::Default),
    };

    Ok(valid)
}

pub fn folder_validator(input: &str) -> Result<Validation, inquire::CustomUserError> {
    let path = PathBuf::from(input);

    if !path.exists() {
        return Ok(Validation::Invalid(ErrorMessage::Custom(
            "Folder doesn't exist".to_owned(),
        )));
    }

    if !path.is_dir() {
        return Ok(Validation::Invalid(ErrorMessage::Custom(
            "Path is not a directory".to_owned(),
        )));
    }

    Ok(Validation::Valid)
}

pub fn not_own_folder_validator(input: &str) -> Result<Validation, inquire::CustomUserError> {
    match input {
        "./" => Ok(Validation::Invalid(ErrorMessage::Custom(
            "Cannot be project director".into(),
        ))),
        _ => Ok(Validation::Valid),
    }
}

pub fn path_formater(path: &str) -> String {
    let mut trimmed = path.trim();

    if let Some(val) = path.split_at_checked(2) {
        if val.0 == "./" {
            let mut chars = trimmed.chars();
            chars.next();
            chars.next();
            trimmed = chars.as_str()
        }
    }

    let last_char = trimmed.chars().last().unwrap();

    if last_char == '/' {
        let mut chars = trimmed.chars();
        chars.next_back();
        return chars.as_str().to_owned();
    }

    trimmed.to_owned()
}
