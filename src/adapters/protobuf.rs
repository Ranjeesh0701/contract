use std::path::Path;

use crate::ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeDefinition,
};

use crate::adapters::traits::{AdapterError, SchemaAdapter};

pub struct ProtobufAdapter;

impl SchemaAdapter for ProtobufAdapter {
    fn name(&self) -> &'static str {
        "protobuf"
    }

    fn supported_extensions(&self) -> &'static [&'static str] {
        &["proto"]
    }

    fn can_handle(&self, path: Option<&Path>, content: &str) -> bool {
        if matches!(
            path.and_then(|p| p.extension()).and_then(|s| s.to_str()),
            Some(ext) if ext.eq_ignore_ascii_case("proto")
        ) {
            return true;
        }
        content.contains("syntax = \"proto")
            || (content.contains("message ") && content.contains('{'))
    }

    fn parse(&self, content: &str) -> Result<Contract, AdapterError> {
        parse_proto(content)
    }
}

pub fn from_protobuf(proto_str: &str) -> Result<Contract, AdapterError> {
    parse_proto(proto_str)
}

fn parse_proto(source: &str) -> Result<Contract, AdapterError> {
    let mut parser = ProtoParser::new(source);
    parser.parse_contract()
}

struct ProtoParser<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProtoToken {
    Ident(String),
    StringLit(String),
    IntLit(i64),
    LBrace,
    RBrace,
    LParen,
    RParen,
    Semicolon,
    Equals,
}

impl<'a> ProtoParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.char_indices().peekable(),
        }
    }

    fn tokenize(&mut self) -> Result<Vec<(ProtoToken, Option<String>)>, AdapterError> {
        let mut tokens = Vec::new();
        let mut pending_comment: Option<String> = None;

        while let Some(&(idx, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
                continue;
            }

            // Single line comment '//'
            if ch == '/' {
                let mut lookahead = self.chars.clone();
                lookahead.next();
                if let Some(&(_, '/')) = lookahead.peek() {
                    self.chars.next();
                    self.chars.next();
                    let mut comment_text = String::new();
                    while let Some(&(_, c)) = self.chars.peek() {
                        self.chars.next();
                        if c == '\n' {
                            break;
                        }
                        comment_text.push(c);
                    }
                    let cleaned = comment_text.trim().to_string();
                    pending_comment = Some(cleaned);
                    continue;
                }
            }

            match ch {
                '{' => {
                    self.chars.next();
                    tokens.push((ProtoToken::LBrace, pending_comment.take()));
                }
                '}' => {
                    self.chars.next();
                    tokens.push((ProtoToken::RBrace, pending_comment.take()));
                }
                '(' => {
                    self.chars.next();
                    tokens.push((ProtoToken::LParen, pending_comment.take()));
                }
                ')' => {
                    self.chars.next();
                    tokens.push((ProtoToken::RParen, pending_comment.take()));
                }
                ';' => {
                    self.chars.next();
                    tokens.push((ProtoToken::Semicolon, pending_comment.take()));
                }
                '=' => {
                    self.chars.next();
                    tokens.push((ProtoToken::Equals, pending_comment.take()));
                }
                '"' => {
                    self.chars.next();
                    let start = idx + 1;
                    let mut str_val = String::new();
                    let mut closed = false;
                    while let Some(&(_, c)) = self.chars.peek() {
                        self.chars.next();
                        if c == '"' {
                            closed = true;
                            break;
                        }
                        str_val.push(c);
                    }
                    if !closed {
                        return Err(AdapterError::Parse(format!(
                            "Unterminated string starting at index {}",
                            start
                        )));
                    }
                    tokens.push((ProtoToken::StringLit(str_val), pending_comment.take()));
                }
                _ if ch.is_alphabetic() || ch == '_' => {
                    let mut ident = String::new();
                    while let Some(&(_, c)) = self.chars.peek() {
                        if c.is_alphanumeric() || c == '_' || c == '.' {
                            ident.push(c);
                            self.chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push((ProtoToken::Ident(ident), pending_comment.take()));
                }
                _ if ch.is_ascii_digit() => {
                    let mut num_str = String::new();
                    while let Some(&(_, c)) = self.chars.peek() {
                        if c.is_ascii_digit() {
                            num_str.push(c);
                            self.chars.next();
                        } else {
                            break;
                        }
                    }
                    let num: i64 = num_str.parse().map_err(|e| {
                        AdapterError::Parse(format!("Invalid integer '{}': {}", num_str, e))
                    })?;
                    tokens.push((ProtoToken::IntLit(num), pending_comment.take()));
                }
                other => {
                    self.chars.next();
                    // Ignore punctuation like commas or colons if any
                    if !other.is_ascii_punctuation() {
                        return Err(AdapterError::Parse(format!(
                            "Unexpected character '{}' in proto file",
                            other
                        )));
                    }
                }
            }
        }

        Ok(tokens)
    }

    fn parse_contract(&mut self) -> Result<Contract, AdapterError> {
        let tokens = self.tokenize()?;
        let mut idx = 0;

        let mut package_name = "ProtoContract".to_string();
        let mut types = Vec::new();
        let mut services = Vec::new();

        while idx < tokens.len() {
            let (ref tok, ref comment) = tokens[idx];

            match tok {
                ProtoToken::Ident(s) if s == "syntax" => {
                    idx += 1;
                    if idx < tokens.len() && tokens[idx].0 == ProtoToken::Equals {
                        idx += 1;
                    }
                    if idx < tokens.len() {
                        idx += 1; // skip "proto3"
                    }
                    if idx < tokens.len() && tokens[idx].0 == ProtoToken::Semicolon {
                        idx += 1;
                    }
                }
                ProtoToken::Ident(s) if s == "package" => {
                    idx += 1;
                    if let Some((ProtoToken::Ident(pkg), _)) = tokens.get(idx) {
                        package_name = to_pascal_case(pkg);
                        idx += 1;
                    }
                    if idx < tokens.len() && tokens[idx].0 == ProtoToken::Semicolon {
                        idx += 1;
                    }
                }
                ProtoToken::Ident(s) if s == "enum" => {
                    idx += 1;
                    let enum_type = Self::parse_enum(&tokens, &mut idx, comment.clone())?;
                    types.push(TypeDefinition::Enum(enum_type));
                }
                ProtoToken::Ident(s) if s == "message" => {
                    idx += 1;
                    let record = Self::parse_message(&tokens, &mut idx, comment.clone())?;
                    types.push(TypeDefinition::Record(record));
                }
                ProtoToken::Ident(s) if s == "service" => {
                    idx += 1;
                    let service = Self::parse_service(&tokens, &mut idx, comment.clone())?;
                    services.push(service);
                }
                _ => {
                    idx += 1;
                }
            }
        }

        Ok(Contract {
            name: package_name,
            version: "1.0.0".to_string(),
            description: Some("Protobuf service contract".to_string()),
            types,
            services,
        })
    }

    fn parse_enum(
        tokens: &[(ProtoToken, Option<String>)],
        idx: &mut usize,
        desc: Option<String>,
    ) -> Result<EnumType, AdapterError> {
        let name = match tokens.get(*idx) {
            Some((ProtoToken::Ident(n), _)) => {
                *idx += 1;
                n.clone()
            }
            _ => return Err(AdapterError::Parse("Expected enum name".to_string())),
        };

        if tokens.get(*idx).map(|t| &t.0) != Some(&ProtoToken::LBrace) {
            return Err(AdapterError::Parse(
                "Expected '{' after enum name".to_string(),
            ));
        }
        *idx += 1;

        let mut variants = Vec::new();
        while *idx < tokens.len() && tokens[*idx].0 != ProtoToken::RBrace {
            if let ProtoToken::Ident(ref var_name) = tokens[*idx].0 {
                let var_desc = tokens[*idx].1.clone();
                *idx += 1;
                // Skip '= <number>;'
                if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::Equals) {
                    *idx += 1;
                }
                if let Some((ProtoToken::IntLit(_), _)) = tokens.get(*idx) {
                    *idx += 1;
                }
                if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::Semicolon) {
                    *idx += 1;
                }
                variants.push(EnumVariant {
                    name: var_name.clone(),
                    description: var_desc,
                    deprecated: None,
                });
            } else {
                *idx += 1;
            }
        }

        if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::RBrace) {
            *idx += 1;
        }

        Ok(EnumType {
            name,
            variants,
            description: desc,
            deprecated: None,
        })
    }

    fn parse_message(
        tokens: &[(ProtoToken, Option<String>)],
        idx: &mut usize,
        desc: Option<String>,
    ) -> Result<Record, AdapterError> {
        let name = match tokens.get(*idx) {
            Some((ProtoToken::Ident(n), _)) => {
                *idx += 1;
                n.clone()
            }
            _ => return Err(AdapterError::Parse("Expected message name".to_string())),
        };

        if tokens.get(*idx).map(|t| &t.0) != Some(&ProtoToken::LBrace) {
            return Err(AdapterError::Parse(
                "Expected '{' after message name".to_string(),
            ));
        }
        *idx += 1;

        let mut fields = Vec::new();
        while *idx < tokens.len() && tokens[*idx].0 != ProtoToken::RBrace {
            let field_desc = tokens[*idx].1.clone();

            let mut is_repeated = false;
            let mut is_optional = false;

            if let Some((ProtoToken::Ident(modifier), _)) = tokens.get(*idx) {
                if modifier == "repeated" {
                    is_repeated = true;
                    *idx += 1;
                } else if modifier == "optional" {
                    is_optional = true;
                    *idx += 1;
                }
            }

            let type_str = match tokens.get(*idx) {
                Some((ProtoToken::Ident(t), _)) => {
                    *idx += 1;
                    t.clone()
                }
                _ => {
                    *idx += 1;
                    continue;
                }
            };

            let field_name = match tokens.get(*idx) {
                Some((ProtoToken::Ident(f), _)) => {
                    *idx += 1;
                    f.clone()
                }
                _ => {
                    *idx += 1;
                    continue;
                }
            };

            // Skip '= <number>;'
            if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::Equals) {
                *idx += 1;
            }
            if let Some((ProtoToken::IntLit(_), _)) = tokens.get(*idx) {
                *idx += 1;
            }
            if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::Semicolon) {
                *idx += 1;
            }

            let base_type = proto_type_to_ir(&type_str);
            let field_type = if is_repeated {
                Type::List(Box::new(base_type))
            } else if is_optional {
                Type::Optional(Box::new(base_type))
            } else {
                base_type
            };

            fields.push(Field {
                name: field_name,
                field_type,
                description: field_desc,
                deprecated: None,
            });
        }

        if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::RBrace) {
            *idx += 1;
        }

        Ok(Record {
            name,
            fields,
            description: desc,
            deprecated: None,
        })
    }

    fn parse_service(
        tokens: &[(ProtoToken, Option<String>)],
        idx: &mut usize,
        desc: Option<String>,
    ) -> Result<Service, AdapterError> {
        let name = match tokens.get(*idx) {
            Some((ProtoToken::Ident(n), _)) => {
                *idx += 1;
                n.clone()
            }
            _ => return Err(AdapterError::Parse("Expected service name".to_string())),
        };

        if tokens.get(*idx).map(|t| &t.0) != Some(&ProtoToken::LBrace) {
            return Err(AdapterError::Parse(
                "Expected '{' after service name".to_string(),
            ));
        }
        *idx += 1;

        let mut operations = Vec::new();
        while *idx < tokens.len() && tokens[*idx].0 != ProtoToken::RBrace {
            let op_desc = tokens[*idx].1.clone();
            if matches!(tokens.get(*idx), Some((ProtoToken::Ident(kw), _)) if kw == "rpc") {
                *idx += 1;
                let op_name = match tokens.get(*idx) {
                    Some((ProtoToken::Ident(n), _)) => {
                        *idx += 1;
                        n.clone()
                    }
                    _ => continue,
                };

                // Input (InputType)
                let input_type = if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::LParen) {
                    *idx += 1;
                    let in_name = match tokens.get(*idx) {
                        Some((ProtoToken::Ident(t), _)) => {
                            *idx += 1;
                            t.clone()
                        }
                        _ => "".to_string(),
                    };
                    if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::RParen) {
                        *idx += 1;
                    }
                    if in_name.is_empty() || in_name == "google.protobuf.Empty" {
                        None
                    } else {
                        Some(Type::Reference(in_name))
                    }
                } else {
                    None
                };

                // Skip "returns"
                if matches!(tokens.get(*idx), Some((ProtoToken::Ident(ret_kw), _)) if ret_kw == "returns")
                {
                    *idx += 1;
                }

                // Output (OutputType)
                let output_type = if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::LParen) {
                    *idx += 1;
                    let out_name = match tokens.get(*idx) {
                        Some((ProtoToken::Ident(t), _)) => {
                            *idx += 1;
                            t.clone()
                        }
                        _ => "".to_string(),
                    };
                    if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::RParen) {
                        *idx += 1;
                    }
                    if out_name.is_empty() || out_name == "google.protobuf.Empty" {
                        None
                    } else {
                        Some(Type::Reference(out_name))
                    }
                } else {
                    None
                };

                if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::Semicolon) {
                    *idx += 1;
                }

                operations.push(Operation {
                    name: op_name,
                    input: input_type,
                    output: output_type,
                    description: op_desc,
                    deprecated: None,
                });
                continue;
            }
            *idx += 1;
        }

        if tokens.get(*idx).map(|t| &t.0) == Some(&ProtoToken::RBrace) {
            *idx += 1;
        }

        Ok(Service {
            name,
            operations,
            description: desc,
            deprecated: None,
        })
    }
}

fn proto_type_to_ir(proto_type: &str) -> Type {
    match proto_type {
        "string" => Type::String,
        "int32" | "sint32" | "uint32" => Type::Int32,
        "int64" | "sint64" | "uint64" => Type::Int64,
        "float" | "double" => Type::Float64,
        "bool" => Type::Bool,
        "bytes" => Type::Bytes,
        other => {
            // Strip any package prefix if present, e.g. "store.User" -> "User"
            let name = other.rsplit('.').next().unwrap_or(other);
            Type::Reference(name.to_string())
        }
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut cap_next = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch == '.' {
            cap_next = true;
        } else if cap_next {
            out.extend(ch.to_uppercase());
            cap_next = false;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "ProtoContract".to_string()
    } else {
        out
    }
}
