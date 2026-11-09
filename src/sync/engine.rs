use std::fs;
use std::path::{Path, PathBuf};

use crate::adapters;
use crate::codegen::{GoGenerator, PythonGenerator, RustGenerator, TypeScriptGenerator};
use crate::diff::{DiffReport, diff};
use crate::ir::Contract;
use crate::parser;
use crate::sync::lockfile::{LockedContract, Lockfile};
use crate::sync::manifest::{DependencySource, Manifest};
use crate::sync::semver::{VersionReq, resolve_best_version};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Manifest error: {0}")]
    Manifest(#[from] crate::sync::manifest::ManifestError),
    #[error("Lockfile error: {0}")]
    Lockfile(#[from] crate::sync::lockfile::LockfileError),
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
    #[error("Failed to fetch remote dependency '{dep}': {reason}")]
    FetchRemote { dep: String, reason: String },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactDriftStatus {
    UpToDate,
    Modified {
        expected_sha256: String,
        actual_sha256: String,
    },
    MissingOnDisk,
    MissingInLockfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDriftReport {
    pub target: crate::sync::manifest::GenerateTarget,
    pub status: ArtifactDriftStatus,
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

            if let Some(old_locked) = lockfile
                .contracts
                .get(dep_name)
                .filter(|old| old.sha256 != sha256)
            {
                let report = diff(&old_locked.contract, &new_contract);
                if !report.is_compatible() && !force {
                    return Err(SyncError::BreakingChange {
                        dep: dep_name.clone(),
                        count: report.breaking_changes().len(),
                    });
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

        let mut locked_artifacts = std::collections::BTreeMap::new();
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

            fs::write(&out_path, &code).map_err(|e| SyncError::Io(out_path.clone(), e))?;
            generated_files.push(out_path);

            let mut hasher = Sha256::new();
            hasher.update(code.as_bytes());
            let artifact_sha = format!("{:x}", hasher.finalize());

            locked_artifacts.insert(
                target.out.clone(),
                crate::sync::lockfile::LockedArtifact {
                    contract: target.contract.clone(),
                    lang: target.lang.clone(),
                    path: target.out.clone(),
                    sha256: artifact_sha,
                },
            );
        }

        lockfile.artifacts = locked_artifacts;
        lockfile.save_to_file(&lockfile_path)?;

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

    pub fn check_artifacts(manifest_dir: &Path) -> Result<Vec<ArtifactDriftReport>, SyncError> {
        let manifest_path = manifest_dir.join("contract.toml");
        let lockfile_path = manifest_dir.join("contract.lock");

        let manifest = Manifest::load_from_file(&manifest_path)?;
        let lockfile = Lockfile::load_from_file(&lockfile_path)?;

        let mut reports = Vec::new();

        for target in &manifest.generate {
            let out_path = manifest_dir.join(&target.out);
            if !out_path.exists() {
                reports.push(ArtifactDriftReport {
                    target: target.clone(),
                    status: ArtifactDriftStatus::MissingOnDisk,
                });
                continue;
            }

            let content = match fs::read_to_string(&out_path) {
                Ok(c) => c,
                Err(e) => return Err(SyncError::Io(out_path, e)),
            };

            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let current_sha = format!("{:x}", hasher.finalize());

            match lockfile.artifacts.get(&target.out) {
                None => {
                    reports.push(ArtifactDriftReport {
                        target: target.clone(),
                        status: ArtifactDriftStatus::MissingInLockfile,
                    });
                }
                Some(locked) => {
                    if locked.sha256 == current_sha {
                        reports.push(ArtifactDriftReport {
                            target: target.clone(),
                            status: ArtifactDriftStatus::UpToDate,
                        });
                    } else {
                        reports.push(ArtifactDriftReport {
                            target: target.clone(),
                            status: ArtifactDriftStatus::Modified {
                                expected_sha256: locked.sha256.clone(),
                                actual_sha256: current_sha,
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
        let (content, desc, source_path) = match source {
            DependencySource::Path { path } => {
                let full_path = base_dir.join(path);
                let text = fs::read_to_string(&full_path)
                    .map_err(|e| SyncError::Io(full_path.clone(), e))?;
                (text, format!("path:{}", path), Some(full_path))
            }
            DependencySource::Url { url } => {
                let (text, desc, path) = Self::fetch_url(base_dir, dep_name, url)?;
                (text, desc, Some(path))
            }
            DependencySource::Git {
                git,
                path,
                branch,
                tag,
                rev,
                version,
            } => {
                let (text, desc, file_path) = Self::fetch_git(
                    base_dir,
                    dep_name,
                    git,
                    path,
                    branch.as_deref(),
                    tag.as_deref(),
                    rev.as_deref(),
                    version.as_deref(),
                )?;
                (text, desc, Some(file_path))
            }
        };

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let sha256 = format!("{:x}", hasher.finalize());

        let adapter_reg = adapters::default_registry();
        let contract = if let Some(adapter) =
            adapter_reg.find_adapter(source_path.as_deref(), &content)
        {
            adapter
                .parse(&content)
                .map_err(|e| SyncError::ParseContract {
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

    #[allow(clippy::too_many_arguments)]
    fn fetch_git(
        base_dir: &Path,
        dep_name: &str,
        git_url: &str,
        contract_subpath: &str,
        branch: Option<&str>,
        tag: Option<&str>,
        rev: Option<&str>,
        version_req: Option<&str>,
    ) -> Result<(String, String, PathBuf), SyncError> {
        let mut hasher = Sha256::new();
        hasher.update(git_url.as_bytes());
        let repo_hash = format!("{:x}", hasher.finalize());

        let cache_git_dir = base_dir.join(".contract").join("cache").join("git");
        let repo_dir = cache_git_dir.join(&repo_hash[..16]);

        let resolved_tag_string;
        let target_ref = if let Some(req_str) = version_req {
            let req = VersionReq::parse(req_str).map_err(|e| SyncError::FetchRemote {
                dep: dep_name.to_string(),
                reason: format!("Invalid version requirement '{}': {}", req_str, e),
            })?;

            let mut tag_names = Vec::new();
            let ls_remote = std::process::Command::new("git")
                .args(["ls-remote", "--tags", git_url])
                .output();

            if let Ok(ref out) = ls_remote
                && out.status.success()
            {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if let Some((_, ref_path)) = line.split_once('\t')
                        && let Some(tname) = ref_path.strip_prefix("refs/tags/")
                        && !tname.ends_with("^{}")
                    {
                        tag_names.push(tname.to_string());
                    }
                }
            }

            if tag_names.is_empty() && repo_dir.exists() {
                let local_tags = std::process::Command::new("git")
                    .current_dir(&repo_dir)
                    .args(["tag", "-l"])
                    .output();
                if let Ok(ref out) = local_tags {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines() {
                        let t = line.trim();
                        if !t.is_empty() {
                            tag_names.push(t.to_string());
                        }
                    }
                }
            }

            let tag_refs: Vec<&str> = tag_names.iter().map(|s| s.as_str()).collect();
            match resolve_best_version(&req, tag_refs) {
                Some((best_tag, _)) => {
                    resolved_tag_string = best_tag.to_string();
                    resolved_tag_string.as_str()
                }
                None => {
                    return Err(SyncError::FetchRemote {
                        dep: dep_name.to_string(),
                        reason: format!(
                            "No remote tag in '{}' satisfies version requirement '{}'",
                            git_url, req_str
                        ),
                    });
                }
            }
        } else {
            rev.or(tag).or(branch).unwrap_or("HEAD")
        };

        if !repo_dir.exists() {
            fs::create_dir_all(&cache_git_dir)
                .map_err(|e| SyncError::Io(cache_git_dir.clone(), e))?;

            let mut clone_cmd = std::process::Command::new("git");
            clone_cmd.args(["clone", "--quiet"]);
            if target_ref != "HEAD" && rev.is_none() {
                clone_cmd.args(["--branch", target_ref, "--depth", "1"]);
            } else if let Some(b) = branch {
                clone_cmd.args(["--branch", b, "--depth", "1"]);
            }
            clone_cmd.arg(git_url).arg(&repo_dir);

            let status = clone_cmd.status().map_err(|e| SyncError::FetchRemote {
                dep: dep_name.to_string(),
                reason: format!("Failed to execute 'git clone': {}", e),
            })?;

            if !status.success() {
                let fallback = std::process::Command::new("git")
                    .args(["clone", "--quiet", git_url])
                    .arg(&repo_dir)
                    .status()
                    .map_err(|e| SyncError::FetchRemote {
                        dep: dep_name.to_string(),
                        reason: format!("Failed to execute fallback 'git clone': {}", e),
                    })?;

                if !fallback.success() {
                    return Err(SyncError::FetchRemote {
                        dep: dep_name.to_string(),
                        reason: format!("'git clone' exited with status {}", fallback),
                    });
                }
            }
        } else {
            let _ = std::process::Command::new("git")
                .current_dir(&repo_dir)
                .args(["fetch", "--quiet", "--tags"])
                .status();
        }

        if target_ref != "HEAD" {
            let co = std::process::Command::new("git")
                .current_dir(&repo_dir)
                .args(["checkout", "--quiet", target_ref])
                .status()
                .map_err(|e| SyncError::FetchRemote {
                    dep: dep_name.to_string(),
                    reason: format!("Failed to checkout '{}': {}", target_ref, e),
                })?;

            if !co.success() {
                return Err(SyncError::FetchRemote {
                    dep: dep_name.to_string(),
                    reason: format!("'git checkout {}' failed with status {}", target_ref, co),
                });
            }
        }

        let full_path = repo_dir.join(contract_subpath);
        if !full_path.exists() {
            return Err(SyncError::FetchRemote {
                dep: dep_name.to_string(),
                reason: format!(
                    "Contract file '{}' not found in git repository '{}' at ref '{}'",
                    contract_subpath, git_url, target_ref
                ),
            });
        }

        let content =
            fs::read_to_string(&full_path).map_err(|e| SyncError::Io(full_path.clone(), e))?;

        let desc = format!("git:{}#{}:{}", git_url, target_ref, contract_subpath);
        Ok((content, desc, full_path))
    }

    fn fetch_url(
        base_dir: &Path,
        dep_name: &str,
        url_str: &str,
    ) -> Result<(String, String, PathBuf), SyncError> {
        let (content, derived_path) = if let Some(stripped) = url_str.strip_prefix("file://") {
            let file_path = PathBuf::from(stripped);
            let full_path = if file_path.is_relative() {
                base_dir.join(&file_path)
            } else {
                file_path
            };
            let text =
                fs::read_to_string(&full_path).map_err(|e| SyncError::Io(full_path.clone(), e))?;
            (text, full_path)
        } else if url_str.starts_with("http://") || url_str.starts_with("https://") {
            let mut hasher = Sha256::new();
            hasher.update(url_str.as_bytes());
            let url_hash = format!("{:x}", hasher.finalize());

            let cache_url_dir = base_dir.join(".contract").join("cache").join("urls");
            fs::create_dir_all(&cache_url_dir)
                .map_err(|e| SyncError::Io(cache_url_dir.clone(), e))?;

            let filename = url_str
                .split('?')
                .next()
                .unwrap_or(url_str)
                .rsplit('/')
                .next()
                .unwrap_or("contract.contract");
            let cache_file = cache_url_dir.join(format!("{}-{}", &url_hash[..8], filename));

            let curl_out = std::process::Command::new("curl")
                .args(["-fsSL", url_str])
                .output();

            let text = match curl_out {
                Ok(out) if out.status.success() => {
                    let s = String::from_utf8(out.stdout).map_err(|e| SyncError::FetchRemote {
                        dep: dep_name.to_string(),
                        reason: format!("URL returned non-UTF8 response: {}", e),
                    })?;
                    let _ = fs::write(&cache_file, &s);
                    s
                }
                _ => {
                    if cache_file.exists() {
                        fs::read_to_string(&cache_file)
                            .map_err(|e| SyncError::Io(cache_file.clone(), e))?
                    } else {
                        return Err(SyncError::FetchRemote {
                            dep: dep_name.to_string(),
                            reason: format!(
                                "Failed to fetch URL '{}' and no cached copy found",
                                url_str
                            ),
                        });
                    }
                }
            };

            (text, cache_file)
        } else {
            return Err(SyncError::FetchRemote {
                dep: dep_name.to_string(),
                reason: format!("Unsupported URL scheme in '{}'", url_str),
            });
        };

        let desc = format!("url:{}", url_str);
        Ok((content, desc, derived_path))
    }
}
