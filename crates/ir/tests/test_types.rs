use ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeAlias,
    TypeDefinition, ValidationError,
};

#[test]
fn test_nested_type() {
    let nested_type = Type::Optional(Box::new(Type::List(Box::new(Type::String))));
    assert_eq!(
        nested_type,
        Type::Optional(Box::new(Type::List(Box::new(Type::String))))
    );
}

#[test]
fn test_record_definition() {
    let record = Record {
        name: "User".to_string(),
        fields: vec![
            Field {
                name: "id".to_string(),
                field_type: Type::Int64,
                description: Some("The ID of the user.".to_string()),
            },
            Field {
                name: "email".to_string(),
                field_type: Type::Optional(Box::new(Type::String)),
                description: None,
            },
        ],
        description: Some("A user record.".to_string()),
    };

    assert_eq!(record.name, "User");
    assert_eq!(record.fields.len(), 2);
    assert_eq!(record.fields[0].name, "id");
    assert_eq!(record.fields[0].field_type, Type::Int64);
    assert_eq!(record.fields[1].name, "email");
    assert_eq!(
        record.fields[1].field_type,
        Type::Optional(Box::new(Type::String))
    );
    assert_eq!(record, record.clone());
}

#[test]
fn test_enum_definition() {
    let enum_type = EnumType {
        name: "Status".to_string(),
        variants: vec![
            EnumVariant {
                name: "Active".to_string(),
                description: Some("The item is active.".to_string()),
            },
            EnumVariant {
                name: "Pending".to_string(),
                description: Some("The item is pending.".to_string()),
            },
        ],
        description: Some("A status enum.".to_string()),
    };

    assert_eq!(enum_type.name, "Status");
    assert_eq!(enum_type.variants.len(), 2);
    assert_eq!(enum_type.variants[0].name, "Active");
    assert_eq!(enum_type.variants[1].name, "Pending");
    assert_eq!(enum_type, enum_type.clone());
}

#[test]
fn test_valid_contract_with_forward_reference_and_service() {
    // User is defined BEFORE Status (forward reference!)
    let contract = Contract {
        name: "UserContract".to_string(),
        version: "1.0.0".to_string(),
        description: Some("A complete contract".to_string()),
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
                        name: "status".to_string(),
                        field_type: Type::Reference("Status".to_string()),
                        description: None,
                    },
                    Field {
                        name: "metadata".to_string(),
                        field_type: Type::Map {
                            key: Box::new(Type::String),
                            value: Box::new(Type::String),
                        },
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
                        name: "Inactive".to_string(),
                        description: None,
                    },
                ],
                description: None,
            }),
            TypeDefinition::Alias(TypeAlias {
                name: "UserId".to_string(),
                target: Type::Int64,
                description: None,
            }),
        ],
        services: vec![Service {
            name: "UserService".to_string(),
            description: Some("Operations on users".to_string()),
            operations: vec![Operation {
                name: "GetUser".to_string(),
                input: Some(Type::Reference("UserId".to_string())),
                output: Some(Type::Reference("User".to_string())),
                description: None,
            }],
        }],
    };

    assert!(contract.validate().is_ok());

    // Verify JSON serialization round-trip
    let json = serde_json::to_string_pretty(&contract).expect("Failed to serialize contract");
    let deserialized: Contract =
        serde_json::from_str(&json).expect("Failed to deserialize contract");
    assert_eq!(contract, deserialized);
}

#[test]
fn test_contract_unresolved_reference() {
    let contract = Contract {
        name: "BrokenContract".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        types: vec![TypeDefinition::Record(Record {
            name: "User".to_string(),
            fields: vec![Field {
                name: "address".to_string(),
                field_type: Type::Optional(Box::new(Type::Reference("Address".to_string()))),
                description: None,
            }],
            description: None,
        })],
        services: vec![],
    };

    let result = contract.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(
        errors,
        vec![ValidationError::UnresolvedReference {
            referrer: "User".to_string(),
            reference: "Address".to_string(),
        }]
    );
}

#[test]
fn test_contract_duplicate_variant() {
    let contract = Contract {
        name: "DuplicateVariantContract".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        types: vec![TypeDefinition::Enum(EnumType {
            name: "Status".to_string(),
            variants: vec![
                EnumVariant {
                    name: "Active".to_string(),
                    description: None,
                },
                EnumVariant {
                    name: "Active".to_string(),
                    description: None,
                },
            ],
            description: None,
        })],
        services: vec![],
    };

    let result = contract.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(
        errors,
        vec![ValidationError::DuplicateVariant {
            r#enum: "Status".to_string(),
            variant: "Active".to_string(),
        }]
    );
}
