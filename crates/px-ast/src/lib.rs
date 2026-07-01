//! Canonical AST for the .px Praxis Intent Language. The language spec: if a construct is not represented here, it does not exist in the language.
//!
//! Skeleton crate (epic M1). Implementation lands in later milestones per
//! docs/epic/PRAXIS-LANG-TRACKER.md. Intentionally empty but buildable.

#![forbid(unsafe_code)]

/// Placeholder marker so the crate compiles and links cleanly in the workspace.
/// Removed once real types land.
pub const CRATE_NAME: &str = "px-ast";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_set() {
        assert_eq!(super::CRATE_NAME, "px-ast");
    }
}
