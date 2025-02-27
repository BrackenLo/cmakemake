use std::{
    io::Write,
    path::{Path, PathBuf},
};

use colored::Colorize;
use config::{ConfigFile, LocalDependency};
use error::{DisplayError, ProjectError};
use util::*;

mod config;
mod error;
mod util;

fn main() -> Result<(), ProjectError> {
    let command = match std::env::args().nth(1) {
        Some(cmd) => cmd,
        None => {
            print_help();
            return Ok(());
        }
    };

    match command.to_lowercase().as_str() {
        "new" => new_project().display_error(),
        "add" => add_dependency().display_error(),
        "cmake" => generate_cmake().display_error(),
        "build" => build_project().display_error(),
        "run" => run_project().display_error(),
        "clean" => clean_project().display_error(),

        "help" => print_help(),

        other => {
            println!("{}: {}", "Unknown command".red(), other);
            print_help();
        }
    }

    Ok(())
}

fn print_help() {
    println!("A c++ project setup tool\n");

    println!(
        "{}\t{}\t{}",
        "Usage:".green().bold(),
        "cmakemake".cyan(),
        "[COMMAND]".cyan()
    );
    println!("\t{}\t\t{}", "cmm".cyan(), "[COMMAND]".cyan());

    let print_command = |cmd: &str, desc: &str| {
        println!("\t{}\t\t{}", cmd.cyan().bold(), desc);
    };

    println!("");
    println!("{}", "Commands:".green().bold());
    print_command("new", "Create a new project");
    print_command("add", "Add a dependency");
    print_command("cmake", "Generate cmake build script");
    print_command("build", "Build project code");
    print_command("run", "Build and run project code");
    print_command(
        "clean",
        "remove c++ build files (and optionally cmake files)",
    );
    print_command("help", "Output this help message");
}

const DEFAULT_MAIN_FILE: &str = r#"#include <iostream>

int main(void)
{
    std::cout << "Hello World!";
    return 0;
}
"#;

const CONFIG_NAME: &str = "CMakeMake.toml";

fn new_project() -> Result<(), ProjectError> {
    // Get Project Name
    let name = std::env::args()
        .nth(2)
        .ok_or_else(|| ProjectError::MissingName)?;

    let path = PathBuf::from(&name);

    // Init Project Folder
    create_dir(&path)?;

    // Init Git Repo
    git2::Repository::init(&path).map_err(|err| ProjectError::FailedToInitGit(err.to_string()))?;

    init_file(&path.join(Path::new(".gitignore")), "build".as_bytes())?;

    // Init Config File
    let config = ConfigFile::new(name);

    init_file(
        &path.join(Path::new(CONFIG_NAME)),
        toml::to_string(&config).unwrap().as_bytes(),
    )?;

    // Init other project folders
    create_dir(&path.join("src"))?;

    // Init main.cpp
    init_file(&path.join("src/main.cpp"), DEFAULT_MAIN_FILE.as_bytes())?;

    // Finished Successfully
    println!(
        "{} {} {}",
        "Finished".green().bold(),
        "creating project at",
        path.canonicalize().unwrap_or(path).display()
    );

    Ok(())
}

fn add_dependency() -> Result<(), ProjectError> {
    let mut config = get_config()?;

    let dep_type = inquire::Select::new(
        "Choose the Dependency Type:",
        vec!["Local", "Fetch", "Conan"],
    )
    .prompt()
    .unwrap();

    match dep_type {
        "Local" => {
            let path = inquire::Text::new("Path:")
                .with_validator(inquire::validator::ValueRequiredValidator::default())
                .with_help_message("Choose a path relative to the project folder")
                .with_autocomplete(FolderAutocomplete(std::env::current_dir().unwrap()))
                .with_validator(folder_validation)
                .prompt()
                .unwrap();

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

            println!("Any variables/flags");

            let mut flags = Vec::new();

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

            let local_type = match inquire::Confirm::new("Dependency uses CMake?")
                .with_placeholder("y/n")
                .prompt()
                .unwrap()
            {
                true => config::LocalType::CMake,

                false => {
                    let files =
                        inquire::Select::new("Included files", vec!["All", "All (Exclude)"])
                            .raw_prompt()
                            .unwrap();

                    let files = match files.index {
                        0 => config::IncludeFiles::All,
                        1 => todo!(),

                        _ => return Err(ProjectError::UnknownArgument(files.value.into())),
                    };

                    let mut dependencies = Vec::new();
                    loop {
                        match inquire::Text::new(" > ")
                            .with_validator(dep_flag_validation)
                            .with_placeholder("[NAME] [VALUES]...")
                            .prompt_skippable()
                            .unwrap()
                        {
                            Some(val) => dependencies.push(val),
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
                .prompt()
                .unwrap()
            {
                config.dependencies.project_dependencies.push(name);
            }
        }

        "Fetch" => {}
        "Conan" => {}
        e => return Err(ProjectError::UnknownArgument(e.into())),
    }

    write_config(config)?;

    println!("{} {}", "Successfully".green(), "added dependency");

    inquire::Confirm::new("Save dependency to cache? (TODO)")
        .prompt()
        .ok();

    Ok(())
}

fn generate_cmake() -> Result<(), ProjectError> {
    println!("Generating CMakeLists.txt from config");

    let instant = std::time::Instant::now();

    let config = get_config()?;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(Path::new("CMakeLists.txt"))
        .unwrap();

    // Project Setup
    writeln!(
        file,
        "cmake_minimum_required(VERSION {})",
        config.cmake.minimum_required
    )
    .unwrap();

    writeln!(file, r#"project("{}")"#, config.project.name).unwrap();

    // Project top config
    writeln!(file, "\n").unwrap();

    if config.dependencies.fetch_content.is_empty() == false {
        writeln!(file, "include(FetchContent)").unwrap();
    }

    writeln!(file, "set(CMAKE_BUILD_TYPE Debug)").unwrap();
    writeln!(file, "set(CMAKE_EXPORT_COMPILE_COMMANDS ON)").unwrap();

    // Project Dependencies
    writeln!(file, "\n").unwrap();

    config.dependencies.local.iter().for_each(|local| {
        local
            .variables
            .iter()
            .for_each(|var| writeln!(file, "set({: <20} {: <20})", var.0, var.1).unwrap());

        match &local.local_type {
            config::LocalType::CMake => writeln!(file, "add_subdirectory({})", local.path).unwrap(),

            config::LocalType::Source {
                files,
                dependencies,
            } => {
                let path = &local.path;
                let name = &local.name;

                let src_name = format!("{}_SOURCES", name.to_uppercase());

                let files = match files {
                    config::IncludeFiles::All => format!(
                        r#"file(GLOB_RECURSE {} "{path}/*.cpp" "{path}/*.hpp" "{path}/.h")"#,
                        src_name,
                    ),

                    config::IncludeFiles::Exclude(items) => format!(
                        r#"file(GLOB_RECURSE {} "{path}/*.cpp" "{path}/*.hpp" "{path}/.h")"
                        list(REMOVE_ITEM SOURCES {})"#,
                        src_name,
                        items.iter().fold(String::new(), |a, b| format!(
                            r#"{} "{}/{}""#,
                            a, local.path, b
                        ))
                    ),
                };

                writeln!(
                    file,
                    r#"{files}
                    add_library({name} ${{{src_name}}})
                    target_include_directories({name} PUBLIC {path})"#
                )
                .unwrap();

                if dependencies.is_empty() == false {
                    writeln!(
                        file,
                        "{name} PUBLIC {}",
                        dependencies
                            .iter()
                            .fold(String::new(), |a, b| format!("{} {}", a, b))
                    )
                    .unwrap();
                }
            }
        }

        writeln!(file, "").unwrap();
    });

    // Project files
    match config.cmake.files {
        config::IncludeFiles::All => {
            writeln!(
                file,
                r#"file(GLOB_RECURSE SOURCES "src/*.cpp" "src/*.hpp" "src/*.h")"#
            )
            .unwrap();
        }

        config::IncludeFiles::Exclude(items) => {
            writeln!(
                file,
                r#"file(GLOB_RECURSE SOURCES "src/*.cpp" "src/*.hpp" "src/*.h")
                list(REMOVE_ITEM SOURCES "{}")"#,
                items
                    .iter()
                    .fold(String::new(), |a, b| format!("{} {}", a, b))
            )
            .unwrap();
        }
    }

    // Link files
    writeln!(file, "\n").unwrap();
    writeln!(file, r#"add_executable("${{PROJECT_NAME}}" ${{SOURCES}})"#).unwrap();
    writeln!(
        file,
        r#"target_link_libraries("${{PROJECT_NAME}}" PRIVATE {})"#,
        config
            .dependencies
            .project_dependencies
            .iter()
            .fold(String::new(), |a, b| format!("{} {}", a, b))
    )
    .unwrap();

    file.flush().unwrap();

    println!(
        "{} {} {:.3}s",
        "Finished".green().bold(),
        "creating CMakeLists.txt in",
        instant.elapsed().as_secs_f32(),
    );

    Ok(())
}

fn build_project() -> Result<(), ProjectError> {
    println!("Building Project");

    if Path::new(CONFIG_NAME).exists() == false {
        return Err(ProjectError::InvalidProjectDirectory);
    }

    if Path::new("CMakeLists.txt").exists() == false {
        println!("{} {}", "warning:".yellow(), "CMakeLists.txt doesn't exist");
        generate_cmake()?;
        println!("");
    }

    println!("{}", "Generating CMake build system".green());

    let instant = std::time::Instant::now();

    let output = duct::cmd!("cmake", "-B", "build")
        .stderr_to_stdout()
        .unchecked()
        .run()
        .unwrap();

    if !output.status.success() {
        Err(ProjectError::FailedToRunProcess(
            String::from("cmake -B build"),
            output.status.code(),
        ))?;
    }

    println!("\n{}", "Compiling c++ project".green());

    let output = duct::cmd!("cmake", "--build", "build")
        .stderr_to_stdout()
        .unchecked()
        .run()
        .unwrap();

    if !output.status.success() {
        Err(ProjectError::FailedToRunProcess(
            "cmake".into(),
            output.status.code(),
        ))?;
    }

    println!(
        "{} {} {:.3}s",
        "Finished".green().bold(),
        "building c++ project in",
        instant.elapsed().as_secs_f32()
    );

    Ok(())
}

fn run_project() -> Result<(), ProjectError> {
    let config = get_config()?;

    let mut rebuild = true;

    if let Some(arg) = std::env::args().nth(2) {
        match arg.as_str() {
            "skip_build" => rebuild = false,
            _ => Err(ProjectError::UnknownArgument(arg.clone()))?,
        }
    }

    if rebuild {
        build_project()?;
        println!("");
    }

    duct::cmd!(format!("./build/{}", config.project.name))
        .stderr_to_stdout()
        .run()
        .unwrap();

    println!("\n\n{} {}", "Finished".green().bold(), "program execution");

    Ok(())
}

fn clean_project() -> Result<(), ProjectError> {
    println!("Cleaning build files");

    if Path::new(CONFIG_NAME).exists() == false {
        return Err(ProjectError::InvalidProjectDirectory);
    }

    let mut clean_all = false;

    if let Some(arg) = std::env::args().nth(2) {
        match arg.as_str() {
            "all" => clean_all = true,
            _ => Err(ProjectError::UnknownArgument(arg.clone()))?,
        }
    }

    if let Err(e) = std::fs::remove_dir_all(Path::new("build")) {
        println!(
            "{} {} {}",
            "warning:".yellow(),
            "failed to remove folder 'build' with error:",
            e.to_string().red()
        )
    }

    if clean_all {
        println!("Cleaning CMake Files");

        if let Err(e) = std::fs::remove_file(Path::new("CMakeLists.txt")) {
            println!(
                "{} {} {}",
                "warning:".yellow(),
                "failed to remove file 'CMakeLists.txt' with error:",
                e.to_string().red()
            )
        }
    }

    println!("{} {}", "Finished".green(), "removing build files");

    Ok(())
}
