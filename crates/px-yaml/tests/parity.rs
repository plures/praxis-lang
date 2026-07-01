//! Parity proof (ADR pillar P4 / PXLANG-M2): the same `.px` example, parsed via
//! the canonical `px-compiler` text front end, produces the **same AST** as its
//! committed `.yaml` deserialized via `px_yaml::from_yaml`. One AST, two
//! surfaces — YAML is not a second source of truth.
//!
//! The `.yaml` fixture is generated from the `.px` by the `emit_yaml` example
//! (`cargo run -p px-yaml --example emit_yaml -- examples/memory_assistant.px`),
//! so it is, by construction, the serde surface of this AST. This test proves
//! the two surfaces converge rather than merely re-checking the generator.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // crates/px-yaml/ -> crates/ -> repo root
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

#[test]
fn px_and_yaml_surfaces_yield_the_same_ast() {
    let root = repo_root();
    let px_path = root.join("examples").join("memory_assistant.px");
    let yaml_path = root.join("examples").join("memory_assistant.yaml");

    let px_src = std::fs::read_to_string(&px_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", px_path.display()));
    let yaml_src = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", yaml_path.display()));

    // Surface A: the .px text, via the canonical compiler front end.
    let from_px = px_compiler::parse(&px_src).expect("parse memory_assistant.px");
    // Surface B: the .yaml, via the YAML surface.
    let from_yaml = px_yaml::from_yaml(&yaml_src).expect("from_yaml memory_assistant.yaml");

    assert!(
        px_yaml::ast_eq(&from_px, &from_yaml),
        "parity mismatch: .px and .yaml surfaces produced different ASTs.\n\
         --- from .px (json) ---\n{}\n--- from .yaml (json) ---\n{}",
        serde_json::to_string_pretty(&from_px).unwrap(),
        serde_json::to_string_pretty(&from_yaml).unwrap(),
    );

    // Both surfaces agree on the construct count (6-construct example).
    assert_eq!(from_px.statements.len(), from_yaml.statements.len());
    assert_eq!(
        from_px.statements.len(),
        9,
        "memory_assistant.px has 9 top-level statements \
         (2 import, entity, fact, config, function, rule, 2 constraint)"
    );
}
