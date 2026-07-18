//! Self-hosting gate for RFC-0001: the RFC's own `.px` specification file must
//! parse cleanly into the canonical AST via `px_compiler::parse`. This proves
//! the epic's self-hosting requirement: the language can describe its own
//! evolution using only currently-shipping syntax.

use std::fs;
use std::path::PathBuf;

fn rfc_px() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/px-compiler
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("rfcs")
        .join("RFC-0001-semantic-core.px")
}

#[test]
fn rfc_0001_self_hosts() {
    let path = rfc_px();
    let src = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match px_compiler::parse(&src) {
        Ok(doc) => assert!(
            !doc.statements.is_empty(),
            "RFC-0001 .px parsed to zero statements"
        ),
        Err(e) => panic!("RFC-0001 .px failed to parse:\n{e}"),
    }
}
