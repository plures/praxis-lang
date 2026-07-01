//! # px-yaml — the YAML authoring surface for `.px`
//!
//! YAML is a **surface over the canonical `px-ast`, not a second source of
//! truth** (ADR pillar P4; PXLANG-M2 reconciliation). A `.px` file and its YAML
//! equivalent deserialize to the *same* [`px_ast::PxDocument`]. This crate does
//! not define any new AST — it drives the existing `px-ast` serde derives (M4
//! stabilized the kind-tagged enums and dropped editor-only spans) through
//! `serde_yaml`.
//!
//! ## Public API
//!
//! - [`to_yaml`] — serialize a [`px_ast::PxDocument`] to a YAML string.
//! - [`from_yaml`] — deserialize a YAML string back into a [`px_ast::PxDocument`].
//! - [`ast_eq`] — structural AST equality via the canonical serde encoding.
//!   Used because most `px-ast` nodes intentionally do not derive `PartialEq`
//!   (e.g. [`px_ast::Value`] carries `f64`), yet the serde projection is total
//!   and stable, so equal ASTs produce equal serialized trees.
//!
//! ## What YAML carries (and what it does not)
//!
//! Because the YAML is exactly the serde encoding of `px-ast`, every construct
//! the AST can represent round-trips: the 10 declarative constructs *and* the
//! imperative layer. Per the M2 reconciliation, V2 procedure code blocks are
//! **not** re-expressed as structural YAML control flow by hand — they live in
//! the AST as their parsed [`px_ast::CodeBlock`] form (produced by
//! `px-compiler` from the embedded `.px` block scalar) and serde carries that
//! subtree verbatim. YAML never becomes a rival grammar for imperative code.
//!
//! ## Not a second truth (the invariant this crate guarantees)
//!
//! `from_yaml(to_yaml(doc))` reconstructs the identical AST, and a hand-written
//! `.yaml` for one of the `examples/*.px` deserializes to the *same* AST that
//! `px-compiler` produces from the `.px` text. Both are proven by the crate
//! tests (`roundtrip.rs`, `parity.rs`).

#![forbid(unsafe_code)]

use px_ast::PxDocument;

/// Errors from the YAML surface.
#[derive(Debug)]
pub enum YamlError {
    /// Serializing a [`PxDocument`] to YAML failed.
    Serialize(serde_yaml::Error),
    /// Deserializing YAML into a [`PxDocument`] failed (syntax or shape).
    Deserialize(serde_yaml::Error),
}

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YamlError::Serialize(e) => write!(f, "px-yaml serialize error: {e}"),
            YamlError::Deserialize(e) => write!(f, "px-yaml deserialize error: {e}"),
        }
    }
}

impl std::error::Error for YamlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            YamlError::Serialize(e) | YamlError::Deserialize(e) => Some(e),
        }
    }
}

/// Serialize a canonical [`PxDocument`] to a YAML string.
///
/// The output is the serde encoding of the AST (kind-tagged enums, spans
/// dropped), so it is a faithful YAML *surface* of the same document — not a
/// lossy re-modeling.
///
/// # Errors
/// [`YamlError::Serialize`] if the document cannot be encoded (practically only
/// on a non-string map key, which `px-ast` never produces).
pub fn to_yaml(doc: &PxDocument) -> Result<String, YamlError> {
    serde_yaml::to_string(doc).map_err(YamlError::Serialize)
}

/// Deserialize a YAML string into a canonical [`PxDocument`].
///
/// This is the YAML *authoring surface*: the YAML must describe the same shape
/// as the `px-ast` serde encoding, and the result is the identical AST type the
/// `.px` text front end produces. It is therefore *not* a second source of
/// truth — just a second way to spell the one AST.
///
/// # Errors
/// [`YamlError::Deserialize`] on a YAML syntax error or a shape that does not
/// match `px-ast`.
pub fn from_yaml(src: &str) -> Result<PxDocument, YamlError> {
    serde_yaml::from_str(src).map_err(YamlError::Deserialize)
}

/// Structural equality of two [`PxDocument`]s via their canonical serde
/// encoding.
///
/// Most `px-ast` nodes deliberately do not derive `PartialEq` (for example
/// [`px_ast::Value`] holds an `f64`), so a direct `==` is unavailable. Because
/// the serde projection is total and deterministic, two ASTs are equal exactly
/// when their `serde_json::Value` encodings are equal. This is the "same AST,
/// two surfaces" oracle used by the round-trip and parity tests.
///
/// Returns `false` (rather than erroring) if either document fails to encode,
/// which cannot happen for well-formed `px-ast` in practice.
pub fn ast_eq(a: &PxDocument, b: &PxDocument) -> bool {
    match (serde_json::to_value(a), serde_json::to_value(b)) {
        (Ok(va), Ok(vb)) => va == vb,
        _ => false,
    }
}
