//! Grammar generation logic.

pub fn generate_full_grammar() -> String {
    let mut out = String::with_capacity(16384);
    out.push_str(HEADER);
    out.push_str(DOCUMENT);
    out.push_str(DECLARATIONS);
    out.push_str(PROCEDURES);
    out.push_str(V1_STEPS);
    out.push_str(V2_CODE_BLOCK);
    out.push_str(V2_EXPRESSIONS);
    out.push_str(V1_EXPRESSIONS);
    out.push_str(TYPES);
    out.push_str(VALUES);
    out.push_str(TOKENS);
    out
}

const HEADER: &str = include_str!("fragments/header.pest");
const DOCUMENT: &str = include_str!("fragments/document.pest");
const DECLARATIONS: &str = include_str!("fragments/declarations.pest");
const PROCEDURES: &str = include_str!("fragments/procedures.pest");
const V1_STEPS: &str = include_str!("fragments/v1_steps.pest");
const V2_CODE_BLOCK: &str = include_str!("fragments/v2_code_block.pest");
const V2_EXPRESSIONS: &str = include_str!("fragments/v2_expressions.pest");
const V1_EXPRESSIONS: &str = include_str!("fragments/v1_expressions.pest");
const TYPES: &str = include_str!("fragments/types.pest");
const VALUES: &str = include_str!("fragments/values.pest");
const TOKENS: &str = include_str!("fragments/tokens.pest");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        assert_eq!(generate_full_grammar(), generate_full_grammar());
    }

    #[test]
    fn all_constructs_present() {
        let g = generate_full_grammar();
        for name in [
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
            "import_decl",
        ] {
            assert!(g.contains(name), "Missing construct: {}", name);
        }
    }
}
