use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ir::Contract;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockfileError {
    #[error("Failed to read lockfile at '{0}': {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("Failed to parse lockfile JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedContract {
    pub source: String,
    pub sha256: String,
    pub name: String,
    pub version: String,
    pub contract: Contract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    pub contracts: BTreeMap<String, LockedContract>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: 1,
            contracts: BTreeMap::new(),
        }
    }
}

impl Lockfile {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, LockfileError> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Ok(Lockfile::default());
        }

        let content = fs::read_to_string(path_ref)
            .map_err(|e| LockfileError::Io(path_ref.to_path_buf(), e))?;

        let lock: Lockfile = serde_json::from_str(&content)?;
        Ok(lock)
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), LockfileError> {
        let path_ref = path.as_ref();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path_ref, content).map_err(|e| LockfileError::Io(path_ref.to_path_buf(), e))?;
        Ok(())
    }
}
