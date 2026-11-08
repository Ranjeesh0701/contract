use std::collections::HashSet;
use std::path::Path;

use ir::{Contract, EnumType, EnumVariant, Field, Record, Type, TypeDefinition};
use serde_json::Value;

use crate::traits::{AdapterError, SchemaAdapter};

pub struct JsonSchemaAdapter;

impl SchemaAdapter for JsonSchemaAdapter {
    fn name(&self) -> &'static str {
        "json_schema"
    }

    fn supported_extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn can_handle(&self, _path: Option<&Path>, content: &str) -> bool {
        let is_openapi = content.contains("\"openapi\"") || content.contains("\"swagger\"");
        if is_openapi {
            return false;
        }

        content.contains("\"$schema\"")
            || (content.contains("\"properties\"") && content.contains("\"type\""))
    }

    fn parse(&self, content: &str) -> Result<Contract, AdapterError> {
        from_json_schema(content)
    }
}

pub fn from_json_schema(json_str: &str) -> Result<Contract, AdapterError> {
    let v: Value = serde_json::from_str(json_str)
        .map_err(|e| AdapterError::Parse(format!("Failed to parse JSON Schema: {}", e)))?;

    let title = v["title"]
        .as_str()
        .unwrap_or("JsonSchemaContract")
        .replace(' ', "");

    let version = v["version"].as_str().unwrap_or("1.0.0").to_string();
    let description = v["description"].as_str().map(|s| s.to_string());

    let mut types = Vec::new();

    // 1. Parse sub-definitions ($defs or definitions)
    let defs_obj = v["$defs"]
        .as_object()
        .or_else(|| v["definitions"].as_object());
    if let Some(defs) = defs_obj {
        for (def_name, def_val) in defs {
            parse_definition(def_name, def_val, &mut types);
        }
    }

    // 2. Parse root schema if it is an object
    if v.get("properties").is_some() || v["type"].as_str() == Some("object") {
        parse_definition(&title, &v, &mut types);
    }

    Ok(Contract {
        name: title,
        version,
        description,
        types,
        services: Vec::new(),
    })
}

fn parse_definition(name: &str, val: &Value, types: &mut Vec<TypeDefinition>) {
    if let Some(enum_vals) = val["enum"].as_array() {
        let variants = enum_vals
            .iter()
            .filter_map(|e| e.as_str())
            .map(|n| EnumVariant {
                name: n.to_string(),
                description: None,
            })
            .collect();

        types.push(TypeDefinition::Enum(EnumType {
            name: name.to_string(),
            variants,
            description: val["description"].as_str().map(|s| s.to_string()),
        }));
        return;
    }

    let required_fields: HashSet<&str> = val["required"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut fields = Vec::new();
    if let Some(props) = val["properties"].as_object() {
        for (prop_name, prop_val) in props {
            let is_required = required_fields.contains(prop_name.as_str());

            // If the property has an inline enum, extract it
            let field_type = if let Some(enum_vals) = prop_val["enum"].as_array() {
                let enum_type_name = format!("{}{}", name, capitalize(prop_name));
                let variants = enum_vals
                    .iter()
                    .filter_map(|e| e.as_str())
                    .map(|n| EnumVariant {
                        name: n.to_string(),
                        description: None,
                    })
                    .collect();

                types.push(TypeDefinition::Enum(EnumType {
                    name: enum_type_name.clone(),
                    variants,
                    description: prop_val["description"].as_str().map(|s| s.to_string()),
                }));
                let ref_type = Type::Reference(enum_type_name);
                if is_required {
                    ref_type
                } else {
                    Type::Optional(Box::new(ref_type))
                }
            } else {
                let base = parse_schema_type(prop_val);
                if is_required {
                    base
                } else {
                    match base {
                        Type::Optional(_) => base,
                        other => Type::Optional(Box::new(other)),
                    }
                }
            };

            fields.push(Field {
                name: prop_name.clone(),
                field_type,
                description: prop_val["description"].as_str().map(|s| s.to_string()),
            });
        }
    }

    types.push(TypeDefinition::Record(Record {
        name: name.to_string(),
        fields,
        description: val["description"].as_str().map(|s| s.to_string()),
    }));
}

fn parse_schema_type(val: &Value) -> Type {
    if let Some(ref_str) = val["$ref"].as_str() {
        let name = ref_str.rsplit('/').next().unwrap_or(ref_str);
        return Type::Reference(name.to_string());
    }

    let type_str = val["type"].as_str().unwrap_or("string");
    match type_str {
        "string" => Type::String,
        "integer" => Type::Int64,
        "number" => Type::Float64,
        "boolean" => Type::Bool,
        "array" => {
            let item_schema = &val["items"];
            let item_type = parse_schema_type(item_schema);
            Type::List(Box::new(item_type))
        }
        "object" => {
            if let Some(additional) = val.get("additionalProperties") {
                let val_type = parse_schema_type(additional);
                Type::Map {
                    key: Box::new(Type::String),
                    value: Box::new(val_type),
                }
            } else {
                Type::Reference("Object".to_string())
            }
        }
        _ => Type::String,
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
