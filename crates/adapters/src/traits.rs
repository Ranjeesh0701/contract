use ir::Contract;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("Adapter '{adapter}' error: {reason}")]
    Adapter { adapter: String, reason: String },

    #[error("Failed to parse schema: {0}")]
    Parse(String),

    #[error("Invalid schema format: {0}")]
    Invalid(String),

    #[error("Unsupported schema format: {0}")]
    Unsupported(String),
}

pub trait SchemaAdapter: Send + Sync {
    fn name(&self) -> &'static str;

    fn supported_extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn can_handle(&self, path: Option<&Path>, content: &str) -> bool;

    fn parse(&self, content: &str) -> Result<Contract, AdapterError>;
}
