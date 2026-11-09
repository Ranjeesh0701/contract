use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Type {
    Bool,
    String,
    Int32,
    Int64,
    Float64,
    Bytes,
    List(Box<Type>),
    Map { key: Box<Type>, value: Box<Type> },
    Optional(Box<Type>),
    Reference(String),
}

impl Type {
    pub fn collect_references(&self, refs: &mut Vec<String>) {
        match self {
            Type::Reference(name) => {
                if !name.eq_ignore_ascii_case("void") {
                    refs.push(name.clone());
                }
            }
            Type::List(inner) | Type::Optional(inner) => inner.collect_references(refs),
            Type::Map { key, value } => {
                key.collect_references(refs);
                value.collect_references(refs);
            }
            _ => {}
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Bool => write!(f, "Bool"),
            Type::String => write!(f, "String"),
            Type::Int32 => write!(f, "Int32"),
            Type::Int64 => write!(f, "Int64"),
            Type::Float64 => write!(f, "Float64"),
            Type::Bytes => write!(f, "Bytes"),
            Type::List(inner) => write!(f, "{}[]", inner),
            Type::Map { key, value } => write!(f, "Map<{}, {}>", key, value),
            Type::Optional(inner) => write!(f, "{}?", inner),
            Type::Reference(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumType {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub field_type: Type,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub name: String,
    pub fields: Vec<Field>,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeAlias {
    pub name: String,
    pub target: Type,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type_kind")]
pub enum TypeDefinition {
    Record(Record),
    Enum(EnumType),
    Alias(TypeAlias),
}

impl TypeDefinition {
    pub fn name(&self) -> &str {
        match self {
            TypeDefinition::Record(r) => &r.name,
            TypeDefinition::Enum(e) => &e.name,
            TypeDefinition::Alias(a) => &a.name,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            TypeDefinition::Record(r) => r.description.as_deref(),
            TypeDefinition::Enum(e) => e.description.as_deref(),
            TypeDefinition::Alias(a) => a.description.as_deref(),
        }
    }

    pub fn deprecated(&self) -> Option<&str> {
        match self {
            TypeDefinition::Record(r) => r.deprecated.as_deref(),
            TypeDefinition::Enum(e) => e.deprecated.as_deref(),
            TypeDefinition::Alias(a) => a.deprecated.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    pub input: Option<Type>,
    pub output: Option<Type>,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub operations: Vec<Operation>,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("Duplicate type definition: '{0}'")]
    DuplicateType(String),

    #[error("Duplicate field '{field}' in record '{record}'")]
    DuplicateField { record: String, field: String },

    #[error("Duplicate variant '{variant}' in enum '{enum}'")]
    DuplicateVariant { r#enum: String, variant: String },

    #[error("Unresolved type reference '{reference}' in '{referrer}'")]
    UnresolvedReference { referrer: String, reference: String },

    #[error("Duplicate service definition: '{0}'")]
    DuplicateService(String),

    #[error("Duplicate operation '{operation}' in service '{service}'")]
    DuplicateOperation { service: String, operation: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub types: Vec<TypeDefinition>,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
}

impl Contract {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        let mut declared_types: HashSet<&str> = HashSet::new();

        for type_def in &self.types {
            let name = type_def.name();
            if !declared_types.insert(name) {
                errors.push(ValidationError::DuplicateType(name.to_string()));
            }
        }

        let mut declared_services: HashSet<&str> = HashSet::new();
        for service in &self.services {
            if !declared_services.insert(&service.name) {
                errors.push(ValidationError::DuplicateService(service.name.clone()));
            }
        }

        for type_def in &self.types {
            match type_def {
                TypeDefinition::Record(record) => {
                    let mut field_names: HashSet<&str> = HashSet::new();

                    for field in &record.fields {
                        if !field_names.insert(&field.name) {
                            errors.push(ValidationError::DuplicateField {
                                record: record.name.clone(),
                                field: field.name.clone(),
                            });
                        }

                        let mut refs = Vec::new();
                        field.field_type.collect_references(&mut refs);
                        for ref_name in refs {
                            if !declared_types.contains(ref_name.as_str()) {
                                errors.push(ValidationError::UnresolvedReference {
                                    referrer: record.name.clone(),
                                    reference: ref_name,
                                });
                            }
                        }
                    }
                }
                TypeDefinition::Enum(enum_type) => {
                    let mut variant_names: HashSet<&str> = HashSet::new();
                    for variant in &enum_type.variants {
                        if !variant_names.insert(&variant.name) {
                            errors.push(ValidationError::DuplicateVariant {
                                r#enum: enum_type.name.clone(),
                                variant: variant.name.clone(),
                            });
                        }
                    }
                }
                TypeDefinition::Alias(alias) => {
                    let mut refs = Vec::new();
                    alias.target.collect_references(&mut refs);
                    for ref_name in refs {
                        if !declared_types.contains(ref_name.as_str()) {
                            errors.push(ValidationError::UnresolvedReference {
                                referrer: alias.name.clone(),
                                reference: ref_name,
                            });
                        }
                    }
                }
            }
        }

        for service in &self.services {
            let mut op_names: HashSet<&str> = HashSet::new();
            for op in &service.operations {
                if !op_names.insert(&op.name) {
                    errors.push(ValidationError::DuplicateOperation {
                        service: service.name.clone(),
                        operation: op.name.clone(),
                    });
                }

                if let Some(ref input_ty) = op.input {
                    let mut refs = Vec::new();
                    input_ty.collect_references(&mut refs);
                    for ref_name in refs {
                        if !declared_types.contains(ref_name.as_str()) {
                            errors.push(ValidationError::UnresolvedReference {
                                referrer: format!(
                                    "service '{}' op '{}' input",
                                    service.name, op.name
                                ),
                                reference: ref_name,
                            });
                        }
                    }
                }

                if let Some(ref output_ty) = op.output {
                    let mut refs = Vec::new();
                    output_ty.collect_references(&mut refs);
                    for ref_name in refs {
                        if !declared_types.contains(ref_name.as_str()) {
                            errors.push(ValidationError::UnresolvedReference {
                                referrer: format!(
                                    "service '{}' op '{}' output",
                                    service.name, op.name
                                ),
                                reference: ref_name,
                            });
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
