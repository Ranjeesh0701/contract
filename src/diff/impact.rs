use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};

use crate::diff::change::{ChangeSeverity, DiffDirection};
use crate::diff::comparator::diff_with_direction;
use crate::ir::{Contract, TypeDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointDirection {
    Input,
    Output,
    Both,
}

impl std::fmt::Display for EndpointDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointDirection::Input => write!(f, "Input"),
            EndpointDirection::Output => write!(f, "Output"),
            EndpointDirection::Both => write!(f, "Both"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactedEndpoint {
    pub service: String,
    pub operation: String,
    pub direction: EndpointDirection,
    pub depth: usize,
    pub path: Vec<String>,
    pub severity: ChangeSeverity,
    pub cause: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    pub blast_radius_score: usize,
    pub highest_severity: Option<ChangeSeverity>,
    pub total_impacted_operations: usize,
    pub total_impacted_services: usize,
    pub total_affected_types: usize,
    pub impacted_endpoints: Vec<ImpactedEndpoint>,
    pub affected_types: Vec<String>,
}

impl ImpactReport {
    pub fn has_breaking_impact(&self) -> bool {
        self.highest_severity == Some(ChangeSeverity::Breaking)
    }

    pub fn is_empty(&self) -> bool {
        self.impacted_endpoints.is_empty() && self.affected_types.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ContractGraph {
    /// Referenced type name -> list of (parent_type_name, field_name)
    pub type_dependents: HashMap<String, Vec<(String, String)>>,
    /// Referenced type name -> list of (service_name, operation_name, direction)
    pub operation_dependents: HashMap<String, Vec<(String, String, EndpointDirection)>>,
    /// Set of all declared type names
    pub declared_types: HashSet<String>,
    /// Set of all declared operations: (service_name, operation_name)
    pub declared_operations: HashSet<(String, String)>,
}

impl ContractGraph {
    pub fn build(contract: &Contract) -> Self {
        let mut graph = Self::default();
        graph.merge_with(contract);
        graph
    }

    pub fn build_combined(old_contract: &Contract, new_contract: &Contract) -> Self {
        let mut graph = Self::build(old_contract);
        graph.merge_with(new_contract);
        graph
    }

    pub fn merge_with(&mut self, contract: &Contract) {
        for type_def in &contract.types {
            let name = type_def.name().to_string();
            self.declared_types.insert(name.clone());

            match type_def {
                TypeDefinition::Record(record) => {
                    for field in &record.fields {
                        let mut refs = Vec::new();
                        field.field_type.collect_references(&mut refs);
                        for ref_name in refs {
                            let entry = self.type_dependents.entry(ref_name).or_default();
                            let tuple = (record.name.clone(), field.name.clone());
                            if !entry.contains(&tuple) {
                                entry.push(tuple);
                            }
                        }
                    }
                }
                TypeDefinition::Alias(alias) => {
                    let mut refs = Vec::new();
                    alias.target.collect_references(&mut refs);
                    for ref_name in refs {
                        let entry = self.type_dependents.entry(ref_name).or_default();
                        let tuple = (alias.name.clone(), "target".to_string());
                        if !entry.contains(&tuple) {
                            entry.push(tuple);
                        }
                    }
                }
                TypeDefinition::Enum(_) => {}
            }
        }

        for service in &contract.services {
            for op in &service.operations {
                self.declared_operations
                    .insert((service.name.clone(), op.name.clone()));

                if let Some(ref in_ty) = op.input {
                    let mut refs = Vec::new();
                    in_ty.collect_references(&mut refs);
                    for ref_name in refs {
                        let entry = self.operation_dependents.entry(ref_name).or_default();
                        let tuple = (
                            service.name.clone(),
                            op.name.clone(),
                            EndpointDirection::Input,
                        );
                        if !entry.contains(&tuple) {
                            entry.push(tuple);
                        }
                    }
                }

                if let Some(ref out_ty) = op.output {
                    let mut refs = Vec::new();
                    out_ty.collect_references(&mut refs);
                    for ref_name in refs {
                        let entry = self.operation_dependents.entry(ref_name).or_default();
                        let tuple = (
                            service.name.clone(),
                            op.name.clone(),
                            EndpointDirection::Output,
                        );
                        if !entry.contains(&tuple) {
                            entry.push(tuple);
                        }
                    }
                }
            }
        }
    }

    /// Traces all upstream operations and models that depend on `start_symbol`.
    pub fn trace_impact(
        &self,
        start_symbol: &str,
        severity: ChangeSeverity,
        cause: Option<&str>,
    ) -> (Vec<ImpactedEndpoint>, HashSet<String>) {
        let mut endpoints = Vec::new();
        let mut affected_types = HashSet::new();

        // 1. Direct operation check (e.g. services.OrderService.operations.GetOrder or services.OrderService.GetOrder)
        if start_symbol.starts_with("services.") {
            let parts: Vec<&str> = start_symbol.split('.').collect();
            if parts.len() >= 4 && parts[2] == "operations" {
                let service = parts[1].to_string();
                let op = parts[3].to_string();
                endpoints.push(ImpactedEndpoint {
                    service: service.clone(),
                    operation: op.clone(),
                    direction: EndpointDirection::Both,
                    depth: 0,
                    path: vec![format!("{}.{}", service, op)],
                    severity,
                    cause: cause.map(|c| c.to_string()),
                });
                return (endpoints, affected_types);
            } else if parts.len() >= 3 {
                let service = parts[1].to_string();
                let op = parts[2].to_string();
                endpoints.push(ImpactedEndpoint {
                    service: service.clone(),
                    operation: op.clone(),
                    direction: EndpointDirection::Both,
                    depth: 0,
                    path: vec![format!("{}.{}", service, op)],
                    severity,
                    cause: cause.map(|c| c.to_string()),
                });
                return (endpoints, affected_types);
            }
        }

        // 2. Extract base type name and starting path
        let (root_type, start_path) = parse_start_symbol(start_symbol);
        affected_types.insert(root_type.clone());

        // Queue holds: (current_type, depth, traversal_path)
        let mut queue = VecDeque::new();
        let mut visited_types = HashSet::new();
        visited_types.insert(root_type.clone());

        queue.push_back((root_type.clone(), 0, vec![start_path]));

        while let Some((current_type, depth, path)) = queue.pop_front() {
            // Check operations referencing current_type
            if let Some(ops) = self.operation_dependents.get(&current_type) {
                for (svc, op, dir) in ops {
                    let mut ep_path = path.clone();
                    ep_path.push(format!("{}.{} ({})", svc, op, dir));
                    endpoints.push(ImpactedEndpoint {
                        service: svc.clone(),
                        operation: op.clone(),
                        direction: *dir,
                        depth: depth + 1,
                        path: ep_path,
                        severity,
                        cause: cause.map(|c| c.to_string()),
                    });
                }
            }

            // Transitive propagation: check types referencing current_type
            if let Some(parents) = self.type_dependents.get(&current_type) {
                for (parent_type, field_name) in parents {
                    affected_types.insert(parent_type.clone());
                    if visited_types.insert(parent_type.clone()) {
                        let mut next_path = path.clone();
                        next_path.push(format!("{}.{}", parent_type, field_name));
                        queue.push_back((parent_type.clone(), depth + 1, next_path));
                    }
                }
            }
        }

        (endpoints, affected_types)
    }
}

fn parse_start_symbol(symbol: &str) -> (String, String) {
    let clean = symbol.strip_prefix("types.").unwrap_or(symbol);
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.is_empty() {
        (clean.to_string(), clean.to_string())
    } else {
        (parts[0].to_string(), clean.to_string())
    }
}

pub fn analyze_impact(old_contract: &Contract, new_contract: &Contract) -> ImpactReport {
    analyze_diff_impact(old_contract, new_contract, None)
}

pub fn analyze_target_impact(contract: &Contract, target: &str) -> ImpactReport {
    let graph = ContractGraph::build(contract);
    let (endpoints, affected_types) = graph.trace_impact(
        target,
        ChangeSeverity::Safe,
        Some(&format!("Target symbol analysis: {}", target)),
    );

    let mut report = build_report_from_endpoints(endpoints, affected_types);
    if report.is_empty() && graph.declared_types.contains(target) {
        report.affected_types.push(target.to_string());
        report.total_affected_types = 1;
    }
    report
}

pub fn analyze_diff_impact(
    old_contract: &Contract,
    new_contract: &Contract,
    filter_target: Option<&str>,
) -> ImpactReport {
    let diff_report = diff_with_direction(old_contract, new_contract, DiffDirection::Both);
    let graph = ContractGraph::build_combined(old_contract, new_contract);

    let mut all_endpoints: HashMap<(String, String), ImpactedEndpoint> = HashMap::new();
    let mut all_affected_types: HashSet<String> = HashSet::new();

    for change in &diff_report.changes {
        if let Some(target) = filter_target {
            let matches_path = change.path.contains(target) || change.message.contains(target);
            if !matches_path {
                continue;
            }
        }

        let (endpoints, affected_types) =
            graph.trace_impact(&change.path, change.severity, Some(&change.message));

        all_affected_types.extend(affected_types);

        for ep in endpoints {
            let key = (ep.service.clone(), ep.operation.clone());
            if let Some(existing) = all_endpoints.get_mut(&key) {
                // Keep highest severity; if equal, shortest depth
                if ep.severity > existing.severity
                    || (ep.severity == existing.severity && ep.depth < existing.depth)
                {
                    *existing = ep;
                }
            } else {
                all_endpoints.insert(key, ep);
            }
        }
    }

    let mut endpoints_vec: Vec<ImpactedEndpoint> = all_endpoints.into_values().collect();
    endpoints_vec.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.service.cmp(&b.service))
            .then_with(|| a.operation.cmp(&b.operation))
    });

    build_report_from_endpoints(endpoints_vec, all_affected_types)
}

fn build_report_from_endpoints(
    mut endpoints: Vec<ImpactedEndpoint>,
    affected_types_set: HashSet<String>,
) -> ImpactReport {
    let mut affected_types: Vec<String> = affected_types_set.into_iter().collect();
    affected_types.sort();

    let mut services = HashSet::new();
    let mut highest_severity: Option<ChangeSeverity> = None;
    let mut score = 0;

    for ep in &endpoints {
        services.insert(ep.service.clone());
        match ep.severity {
            ChangeSeverity::Breaking => score += 10,
            ChangeSeverity::Warning => score += 3,
            ChangeSeverity::Safe => score += 1,
        }
        highest_severity = match highest_severity {
            None => Some(ep.severity),
            Some(curr) => Some(curr.max(ep.severity)),
        };
    }

    score += affected_types.len();

    endpoints.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.service.cmp(&b.service))
            .then_with(|| a.operation.cmp(&b.operation))
    });

    ImpactReport {
        blast_radius_score: score,
        highest_severity,
        total_impacted_operations: endpoints.len(),
        total_impacted_services: services.len(),
        total_affected_types: affected_types.len(),
        impacted_endpoints: endpoints,
        affected_types,
    }
}
