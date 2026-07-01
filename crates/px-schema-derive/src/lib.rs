//! Derive macros for the px-schema layer of the .px Praxis Intent Language.
//!
//! Skeleton crate (epic M1). Real derives land in later milestones per
//! docs/epic/PRAXIS-LANG-TRACKER.md. Intentionally minimal but buildable.

use proc_macro::TokenStream;

/// Placeholder derive so the proc-macro crate compiles and links.
/// Real `#[derive(PxSchema)]` support lands in a later milestone.
#[proc_macro_derive(PxSchemaPlaceholder)]
pub fn px_schema_placeholder(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
