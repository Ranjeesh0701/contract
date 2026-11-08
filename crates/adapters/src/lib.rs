pub mod openapi;
pub mod registry;
pub mod traits;

pub use openapi::{OpenApiAdapter, OpenApiError, from_openapi_json};
pub use registry::AdapterRegistry;
pub use traits::{AdapterError, SchemaAdapter};

pub fn default_registry() -> AdapterRegistry {
    AdapterRegistry::new_with_defaults()
}
