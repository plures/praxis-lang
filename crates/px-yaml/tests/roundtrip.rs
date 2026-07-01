//! Round-trip proof: `from_yaml(to_yaml(doc)) == doc` for a representative
//! program that exercises many construct kinds.
//!
//! Equality is structural via [`px_yaml::ast_eq`] (canonical serde encoding),
//! because most `px-ast` nodes do not derive `PartialEq`. The document under
//! test is produced by the canonical `px-compiler` front end so we round-trip a
//! *real* AST, not a hand-built one.

/// A representative `.px` source touching import, entity, fact, config,
/// function, rule (with let/then/capture), and two constraints (with `when`,
/// `require`, severities) — plus the v1 expression layer and the type system.
const SRC: &str = r#"
import core::memory as mem
import core::llm

entity Conversation:
  prefix: "conv"
  fields:
    id: string
    participant: string
    turns: list[string]
    priority: int

fact MemoryEntry:
  content: string
  importance: float
  timestamp: int
  tags: map[string, string]
  labels: optional[string]

config defaults:
  max_turns: 200
  retention_days: 90
  scoring:
    base_importance: 0.5
    urgent_boost: 0.4

function classify_intent(message: string) -> string:
  mode: deterministic
  """Classify the user's intent from message content."""

rule capture_urgent:
  priority: 10
  when:
    - contains($message, "urgent")
    - $priority > 5
  let score = $priority + 5
  then:
    - action: flag_priority level: "high" score: $score
    - if $score > 12: action: page_oncall
  capture:
    - fact: "urgent_message" category: alerts tags: ["urgent", "priority"]

constraint no_empty_response:
  scope: response
  phase: pre_send
  require: len($response) > 0
  severity: error
  message: "Empty responses are never acceptable"

constraint bounded_turns:
  when: $turns > 0
  require: $turns <= 200
  severity: warning
  message: "Conversation exceeds recommended turn budget"
"#;

#[test]
fn from_yaml_of_to_yaml_reconstructs_the_same_ast() {
    // Canonical AST from the .px text front end.
    let doc = px_compiler::parse(SRC).expect("parse representative .px");

    // Serialize the AST to the YAML surface, then read it straight back.
    let yaml = px_yaml::to_yaml(&doc).expect("to_yaml");
    let round = px_yaml::from_yaml(&yaml).expect("from_yaml");

    // Same AST, two surfaces (structural serde equality — nodes lack PartialEq).
    assert!(
        px_yaml::ast_eq(&doc, &round),
        "round-trip AST mismatch.\n--- original json ---\n{}\n--- round-tripped json ---\n{}",
        serde_json::to_string_pretty(&doc).unwrap(),
        serde_json::to_string_pretty(&round).unwrap(),
    );

    // And the statement count is preserved (sanity beyond the opaque oracle).
    assert_eq!(doc.statements.len(), round.statements.len());
    assert_eq!(
        doc.statements.len(),
        9,
        "expected 9 top-level statements (2 import, entity, fact, config, function, rule, 2 constraint)"
    );
}

#[test]
fn to_yaml_is_deterministic() {
    let doc = px_compiler::parse(SRC).expect("parse representative .px");
    let a = px_yaml::to_yaml(&doc).expect("to_yaml #1");
    let b = px_yaml::to_yaml(&doc).expect("to_yaml #2");
    assert_eq!(a, b, "to_yaml must be deterministic for a stable surface");
}

#[test]
fn from_yaml_rejects_malformed_yaml() {
    // A YAML scalar where a document (mapping with `statements`) is required.
    let err = px_yaml::from_yaml("just a bare scalar, not a px document");
    assert!(err.is_err(), "malformed YAML must be an honest error");
}
