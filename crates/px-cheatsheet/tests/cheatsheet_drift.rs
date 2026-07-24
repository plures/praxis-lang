//! Cheatsheet drift gate — proves `docs/px-grammar-cheatsheet.md` is a current
//! projection of `px-ast`/`grammar.pest`, mirroring `px-schema`'s
//! `schema_drift.rs` gate (C-DRIFT-001).
//!
//! Reads the actual committed doc + the actual committed `.px` corpus from
//! disk and asserts the committed doc is byte-identical to freshly generated
//! output from that same corpus. If a construct is added/removed/renamed in
//! `px-ast` (or the corpus changes) without regenerating, this test goes RED.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("px-cheatsheet is at <root>/crates/px-cheatsheet")
        .to_path_buf()
}

fn load_corpus(root: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for rel_dir in ["examples", "docs/rfcs"] {
        let dir = root.join(rel_dir);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|x| x == "px").unwrap_or(false))
            .collect();
        paths.sort();
        for p in paths {
            if let Ok(content) = std::fs::read_to_string(&p) {
                let rel = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((rel, content));
            }
        }
    }
    files
}

#[test]
fn committed_cheatsheet_matches_live_projection() {
    let root = repo_root();
    let committed_path = root.join("docs").join("px-grammar-cheatsheet.md");
    let committed = std::fs::read_to_string(&committed_path).unwrap_or_else(|e| {
        panic!(
            "cannot read committed artifact {}: {e}",
            committed_path.display()
        )
    });

    let corpus_owned = load_corpus(&root);
    let corpus_refs: Vec<(&str, &str)> = corpus_owned
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let generated = px_cheatsheet::build_cheatsheet(&corpus_refs);

    assert_eq!(
        committed, generated,
        "docs/px-grammar-cheatsheet.md is STALE relative to px-ast/grammar.pest/the .px corpus. \
         Regenerate: `cargo run -p px-cheatsheet -- docs/px-grammar-cheatsheet.md` \
         (or scripts/regen-cheatsheet.ps1) and recommit."
    );
}

#[test]
fn cheatsheet_is_deterministic() {
    let root = repo_root();
    let corpus_owned = load_corpus(&root);
    let corpus_refs: Vec<(&str, &str)> = corpus_owned
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    assert_eq!(
        px_cheatsheet::build_cheatsheet(&corpus_refs),
        px_cheatsheet::build_cheatsheet(&corpus_refs)
    );
}

#[test]
fn drift_gate_detects_a_stale_cheatsheet() {
    let root = repo_root();
    let corpus_owned = load_corpus(&root);
    let corpus_refs: Vec<(&str, &str)> = corpus_owned
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let current = px_cheatsheet::build_cheatsheet(&corpus_refs);

    assert!(
        current.contains("### `entity` — Entity (EntityDecl)"),
        "sanity: current projection should contain the entity construct section"
    );
    let stale = current.replacen(
        "### `entity` — Entity (EntityDecl)",
        "### `__removed_entity__`",
        1,
    );
    assert_ne!(
        current, stale,
        "the simulated stale cheatsheet must differ from the current projection"
    );
}
