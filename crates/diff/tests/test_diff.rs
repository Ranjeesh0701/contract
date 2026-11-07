use diff::{ChangeSeverity, diff};
use ir::{Contract, EnumType, EnumVariant, Field, Record, Type, TypeDefinition};

fn create_base_contract() -> Contract {
    Contract {
        name: "TestContract".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        types: vec![
            TypeDefinition::Record(Record {
                name: "User".to_string(),
                fields: vec![
                    Field {
                        name: "id".to_string(),
                        field_type: Type::Int64,
                        description: None,
                    },
                    Field {
                        name: "email".to_string(),
                        field_type: Type::String,
                        description: None,
                    },
                ],
                description: None,
            }),
            TypeDefinition::Enum(EnumType {
                name: "Status".to_string(),
                variants: vec![
                    EnumVariant {
                        name: "Active".to_string(),
                        description: None,
                    },
                    EnumVariant {
                        name: "Pending".to_string(),
                        description: None,
                    },
                ],
                description: None,
            }),
        ],
        services: vec![],
    }
}

#[test]
fn test_no_changes() {
    let old = create_base_contract();
    let new = create_base_contract();

    let report = diff(&old, &new);
    assert!(report.changes.is_empty());
    assert!(report.is_compatible());
}

#[test]
fn test_safe_changes_added_optional_field_and_variant() {
    let old = create_base_contract();
    let mut new = create_base_contract();

    if let TypeDefinition::Record(ref mut rec) = new.types[0] {
        rec.fields.push(Field {
            name: "avatar_url".to_string(),
            field_type: Type::Optional(Box::new(Type::String)),
            description: None,
        });
    }

    if let TypeDefinition::Enum(ref mut enm) = new.types[1] {
        enm.variants.push(EnumVariant {
            name: "Suspended".to_string(),
            description: None,
        });
    }

    let report = diff(&old, &new);
    assert!(report.is_compatible());
    assert_eq!(report.changes.len(), 2);
    assert!(
        report
            .changes
            .iter()
            .all(|c| c.severity == ChangeSeverity::Safe)
    );
}

#[test]
fn test_breaking_changes_removed_field_and_changed_type() {
    let old = create_base_contract();
    let mut new = create_base_contract();

    if let TypeDefinition::Record(ref mut rec) = new.types[0] {
        rec.fields.remove(1);
        rec.fields[0].field_type = Type::String;
    }

    let report = diff(&old, &new);
    assert!(!report.is_compatible());
    let breaking = report.breaking_changes();
    assert_eq!(breaking.len(), 2);
}

#[test]
fn test_breaking_change_added_required_field() {
    let old = create_base_contract();
    let mut new = create_base_contract();

    if let TypeDefinition::Record(ref mut rec) = new.types[0] {
        rec.fields.push(Field {
            name: "phone".to_string(),
            field_type: Type::String,
            description: None,
        });
    }

    let report = diff(&old, &new);
    assert!(!report.is_compatible());
    assert_eq!(report.breaking_changes().len(), 1);
}
