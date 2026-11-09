pub mod graphql;
pub mod json_schema;
pub mod openapi;
pub mod protobuf;
pub mod registry;
pub mod traits;

pub use graphql::{GraphQLAdapter, from_graphql};
pub use json_schema::{JsonSchemaAdapter, from_json_schema};
pub use openapi::{OpenApiAdapter, OpenApiError, from_openapi_json, to_openapi_json};
pub use protobuf::{ProtobufAdapter, from_protobuf};
pub use registry::AdapterRegistry;
pub use traits::{AdapterError, SchemaAdapter};

pub fn default_registry() -> AdapterRegistry {
    AdapterRegistry::new_with_defaults()
}
