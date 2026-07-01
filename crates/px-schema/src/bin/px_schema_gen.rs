//! px-schema-gen binary — emits the generated `.px` schema artifacts.
//!
//! Usage:
//!   cargo run -p px-schema -- <out-dir>
//!
//! Writes `<out-dir>/px.schema.json` and `<out-dir>/px.schema.px` as raw
//! UTF-8/LF bytes. We write bytes DIRECTLY to files (never via stdout) so no
//! shell/console code page can re-encode the multibyte characters in the
//! schema — the same codepage-safe pattern px-grammar-gen uses (ADR §M4,
//! TOOLS.md exec/PowerShell discipline).
//!
//! Both artifacts are pure projections of `px-ast` (via schemars). A CI drift
//! gate regenerates into a temp dir and rejects any mismatch with the committed
//! files, so the schema can never silently diverge from the AST (C-DRIFT-001).

use std::path::Path;
use std::process::ExitCode;

use px_schema::projection::{
    json_schema_string, px_schema_string, JSON_SCHEMA_FILE, PX_SCHEMA_FILE,
};

fn main() -> ExitCode {
    let out_dir = match std::env::args().nth(1) {
        Some(d) => d,
        None => {
            eprintln!("px-schema-gen: usage: cargo run -p px-schema -- <out-dir>");
            return ExitCode::FAILURE;
        }
    };
    let dir = Path::new(&out_dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("px-schema-gen: failed to create {out_dir}: {e}");
        return ExitCode::FAILURE;
    }

    let json_path = dir.join(JSON_SCHEMA_FILE);
    let px_path = dir.join(PX_SCHEMA_FILE);

    // Raw byte writes — no platform newline translation, no code-page re-encode.
    if let Err(e) = std::fs::write(&json_path, json_schema_string().as_bytes()) {
        eprintln!(
            "px-schema-gen: failed to write {}: {e}",
            json_path.display()
        );
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::write(&px_path, px_schema_string().as_bytes()) {
        eprintln!("px-schema-gen: failed to write {}: {e}", px_path.display());
        return ExitCode::FAILURE;
    }

    eprintln!(
        "px-schema-gen: wrote {} and {}",
        json_path.display(),
        px_path.display()
    );
    ExitCode::SUCCESS
}
