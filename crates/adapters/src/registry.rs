use std::path::Path;

use ir::Contract;

use crate::openapi::OpenApiAdapter;
use crate::traits::{AdapterError, SchemaAdapter};

pub struct AdapterRegistry {
    adapters: Vec<Box<dyn SchemaAdapter>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn new_with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(OpenApiAdapter);
        registry
    }

    pub fn register<A: SchemaAdapter + 'static>(&mut self, adapter: A) {
        self.adapters.push(Box::new(adapter));
    }

    pub fn find_adapter(&self, path: Option<&Path>, content: &str) -> Option<&dyn SchemaAdapter> {
        self.adapters
            .iter()
            .find(|a| a.can_handle(path, content))
            .map(|b| b.as_ref())
    }

    pub fn get_adapter(&self, name: &str) -> Option<&dyn SchemaAdapter> {
        self.adapters
            .iter()
            .find(|a| a.name().eq_ignore_ascii_case(name))
            .map(|b| b.as_ref())
    }

    pub fn auto_parse(
        &self,
        path: Option<&Path>,
        content: &str,
    ) -> Option<Result<Contract, AdapterError>> {
        self.find_adapter(path, content).map(|a| a.parse(content))
    }
}
