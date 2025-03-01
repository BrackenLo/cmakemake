use std::{
    hash::{Hash, Hasher},
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use colored::Colorize;
use config::ConfigFile;
use error::{DisplayError, ProjectError};
use util::*;

mod config;
mod dependencies;
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
        "ignore" => add_ignore().display_error(),
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
    print_command("ignore", "Create a .ignore file for external/ and res/");
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

    init_file(&path.join(Path::new(".gitignore")), b"build")?;

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
        vec![
            "Pre-Cached",    // 0
            "Git Submodule", // 1
            "Find",          // 2
            "Local",         // 3
        ],
    )
    .raw_prompt()
    .unwrap();

    match dep_type.index {
        0 => dependencies::add_cached_dependency(&mut config)?,
        1 => dependencies::add_git_submodule(&mut config)?,
        2 => dependencies::add_find_dependency(&mut config)?,
        3 => dependencies::add_local_dependency(&mut config)?,
        _ => return Err(ProjectError::UnknownArgument(dep_type.value.into())),
    }

    write_config(config)?;

    println!("{} {}", "Successfully".green(), "added dependency");

    Ok(())
}

fn write_include_flags(
    file: &mut std::fs::File,
    source_name: &str,
    path: &str,
    files: &config::IncludeFiles,
) -> Result<(), std::io::Error> {
    let glob_sources = |glob: &str| {
        format!(
            r#"file({glob} {source_name} "{path}/*.cpp" "{path}/*.c" "{path}/*.hpp" "{path}/*.h")"#
        )
    };

    match files {
        config::IncludeFiles::All => writeln!(file, "{}", glob_sources("GLOB_RECURSE"),),
        config::IncludeFiles::Root => writeln!(file, "{}", glob_sources("GLOB")),
        config::IncludeFiles::Exclude(items) => write!(
            file,
            "{}\nlist(REMOVE_ITEM SOURCES {})",
            glob_sources("GLOB_RECURSE"),
            items
                .iter()
                .fold(String::new(), |a, b| format!(r#"{} "{}/{}""#, a, path, b))
        ),
        config::IncludeFiles::Header => Ok(()),
    }
}

fn generate_cmake() -> Result<(), ProjectError> {
    println!("Generating CMakeLists.txt from config");

    let instant = std::time::Instant::now();

    let config = get_config()?;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Path::new("CMakeLists.txt"))
        .unwrap();

    // Config Hash
    let mut hasher = std::hash::DefaultHasher::new();
    config.hash(&mut hasher);
    let config_hash = hasher.finish();

    writeln!(file, "# {}\n", config_hash).unwrap();

    // Project Setup
    writeln!(
        file,
        "cmake_minimum_required(VERSION {})",
        config.cmake.minimum_required
    )
    .unwrap();

    writeln!(file, r#"project("{}")"#, config.project.name).unwrap();

    // Project top config
    writeln!(file, "\n#Project Config Flags:").unwrap();

    writeln!(file, "set(CMAKE_BUILD_TYPE Debug)").unwrap();
    writeln!(file, "set(CMAKE_EXPORT_COMPILE_COMMANDS ON)").unwrap();

    // Project Dependencies
    writeln!(file, "\n#Project Dependencies: ").unwrap();

    config.dependencies.find.iter().for_each(|find| {
        let required = match find.required {
            true => " REQUIRED",
            false => "",
        };

        writeln!(file, "find_package({}{})", find.name, required).unwrap();
    });

    if config.dependencies.find.is_empty() == false {
        writeln!(file, "").unwrap();
    }

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

                match files {
                    config::IncludeFiles::Header => {
                        writeln!(file, "add_library({name} INTERFACE)").unwrap();
                        writeln!(file, "target_include_directories({name} INTERFACE {path})")
                            .unwrap();
                    }

                    _ => {
                        write_include_flags(&mut file, &src_name, &local.path, files).unwrap();
                        writeln!(file, "add_library({name} ${{{src_name}}})").unwrap();
                        writeln!(file, "target_include_directories({name} PUBLIC {path})").unwrap();
                    }
                }

                if dependencies.is_empty() == false {
                    writeln!(
                        file,
                        "target_link_libraries({name} PUBLIC {})",
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

    writeln!(file, "#Project Files:").unwrap();

    // Project files
    write_include_flags(&mut file, "SOURCES", "src", &config.cmake.files).unwrap();

    // Link files
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

    let config = get_config()?;

    if Path::new("CMakeLists.txt").exists() == false {
        println!("{} {}", "warning:".yellow(), "CMakeLists.txt doesn't exist");
        generate_cmake()?;
        println!("");
    } else {
        let mut hasher = std::hash::DefaultHasher::new();
        config.hash(&mut hasher);
        let config_hash = hasher.finish();

        let cmake_file = open_file(&Path::new("CMakeLists.txt"))?;
        let mut buffer = std::io::BufReader::new(cmake_file);
        let mut first_line = String::new();
        buffer.read_line(&mut first_line).unwrap();

        if first_line != format!("# {}\n", config_hash) {
            println!(
                "{} {}",
                "warning:".yellow(),
                "CMakeLists.txt out of date. Regenerating."
            );
            generate_cmake()?;
            println!("");
        }
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

fn add_ignore() -> Result<(), ProjectError> {
    println!("Adding .ignore");

    if Path::new(CONFIG_NAME).exists() == false {
        return Err(ProjectError::InvalidProjectDirectory);
    }

    let ignore_path = Path::new(".ignore");

    match ignore_path.exists() {
        true => {
            println!(".ignore file already exists");
        }
        false => {
            println!("Creating .ignore file");
            init_file(&Path::new(".ignore"), b"external/\nres/")?;
        }
    }

    Ok(())
}
