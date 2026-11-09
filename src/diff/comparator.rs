use std::collections::{HashMap, HashSet, VecDeque};

use crate::ir::{Contract, EnumVariant, Field, Operation, Service, Type, TypeDefinition};

use crate::diff::change::{Change, DiffDirection, DiffReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRole {
    InputOnly,
    OutputOnly,
    Bidirectional,
    Unreferenced,
}

pub fn collect_type_roles(contract: &Contract) -> HashMap<String, TypeRole> {
    let mut input_types: HashSet<String> = HashSet::new();
    let mut output_types: HashSet<String> = HashSet::new();

    // 1. Collect direct references from service operations
    for svc in &contract.services {
        for op in &svc.operations {
            if let Some(ref in_ty) = op.input {
                let mut refs = Vec::new();
                in_ty.collect_references(&mut refs);
                input_types.extend(refs);
            }
            if let Some(ref out_ty) = op.output {
                let mut refs = Vec::new();
                out_ty.collect_references(&mut refs);
                output_types.extend(refs);
            }
        }
    }

    // 2. Precompute reference dependencies once and propagate via worklists
    let mut dependencies: HashMap<&str, Vec<String>> =
        HashMap::with_capacity(contract.types.len());
    for type_def in &contract.types {
        let mut refs = Vec::new();
        match type_def {
            TypeDefinition::Record(r) => {
                for f in &r.fields {
                    f.field_type.collect_references(&mut refs);
                }
            }
            TypeDefinition::Alias(a) => {
                a.target.collect_references(&mut refs);
            }
            TypeDefinition::Enum(_) => {}
        }
        dependencies.insert(type_def.name(), refs);
    }

    let mut in_queue: VecDeque<String> = input_types.iter().cloned().collect();
    while let Some(curr) = in_queue.pop_front() {
        if let Some(deps) = dependencies.get(curr.as_str()) {
            for dep in deps {
                if input_types.insert(dep.clone()) {
                    in_queue.push_back(dep.clone());
                }
            }
        }
    }

    let mut out_queue: VecDeque<String> = output_types.iter().cloned().collect();
    while let Some(curr) = out_queue.pop_front() {
        if let Some(deps) = dependencies.get(curr.as_str()) {
            for dep in deps {
                if output_types.insert(dep.clone()) {
                    out_queue.push_back(dep.clone());
                }
            }
        }
    }

    // 3. Classify roles for all defined types
    let mut roles = HashMap::with_capacity(contract.types.len());
    for type_def in &contract.types {
        let name = type_def.name().to_string();
        let in_scope = input_types.contains(&name);
        let out_scope = output_types.contains(&name);
        let role = match (in_scope, out_scope) {
            (true, false) => TypeRole::InputOnly,
            (false, true) => TypeRole::OutputOnly,
            (true, true) => TypeRole::Bidirectional,
            (false, false) => TypeRole::Unreferenced,
        };
        roles.insert(name, role);
    }

    roles
}

fn resolve_combined_role(
    type_name: &str,
    old_roles: &HashMap<String, TypeRole>,
    new_roles: &HashMap<String, TypeRole>,
) -> TypeRole {
    let r_old = old_roles
        .get(type_name)
        .copied()
        .unwrap_or(TypeRole::Unreferenced);
    let r_new = new_roles
        .get(type_name)
        .copied()
        .unwrap_or(TypeRole::Unreferenced);

    match (r_old, r_new) {
        (TypeRole::Bidirectional, _) | (_, TypeRole::Bidirectional) => TypeRole::Bidirectional,
        (TypeRole::InputOnly, TypeRole::OutputOnly)
        | (TypeRole::OutputOnly, TypeRole::InputOnly) => TypeRole::Bidirectional,
        (TypeRole::InputOnly, _) | (_, TypeRole::InputOnly) => TypeRole::InputOnly,
        (TypeRole::OutputOnly, _) | (_, TypeRole::OutputOnly) => TypeRole::OutputOnly,
        (TypeRole::Unreferenced, TypeRole::Unreferenced) => TypeRole::Unreferenced,
    }
}

pub fn diff(old: &Contract, new: &Contract) -> DiffReport {
    diff_with_direction(old, new, DiffDirection::Both)
}

pub fn diff_with_direction(old: &Contract, new: &Contract, direction: DiffDirection) -> DiffReport {
    let old_roles = collect_type_roles(old);
    let new_roles = collect_type_roles(new);

    let mut changes = Vec::new();

    let mut new_types: HashMap<&str, &TypeDefinition> = HashMap::with_capacity(new.types.len());
    for t in &new.types {
        new_types.insert(t.name(), t);
    }
    let mut old_types: HashMap<&str, &TypeDefinition> = HashMap::with_capacity(old.types.len());
    for t in &old.types {
        old_types.insert(t.name(), t);
    }

    for old_type in &old.types {
        let type_name = old_type.name();
        let role = resolve_combined_role(type_name, &old_roles, &new_roles);

        match new_types.get(type_name) {
            None => {
                changes.push(Change::breaking(
                    format!("types.{type_name}"),
                    format!("Type '{type_name}' was removed"),
                ));
            }
            Some(new_type) => {
                diff_type_definition(old_type, new_type, role, &mut changes);
            }
        }
    }

    for new_type in &new.types {
        let type_name = new_type.name();
        if !old_types.contains_key(type_name) {
            changes.push(Change::safe(
                format!("types.{type_name}"),
                format!("Added new type '{type_name}'"),
            ));
        }
    }

    let mut new_services: HashMap<&str, &Service> = HashMap::with_capacity(new.services.len());
    for s in &new.services {
        new_services.insert(s.name.as_str(), s);
    }
    let mut old_services: HashMap<&str, &Service> = HashMap::with_capacity(old.services.len());
    for s in &old.services {
        old_services.insert(s.name.as_str(), s);
    }

    for old_svc in &old.services {
        match new_services.get(old_svc.name.as_str()) {
            None => {
                changes.push(Change::breaking(
                    format!("services.{}", old_svc.name),
                    format!("Service '{}' was removed", old_svc.name),
                ));
            }
            Some(new_svc) => {
                diff_service(old_svc, new_svc, &mut changes);
            }
        }
    }

    for new_svc in &new.services {
        if !old_services.contains_key(new_svc.name.as_str()) {
            changes.push(Change::safe(
                format!("services.{}", new_svc.name),
                format!("Added new service '{}'", new_svc.name),
            ));
        }
    }

    let filtered_changes = match direction {
        DiffDirection::Both => changes,
        DiffDirection::Consumer => changes
            .into_iter()
            .filter(|c| !c.message.contains("(clients may still send it)"))
            .collect(),
        DiffDirection::Producer => changes
            .into_iter()
            .filter(|c| !c.message.contains("response record"))
            .collect(),
    };

    DiffReport {
        changes: filtered_changes,
    }
}

fn diff_type_definition(
    old_def: &TypeDefinition,
    new_def: &TypeDefinition,
    role: TypeRole,
    changes: &mut Vec<Change>,
) {
    let type_name = old_def.name();

    match (old_def, new_def) {
        (TypeDefinition::Record(old_rec), TypeDefinition::Record(new_rec)) => {
            if old_rec.deprecated.is_none() && new_rec.deprecated.is_some() {
                changes.push(Change::warning(
                    format!("types.{type_name}"),
                    format!(
                        "Record '{type_name}' was marked as deprecated: {}",
                        new_rec.deprecated.as_deref().unwrap_or("Deprecated")
                    ),
                ));
            }

            let old_fields: HashMap<&str, &Field> = old_rec
                .fields
                .iter()
                .map(|f| (f.name.as_str(), f))
                .collect();
            let new_fields: HashMap<&str, &Field> = new_rec
                .fields
                .iter()
                .map(|f| (f.name.as_str(), f))
                .collect();

            for old_field in &old_rec.fields {
                let field_name = &old_field.name;
                match new_fields.get(field_name.as_str()) {
                    None => match role {
                        TypeRole::InputOnly => {
                            changes.push(Change::warning(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Field '{field_name}' was removed from request record '{type_name}' (clients may still send it)"
                                ),
                            ));
                        }
                        TypeRole::OutputOnly => {
                            changes.push(Change::breaking(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Field '{field_name}' was removed from response record '{type_name}'"
                                ),
                            ));
                        }
                        TypeRole::Bidirectional | TypeRole::Unreferenced => {
                            changes.push(Change::breaking(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Field '{field_name}' was removed from record '{type_name}'"
                                ),
                            ));
                        }
                    },
                    Some(new_field) => {
                        if old_field.deprecated.is_none() && new_field.deprecated.is_some() {
                            changes.push(Change::warning(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Field '{field_name}' in record '{type_name}' was marked as deprecated: {}",
                                    new_field.deprecated.as_deref().unwrap_or("Deprecated")
                                ),
                            ));
                        }
                        if old_field.field_type != new_field.field_type {
                            changes.push(Change::breaking(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Field '{field_name}' changed type from {} to {}",
                                    old_field.field_type, new_field.field_type
                                ),
                            ));
                        }
                    }
                }
            }

            for new_field in &new_rec.fields {
                let field_name = &new_field.name;
                if !old_fields.contains_key(field_name.as_str()) {
                    match role {
                        TypeRole::OutputOnly => {
                            changes.push(Change::safe(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Added field '{field_name}' to response record '{type_name}'"
                                ),
                            ));
                        }
                        TypeRole::InputOnly => match new_field.field_type {
                            Type::Optional(_) => {
                                changes.push(Change::safe(
                                    format!("types.{type_name}.fields.{field_name}"),
                                    format!(
                                        "Added optional field '{field_name}' to request record '{type_name}'"
                                    ),
                                ));
                            }
                            _ => {
                                changes.push(Change::breaking(
                                    format!("types.{type_name}.fields.{field_name}"),
                                    format!(
                                        "Added required field '{field_name}' to request record '{type_name}' without default"
                                    ),
                                ));
                            }
                        },
                        TypeRole::Bidirectional | TypeRole::Unreferenced => {
                            match new_field.field_type {
                                Type::Optional(_) => {
                                    changes.push(Change::safe(
                                        format!("types.{type_name}.fields.{field_name}"),
                                        format!(
                                            "Added optional field '{field_name}' to record '{type_name}'"
                                        ),
                                    ));
                                }
                                _ => {
                                    changes.push(Change::breaking(
                                        format!("types.{type_name}.fields.{field_name}"),
                                        format!(
                                            "Added required field '{field_name}' to record '{type_name}' without default"
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        (TypeDefinition::Enum(old_enum), TypeDefinition::Enum(new_enum)) => {
            if old_enum.deprecated.is_none() && new_enum.deprecated.is_some() {
                changes.push(Change::warning(
                    format!("types.{type_name}"),
                    format!(
                        "Enum '{type_name}' was marked as deprecated: {}",
                        new_enum.deprecated.as_deref().unwrap_or("Deprecated")
                    ),
                ));
            }

            let old_variants: HashSet<&str> =
                old_enum.variants.iter().map(|v| v.name.as_str()).collect();
            let new_variants: HashSet<&str> =
                new_enum.variants.iter().map(|v| v.name.as_str()).collect();
            let new_var_map: HashMap<&str, &EnumVariant> = new_enum
                .variants
                .iter()
                .map(|v| (v.name.as_str(), v))
                .collect();

            for old_var in &old_enum.variants {
                if !new_variants.contains(old_var.name.as_str()) {
                    changes.push(Change::breaking(
                        format!("types.{type_name}.variants.{}", old_var.name),
                        format!(
                            "Variant '{}' was removed from enum '{type_name}'",
                            old_var.name
                        ),
                    ));
                } else if let Some(new_var) = new_var_map.get(old_var.name.as_str())
                    && old_var.deprecated.is_none()
                    && new_var.deprecated.is_some()
                {
                    changes.push(Change::warning(
                        format!("types.{type_name}.variants.{}", old_var.name),
                        format!(
                            "Variant '{}' in enum '{type_name}' was marked as deprecated: {}",
                            old_var.name,
                            new_var.deprecated.as_deref().unwrap_or("Deprecated")
                        ),
                    ));
                }
            }

            for new_var in &new_enum.variants {
                if !old_variants.contains(new_var.name.as_str()) {
                    changes.push(Change::safe(
                        format!("types.{type_name}.variants.{}", new_var.name),
                        format!("Added variant '{}' to enum '{type_name}'", new_var.name),
                    ));
                }
            }
        }

        (TypeDefinition::Alias(old_alias), TypeDefinition::Alias(new_alias)) => {
            if old_alias.deprecated.is_none() && new_alias.deprecated.is_some() {
                changes.push(Change::warning(
                    format!("types.{type_name}"),
                    format!(
                        "Type alias '{type_name}' was marked as deprecated: {}",
                        new_alias.deprecated.as_deref().unwrap_or("Deprecated")
                    ),
                ));
            }
            if old_alias.target != new_alias.target {
                changes.push(Change::breaking(
                    format!("types.{type_name}"),
                    format!(
                        "Alias '{type_name}' changed target from {} to {}",
                        old_alias.target, new_alias.target
                    ),
                ));
            }
        }

        _ => {
            changes.push(Change::breaking(
                format!("types.{type_name}"),
                format!("Kind of type '{type_name}' changed (e.g. record to enum or alias)"),
            ));
        }
    }
}

fn diff_service(old_svc: &Service, new_svc: &Service, changes: &mut Vec<Change>) {
    let svc_name = &old_svc.name;
    if old_svc.deprecated.is_none() && new_svc.deprecated.is_some() {
        changes.push(Change::warning(
            format!("services.{svc_name}"),
            format!(
                "Service '{svc_name}' was marked as deprecated: {}",
                new_svc.deprecated.as_deref().unwrap_or("Deprecated")
            ),
        ));
    }

    let old_ops: HashMap<&str, &Operation> = old_svc
        .operations
        .iter()
        .map(|o| (o.name.as_str(), o))
        .collect();
    let new_ops: HashMap<&str, &Operation> = new_svc
        .operations
        .iter()
        .map(|o| (o.name.as_str(), o))
        .collect();

    for old_op in &old_svc.operations {
        let op_name = &old_op.name;
        match new_ops.get(op_name.as_str()) {
            None => {
                changes.push(Change::breaking(
                    format!("services.{svc_name}.operations.{op_name}"),
                    format!("Operation '{op_name}' was removed from service '{svc_name}'"),
                ));
            }
            Some(new_op) => {
                if old_op.deprecated.is_none() && new_op.deprecated.is_some() {
                    changes.push(Change::warning(
                        format!("services.{svc_name}.operations.{op_name}"),
                        format!(
                            "Operation '{op_name}' in service '{svc_name}' was marked as deprecated: {}",
                            new_op.deprecated.as_deref().unwrap_or("Deprecated")
                        ),
                    ));
                }
                if old_op.input != new_op.input {
                    let old_in = old_op
                        .input
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "Void".to_string());
                    let new_in = new_op
                        .input
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "Void".to_string());
                    changes.push(Change::breaking(
                        format!("services.{svc_name}.operations.{op_name}.input"),
                        format!(
                            "Input for operation '{op_name}' changed from {old_in} to {new_in}"
                        ),
                    ));
                }
                if old_op.output != new_op.output {
                    let old_out = old_op
                        .output
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "Void".to_string());
                    let new_out = new_op
                        .output
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "Void".to_string());
                    changes.push(Change::breaking(
                        format!("services.{svc_name}.operations.{op_name}.output"),
                        format!(
                            "Output for operation '{op_name}' changed from {old_out} to {new_out}"
                        ),
                    ));
                }
            }
        }
    }

    for new_op in &new_svc.operations {
        let op_name = &new_op.name;
        if !old_ops.contains_key(op_name.as_str()) {
            changes.push(Change::safe(
                format!("services.{svc_name}.operations.{op_name}"),
                format!("Added operation '{op_name}' to service '{svc_name}'"),
            ));
        }
    }
}
