use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ir::Contract;
use crate::parser::resolve_contract_file;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("IO error in '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse bundle JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Bundle checksum mismatch: expected {expected}, calculated {actual}")]
    ChecksumMismatch {
        expected: String,
        actual: String,
    },
    #[error("Failed to resolve contract: {0}")]
    Resolve(#[from] crate::parser::ResolverError),
    #[error("Invalid bundle: missing root file '{0}'")]
    MissingRootFile(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleManifest {
    pub name: String,
    pub version: String,
    pub root_file: String,
    pub file_count: usize,
    pub total_types: usize,
    pub total_services: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractBundle {
    pub format_version: String,
    pub manifest: BundleManifest,
    pub root_contract: Contract,
    /// Deterministically sorted relative paths -> file contents
    pub files: BTreeMap<String, String>,
    pub checksum: String,
}

impl ContractBundle {
    pub fn pack(entry_file: impl AsRef<Path>, flatten: bool) -> Result<Self, BundleError> {
        let entry_path = entry_file.as_ref();
        let canonical_entry = fs::canonicalize(entry_path).map_err(|e| BundleError::Io {
            path: entry_path.to_path_buf(),
            source: e,
        })?;

        // 1. Resolve contract graph and validate
        let mut resolved_contract = resolve_contract_file(&canonical_entry)?;

        // 2. Discover all constituent files
        let mut raw_files: Vec<(PathBuf, String)> = Vec::new();
        let mut visited_paths: HashSet<PathBuf> = HashSet::new();

        collect_constituent_files(&canonical_entry, &mut raw_files, &mut visited_paths)?;

        let all_paths: Vec<PathBuf> = raw_files.iter().map(|(p, _)| p.clone()).collect();
        let common_root = find_common_ancestor(&all_paths);

        // 3. Map to relative paths
        let mut files = BTreeMap::new();
        let mut root_file_rel = String::new();

        for (canonical_path, content) in raw_files {
            let rel_str = canonical_path
                .strip_prefix(&common_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| {
                    canonical_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });

            if canonical_path == canonical_entry {
                root_file_rel = rel_str.clone();
            }

            files.insert(rel_str, content);
        }

        if flatten {
            resolved_contract.imports.clear();
        }

        let manifest = BundleManifest {
            name: resolved_contract.name.clone(),
            version: resolved_contract.version.clone(),
            root_file: root_file_rel,
            file_count: files.len(),
            total_types: resolved_contract.types.len(),
            total_services: resolved_contract.services.len(),
        };

        let checksum = compute_bundle_checksum(&manifest, &files);

        Ok(Self {
            format_version: "1.0".to_string(),
            manifest,
            root_contract: resolved_contract,
            files,
            checksum,
        })
    }

    pub fn verify(&self) -> Result<(), BundleError> {
        let calculated = compute_bundle_checksum(&self.manifest, &self.files);
        if calculated != self.checksum {
            return Err(BundleError::ChecksumMismatch {
                expected: self.checksum.clone(),
                actual: calculated,
            });
        }

        if !self.files.contains_key(&self.manifest.root_file) {
            return Err(BundleError::MissingRootFile(
                self.manifest.root_file.clone(),
            ));
        }

        Ok(())
    }

    pub fn unpack(&self, target_dir: impl AsRef<Path>) -> Result<(), BundleError> {
        self.verify()?;

        let target = target_dir.as_ref();
        for (rel_path, content) in &self.files {
            let dest_path = target.join(rel_path);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| BundleError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::write(&dest_path, content).map_err(|e| BundleError::Io {
                path: dest_path,
                source: e,
            })?;
        }

        Ok(())
    }

    pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<(), BundleError> {
        let json = serde_json::to_string_pretty(self)?;
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| BundleError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        fs::write(p, json).map_err(|e| BundleError::Io {
            path: p.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }

    pub fn read_from_file(path: impl AsRef<Path>) -> Result<Self, BundleError> {
        let p = path.as_ref();
        let content = fs::read_to_string(p).map_err(|e| BundleError::Io {
            path: p.to_path_buf(),
            source: e,
        })?;
        let bundle: Self = serde_json::from_str(&content)?;
        bundle.verify()?;
        Ok(bundle)
    }
}

fn collect_constituent_files(
    file_path: &Path,
    raw_files: &mut Vec<(PathBuf, String)>,
    visited_paths: &mut HashSet<PathBuf>,
) -> Result<(), BundleError> {
    let canonical = fs::canonicalize(file_path).map_err(|e| BundleError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;

    if !visited_paths.insert(canonical.clone()) {
        return Ok(());
    }

    let content = fs::read_to_string(&canonical).map_err(|e| BundleError::Io {
        path: canonical.clone(),
        source: e,
    })?;

    let parent_dir = canonical.parent().unwrap_or_else(|| Path::new("."));

    if let Ok(parsed) = crate::parser::parse(&content) {
        for import_str in &parsed.imports {
            let sub_path = if Path::new(import_str).is_absolute() {
                PathBuf::from(import_str)
            } else {
                parent_dir.join(import_str)
            };
            if sub_path.exists() {
                collect_constituent_files(&sub_path, raw_files, visited_paths)?;
            }
        }
    }

    raw_files.push((canonical, content));
    Ok(())
}

fn find_common_ancestor(paths: &[PathBuf]) -> PathBuf {
    if paths.is_empty() {
        return PathBuf::from(".");
    }
    let mut common = paths[0].clone();
    if common.is_file() {
        common = common
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
    }

    for p in &paths[1..] {
        while !p.starts_with(&common) {
            if let Some(parent) = common.parent() {
                common = parent.to_path_buf();
            } else {
                break;
            }
        }
    }
    common
}

fn compute_bundle_checksum(manifest: &BundleManifest, files: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"contract-bundle-v1\n");
    hasher.update(manifest.name.as_bytes());
    hasher.update(b"\n");
    hasher.update(manifest.version.as_bytes());
    hasher.update(b"\n");
    hasher.update(manifest.root_file.as_bytes());
    hasher.update(b"\n");

    for (rel_path, content) in files {
        hasher.update(rel_path.as_bytes());
        hasher.update(b":");
        hasher.update(content.as_bytes());
        hasher.update(b"\n");
    }

    format!("{:x}", hasher.finalize())
}
