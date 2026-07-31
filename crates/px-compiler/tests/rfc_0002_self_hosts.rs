//! Self-hosting gate for RFC-0002: the RFC's own `.px` specification file must
//! parse cleanly into the canonical AST via `px_compiler::parse`, using ONLY
//! current (RFC-0001-era) syntax. RFC-0002 is design-only — it deliberately
//! does not introduce `type`/`where` syntax; this test proves the design
//! document itself is expressible in today's shipping grammar.

use std::fs;
use std::path::PathBuf;

use px_ast::{Expr, Statement, TypeExpr};

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

#[test]
fn refinement_alias_parses_to_the_new_surface_ast_construct() {
    let doc = px_compiler::parse("type PositiveInt = int where value > 0\n")
        .expect("RFC-0002 refinement alias must parse");

    assert_eq!(doc.statements.len(), 1);
    let Statement::TypeAlias(alias) = &doc.statements[0] else {
        panic!("expected a type alias statement");
    };
    assert_eq!(alias.name.name, "PositiveInt");
    let TypeExpr::Refined { base, predicate } = &alias.aliased else {
        panic!("expected refined alias target");
    };
    assert!(matches!(base.as_ref(), TypeExpr::Base(_)));
    assert!(matches!(predicate.as_ref(), Expr::Binary { .. }));
}

#[test]
fn rfc_0002_non_goals_are_not_accepted_by_the_grammar() {
    for src in [
        "type RefinedNamed = Customer where value != null\n",
        "type Nested = int where value > 0 where value < 100\n",
    ] {
        assert!(
            px_compiler::parse(src).is_err(),
            "RFC-0002 excludes this refinement form: {src}"
        );
    }
}
