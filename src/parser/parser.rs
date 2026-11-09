use crate::ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeAlias,
    TypeDefinition,
};
use thiserror::Error;

use crate::parser::lexer::{Lexer, LexerError, Token};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("Lexer error: {0}")]
    Lexer(#[from] LexerError),
    #[error("Expected {expected}, found {found:?} at line {line}, col {col}")]
    UnexpectedToken {
        expected: String,
        found: Token,
        line: usize,
        col: usize,
    },
    #[error("Unexpected end of file")]
    UnexpectedEof,
}

pub struct Parser {
    tokens: Vec<(Token, usize, usize)>,
    pos: usize,
}

impl Parser {
    pub fn parse_str(source: &str) -> Result<Contract, ParseError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser { tokens, pos: 0 };
        parser.parse_contract()
    }

    fn peek(&self) -> &(Token, usize, usize) {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos]
        } else {
            &self.tokens[self.tokens.len() - 1]
        }
    }

    fn bump(&mut self) -> (Token, usize, usize) {
        let tok = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let (tok, line, col) = self.peek().clone();
        if tok == expected {
            self.bump();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: tok,
                line,
                col,
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let (tok, line, col) = self.peek().clone();
        match tok {
            Token::Ident(s) => {
                self.bump();
                Ok(s)
            }
            other => Err(ParseError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: other,
                line,
                col,
            }),
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        let (tok, line, col) = self.peek().clone();
        match tok {
            Token::StringLit(s) => {
                self.bump();
                Ok(s)
            }
            other => Err(ParseError::UnexpectedToken {
                expected: "string literal".to_string(),
                found: other,
                line,
                col,
            }),
        }
    }

    pub fn parse_contract(&mut self) -> Result<Contract, ParseError> {
        let mut name = "UntitledContract".to_string();
        let mut version = "1.0.0".to_string();
        let mut description = None;
        let mut types = Vec::new();
        let mut services = Vec::new();

        if let (Token::Contract, _, _) = self.peek() {
            self.bump();
            name = self.expect_ident()?;
            self.expect(Token::LBrace)?;

            loop {
                let (tok, _, _) = self.peek();
                if tok == &Token::RBrace || tok == &Token::Eof {
                    break;
                }
                match tok.clone() {
                    Token::Model | Token::Record => {
                        self.bump();
                        let record = self.parse_record()?;
                        types.push(TypeDefinition::Record(record));
                    }
                    Token::Enum => {
                        self.bump();
                        let enum_type = self.parse_enum()?;
                        types.push(TypeDefinition::Enum(enum_type));
                    }
                    Token::TypeAlias => {
                        self.bump();
                        let alias = self.parse_alias()?;
                        types.push(TypeDefinition::Alias(alias));
                    }
                    Token::Service => {
                        self.bump();
                        let service = self.parse_service()?;
                        services.push(service);
                    }
                    Token::Semicolon | Token::Comma => {
                        self.bump();
                    }
                    _ => {
                        let key = self.expect_ident()?;
                        self.expect(Token::Colon)?;
                        let val = self.expect_string()?;

                        match key.as_str() {
                            "version" => version = val,
                            "description" => description = Some(val),
                            _ => {}
                        }
                        if matches!(self.peek().0, Token::Semicolon | Token::Comma) {
                            self.bump();
                        }
                    }
                }
            }
            self.expect(Token::RBrace)?;
        }

        loop {
            let (tok, _, _) = self.peek();
            if tok == &Token::Eof {
                break;
            }
            match tok.clone() {
                Token::Model | Token::Record => {
                    self.bump();
                    let record = self.parse_record()?;
                    types.push(TypeDefinition::Record(record));
                }
                Token::Enum => {
                    self.bump();
                    let enum_type = self.parse_enum()?;
                    types.push(TypeDefinition::Enum(enum_type));
                }
                Token::TypeAlias => {
                    self.bump();
                    let alias = self.parse_alias()?;
                    types.push(TypeDefinition::Alias(alias));
                }
                Token::Service => {
                    self.bump();
                    let service = self.parse_service()?;
                    services.push(service);
                }
                _ => {
                    let (t, line, col) = self.peek().clone();
                    return Err(ParseError::UnexpectedToken {
                        expected: "model, enum, type, or service declaration".to_string(),
                        found: t,
                        line,
                        col,
                    });
                }
            }
        }

        Ok(Contract {
            name,
            version,
            description,
            types,
            services,
        })
    }

    fn parse_record(&mut self) -> Result<Record, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        loop {
            let (tok, _, _) = self.peek();
            if tok == &Token::RBrace || tok == &Token::Eof {
                break;
            }
            if matches!(tok, Token::Comma | Token::Semicolon) {
                self.bump();
                continue;
            }
            let field_name = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let field_type = self.parse_type()?;

            fields.push(Field {
                name: field_name,
                field_type,
                description: None,
            });

            if matches!(self.peek().0, Token::Comma | Token::Semicolon) {
                self.bump();
            }
        }

        self.expect(Token::RBrace)?;
        Ok(Record {
            name,
            fields,
            description: None,
        })
    }

    fn parse_enum(&mut self) -> Result<EnumType, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;

        let mut variants = Vec::new();
        loop {
            let (tok, _, _) = self.peek();
            if tok == &Token::RBrace || tok == &Token::Eof {
                break;
            }
            if matches!(tok, Token::Comma | Token::Semicolon) {
                self.bump();
                continue;
            }
            let variant_name = self.expect_ident()?;
            variants.push(EnumVariant {
                name: variant_name,
                description: None,
            });

            if matches!(self.peek().0, Token::Comma | Token::Semicolon) {
                self.bump();
            }
        }

        self.expect(Token::RBrace)?;
        Ok(EnumType {
            name,
            variants,
            description: None,
        })
    }

    fn parse_alias(&mut self) -> Result<TypeAlias, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::Colon)?;
        let target = self.parse_type()?;

        if matches!(self.peek().0, Token::Comma | Token::Semicolon) {
            self.bump();
        }

        Ok(TypeAlias {
            name,
            target,
            description: None,
        })
    }

    fn parse_service(&mut self) -> Result<Service, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;

        let mut operations = Vec::new();
        loop {
            let (tok, _, _) = self.peek();
            if tok == &Token::RBrace || tok == &Token::Eof {
                break;
            }
            if matches!(tok, Token::Comma | Token::Semicolon) {
                self.bump();
                continue;
            }
            let op_name = self.expect_ident()?;
            self.expect(Token::LParen)?;

            let input = if let (Token::RParen, _, _) = self.peek() {
                None
            } else {
                Some(self.parse_type()?)
            };

            self.expect(Token::RParen)?;

            let output = if let (Token::Colon, _, _) = self.peek() {
                self.bump();
                let ty = self.parse_type()?;
                match &ty {
                    Type::Reference(name) if name.eq_ignore_ascii_case("void") => None,
                    _ => Some(ty),
                }
            } else {
                None
            };

            operations.push(Operation {
                name: op_name,
                input,
                output,
                description: None,
            });

            if matches!(self.peek().0, Token::Comma | Token::Semicolon) {
                self.bump();
            }
        }

        self.expect(Token::RBrace)?;
        Ok(Service {
            name,
            operations,
            description: None,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut base_ty = match self.peek().0.clone() {
            Token::Ident(ref id) => match id.as_str() {
                "Bool" | "bool" | "Boolean" => {
                    self.bump();
                    Type::Bool
                }
                "String" | "string" => {
                    self.bump();
                    Type::String
                }
                "Int32" | "int32" | "Int" => {
                    self.bump();
                    Type::Int32
                }
                "Int64" | "int64" | "Long" => {
                    self.bump();
                    Type::Int64
                }
                "Float64" | "float64" | "Float" | "Double" => {
                    self.bump();
                    Type::Float64
                }
                "Bytes" | "bytes" => {
                    self.bump();
                    Type::Bytes
                }
                "Map" => {
                    self.bump();
                    self.expect(Token::Lt)?;
                    let key = self.parse_type()?;
                    self.expect(Token::Comma)?;
                    let val = self.parse_type()?;
                    self.expect(Token::Gt)?;
                    Type::Map {
                        key: Box::new(key),
                        value: Box::new(val),
                    }
                }
                _ => {
                    let name = self.expect_ident()?;
                    Type::Reference(name)
                }
            },
            other => {
                let (_, line, col) = self.peek().clone();
                return Err(ParseError::UnexpectedToken {
                    expected: "type name".to_string(),
                    found: other,
                    line,
                    col,
                });
            }
        };

        loop {
            match self.peek().0 {
                Token::LBracket => {
                    self.bump();
                    self.expect(Token::RBracket)?;
                    base_ty = Type::List(Box::new(base_ty));
                }
                Token::Question => {
                    self.bump();
                    base_ty = Type::Optional(Box::new(base_ty));
                }
                _ => break,
            }
        }

        Ok(base_ty)
    }
}
