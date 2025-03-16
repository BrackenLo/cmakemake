#[derive(serde::Deserialize, serde::Serialize, Default, Hash)]
#[serde(default)]
pub struct ConfigFile {
    pub project: Project,
    pub cmake: CMake,
    pub dependencies: Dependencies,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
#[serde(default)]
pub struct Project {
    pub name: String,
    pub version: ordered_float::OrderedFloat<f64>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: String::from("Unnamed Project"),
            version: ordered_float::OrderedFloat(1.0),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
#[serde(default)]
pub struct CMake {
    pub minimum_required: ordered_float::OrderedFloat<f64>,
    pub files: ProjectFiles,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Hash)]
#[serde(default)]
pub struct ProjectFiles {
    pub source_files: Vec<(SourceType, Vec<String>)>,
    pub include_dirs: Vec<(IncludeType, Vec<String>)>,
    pub exclude_files: Vec<String>,
}

impl Default for ProjectFiles {
    fn default() -> Self {
        Self::all()
    }
}

impl ProjectFiles {
    pub fn all() -> Self {
        Self {
            source_files: vec![(SourceType::GlobRecurse, vec![".".into()])],
            include_dirs: vec![(IncludeType::Public, vec![".".into()])],
            exclude_files: Vec::new(),
        }
    }

    pub fn root() -> Self {
        Self {
            source_files: vec![(SourceType::Glob, vec![".".into()])],
            include_dirs: vec![(IncludeType::Public, vec![".".into()])],
            exclude_files: Vec::new(),
        }
    }

    pub fn header() -> Self {
        Self {
            source_files: Vec::new(),
            include_dirs: vec![(IncludeType::Interface, Vec::new())],
            exclude_files: Vec::new(),
        }
    }
}

impl Default for CMake {
    fn default() -> Self {
        Self {
            minimum_required: ordered_float::OrderedFloat(3.15),
            files: ProjectFiles::default(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Hash)]
pub enum SourceType {
    File,
    Glob,
    GlobRecurse,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Hash)]
pub enum IncludeType {
    Public,
    Interface,
}

#[derive(serde::Deserialize, serde::Serialize, Default, Hash)]
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
    #[serde(default)]
    pub custom_link_name: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Hash)]
pub struct LocalDependency {
    pub path: String,
    pub name: String,
    pub local_type: LocalType,
    pub variables: Vec<(String, String)>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Hash)]
pub enum LocalType {
    CMake,
    Source {
        files: ProjectFiles,
        dependencies: Vec<String>,
    },
}

impl ConfigFile {
    pub fn new(name: String) -> Self {
        let mut config = Self::default();
        config.project.name = name;
        config
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

    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,

    pub local_setup: LocalDependency,
}
