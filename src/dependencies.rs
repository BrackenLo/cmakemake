use std::path::PathBuf;

use crate::{
    config::{self, ConfigFile, FetchDependency, LocalDependency},
    error::{DisplayError, ProjectError},
    util::{
        dep_flag_validation, folder_validator, not_own_folder_validator, path_formater,
        FolderAutocomplete,
    },
};

fn get_dependency_variables() -> Vec<(String, String)> {
    let mut flags = Vec::new();

    println!("Any variables/flags");
    loop {
        match inquire::Text::new(" > ")
            .with_validator(dep_flag_validation)
            .with_placeholder("[NAME] [VALUES]...")
            .with_help_message("Any Variables/Flags")
            .prompt_skippable()
            .unwrap()
        {
            Some(val) => {
                if val.is_empty() {
                    break;
                }
                flags.push(val)
            }
            None => break,
        }
    }

    let variables = flags
        .into_iter()
        .map(|var| {
            let name = var.split_whitespace().nth(0).unwrap().to_owned();
            let value = var[name.len()..].to_owned();

            (name, value)
        })
        .collect();

    variables
}

pub fn add_local_dependency_path(
    config: &mut ConfigFile,
    path: String,
) -> Result<(), ProjectError> {
    let path_buf = PathBuf::from(&path);
    if path_buf.exists() {
        ProjectError::CannotOpenFile(path_buf.clone(), "Path doesn't exist".to_owned());
    }

    let default_name = path_buf.file_name().unwrap_or_default().to_str().unwrap();
    let name = inquire::Text::new("Dependency Name:")
        .with_default(default_name)
        .with_validator(inquire::validator::ValueRequiredValidator::default())
        .prompt()
        .unwrap();

    let variables = get_dependency_variables();

    let local_type = match inquire::Confirm::new("Dependency uses CMake?")
        .with_placeholder("y/n")
        .prompt()
        .unwrap()
    {
        true => config::LocalType::CMake,

        false => {
            let files = inquire::Select::new(
                "Included files",
                vec![
                    "All (recursive)", // 0
                    "All",             // 1
                    "All (Exclude)",   // 2
                ],
            )
            .raw_prompt()
            .unwrap();

            let files = match files.index {
                0 => config::IncludeFiles::AllRecurse,
                1 => config::IncludeFiles::All,
                2 => todo!(),

                _ => return Err(ProjectError::UnknownArgument(files.value.into())),
            };

            let mut dependencies = Vec::new();
            println!("Library dependencies");
            loop {
                match inquire::Text::new(" > ")
                    .with_help_message("Press enter or esc to proceed")
                    .prompt_skippable()
                    .unwrap()
                {
                    Some(val) => {
                        if val.is_empty() {
                            break;
                        }
                        dependencies.push(val);
                    }
                    None => break,
                }
            }

            config::LocalType::Source {
                files,
                dependencies,
            }
        }
    };

    config.dependencies.local.push(LocalDependency {
        path,
        name: name.clone(),
        local_type,
        variables,
    });

    if inquire::Confirm::new("Add as project dependency?")
        .with_placeholder("y/n")
        .with_default(true)
        .prompt()
        .unwrap()
    {
        config.dependencies.project_dependencies.push(name);
    }

    Ok(())
}

pub fn add_local_dependency(config: &mut ConfigFile) -> Result<(), ProjectError> {
    let path = inquire::Text::new("Path:")
        .with_validator(inquire::validator::ValueRequiredValidator::default())
        .with_help_message("Choose a path relative to the project folder")
        .with_autocomplete(FolderAutocomplete(std::env::current_dir().unwrap()))
        .with_validator(folder_validator)
        .with_validator(not_own_folder_validator)
        .with_formatter(&path_formater)
        .prompt()
        .unwrap();

    add_local_dependency_path(config, path)
}

pub fn add_fetch_dependency(config: &mut ConfigFile) -> Result<(), ProjectError> {
    let dep_name = inquire::Text::new("Dependency Name:")
        .with_validator(inquire::validator::ValueRequiredValidator::default())
        .prompt()
        .unwrap();

    let repo = inquire::Text::new("Fetch Git Repo:")
        .with_validator(inquire::validator::ValueRequiredValidator::default())
        .prompt()
        .unwrap();

    let tag = match inquire::Text::new("Git Tag (optional):")
        .prompt_skippable()
        .unwrap()
    {
        Some(val) => match val.is_empty() {
            true => None,
            false => Some(val),
        },
        None => None,
    };

    let branch = match inquire::Text::new("Git Branch (optional):")
        .prompt_skippable()
        .unwrap()
    {
        Some(val) => match val.is_empty() {
            true => None,
            false => Some(val),
        },
        None => None,
    };

    let variables = get_dependency_variables();

    config.dependencies.fetch_content.push(FetchDependency {
        name: dep_name,
        variables,
        repo,
        tag,
        branch,
    });

    Ok(())
}

pub fn add_git_submodule(config: &mut ConfigFile) -> Result<(), ProjectError> {
    let repo = inquire::Text::new("Fetch Git Repo:")
        .with_validator(inquire::validator::ValueRequiredValidator::default())
        .prompt()
        .unwrap();

    let tag = match inquire::Text::new("Git Tag (optional):")
        .prompt_skippable()
        .unwrap()
    {
        Some(val) => match val.is_empty() {
            true => None,
            false => Some(val),
        },
        None => None,
    };

    let branch = match inquire::Text::new("Git Branch (optional):")
        .prompt_skippable()
        .unwrap()
    {
        Some(val) => match val.is_empty() {
            true => None,
            false => Some(val),
        },
        None => None,
    };

    // let fetch_shallow = inquire::Confirm::new("Fetch Git Shallow?")
    //     .with_default(true)
    //     .with_placeholder("Y/n")
    //     .prompt()
    //     .unwrap();

    std::fs::create_dir(std::path::Path::new("external")).ok();

    let lib_name = repo.split(|val| val == '/').last().unwrap();
    let lib_name = lib_name
        .split(|char| char == '.')
        .next()
        .unwrap_or(lib_name);
    let folder_path = format!("external/{}", lib_name);

    let cmd_output = duct::cmd!("git", "submodule", "add", &repo, &folder_path)
        .stderr_to_stdout()
        .unchecked()
        .run()
        .unwrap();

    if !cmd_output.status.success() {
        Err(ProjectError::FailedToRunProcess(
            format!("git submodule add {} {}", &repo, &folder_path),
            cmd_output.status.code(),
        ))?;
    }

    let cmd_output = duct::cmd!("git", "submodule", "update", "--init", "--recursive")
        .stderr_to_stdout()
        .unchecked()
        .run()
        .unwrap();

    if !cmd_output.status.success() {
        // Don't return from function with error's at this point
        Err(ProjectError::FailedToRunProcess(
            format!("git submodule update --init --recursive"),
            cmd_output.status.code(),
        ))
        .display_error();
    }

    let cmd = match (tag, branch) {
        (Some(tag), Some(branch)) => {
            let tags = format!("tags/{}", tag);
            println!("Switching to '{}' on branch '{}'", tags, branch);
            Some(duct::cmd!("git", "checkout", tags, "-b", branch))
        }

        (Some(tag), None) => {
            let tags = format!("tags/{}", tag);
            println!("Switching to '{}'", tags);
            Some(duct::cmd!("git", "checkout", tags))
        }

        (None, Some(branch)) => {
            println!("Switching to branch '{}'", branch);
            Some(duct::cmd!("git", "checkout", "-b", branch))
        }

        (None, None) => None,
    };

    if let Some(cmd) = cmd {
        // git checkout with tags gives quite verbose info so don't output to std
        let cmd_output = cmd
            .dir(std::path::Path::new(&folder_path))
            .stderr_capture()
            .unchecked()
            .run()
            .unwrap();

        if !cmd_output.status.success() {
            // Don't return from function with error's at this point
            Err(ProjectError::FailedToRunProcess(
                format!("git checkout..."),
                cmd_output.status.code(),
            ))
            .display_error();
        }
    }

    add_local_dependency_path(config, folder_path)
}
