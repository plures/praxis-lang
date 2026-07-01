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
//! This crate exposes the grammar source string **and** the pest `Parser`
//! binding derived over it. Downstream crates (`px-compiler`, `px-eval`)
//! parse `.px` source via [`PxParser`] against the [`Rule`] set.

use pest_derive::Parser;

/// The pest parser for the `.px` Praxis Intent Language.
///
/// Derived directly over the committed, generated `grammar.pest` (ADR-0021).
/// The `#[grammar = ...]` path is resolved by `pest_derive` relative to this
/// crate's `src/` directory — i.e. the **same** file [`GRAMMAR_PEST`] embeds,
/// so the parser and the embedded string can never disagree.
///
/// # Example
/// ```
/// use px_grammar::{PxParser, Rule};
/// use pest::Parser;
///
/// let pairs = PxParser::parse(Rule::document, "import core::memory\n").unwrap();
/// assert!(pairs.count() > 0);
/// ```
#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct PxParser;

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

    #[test]
    fn parser_parses_a_minimal_document() {
        use pest::Parser;
        // The derived parser must accept a trivial well-formed document.
        let src = "import core::memory\n";
        let pairs = PxParser::parse(Rule::document, src).expect("minimal document should parse");
        assert!(pairs.count() > 0, "expected at least the document pair");
    }

    #[test]
    fn parser_entry_rule_compiles_end_to_end() {
        use pest::Parser;
        // Parsing via the derived parser proves the whole grammar compiled into
        // a `Rule` enum (one variant per named rule) and the entry rule works.
        assert!(PxParser::parse(Rule::document, "\n").is_ok());
    }
}
