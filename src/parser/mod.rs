pub mod emitter;
pub mod lexer;
#[allow(clippy::module_inception)]
pub mod parser;

pub use self::parser::{ParseError, Parser};
pub use emitter::to_dsl;
pub use lexer::{Lexer, LexerError, Token};

pub fn parse(source: &str) -> Result<crate::ir::Contract, ParseError> {
    Parser::parse_str(source)
}
