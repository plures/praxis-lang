//! px-grammar-gen — Generates `grammar.pest` from `px-ast` canonical types.
//!
//! Architecture:
//! - Expression grammar (v1 + v2) is a PINNED FRAGMENT (hand-curated operator precedence)
//! - Declaration grammar is GENERATED from px-ast construct types
//! - Procedure grammar is SEMI-GENERATED (structure from types, keywords pinned)
//! - Tokens/values are PINNED (shared)
//!
//! Output is deterministic. CI verifies (regenerate-and-diff, C-DRIFT-001):
//!   cargo run -p px-grammar-gen > /tmp/gen.pest && diff grammar.pest /tmp/gen.pest
//!
//! Exposed as a library so downstream crates (e.g. `px-grammar`) can assert the
//! committed grammar artifact matches this generator's output in-process.

mod grammar;

pub use grammar::generate_full_grammar;
