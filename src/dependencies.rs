use std::path::PathBuf;

use crate::{
    config::{self, CacheSubmodule, ConfigFile, FindDependency, LocalDependency},
    error::{DisplayError, ProjectError},
    util::{
        dep_flag_validation, folder_validator, get_cache, not_own_folder_validator, path_formater,
        write_cache, FolderAutocomplete,
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

fn get_is_project_dependency(config: &mut ConfigFile, name: String) {
    if inquire::Confirm::new("Add as project dependency?")
        .with_placeholder("y/n")
        .with_default(true)
        .prompt()
        .unwrap()
    {
        config.dependencies.project_dependencies.push(name);
    }
}

pub fn add_local_dependency_path(
    config: &mut ConfigFile,
    path: String,
) -> Result<LocalDependency, ProjectError> {
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
                0 => config::IncludeFiles::All,
                1 => config::IncludeFiles::Root,
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

    let local_dependency = LocalDependency {
        path,
        name: name.clone(),
        local_type,
        variables,
    };

    config.dependencies.local.push(local_dependency.clone());

    get_is_project_dependency(config, name);

    Ok(local_dependency)
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

    add_local_dependency_path(config, path)?;

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

    let folder_path = add_submodule(&repo, tag.as_ref(), branch.as_ref())?;
    let local_setup = add_local_dependency_path(config, folder_path)?;

    if inquire::Confirm::new("Save dependency to cache?")
        .with_default(true)
        .with_placeholder("Y/n")
        .prompt()
        .unwrap()
    {
        cache_git_submodule(config::GitSubmodule {
            repo,
            tag,
            branch,
            local_setup,
        })?;
    }

    Ok(())
}

fn submodule_name(repo: &str) -> &str {
    let lib_name = repo.split(|val| val == '/').last().unwrap();
    let lib_name = lib_name
        .split(|char| char == '.')
        .next()
        .unwrap_or(lib_name);

    lib_name
}

fn add_submodule(
    repo: &str,
    tag: Option<&String>,
    branch: Option<&String>,
) -> Result<String, ProjectError> {
    std::fs::create_dir(std::path::Path::new("external")).ok();
    let lib_name = submodule_name(repo);

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
        Err::<(), _>(ProjectError::FailedToRunProcess(
            format!("git submodule update --init --recursive"),
            cmd_output.status.code(),
        ))
        .display_error();
    }

    let cmd = match (&tag, &branch) {
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
            Err::<(), _>(ProjectError::FailedToRunProcess(
                format!("git checkout..."),
                cmd_output.status.code(),
            ))
            .display_error();
        }
    }

    Ok(folder_path)
}

fn cache_git_submodule(submodule: config::GitSubmodule) -> Result<(), ProjectError> {
    let mut cache = get_cache()?;

    let mut name = submodule_name(&submodule.repo).to_owned();

    if let Some(tag) = &submodule.tag {
        name = format!("{} - tags/{}", name, tag);
    }

    if let Some(branch) = &submodule.branch {
        name = format!("{} - branch/{}", name, branch);
    }

    cache
        .git_submodules
        .push(CacheSubmodule { name, submodule });

    write_cache(cache)?;
    Ok(())
}

pub fn add_cached_dependency(config: &mut ConfigFile) -> Result<(), ProjectError> {
    let cache = get_cache()?;

    if cache.git_submodules.is_empty() {
        println!("No cached dependencies available.");
        return Ok(());
    }

    let selection = inquire::MultiSelect::new(
        "Choose a dependency:",
        cache.git_submodules.iter().map(|val| &val.name).collect(),
    )
    .raw_prompt()
    .unwrap();

    let val = selection
        .into_iter()
        .map(|entry| {
            let CacheSubmodule { submodule, .. } = cache.git_submodules.get(entry.index).unwrap();

            add_submodule(
                &submodule.repo,
                submodule.tag.as_ref(),
                submodule.branch.as_ref(),
            )?;

            config
                .dependencies
                .local
                .push(submodule.local_setup.clone());

            get_is_project_dependency(config, submodule.local_setup.name.clone());

            Ok(())
        })
        .collect::<Vec<Result<(), ProjectError>>>();

    val.into_iter()
        .filter_map(|val| val.err())
        .for_each(|err| Err::<(), _>(err).display_error());

    Ok(())
}

pub fn add_find_dependency(config: &mut ConfigFile) -> Result<(), ProjectError> {
    let name = inquire::Text::new("Dependency Name:")
        .with_validator(inquire::validator::ValueRequiredValidator::default())
        .prompt()
        .unwrap();

    let required = inquire::Confirm::new("Dependency required?")
        .with_default(true)
        .with_placeholder("Y/n")
        .prompt()
        .unwrap();

    config.dependencies.find.push(FindDependency {
        name: name.clone(),
        required,
    });

    get_is_project_dependency(config, name);

    Ok(())
}
