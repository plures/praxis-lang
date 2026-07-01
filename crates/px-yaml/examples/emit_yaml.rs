//! Emit the YAML surface for a committed `.px` example.
//!
//! This is the generator for `examples/<name>.yaml`: it parses the `.px` text
//! with `px-compiler` (the canonical front end) and prints `px_yaml::to_yaml`
//! of the resulting AST. The committed `.yaml` is therefore, by construction,
//! the serde surface of the *same* AST — which is exactly what the parity test
//! (`tests/parity.rs`) then re-reads and asserts equal to the `.px` parse.
//!
//! Usage: `cargo run -p px-yaml --example emit_yaml -- examples/memory_assistant.px`

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: emit_yaml <path-to.px>");
    let src = std::fs::read_to_string(&path).expect("read .px source");
    let doc = px_compiler::parse(&src).expect("parse .px via px-compiler");
    let yaml = px_yaml::to_yaml(&doc).expect("serialize AST to YAML");
    print!("{yaml}");
}
