pub mod emitter;
pub mod lexer;
#[allow(clippy::module_inception)]
pub mod parser;
pub mod resolver;

pub use self::parser::{ParseError, Parser};
pub use emitter::to_dsl;
pub use lexer::{Lexer, LexerError, Token};
pub use resolver::{
    ModuleResolver, ResolverError, resolve_contract_file, resolve_contract_str,
};

pub fn parse(source: &str) -> Result<crate::ir::Contract, ParseError> {
    Parser::parse_str(source)
}
