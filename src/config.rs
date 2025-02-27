#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct ConfigFile {
    pub project: Project,
    pub cmake: CMake,
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

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub enum IncludeFiles {
    AllRecurse,
    All,
    Exclude(Vec<String>),
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct Dependencies {
    pub local: Vec<LocalDependency>,
    pub fetch_content: Vec<FetchDependency>,

    pub project_dependencies: Vec<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct LocalDependency {
    pub path: String,
    pub name: String,
    pub local_type: LocalType,
    pub variables: Vec<(String, String)>,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub enum LocalType {
    CMake,
    Source {
        files: IncludeFiles,
        dependencies: Vec<String>,
    },
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct FetchDependency {
    pub name: String,
    pub variables: Vec<(String, String)>,

    pub repo: String,
    pub tag: Option<String>,
    pub branch: Option<String>,
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
                files: IncludeFiles::AllRecurse,
            },
            dependencies: Dependencies {
                local: Vec::new(),
                fetch_content: Vec::new(),
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
