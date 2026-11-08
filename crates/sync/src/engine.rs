use std::fs;
use std::path::{Path, PathBuf};

use codegen::{GoGenerator, PythonGenerator, RustGenerator, TypeScriptGenerator};
use diff::{DiffReport, diff};
use ir::Contract;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::lockfile::{LockedContract, Lockfile};
use crate::manifest::{DependencySource, Manifest};

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Manifest error: {0}")]
    Manifest(#[from] crate::manifest::ManifestError),
    #[error("Lockfile error: {0}")]
    Lockfile(#[from] crate::lockfile::LockfileError),
    #[error("I/O error at '{0}': {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("Failed to parse contract for dependency '{dep}': {reason}")]
    ParseContract { dep: String, reason: String },
    #[error("Contract validation failed for '{dep}': {count} error(s)")]
    Validation { dep: String, count: usize },
    #[error(
        "Breaking change detected in '{dep}'. Pass --force to sync anyway: {count} breaking change(s)"
    )]
    BreakingChange { dep: String, count: usize },
    #[error("Unknown contract '{0}' specified in [[generate]]")]
    UnknownContract(String),
    #[error("Unsupported target language '{0}'")]
    UnsupportedLanguage(String),
}

#[derive(Debug, Clone)]
pub struct SyncReport {
    pub synced_contracts: Vec<String>,
    pub generated_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum DriftStatus {
    UpToDate,
    Drifted {
        report: DiffReport,
        is_breaking: bool,
    },
    MissingInLockfile,
}

#[derive(Debug, Clone)]
pub struct DriftReport {
    pub dependency: String,
    pub status: DriftStatus,
}

pub struct SyncEngine;

impl SyncEngine {
    pub fn sync(manifest_dir: &Path, force: bool) -> Result<SyncReport, SyncError> {
        let manifest_path = manifest_dir.join("contract.toml");
        let lockfile_path = manifest_dir.join("contract.lock");

        let manifest = Manifest::load_from_file(&manifest_path)?;
        let mut lockfile = Lockfile::load_from_file(&lockfile_path)?;

        let mut synced_contracts = Vec::new();
        let mut generated_files = Vec::new();

        for (dep_name, source) in &manifest.dependencies {
            let (new_contract, sha256, source_desc) =
                Self::fetch_and_parse(manifest_dir, dep_name, source)?;

            if let Some(old_locked) = lockfile.contracts.get(dep_name) {
                if old_locked.sha256 != sha256 {
                    let report = diff(&old_locked.contract, &new_contract);
                    if !report.is_compatible() && !force {
                        return Err(SyncError::BreakingChange {
                            dep: dep_name.clone(),
                            count: report.breaking_changes().len(),
                        });
                    }
                }
            }

            lockfile.contracts.insert(
                dep_name.clone(),
                LockedContract {
                    source: source_desc,
                    sha256,
                    name: new_contract.name.clone(),
                    version: new_contract.version.clone(),
                    contract: new_contract,
                },
            );

            synced_contracts.push(dep_name.clone());
        }

        lockfile.save_to_file(&lockfile_path)?;

        for target in &manifest.generate {
            let locked = lockfile
                .contracts
                .get(&target.contract)
                .ok_or_else(|| SyncError::UnknownContract(target.contract.clone()))?;

            let code = match target.lang.to_lowercase().as_str() {
                "ts" | "typescript" => TypeScriptGenerator::generate(&locked.contract),
                "rs" | "rust" => RustGenerator::generate(&locked.contract),
                "py" | "python" => PythonGenerator::generate(&locked.contract),
                "go" | "golang" => GoGenerator::generate(&locked.contract),
                other => return Err(SyncError::UnsupportedLanguage(other.to_string())),
            };

            let out_path = manifest_dir.join(&target.out);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| SyncError::Io(parent.to_path_buf(), e))?;
            }

            fs::write(&out_path, code).map_err(|e| SyncError::Io(out_path.clone(), e))?;
            generated_files.push(out_path);
        }

        Ok(SyncReport {
            synced_contracts,
            generated_files,
        })
    }

    pub fn check_drift(manifest_dir: &Path) -> Result<Vec<DriftReport>, SyncError> {
        let manifest_path = manifest_dir.join("contract.toml");
        let lockfile_path = manifest_dir.join("contract.lock");

        let manifest = Manifest::load_from_file(&manifest_path)?;
        let lockfile = Lockfile::load_from_file(&lockfile_path)?;

        let mut reports = Vec::new();

        for (dep_name, source) in &manifest.dependencies {
            let (live_contract, live_sha, _) =
                Self::fetch_and_parse(manifest_dir, dep_name, source)?;

            match lockfile.contracts.get(dep_name) {
                None => {
                    reports.push(DriftReport {
                        dependency: dep_name.clone(),
                        status: DriftStatus::MissingInLockfile,
                    });
                }
                Some(locked) => {
                    if locked.sha256 == live_sha {
                        reports.push(DriftReport {
                            dependency: dep_name.clone(),
                            status: DriftStatus::UpToDate,
                        });
                    } else {
                        let diff_report = diff(&locked.contract, &live_contract);
                        let is_breaking = !diff_report.is_compatible();
                        reports.push(DriftReport {
                            dependency: dep_name.clone(),
                            status: DriftStatus::Drifted {
                                report: diff_report,
                                is_breaking,
                            },
                        });
                    }
                }
            }
        }

        Ok(reports)
    }

    fn fetch_and_parse(
        base_dir: &Path,
        dep_name: &str,
        source: &DependencySource,
    ) -> Result<(Contract, String, String), SyncError> {
        let (content, desc) = match source {
            DependencySource::Path { path } => {
                let full_path = base_dir.join(path);
                let text = fs::read_to_string(&full_path)
                    .map_err(|e| SyncError::Io(full_path.clone(), e))?;
                (text, format!("path:{}", path))
            }
            DependencySource::Url { url } => {
                // For file/local simulation or HTTP
                (String::new(), format!("url:{}", url))
            }
            DependencySource::Git { git, path, branch } => {
                let b = branch.as_deref().unwrap_or("main");
                (String::new(), format!("git:{}#{}:{}", git, b, path))
            }
        };

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let sha256 = format!("{:x}", hasher.finalize());

        let adapter_reg = adapters::default_registry();
        let contract = if let Some(adapter) = adapter_reg.find_adapter(None, &content) {
            adapter.parse(&content).map_err(|e| SyncError::ParseContract {
                dep: dep_name.to_string(),
                reason: e.to_string(),
            })?
        } else if content.trim_start().starts_with('{') {
            serde_json::from_str::<Contract>(&content).map_err(|e| SyncError::ParseContract {
                dep: dep_name.to_string(),
                reason: e.to_string(),
            })?
        } else {
            parser::parse(&content).map_err(|e| SyncError::ParseContract {
                dep: dep_name.to_string(),
                reason: e.to_string(),
            })?
        };

        if let Err(errors) = contract.validate() {
            return Err(SyncError::Validation {
                dep: dep_name.to_string(),
                count: errors.len(),
            });
        }

        Ok((contract, sha256, desc))
    }
}
