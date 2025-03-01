#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct ConfigFile {
    pub project: Project,
    pub cmake: CMake,
    #[serde(default)]
    pub dependencies: Dependencies,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct Project {
    pub name: String,
    pub version: ordered_float::OrderedFloat<f64>,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct CMake {
    pub minimum_required: ordered_float::OrderedFloat<f64>,
    pub files: IncludeFiles,
}

#[derive(serde::Deserialize, serde::Serialize, Hash, Clone)]
pub enum IncludeFiles {
    All,
    Root,
    Exclude(Vec<String>),
}

#[derive(serde::Deserialize, serde::Serialize, Hash, Default)]
#[serde(default)]
pub struct Dependencies {
    pub find: Vec<FindDependency>,
    pub local: Vec<LocalDependency>,

    pub project_dependencies: Vec<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Hash, Clone)]
pub struct FindDependency {
    pub name: String,
    pub required: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Hash, Clone)]
pub struct LocalDependency {
    pub path: String,
    pub name: String,
    pub local_type: LocalType,
    pub variables: Vec<(String, String)>,
}

#[derive(serde::Deserialize, serde::Serialize, Hash, Clone)]
pub enum LocalType {
    CMake,
    Source {
        files: IncludeFiles,
        dependencies: Vec<String>,
    },
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            project: Project {
                name: String::from("UNKNOWN"),
                version: ordered_float::OrderedFloat(1.0),
            },
            cmake: CMake {
                minimum_required: ordered_float::OrderedFloat(3.15),
                files: IncludeFiles::All,
            },
            dependencies: Dependencies {
                find: Vec::new(),
                local: Vec::new(),
                project_dependencies: Vec::new(),
            },
        }
    }
}

impl ConfigFile {
    pub fn new(name: String) -> Self {
        Self {
            project: Project {
                name,
                version: ordered_float::OrderedFloat(1.0),
            },
            ..Default::default()
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct Cache {
    pub git_submodules: Vec<CacheSubmodule>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct CacheSubmodule {
    pub name: String,
    pub submodule: GitSubmodule,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct GitSubmodule {
    pub repo: String,
    pub tag: Option<String>,
    pub branch: Option<String>,

    pub local_setup: LocalDependency,
}
