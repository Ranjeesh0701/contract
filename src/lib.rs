//! Contract: Universal API contract language with zero-drift synchronization,
//! multi-language codegen, and bidirectional adapter conversions.
//!
//! # Example
//!
//! ```rust
//! use contract::parser::parse;
//! use contract::diff::diff;
//! use contract::codegen::TypeScriptGenerator;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let old_ast = parse("contract Api { model User { id: String } }")?;
//! let new_ast = parse("contract Api { model User { id: Int64 } }")?;
//!
//! let report = diff(&old_ast, &new_ast);
//! if report.has_breaking_changes() {
//!     assert_eq!(report.breaking_changes().len(), 1);
//! }
//!
//! let ts_sdk = TypeScriptGenerator::generate(&new_ast);
//! assert!(ts_sdk.contains("export interface User"));
//! # Ok(())
//! # }
//! ```

pub mod adapters;
pub mod codegen;
pub mod diff;
pub mod ir;
pub mod parser;
pub mod sync;
