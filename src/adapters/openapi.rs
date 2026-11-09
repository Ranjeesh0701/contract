use std::collections::HashSet;
use std::path::Path;

use crate::ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeDefinition,
};
use serde_json::Value;
use thiserror::Error;

use crate::adapters::traits::{AdapterError, SchemaAdapter};

#[derive(Debug, Error)]
pub enum OpenApiError {
    #[error("Failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid OpenAPI document: {0}")]
    InvalidDocument(String),
}

pub struct OpenApiAdapter;

impl SchemaAdapter for OpenApiAdapter {
    fn name(&self) -> &'static str {
        "openapi"
    }

    fn supported_extensions(&self) -> &'static [&'static str] {
        &["json", "yaml", "yml"]
    }

    fn can_handle(&self, _path: Option<&Path>, content: &str) -> bool {
        content.contains("\"openapi\"") || content.contains("\"swagger\"")
    }

    fn parse(&self, content: &str) -> Result<Contract, AdapterError> {
        from_openapi_json(content).map_err(|e| AdapterError::Parse(e.to_string()))
    }
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
                        deprecated: None,
                    })
                    .collect();

                types.push(TypeDefinition::Enum(EnumType {
                    name: schema_name.clone(),
                    variants,
                    description: schema_val["description"].as_str().map(|s| s.to_string()),
                    deprecated: schema_val["deprecated"]
                        .as_bool()
                        .filter(|&b| b)
                        .map(|_| "Deprecated in OpenAPI".to_string()),
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
                            deprecated: prop_val["deprecated"]
                                .as_bool()
                                .filter(|&b| b)
                                .map(|_| "Deprecated in OpenAPI".to_string()),
                        });
                    }
                }

                types.push(TypeDefinition::Record(Record {
                    name: schema_name.clone(),
                    fields,
                    description: schema_val["description"].as_str().map(|s| s.to_string()),
                    deprecated: schema_val["deprecated"]
                        .as_bool()
                        .filter(|&b| b)
                        .map(|_| "Deprecated in OpenAPI".to_string()),
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
                        deprecated: op_val["deprecated"]
                            .as_bool()
                            .filter(|&b| b)
                            .map(|_| "Deprecated in OpenAPI".to_string()),
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
            deprecated: None,
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

pub fn to_openapi_json(contract: &Contract) -> Result<String, OpenApiError> {
    let mut root = serde_json::Map::new();
    root.insert("openapi".to_string(), Value::String("3.1.0".to_string()));

    let mut info = serde_json::Map::new();
    info.insert("title".to_string(), Value::String(contract.name.clone()));
    info.insert(
        "version".to_string(),
        Value::String(contract.version.clone()),
    );
    if let Some(ref desc) = contract.description {
        info.insert("description".to_string(), Value::String(desc.clone()));
    }
    root.insert("info".to_string(), Value::Object(info));

    // Components & Schemas
    let mut schemas = serde_json::Map::new();
    for type_def in &contract.types {
        match type_def {
            TypeDefinition::Record(rec) => {
                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                if let Some(ref desc) = rec.description {
                    schema.insert("description".to_string(), Value::String(desc.clone()));
                }
                if rec.deprecated.is_some() {
                    schema.insert("deprecated".to_string(), Value::Bool(true));
                }

                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();

                for field in &rec.fields {
                    let (field_type_val, is_optional) = type_to_openapi_schema(&field.field_type);
                    let mut prop_obj = match field_type_val {
                        Value::Object(map) => map,
                        other => {
                            let mut m = serde_json::Map::new();
                            m.insert("type".to_string(), other);
                            m
                        }
                    };
                    if let Some(ref desc) = field.description {
                        prop_obj.insert("description".to_string(), Value::String(desc.clone()));
                    }
                    if field.deprecated.is_some() {
                        prop_obj.insert("deprecated".to_string(), Value::Bool(true));
                    }
                    if !is_optional {
                        required.push(Value::String(field.name.clone()));
                    }
                    properties.insert(field.name.clone(), Value::Object(prop_obj));
                }

                schema.insert("properties".to_string(), Value::Object(properties));
                if !required.is_empty() {
                    schema.insert("required".to_string(), Value::Array(required));
                }
                schemas.insert(rec.name.clone(), Value::Object(schema));
            }
            TypeDefinition::Enum(enm) => {
                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("string".to_string()));
                if let Some(ref desc) = enm.description {
                    schema.insert("description".to_string(), Value::String(desc.clone()));
                }
                if enm.deprecated.is_some() {
                    schema.insert("deprecated".to_string(), Value::Bool(true));
                }
                let variants: Vec<Value> = enm
                    .variants
                    .iter()
                    .map(|v| Value::String(v.name.clone()))
                    .collect();
                schema.insert("enum".to_string(), Value::Array(variants));
                schemas.insert(enm.name.clone(), Value::Object(schema));
            }
            TypeDefinition::Alias(alias) => {
                let (type_val, _) = type_to_openapi_schema(&alias.target);
                let mut schema = match type_val {
                    Value::Object(map) => map,
                    other => {
                        let mut m = serde_json::Map::new();
                        m.insert("type".to_string(), other);
                        m
                    }
                };
                if let Some(ref desc) = alias.description {
                    schema.insert("description".to_string(), Value::String(desc.clone()));
                }
                if alias.deprecated.is_some() {
                    schema.insert("deprecated".to_string(), Value::Bool(true));
                }
                schemas.insert(alias.name.clone(), Value::Object(schema));
            }
        }
    }

    let mut components = serde_json::Map::new();
    components.insert("schemas".to_string(), Value::Object(schemas));
    root.insert("components".to_string(), Value::Object(components));

    // Paths from services & operations
    let mut paths = serde_json::Map::new();
    for service in &contract.services {
        for op in &service.operations {
            let path_key = format!(
                "/{}/{}",
                to_kebab_case(&service.name),
                to_kebab_case(&op.name)
            );
            let mut operation_obj = serde_json::Map::new();
            operation_obj.insert("operationId".to_string(), Value::String(op.name.clone()));
            if let Some(ref desc) = op.description {
                operation_obj.insert("summary".to_string(), Value::String(desc.clone()));
            }
            if op.deprecated.is_some() {
                operation_obj.insert("deprecated".to_string(), Value::Bool(true));
            }

            if let Some(ref input_ty) = op.input {
                let (schema_val, _) = type_to_openapi_schema(input_ty);
                let mut content = serde_json::Map::new();
                let mut app_json = serde_json::Map::new();
                app_json.insert("schema".to_string(), schema_val);
                content.insert("application/json".to_string(), Value::Object(app_json));

                let mut req_body = serde_json::Map::new();
                req_body.insert("required".to_string(), Value::Bool(true));
                req_body.insert("content".to_string(), Value::Object(content));
                operation_obj.insert("requestBody".to_string(), Value::Object(req_body));
            }

            let mut responses = serde_json::Map::new();
            if let Some(ref output_ty) = op.output {
                let (schema_val, _) = type_to_openapi_schema(output_ty);
                let mut content = serde_json::Map::new();
                let mut app_json = serde_json::Map::new();
                app_json.insert("schema".to_string(), schema_val);
                content.insert("application/json".to_string(), Value::Object(app_json));

                let mut res_200 = serde_json::Map::new();
                res_200.insert(
                    "description".to_string(),
                    Value::String("Successful response".to_string()),
                );
                res_200.insert("content".to_string(), Value::Object(content));
                responses.insert("200".to_string(), Value::Object(res_200));
            } else {
                let mut res_204 = serde_json::Map::new();
                res_204.insert(
                    "description".to_string(),
                    Value::String("No content".to_string()),
                );
                responses.insert("204".to_string(), Value::Object(res_204));
            }
            operation_obj.insert("responses".to_string(), Value::Object(responses));

            let mut path_item = serde_json::Map::new();
            path_item.insert("post".to_string(), Value::Object(operation_obj));
            paths.insert(path_key, Value::Object(path_item));
        }
    }
    root.insert("paths".to_string(), Value::Object(paths));

    let json_str = serde_json::to_string_pretty(&Value::Object(root))?;
    Ok(json_str)
}

fn type_to_openapi_schema(ty: &Type) -> (Value, bool) {
    match ty {
        Type::Bool => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("boolean".to_string()));
            (Value::Object(m), false)
        }
        Type::String => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("string".to_string()));
            (Value::Object(m), false)
        }
        Type::Int32 => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("integer".to_string()));
            m.insert("format".to_string(), Value::String("int32".to_string()));
            (Value::Object(m), false)
        }
        Type::Int64 => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("integer".to_string()));
            m.insert("format".to_string(), Value::String("int64".to_string()));
            (Value::Object(m), false)
        }
        Type::Float64 => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("number".to_string()));
            m.insert("format".to_string(), Value::String("double".to_string()));
            (Value::Object(m), false)
        }
        Type::Bytes => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("string".to_string()));
            m.insert("format".to_string(), Value::String("binary".to_string()));
            (Value::Object(m), false)
        }
        Type::List(inner) => {
            let (inner_val, _) = type_to_openapi_schema(inner);
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("array".to_string()));
            m.insert("items".to_string(), inner_val);
            (Value::Object(m), false)
        }
        Type::Map { value, .. } => {
            let (val_schema, _) = type_to_openapi_schema(value);
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), Value::String("object".to_string()));
            m.insert("additionalProperties".to_string(), val_schema);
            (Value::Object(m), false)
        }
        Type::Optional(inner) => {
            let (val, _) = type_to_openapi_schema(inner);
            (val, true)
        }
        Type::Reference(name) => {
            let mut m = serde_json::Map::new();
            m.insert(
                "$ref".to_string(),
                Value::String(format!("#/components/schemas/{}", name)),
            );
            (Value::Object(m), false)
        }
    }
}

fn to_kebab_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                result.push('-');
            }
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}
