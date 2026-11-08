pub mod emitter;
pub mod lexer;
pub mod parser;

pub use emitter::to_dsl;
pub use lexer::{Lexer, LexerError, Token};
pub use self::parser::{ParseError, Parser};

pub fn parse(source: &str) -> Result<crate::ir::Contract, ParseError> {
    Parser::parse_str(source)
}
