use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::ir::{Contract, Service, TypeDefinition};
use crate::parser::ParseError;

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("IO error reading '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Imported file not found: '{0}'")]
    FileNotFound(PathBuf),
    #[error("Parse error in '{path}': {source}")]
    Parse {
        path: PathBuf,
        source: ParseError,
    },
    #[error("Adapter error in '{path}' ({adapter}): {reason}")]
    Adapter {
        path: PathBuf,
        adapter: String,
        reason: String,
    },
    #[error("Circular import detected: {}", cycle.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" -> "))]
    CircularImport {
        cycle: Vec<PathBuf>,
    },
    #[error("Duplicate type '{type_name}' defined in both '{}' and '{}'", path_a.display(), path_b.display())]
    TypeConflict {
        type_name: String,
        path_a: PathBuf,
        path_b: PathBuf,
    },
    #[error("Duplicate service '{service_name}' defined in both '{}' and '{}'", path_a.display(), path_b.display())]
    ServiceConflict {
        service_name: String,
        path_a: PathBuf,
        path_b: PathBuf,
    },
}

pub struct ModuleResolver {
    visited: HashSet<PathBuf>,
    active_stack: Vec<PathBuf>,
    defined_types: HashMap<String, PathBuf>,
    defined_services: HashMap<String, PathBuf>,
    accumulated_types: Vec<TypeDefinition>,
    accumulated_services: Vec<Service>,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self {
            visited: HashSet::new(),
            active_stack: Vec::new(),
            defined_types: HashMap::new(),
            defined_services: HashMap::new(),
            accumulated_types: Vec::new(),
            accumulated_services: Vec::new(),
        }
    }

    pub fn resolve_file(path: impl AsRef<Path>) -> Result<Contract, ResolverError> {
        let mut resolver = Self::new();
        resolver.resolve_entry_file(path.as_ref())
    }

    pub fn resolve_str(source: &str, base_dir: impl AsRef<Path>) -> Result<Contract, ResolverError> {
        let mut root_contract = crate::parser::parse(source).map_err(|e| ResolverError::Parse {
            path: base_dir.as_ref().to_path_buf(),
            source: e,
        })?;

        if root_contract.imports.is_empty() {
            return Ok(root_contract);
        }

        let mut resolver = Self::new();
        let pseudo_root = base_dir.as_ref().join("_virtual_root.contract");
        resolver.active_stack.push(pseudo_root.clone());

        for type_def in &root_contract.types {
            resolver.defined_types.insert(type_def.name().to_string(), pseudo_root.clone());
        }
        for svc in &root_contract.services {
            resolver.defined_services.insert(svc.name.clone(), pseudo_root.clone());
        }

        for import_str in &root_contract.imports {
            let target_path = if Path::new(import_str).is_absolute() {
                PathBuf::from(import_str)
            } else {
                base_dir.as_ref().join(import_str)
            };
            resolver.resolve_dependency(&target_path)?;
        }

        resolver.active_stack.pop();

        let mut final_types = resolver.accumulated_types;
        final_types.extend(root_contract.types);
        root_contract.types = final_types;

        let mut final_services = resolver.accumulated_services;
        final_services.extend(root_contract.services);
        root_contract.services = final_services;

        Ok(root_contract)
    }

    fn resolve_entry_file(&mut self, path: &Path) -> Result<Contract, ResolverError> {
        if !path.exists() {
            return Err(ResolverError::FileNotFound(path.to_path_buf()));
        }

        let canonical_root = fs::canonicalize(path).map_err(|e| ResolverError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let mut root_contract = self.read_and_parse(&canonical_root)?;
        self.active_stack.push(canonical_root.clone());

        for type_def in &root_contract.types {
            self.defined_types.insert(type_def.name().to_string(), canonical_root.clone());
        }
        for svc in &root_contract.services {
            self.defined_services.insert(svc.name.clone(), canonical_root.clone());
        }

        let parent_dir = canonical_root.parent().unwrap_or_else(|| Path::new("."));

        for import_str in &root_contract.imports {
            let target_path = if Path::new(import_str).is_absolute() {
                PathBuf::from(import_str)
            } else {
                parent_dir.join(import_str)
            };
            self.resolve_dependency(&target_path)?;
        }

        self.active_stack.pop();
        self.visited.insert(canonical_root);

        let mut final_types = std::mem::take(&mut self.accumulated_types);
        final_types.extend(root_contract.types);
        root_contract.types = final_types;

        let mut final_services = std::mem::take(&mut self.accumulated_services);
        final_services.extend(root_contract.services);
        root_contract.services = final_services;

        Ok(root_contract)
    }

    fn resolve_dependency(&mut self, target_path: &Path) -> Result<(), ResolverError> {
        if !target_path.exists() {
            return Err(ResolverError::FileNotFound(target_path.to_path_buf()));
        }

        let canonical = fs::canonicalize(target_path).map_err(|e| ResolverError::Io {
            path: target_path.to_path_buf(),
            source: e,
        })?;

        if let Some(pos) = self.active_stack.iter().position(|p| p == &canonical) {
            let mut cycle = self.active_stack[pos..].to_vec();
            cycle.push(canonical);
            return Err(ResolverError::CircularImport { cycle });
        }

        if self.visited.contains(&canonical) {
            return Ok(());
        }

        self.active_stack.push(canonical.clone());

        let dep_contract = self.read_and_parse(&canonical)?;
        let parent_dir = canonical.parent().unwrap_or_else(|| Path::new("."));

        for import_str in &dep_contract.imports {
            let sub_path = if Path::new(import_str).is_absolute() {
                PathBuf::from(import_str)
            } else {
                parent_dir.join(import_str)
            };
            self.resolve_dependency(&sub_path)?;
        }

        for type_def in dep_contract.types {
            let name = type_def.name().to_string();
            if let Some(prev_path) = self.defined_types.get(&name) {
                return Err(ResolverError::TypeConflict {
                    type_name: name,
                    path_a: prev_path.clone(),
                    path_b: canonical.clone(),
                });
            }
            self.defined_types.insert(name, canonical.clone());
            self.accumulated_types.push(type_def);
        }

        for svc in dep_contract.services {
            let name = svc.name.clone();
            if let Some(prev_path) = self.defined_services.get(&name) {
                return Err(ResolverError::ServiceConflict {
                    service_name: name,
                    path_a: prev_path.clone(),
                    path_b: canonical.clone(),
                });
            }
            self.defined_services.insert(name, canonical.clone());
            self.accumulated_services.push(svc);
        }

        self.active_stack.pop();
        self.visited.insert(canonical);

        Ok(())
    }

    fn read_and_parse(&self, path: &Path) -> Result<Contract, ResolverError> {
        let content = fs::read_to_string(path).map_err(|e| ResolverError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let adapter_reg = crate::adapters::default_registry();
        if let Some(adapter) = adapter_reg.find_adapter(Some(path), &content) {
            adapter.parse(&content).map_err(|e| ResolverError::Adapter {
                path: path.to_path_buf(),
                adapter: adapter.name().to_string(),
                reason: e.to_string(),
            })
        } else {
            crate::parser::parse(&content).map_err(|e| ResolverError::Parse {
                path: path.to_path_buf(),
                source: e,
            })
        }
    }
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

pub fn resolve_contract_file(path: impl AsRef<Path>) -> Result<Contract, ResolverError> {
    ModuleResolver::resolve_file(path)
}

pub fn resolve_contract_str(source: &str, base_dir: impl AsRef<Path>) -> Result<Contract, ResolverError> {
    ModuleResolver::resolve_str(source, base_dir)
}
