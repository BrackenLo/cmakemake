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
    println!(
        "\t{}\t\t{}\t (recommended alias)",
        "cmm".cyan(),
        "[COMMAND]".cyan()
    );

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

    init_file(&path.join(Path::new(".gitignore")), b"build/\n.cache/")?;

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

fn write_source_files(
    file: &mut std::fs::File,
    source_name: &str,
    path: &str,
    files: &config::ProjectFiles,
) -> Result<(), std::io::Error> {
    if files.source_files.is_empty() {
        return Ok(());
    }

    let mut individual_files = Vec::new();
    let mut glob_dirs = Vec::new();
    let mut glob_recurse_dirs = Vec::new();

    files
        .source_files
        .iter()
        .for_each(|(source_type, files)| match source_type {
            config::SourceType::File => individual_files.extend(files),
            config::SourceType::Glob => glob_dirs.extend(files),
            config::SourceType::GlobRecurse => glob_recurse_dirs.extend(files),
        });

    let mut files_initialized = false;

    if individual_files.is_empty() == false {
        writeln!(
            file,
            "set({source_name} {})",
            individual_files
                .into_iter()
                .fold(String::new(), |a, b| format!(r#"{}"{}/{}" "#, a, path, b)) // TODO - Check trim
        )?;
        files_initialized = true;
    }

    let mut write_glob_type = |glob_type: &str, dirs: Vec<&String>| -> Result<(), std::io::Error> {
        if dirs.is_empty() {
            return Ok(());
        }

        let dirs_string = dirs.iter().fold(String::new(), |a, dir| {
            let path = match dir.as_str() == "." {
                true => format!("{path}"),
                false => format!("{path}/{dir}"),
            };

            format!(
                r#"{}"{path}/*.cpp" "{path}/*.c" "{path}/*.hpp" "{path}/*.h" "#,
                a
            )
        });

        let src_name = format!("{source_name}_{glob_type}");

        match files_initialized {
            true => {
                writeln!(file, r#"file({glob_type} {src_name} {})"#, dirs_string)?;
                writeln!(file, "list(APPEND {source_name} ${{{src_name}}})")?;
            }

            false => {
                writeln!(file, r#"file(GLOB {source_name} {})"#, dirs_string)?;
                files_initialized = true;
            }
        }

        Ok(())
    };

    write_glob_type("GLOB", glob_dirs)?;
    write_glob_type("GLOB_RECURSE", glob_recurse_dirs)?;

    if files.exclude_files.is_empty() == false {
        let to_remove = files
            .exclude_files
            .iter()
            .fold(String::new(), |a, b| format!(r#"{}"{}/{}" "#, a, path, b));

        writeln!(file, "list(REMOVE_ITEM {source_name} {to_remove})")?;
    }

    Ok(())
}

fn write_include_dirs(
    file: &mut std::fs::File,
    name: &str,
    path: &str,
    files: &config::ProjectFiles,
) -> Result<(), std::io::Error> {
    let mut other = Vec::new();
    let mut interfaces = Vec::new();

    files
        .include_dirs
        .iter()
        .for_each(|(include_type, dirs)| match include_type {
            config::IncludeType::Public => other.extend(dirs),
            config::IncludeType::Interface => interfaces.extend(dirs),
        });

    let mut write_include_type =
        |include_type: &str, dirs: Vec<&String>| -> Result<(), std::io::Error> {
            if dirs.is_empty() {
                return Ok(());
            }

            let dirs = dirs
                .into_iter()
                .fold(String::new(), |a, dir| match dir.as_str() == "." {
                    true => format!(r#"{}"{}" "#, a, path),
                    false => format!(r#"{}"{}/{}" "#, a, path, dir),
                });

            writeln!(
                file,
                "target_include_directories({name} {include_type} {dirs})"
            )?;

            Ok(())
        };

    write_include_type("PUBLIC", other)?;
    write_include_type("INTERFACE", interfaces)?;

    Ok(())
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
            .for_each(|var| writeln!(file, "set({: <20} {})", var.0, var.1).unwrap());

        match &local.local_type {
            config::LocalType::CMake => writeln!(file, "add_subdirectory({})", local.path).unwrap(),

            config::LocalType::Source {
                files,
                dependencies,
            } => {
                let name = &local.name;

                let src_name = format!("{}_SOURCES", name.to_uppercase());

                write_source_files(&mut file, &src_name, &local.path, files).unwrap();

                match files.source_files.is_empty() {
                    true => writeln!(file, "add_library({name} INTERFACE)").unwrap(),
                    false => writeln!(file, "add_library({name} ${{{src_name}}})").unwrap(),
                }

                write_include_dirs(&mut file, name, &local.path, files).unwrap();

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
    write_source_files(&mut file, "SOURCES", "src", &config.cmake.files).unwrap();

    // Link files
    writeln!(file, r#"add_executable("${{PROJECT_NAME}}" ${{SOURCES}})"#).unwrap();

    if config.dependencies.project_dependencies.is_empty() == false {
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
    }

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

    let cmd_output = duct::cmd!(format!("./build/{}", config.project.name))
        .stderr_to_stdout()
        .unchecked()
        .run()
        .unwrap();

    match cmd_output.status.success() {
        true => println!(
            "\n\n{} {} {}",
            "Finished".green().bold(),
            "program execution with exit code",
            cmd_output.status.code().unwrap_or(0)
        ),

        false => println!(
            "\n\n{} {} {}",
            "Finished".red().bold(),
            "program execution with exit code",
            cmd_output.status.code().unwrap_or(255)
        ),
    }

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
            init_file(&Path::new(".ignore"), b"external/\nres/\n")?;
        }
    }

    Ok(())
}
