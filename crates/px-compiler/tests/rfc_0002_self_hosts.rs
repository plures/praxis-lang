//! Self-hosting gate for RFC-0002: the RFC's own `.px` specification file must
//! parse cleanly into the canonical AST via `px_compiler::parse`, using ONLY
//! current (RFC-0001-era) syntax. RFC-0002 is design-only — it deliberately
//! does not introduce `type`/`where` syntax; this test proves the design
//! document itself is expressible in today's shipping grammar.

use std::fs;
use std::path::PathBuf;

fn rfc_px() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/px-compiler
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("rfcs")
        .join("RFC-0002-structural-refinement-types.px")
}

#[test]
fn rfc_0002_self_hosts() {
    let path = rfc_px();
    let src =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match px_compiler::parse(&src) {
        Ok(doc) => assert!(
            !doc.statements.is_empty(),
            "RFC-0002 .px parsed to zero statements"
        ),
        Err(e) => panic!("RFC-0002 .px failed to parse:\n{e}"),
    }
}
