use std::{
    io::Write,
    path::{Path, PathBuf},
};

use colored::Colorize;
use error::{DisplayError, ProjectError};
use util::*;

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
    print_command("cmake", "Generate cmake build script");
    print_command("build", "Build project code");
    print_command("run", "Build and run project code");
    print_command(
        "clean",
        "remove c++ build files (and optionally cmake files)",
    );
    print_command("help", "Output this help message");
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ConfigFile {
    project: Project,
    cmake: CMake,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Project {
    name: String,
    version: f32,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CMake {
    minimum_required: f64,
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
    let config = ConfigFile {
        project: Project {
            name: name.clone(),
            version: 1.0,
        },
        cmake: CMake {
            minimum_required: 3.15,
        },
    };

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

fn generate_cmake() -> Result<(), ProjectError> {
    println!("Generating CMakeLists.txt from config");

    let instant = std::time::Instant::now();

    let config = get_config()?;

    let cmake_path = PathBuf::from("CMakeLists.txt");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&cmake_path)
        .unwrap();

    writeln!(
        file,
        "cmake_minimum_required(VERSION {})",
        config.cmake.minimum_required
    )
    .unwrap();

    writeln!(file, r#"project("{}")"#, config.project.name).unwrap();

    writeln!(file, "set(CMAKE_BUILD_TYPE Debug)").unwrap();
    writeln!(file, "set(CMAKE_EXPORT_COMPILE_COMMANDS ON)").unwrap();

    writeln!(
        file,
        r#"file(GLOB_RECURSE SOURCES "src/*.cpp" "src/*.hpp" "src/*.h")"#
    )
    .unwrap();

    writeln!(file, r#"add_executable("${{PROJECT_NAME}}" ${{SOURCES}})"#).unwrap();

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
