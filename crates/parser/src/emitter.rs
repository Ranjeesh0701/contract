use ir::{Contract, EnumType, Record, Service, Type, TypeAlias, TypeDefinition};

pub fn to_dsl(contract: &Contract) -> String {
    let mut out = String::new();

    out.push_str(&format!("contract {} {{\n", contract.name));
    out.push_str(&format!("    version: \"{}\"\n", contract.version));
    if let Some(ref desc) = contract.description {
        out.push_str(&format!("    description: \"{}\"\n", desc));
    }
    out.push_str("}\n\n");

    for type_def in &contract.types {
        match type_def {
            TypeDefinition::Enum(enm) => emit_enum(enm, &mut out),
            TypeDefinition::Record(rec) => emit_record(rec, &mut out),
            TypeDefinition::Alias(alias) => emit_alias(alias, &mut out),
        }
        out.push('\n');
    }

    for svc in &contract.services {
        emit_service(svc, &mut out);
        out.push('\n');
    }

    out
}

fn emit_enum(enm: &EnumType, out: &mut String) {
    out.push_str(&format!("enum {} {{\n", enm.name));
    for (i, v) in enm.variants.iter().enumerate() {
        let comma = if i + 1 < enm.variants.len() { "," } else { "" };
        out.push_str(&format!("    {}{}\n", v.name, comma));
    }
    out.push_str("}\n");
}

fn emit_record(rec: &Record, out: &mut String) {
    out.push_str(&format!("model {} {{\n", rec.name));
    for f in &rec.fields {
        out.push_str(&format!("    {}: {}\n", f.name, format_type(&f.field_type)));
    }
    out.push_str("}\n");
}

fn emit_alias(alias: &TypeAlias, out: &mut String) {
    out.push_str(&format!(
        "type {}: {}\n",
        alias.name,
        format_type(&alias.target)
    ));
}

fn emit_service(svc: &Service, out: &mut String) {
    out.push_str(&format!("service {} {{\n", svc.name));
    for op in &svc.operations {
        let input_str = op
            .input
            .as_ref()
            .map(format_type)
            .unwrap_or_else(|| "".to_string());
        let output_str = op
            .output
            .as_ref()
            .map(format_type)
            .unwrap_or_else(|| "Void".to_string());

        out.push_str(&format!("    {}({}): {}\n", op.name, input_str, output_str));
    }
    out.push_str("}\n");
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Bool => "Bool".to_string(),
        Type::String => "String".to_string(),
        Type::Int32 => "Int32".to_string(),
        Type::Int64 => "Int64".to_string(),
        Type::Float64 => "Float64".to_string(),
        Type::Bytes => "Bytes".to_string(),
        Type::List(inner) => format!("{}[]", format_type(inner)),
        Type::Map { key, value } => format!("Map<{}, {}>", format_type(key), format_type(value)),
        Type::Optional(inner) => format!("{}?", format_type(inner)),
        Type::Reference(name) => name.clone(),
    }
}
