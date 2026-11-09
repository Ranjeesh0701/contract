use std::fmt::Write;

use crate::ir::{Contract, EnumType, Record, Service, Type, TypeAlias, TypeDefinition};

pub fn to_dsl(contract: &Contract) -> String {
    let mut out = String::with_capacity(1024);

    for import_path in &contract.imports {
        let _ = writeln!(out, "import \"{}\";", import_path);
    }
    if !contract.imports.is_empty() {
        out.push('\n');
    }

    let _ = writeln!(out, "contract {} {{", contract.name);
    let _ = writeln!(out, "    version: \"{}\"", contract.version);
    if let Some(ref desc) = contract.description {
        let _ = writeln!(out, "    description: \"{}\"", desc);
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
    if let Some(ref desc) = enm.description {
        for line in desc.lines() {
            let _ = writeln!(out, "/// {}", line);
        }
    }
    let _ = writeln!(out, "enum {} {{", enm.name);
    for (i, v) in enm.variants.iter().enumerate() {
        if let Some(ref desc) = v.description {
            for line in desc.lines() {
                let _ = writeln!(out, "    /// {}", line);
            }
        }
        let comma = if i + 1 < enm.variants.len() { "," } else { "" };
        let _ = writeln!(out, "    {}{}", v.name, comma);
    }
    out.push_str("}\n");
}

fn emit_record(rec: &Record, out: &mut String) {
    if let Some(ref desc) = rec.description {
        for line in desc.lines() {
            let _ = writeln!(out, "/// {}", line);
        }
    }
    let _ = writeln!(out, "model {} {{", rec.name);
    for f in &rec.fields {
        if let Some(ref desc) = f.description {
            for line in desc.lines() {
                let _ = writeln!(out, "    /// {}", line);
            }
        }
        let _ = writeln!(out, "    {}: {}", f.name, format_type(&f.field_type));
    }
    out.push_str("}\n");
}

fn emit_alias(alias: &TypeAlias, out: &mut String) {
    if let Some(ref desc) = alias.description {
        for line in desc.lines() {
            let _ = writeln!(out, "/// {}", line);
        }
    }
    let _ = writeln!(out, "type {}: {}", alias.name, format_type(&alias.target));
}

fn emit_service(svc: &Service, out: &mut String) {
    if let Some(ref desc) = svc.description {
        for line in desc.lines() {
            let _ = writeln!(out, "/// {}", line);
        }
    }
    let _ = writeln!(out, "service {} {{", svc.name);
    for op in &svc.operations {
        if let Some(ref desc) = op.description {
            for line in desc.lines() {
                let _ = writeln!(out, "    /// {}", line);
            }
        }
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

        let _ = writeln!(out, "    {}({}): {}", op.name, input_str, output_str);
    }
    out.push_str("}\n");
}

fn format_type(ty: &Type) -> String {
    ty.to_string()
}
