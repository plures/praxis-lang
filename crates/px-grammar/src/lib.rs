//! # px-grammar — the generated `.px` grammar (pest) for the Praxis Intent Language.
//!
//! `grammar.pest` in this crate's `src/` is a **generated, committed artifact**
//! (ADR-0021): it is produced by `px-grammar-gen` from the `px-ast` canonical
//! types and MUST NOT be hand-edited. Regenerate with:
//!
//! ```text
//! cargo run -p px-grammar-gen > crates/px-grammar/src/grammar.pest
//! ```
//!
//! CI enforces that the committed grammar matches the generator output
//! (regenerate-and-diff drift gate, C-DRIFT-001).
//!
//! This crate exposes the grammar source string. The pest `Parser` binding
//! (deriving a parser over this grammar) lands with `px-compiler` in the
//! next migration wave (epic M3 wave 2).

/// The canonical `.px` grammar source (pest PEG), embedded from the committed
/// generated artifact. This is the exact text `px-grammar-gen` emits.
pub const GRAMMAR_PEST: &str = include_str!("grammar.pest");

/// Every top-level construct rule that the grammar is required to define.
/// Mirrors the `Statement` variants in `px-ast` (the language spec).
pub const REQUIRED_CONSTRUCT_RULES: &[&str] = &[
    "import_decl",
    "entity_decl",
    "config_decl",
    "fact_decl",
    "rule_decl",
    "constraint_decl",
    "contract_decl",
    "function_decl",
    "trigger_decl",
    "dataflow_procedure_decl",
    "procedure_decl",
    "scenario_decl",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_is_non_empty() {
        assert!(
            GRAMMAR_PEST.len() > 1024,
            "embedded grammar.pest looks empty/truncated ({} bytes)",
            GRAMMAR_PEST.len()
        );
    }

    #[test]
    fn grammar_defines_all_required_constructs() {
        for rule in REQUIRED_CONSTRUCT_RULES {
            let needle = format!("{rule} =");
            assert!(
                GRAMMAR_PEST.contains(&needle),
                "grammar.pest is missing required construct rule: {rule}"
            );
        }
    }

    #[test]
    fn grammar_matches_generator_output() {
        // Parity guard: the committed artifact must equal what px-grammar-gen
        // produces from px-ast today. This is the in-crate mirror of the CI
        // regenerate-and-diff gate, so drift is caught by `cargo test` too.
        let regenerated = px_grammar_gen::generate_full_grammar();
        assert_eq!(
            GRAMMAR_PEST, regenerated,
            "committed grammar.pest is stale; regenerate with \
             `cargo run -p px-grammar-gen > crates/px-grammar/src/grammar.pest`"
        );
    }
}
