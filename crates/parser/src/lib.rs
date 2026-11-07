pub mod emitter;
pub mod lexer;
pub mod parser;

pub use emitter::to_dsl;
pub use lexer::{Lexer, LexerError, Token};
pub use parser::{ParseError, Parser};

pub fn parse(source: &str) -> Result<ir::Contract, ParseError> {
    Parser::parse_str(source)
}
