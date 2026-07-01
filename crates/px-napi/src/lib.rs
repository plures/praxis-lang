//! NAPI-RS bindings for the .px Praxis Intent Language.
//!
//! Skeleton crate (epic M1). The actual NAPI-RS bindings (native addon,
//! published TS package) land in milestone M5 per
//! docs/epic/PRAXIS-LANG-TRACKER.md. Intentionally empty but buildable.

#![forbid(unsafe_code)]

/// Placeholder marker so the crate compiles and links cleanly in the workspace.
/// Removed once the NAPI surface lands in M5.
pub const CRATE_NAME: &str = "px-napi";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_set() {
        assert_eq!(super::CRATE_NAME, "px-napi");
    }
}
