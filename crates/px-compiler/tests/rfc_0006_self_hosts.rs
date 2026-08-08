//! Self-hosting gate for RFC-0006: the RFC's own `.px` specification file must
//! parse cleanly into the canonical AST via `px_compiler::parse`, using ONLY
//! current syntax. RFC-0006 is design-only — it does not introduce new syntax
//! (the checker is an analysis pass, not a grammar change); this test proves the
//! design document itself is expressible in today's shipping grammar.

use std::fs;
use std::path::PathBuf;

fn rfc_px() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/px-compiler
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("rfcs")
        .join("RFC-0006-bounded-verification.px")
}

#[test]
fn rfc_0006_self_hosts() {
    let path = rfc_px();
    let src =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match px_compiler::parse(&src) {
        Ok(doc) => assert!(
            !doc.statements.is_empty(),
            "RFC-0006 .px parsed to zero statements"
        ),
        Err(e) => panic!("RFC-0006 .px failed to parse:\n{e}"),
    }
}
