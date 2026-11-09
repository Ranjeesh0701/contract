use std::path::Path;

use crate::ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeDefinition,
};

use crate::adapters::traits::{AdapterError, SchemaAdapter};

pub struct GraphQLAdapter;

impl SchemaAdapter for GraphQLAdapter {
    fn name(&self) -> &'static str {
        "graphql"
    }

    fn supported_extensions(&self) -> &'static [&'static str] {
        &["graphql", "gql"]
    }

    fn can_handle(&self, path: Option<&Path>, content: &str) -> bool {
        if matches!(
            path.and_then(|p| p.extension()).and_then(|s| s.to_str()),
            Some(ext) if ext.eq_ignore_ascii_case("graphql") || ext.eq_ignore_ascii_case("gql")
        ) {
            return true;
        }
        content.contains("type Query")
            || content.contains("type Mutation")
            || content.contains("schema {")
            || (content.contains("type ")
                && (content.contains("String!") || content.contains("ID!")))
    }

    fn parse(&self, content: &str) -> Result<Contract, AdapterError> {
        from_graphql(content)
    }
}

pub fn from_graphql(source: &str) -> Result<Contract, AdapterError> {
    let mut parser = GraphQLParser::new(source);
    parser.parse_contract()
}

struct GraphQLParser<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GqlToken {
    Ident(String),
    StringLit(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Bang,
}

impl<'a> GraphQLParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.char_indices().peekable(),
        }
    }

    fn tokenize(&mut self) -> Result<Vec<(GqlToken, Option<String>)>, AdapterError> {
        let mut tokens = Vec::new();
        let mut pending_comment: Option<String> = None;

        while let Some(&(idx, ch)) = self.chars.peek() {
            if ch.is_whitespace() || ch == ',' {
                self.chars.next();
                continue;
            }

            // Single line comment '#'
            if ch == '#' {
                self.chars.next();
                let mut comment_text = String::new();
                while let Some(&(_, c)) = self.chars.peek() {
                    self.chars.next();
                    if c == '\n' {
                        break;
                    }
                    comment_text.push(c);
                }
                pending_comment = Some(comment_text.trim().to_string());
                continue;
            }

            match ch {
                '{' => {
                    self.chars.next();
                    tokens.push((GqlToken::LBrace, pending_comment.take()));
                }
                '}' => {
                    self.chars.next();
                    tokens.push((GqlToken::RBrace, pending_comment.take()));
                }
                '(' => {
                    self.chars.next();
                    tokens.push((GqlToken::LParen, pending_comment.take()));
                }
                ')' => {
                    self.chars.next();
                    tokens.push((GqlToken::RParen, pending_comment.take()));
                }
                '[' => {
                    self.chars.next();
                    tokens.push((GqlToken::LBracket, pending_comment.take()));
                }
                ']' => {
                    self.chars.next();
                    tokens.push((GqlToken::RBracket, pending_comment.take()));
                }
                ':' => {
                    self.chars.next();
                    tokens.push((GqlToken::Colon, pending_comment.take()));
                }
                '!' => {
                    self.chars.next();
                    tokens.push((GqlToken::Bang, pending_comment.take()));
                }
                '"' => {
                    self.chars.next();
                    let start = idx + 1;
                    let mut s = String::new();
                    let mut closed = false;
                    while let Some(&(_, c)) = self.chars.peek() {
                        self.chars.next();
                        if c == '"' {
                            closed = true;
                            break;
                        }
                        s.push(c);
                    }
                    if !closed {
                        return Err(AdapterError::Parse(format!(
                            "Unterminated string in GraphQL SDL at {}",
                            start
                        )));
                    }
                    tokens.push((GqlToken::StringLit(s), pending_comment.take()));
                }
                _ if ch.is_alphabetic() || ch == '_' => {
                    let mut ident = String::new();
                    while let Some(&(_, c)) = self.chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            ident.push(c);
                            self.chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push((GqlToken::Ident(ident), pending_comment.take()));
                }
                other => {
                    self.chars.next();
                    if !other.is_ascii_punctuation() {
                        return Err(AdapterError::Parse(format!(
                            "Unexpected character '{}' in GraphQL SDL",
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

        let mut types = Vec::new();
        let mut query_ops = Vec::new();
        let mut mutation_ops = Vec::new();

        while idx < tokens.len() {
            let (ref tok, ref comment) = tokens[idx];

            match tok {
                GqlToken::Ident(s) if s == "type" => {
                    idx += 1;
                    let type_name = match tokens.get(idx) {
                        Some((GqlToken::Ident(n), _)) => {
                            idx += 1;
                            n.clone()
                        }
                        _ => return Err(AdapterError::Parse("Expected type name".to_string())),
                    };

                    if type_name == "Query" {
                        Self::parse_operations(&tokens, &mut idx, &mut query_ops)?;
                    } else if type_name == "Mutation" {
                        Self::parse_operations(&tokens, &mut idx, &mut mutation_ops)?;
                    } else {
                        let record =
                            Self::parse_type_record(&tokens, &mut idx, type_name, comment.clone())?;
                        types.push(TypeDefinition::Record(record));
                    }
                }
                GqlToken::Ident(s) if s == "input" => {
                    idx += 1;
                    let input_name = match tokens.get(idx) {
                        Some((GqlToken::Ident(n), _)) => {
                            idx += 1;
                            n.clone()
                        }
                        _ => return Err(AdapterError::Parse("Expected input name".to_string())),
                    };
                    let record =
                        Self::parse_type_record(&tokens, &mut idx, input_name, comment.clone())?;
                    types.push(TypeDefinition::Record(record));
                }
                GqlToken::Ident(s) if s == "enum" => {
                    idx += 1;
                    let enum_type = Self::parse_enum(&tokens, &mut idx, comment.clone())?;
                    types.push(TypeDefinition::Enum(enum_type));
                }
                _ => {
                    idx += 1;
                }
            }
        }

        let mut services = Vec::new();
        if !query_ops.is_empty() || !mutation_ops.is_empty() {
            let mut all_ops = query_ops;
            all_ops.extend(mutation_ops);
            services.push(Service {
                name: "GraphQLApi".to_string(),
                operations: all_ops,
                description: Some("GraphQL API operations".to_string()),
                deprecated: None,
            });
        }

        Ok(Contract {
            name: "GraphQLContract".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Ingested from GraphQL SDL".to_string()),
            types,
            services,
        })
    }

    fn parse_enum(
        tokens: &[(GqlToken, Option<String>)],
        idx: &mut usize,
        desc: Option<String>,
    ) -> Result<EnumType, AdapterError> {
        let name = match tokens.get(*idx) {
            Some((GqlToken::Ident(n), _)) => {
                *idx += 1;
                n.clone()
            }
            _ => return Err(AdapterError::Parse("Expected enum name".to_string())),
        };

        if tokens.get(*idx).map(|t| &t.0) != Some(&GqlToken::LBrace) {
            return Err(AdapterError::Parse(
                "Expected '{' after enum name".to_string(),
            ));
        }
        *idx += 1;

        let mut variants = Vec::new();
        while *idx < tokens.len() && tokens[*idx].0 != GqlToken::RBrace {
            if let GqlToken::Ident(ref var_name) = tokens[*idx].0 {
                let var_desc = tokens[*idx].1.clone();
                variants.push(EnumVariant {
                    name: var_name.clone(),
                    description: var_desc,
                    deprecated: None,
                });
            }
            *idx += 1;
        }

        if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::RBrace) {
            *idx += 1;
        }

        Ok(EnumType {
            name,
            variants,
            description: desc,
            deprecated: None,
        })
    }

    fn parse_type_record(
        tokens: &[(GqlToken, Option<String>)],
        idx: &mut usize,
        name: String,
        desc: Option<String>,
    ) -> Result<Record, AdapterError> {
        if tokens.get(*idx).map(|t| &t.0) != Some(&GqlToken::LBrace) {
            return Err(AdapterError::Parse(
                "Expected '{' after type name".to_string(),
            ));
        }
        *idx += 1;

        let mut fields = Vec::new();
        while *idx < tokens.len() && tokens[*idx].0 != GqlToken::RBrace {
            let field_desc = tokens[*idx].1.clone();
            if let GqlToken::Ident(ref field_name) = tokens[*idx].0 {
                let fname = field_name.clone();
                *idx += 1;

                if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::Colon) {
                    *idx += 1;
                    let field_type = Self::parse_type_ref(tokens, idx)?;
                    fields.push(Field {
                        name: fname,
                        field_type,
                        description: field_desc,
                        deprecated: None,
                    });
                    continue;
                }
            }
            *idx += 1;
        }

        if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::RBrace) {
            *idx += 1;
        }

        Ok(Record {
            name,
            fields,
            description: desc,
            deprecated: None,
        })
    }

    fn parse_operations(
        tokens: &[(GqlToken, Option<String>)],
        idx: &mut usize,
        ops: &mut Vec<Operation>,
    ) -> Result<(), AdapterError> {
        if tokens.get(*idx).map(|t| &t.0) != Some(&GqlToken::LBrace) {
            return Err(AdapterError::Parse(
                "Expected '{' for operations".to_string(),
            ));
        }
        *idx += 1;

        while *idx < tokens.len() && tokens[*idx].0 != GqlToken::RBrace {
            let op_desc = tokens[*idx].1.clone();
            if let GqlToken::Ident(ref op_name) = tokens[*idx].0 {
                let name = op_name.clone();
                *idx += 1;

                // Optional inputs: (id: ID!, name: String)
                let input_type = if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::LParen) {
                    *idx += 1;
                    let mut first_input = None;
                    while *idx < tokens.len() && tokens[*idx].0 != GqlToken::RParen {
                        if let GqlToken::Ident(_) = &tokens[*idx].0 {
                            *idx += 1;
                            if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::Colon) {
                                *idx += 1;
                                let ty = Self::parse_type_ref(tokens, idx)?;
                                if first_input.is_none() {
                                    first_input = Some(ty);
                                }
                            }
                        } else {
                            *idx += 1;
                        }
                    }
                    if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::RParen) {
                        *idx += 1;
                    }
                    first_input
                } else {
                    None
                };

                if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::Colon) {
                    *idx += 1;
                    let output_type = Self::parse_type_ref(tokens, idx)?;
                    ops.push(Operation {
                        name,
                        input: input_type,
                        output: Some(output_type),
                        description: op_desc,
                        deprecated: None,
                    });
                    continue;
                }
            }
            *idx += 1;
        }

        if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::RBrace) {
            *idx += 1;
        }

        Ok(())
    }

    fn parse_type_ref(
        tokens: &[(GqlToken, Option<String>)],
        idx: &mut usize,
    ) -> Result<Type, AdapterError> {
        // Handle list: [Type] or [Type!]!
        if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::LBracket) {
            *idx += 1;
            let inner = Self::parse_type_ref(tokens, idx)?;
            if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::RBracket) {
                *idx += 1;
            }
            let is_required = if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::Bang) {
                *idx += 1;
                true
            } else {
                false
            };
            let list_type = Type::List(Box::new(inner));
            return Ok(if is_required {
                list_type
            } else {
                Type::Optional(Box::new(list_type))
            });
        }

        let type_name = match tokens.get(*idx) {
            Some((GqlToken::Ident(n), _)) => {
                *idx += 1;
                n.clone()
            }
            _ => return Err(AdapterError::Parse("Expected type name".to_string())),
        };

        // In GraphQL: Type without '!' is optional; Type with '!' is required
        let is_required = if tokens.get(*idx).map(|t| &t.0) == Some(&GqlToken::Bang) {
            *idx += 1;
            true
        } else {
            false
        };

        let base = gql_type_to_ir(&type_name);
        Ok(if is_required {
            base
        } else {
            Type::Optional(Box::new(base))
        })
    }
}

fn gql_type_to_ir(name: &str) -> Type {
    match name {
        "ID" | "String" => Type::String,
        "Int" => Type::Int64,
        "Float" => Type::Float64,
        "Boolean" => Type::Bool,
        other => Type::Reference(other.to_string()),
    }
}
