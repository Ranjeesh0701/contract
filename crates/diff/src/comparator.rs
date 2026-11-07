use std::collections::{HashMap, HashSet};

use ir::{Contract, Field, Operation, Service, Type, TypeDefinition};

use crate::change::{Change, DiffReport};

pub fn diff(old: &Contract, new: &Contract) -> DiffReport {
    let mut changes = Vec::new();

    let new_types: HashMap<&str, &TypeDefinition> =
        new.types.iter().map(|t| (t.name(), t)).collect();
    let old_types: HashMap<&str, &TypeDefinition> =
        old.types.iter().map(|t| (t.name(), t)).collect();

    for old_type in &old.types {
        let type_name = old_type.name();
        match new_types.get(type_name) {
            None => {
                changes.push(Change::breaking(
                    format!("types.{type_name}"),
                    format!("Type '{type_name}' was removed"),
                ));
            }
            Some(new_type) => {
                diff_type_definition(old_type, new_type, &mut changes);
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

    let new_services: HashMap<&str, &Service> =
        new.services.iter().map(|s| (s.name.as_str(), s)).collect();
    let old_services: HashMap<&str, &Service> =
        old.services.iter().map(|s| (s.name.as_str(), s)).collect();

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

    DiffReport { changes }
}

fn diff_type_definition(
    old_def: &TypeDefinition,
    new_def: &TypeDefinition,
    changes: &mut Vec<Change>,
) {
    let type_name = old_def.name();

    match (old_def, new_def) {
        (TypeDefinition::Record(old_rec), TypeDefinition::Record(new_rec)) => {
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
                    None => {
                        changes.push(Change::breaking(
                            format!("types.{type_name}.fields.{field_name}"),
                            format!("Field '{field_name}' was removed from record '{type_name}'"),
                        ));
                    }
                    Some(new_field) => {
                        if old_field.field_type != new_field.field_type {
                            changes.push(Change::breaking(
                                format!("types.{type_name}.fields.{field_name}"),
                                format!(
                                    "Field '{field_name}' changed type from {:?} to {:?}",
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

        (TypeDefinition::Enum(old_enum), TypeDefinition::Enum(new_enum)) => {
            let old_variants: HashSet<&str> =
                old_enum.variants.iter().map(|v| v.name.as_str()).collect();
            let new_variants: HashSet<&str> =
                new_enum.variants.iter().map(|v| v.name.as_str()).collect();

            for old_var in &old_enum.variants {
                if !new_variants.contains(old_var.name.as_str()) {
                    changes.push(Change::breaking(
                        format!("types.{type_name}.variants.{}", old_var.name),
                        format!(
                            "Variant '{}' was removed from enum '{type_name}'",
                            old_var.name
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
            if old_alias.target != new_alias.target {
                changes.push(Change::breaking(
                    format!("types.{type_name}"),
                    format!(
                        "Alias '{type_name}' changed target from {:?} to {:?}",
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
                if old_op.input != new_op.input {
                    changes.push(Change::breaking(
                        format!("services.{svc_name}.operations.{op_name}.input"),
                        format!(
                            "Input for operation '{op_name}' changed from {:?} to {:?}",
                            old_op.input, new_op.input
                        ),
                    ));
                }
                if old_op.output != new_op.output {
                    changes.push(Change::breaking(
                        format!("services.{svc_name}.operations.{op_name}.output"),
                        format!(
                            "Output for operation '{op_name}' changed from {:?} to {:?}",
                            old_op.output, new_op.output
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
