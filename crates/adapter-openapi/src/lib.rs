use std::collections::HashSet;

use ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeDefinition,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpenApiError {
    #[error("Failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid OpenAPI document: {0}")]
    InvalidDocument(String),
}

pub fn from_openapi_json(json_str: &str) -> Result<Contract, OpenApiError> {
    let v: Value = serde_json::from_str(json_str)?;

    let title = v["info"]["title"]
        .as_str()
        .unwrap_or("OpenApiContract")
        .replace(' ', "");

    let version = v["info"]["version"].as_str().unwrap_or("1.0.0").to_string();
    let description = v["info"]["description"].as_str().map(|s| s.to_string());

    let mut types = Vec::new();

    if let Some(schemas) = v["components"]["schemas"].as_object() {
        for (schema_name, schema_val) in schemas {
            if let Some(enum_vals) = schema_val["enum"].as_array() {
                let variants = enum_vals
                    .iter()
                    .filter_map(|e| e.as_str())
                    .map(|name| EnumVariant {
                        name: sanitize_ident(name),
                        description: None,
                    })
                    .collect();

                types.push(TypeDefinition::Enum(EnumType {
                    name: schema_name.clone(),
                    variants,
                    description: schema_val["description"].as_str().map(|s| s.to_string()),
                }));
            } else {
                let required_fields: HashSet<&str> = schema_val["required"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();

                let mut fields = Vec::new();
                if let Some(props) = schema_val["properties"].as_object() {
                    for (prop_name, prop_val) in props {
                        let is_required = required_fields.contains(prop_name.as_str());
                        let parsed_type = parse_openapi_schema_type(prop_val);

                        let field_type = if is_required {
                            parsed_type
                        } else {
                            match parsed_type {
                                Type::Optional(_) => parsed_type,
                                other => Type::Optional(Box::new(other)),
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
                    name: schema_name.clone(),
                    fields,
                    description: schema_val["description"].as_str().map(|s| s.to_string()),
                }));
            }
        }
    }

    let mut operations = Vec::new();
    if let Some(paths) = v["paths"].as_object() {
        for (path_str, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, op_val) in methods_obj {
                    let op_name = if let Some(op_id) = op_val["operationId"].as_str() {
                        op_id.to_string()
                    } else {
                        format!("{}{}", method, sanitize_path(path_str))
                    };

                    let input = op_val["requestBody"]["content"]["application/json"]["schema"]
                        .as_object()
                        .map(|schema| parse_openapi_schema_type(&Value::Object(schema.clone())));

                    let output =
                        op_val["responses"]["200"]["content"]["application/json"]["schema"]
                            .as_object()
                            .or_else(|| {
                                op_val["responses"]["201"]["content"]["application/json"]["schema"]
                                    .as_object()
                            })
                            .map(|schema| {
                                parse_openapi_schema_type(&Value::Object(schema.clone()))
                            });

                    operations.push(Operation {
                        name: op_name,
                        input,
                        output,
                        description: op_val["summary"]
                            .as_str()
                            .or_else(|| op_val["description"].as_str())
                            .map(|s| s.to_string()),
                    });
                }
            }
        }
    }

    let services = if operations.is_empty() {
        Vec::new()
    } else {
        vec![Service {
            name: format!("{}Api", title),
            operations,
            description: Some("API service operations extracted from OpenAPI".to_string()),
        }]
    };

    Ok(Contract {
        name: title,
        version,
        description,
        types,
        services,
    })
}

fn parse_openapi_schema_type(val: &Value) -> Type {
    if let Some(ref_str) = val["$ref"].as_str() {
        let name = ref_str.rsplit('/').next().unwrap_or(ref_str);
        return Type::Reference(name.to_string());
    }

    let type_str = val["type"].as_str().unwrap_or("object");
    match type_str {
        "string" => Type::String,
        "integer" => Type::Int64,
        "number" => Type::Float64,
        "boolean" => Type::Bool,
        "array" => {
            let item_schema = &val["items"];
            let item_type = parse_openapi_schema_type(item_schema);
            Type::List(Box::new(item_type))
        }
        _ => Type::String,
    }
}

fn sanitize_ident(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "Unknown".to_string()
    } else {
        out
    }
}

fn sanitize_path(s: &str) -> String {
    s.split('/')
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let clean = seg.trim_matches(|c| c == '{' || c == '}');
            let mut c = clean.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}
