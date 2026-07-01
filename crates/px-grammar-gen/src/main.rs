//! px-grammar-gen binary — emits the generated `.px` grammar.
//!
//! Usage:
//!   cargo run -p px-grammar-gen                       # print grammar to stdout
//!   cargo run -p px-grammar-gen -- <path/to/grammar.pest>   # write grammar to a file (UTF-8, LF)
//!
//! Writing to a file directly avoids any shell/console re-encoding of the
//! grammar's multibyte UTF-8 box-drawing/em-dash characters (which corrupts
//! stdout redirection under some Windows PowerShell code pages). CI on Linux
//! uses stdout redirection; local Windows regeneration should pass the path.

use std::io::Write;

fn main() {
    let grammar = px_grammar_gen::generate_full_grammar();

    match std::env::args().nth(1) {
        Some(path) => {
            // Write raw UTF-8 bytes; no platform newline translation.
            if let Err(e) = std::fs::write(&path, grammar.as_bytes()) {
                eprintln!("px-grammar-gen: failed to write {path}: {e}");
                std::process::exit(1);
            }
            eprintln!("px-grammar-gen: wrote {} bytes to {path}", grammar.len());
        }
        None => {
            // Print raw UTF-8 bytes to stdout (avoids lossy re-encode).
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            if let Err(e) = lock.write_all(grammar.as_bytes()) {
                eprintln!("px-grammar-gen: failed to write stdout: {e}");
                std::process::exit(1);
            }
        }
    }
}
