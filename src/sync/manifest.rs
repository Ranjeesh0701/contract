use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Failed to read manifest file at '{0}': {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("Failed to parse TOML manifest: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencySource {
    Git {
        git: String,
        path: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
    },
    Path {
        path: String,
    },
    Url {
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "GenerateTargetRaw")]
pub struct GenerateTarget {
    pub contract: String,
    pub lang: String,
    pub out: String,
}

#[derive(Deserialize)]
struct GenerateTargetRaw {
    contract: String,
    lang: String,
    out: Option<String>,
    output: Option<String>,
}

impl TryFrom<GenerateTargetRaw> for GenerateTarget {
    type Error = &'static str;

    fn try_from(raw: GenerateTargetRaw) -> Result<Self, Self::Error> {
        let out = raw
            .out
            .or(raw.output)
            .ok_or("missing field `out` or `output` in [[generate]]")?;
        Ok(Self {
            contract: raw.contract,
            lang: raw.lang,
            out,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySource>,
    #[serde(default)]
    pub generate: Vec<GenerateTarget>,
}

impl Manifest {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref)
            .map_err(|e| ManifestError::Io(path_ref.to_path_buf(), e))?;

        let manifest: Manifest = toml::from_str(&content)?;
        Ok(manifest)
    }
}
