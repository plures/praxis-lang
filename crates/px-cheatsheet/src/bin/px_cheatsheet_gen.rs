//! px-cheatsheet-gen binary — emits `docs/px-grammar-cheatsheet.md`.
//!
//! Usage: cargo run -p px-cheatsheet -- <out-file>
//!
//! Reads the real, committed `.px` corpus (`examples/*.px`,
//! `docs/rfcs/*.px`) from disk at generation time and writes bytes directly
//! (never via stdout) — same codepage-safe pattern as px-schema-gen /
//! px-grammar-gen (TOOLS.md exec/PowerShell discipline).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/px-cheatsheet
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("px-cheatsheet is at <root>/crates/px-cheatsheet")
        .to_path_buf()
}

/// Load the real corpus files from disk, returning owned (path, contents)
/// pairs. Paths are repo-relative for display in the generated doc.
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

fn main() -> ExitCode {
    let out_path = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("px-cheatsheet-gen: usage: cargo run -p px-cheatsheet -- <out-file>");
            return ExitCode::FAILURE;
        }
    };

    let root = repo_root();
    let corpus_owned = load_corpus(&root);
    let corpus_refs: Vec<(&str, &str)> = corpus_owned
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();

    let doc = px_cheatsheet::build_cheatsheet(&corpus_refs);

    if let Some(parent) = out_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "px-cheatsheet-gen: failed to create {}: {e}",
                parent.display()
            );
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = std::fs::write(&out_path, doc.as_bytes()) {
        eprintln!(
            "px-cheatsheet-gen: failed to write {}: {e}",
            out_path.display()
        );
        return ExitCode::FAILURE;
    }
    eprintln!("px-cheatsheet-gen: wrote {}", out_path.display());
    ExitCode::SUCCESS
}
