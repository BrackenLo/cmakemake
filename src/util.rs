use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
