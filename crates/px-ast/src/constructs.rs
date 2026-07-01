//! Top-level declaration constructs.
//!
//! These map 1:1 to .px keywords that appear at the document root:
//! entity, config, fact, rule, constraint, contract, function, trigger, scenario, import.

use crate::common::*;
use crate::expressions::Expr;
use crate::types::TypeExpr;
use crate::values::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// IMPORT
// ═══════════════════════════════════════════════════════════════════════════════

/// `import path::to::module as alias`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImportDecl {
    /// The Rust-style path (e.g., `core::memory`)
    pub path: Vec<Ident>,
    /// Optional alias
    pub alias: Option<Ident>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENTITY (typed PluresDB node schemas)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// entity Player:
///   prefix: "player"
///   fields:
///     health: int
///     name: string
///     inventory: list[string]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityDecl {
    pub name: Ident,
    pub prefix: Option<StringLiteral>,
    pub fields: Vec<FieldDecl>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

/// A typed field declaration used in entities, facts, and function params.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FieldDecl {
    pub name: Ident,
    pub field_type: TypeExpr,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG (static key-value blocks)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// config defaults:
///   max_retries: 3
///   timeout: 5000
///   nested:
///     key: "value"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigDecl {
    pub name: Ident,
    pub entries: Vec<ConfigEntry>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigEntry {
    pub key: Ident,
    pub value: ConfigValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum ConfigValue {
    Scalar(Value),
    Nested(Vec<ConfigEntry>),
}

// ═══════════════════════════════════════════════════════════════════════════════
// FACT (lightweight typed structures)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// fact MemoryEntry:
///   content: string
///   category: string
///   timestamp: int
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FactDecl {
    pub name: Ident,
    pub fields: Vec<FieldDecl>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// RULE (reactive: when conditions → then actions)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// rule detect_urgency:
///   priority: 10
///   when:
///     - message.contains("urgent")
///   then:
///     - action: flag_priority level: "high"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuleDecl {
    pub name: Ident,
    pub priority: Option<i64>,
    pub conditions: Vec<Expr>,
    pub let_bindings: Vec<LetBinding>,
    pub actions: Vec<ActionStmt>,
    pub captures: Vec<CaptureEntry>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LetBinding {
    pub name: Ident,
    pub value: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionStmt {
    /// Optional condition (if expr: action)
    pub condition: Option<Expr>,
    pub action_name: Ident,
    pub params: Vec<ParamPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParamPair {
    pub key: Ident,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaptureEntry {
    pub fact: StringLiteral,
    pub category: Option<Ident>,
    pub tags: Option<Vec<Value>>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTRAINT (invariant enforcement)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// constraint no_empty_responses:
///   scope: response
///   phase: pre_send
///   require: response.length > 0
///   severity: error
///   message: "Empty responses are never acceptable"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConstraintDecl {
    pub name: Ident,
    pub scope: Option<Ident>,
    pub phase: Vec<Ident>,
    pub trait_name: Option<Ident>,
    pub weight: Option<f64>,
    pub prompt: Option<StringLiteral>,
    pub when: Option<Expr>,
    pub require: Option<Expr>,
    pub severity: Severity,
    pub message: Option<StringLiteral>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONTRACT (behavioral specifications with examples)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// contract tone_matching:
///   given: "User sends casual message"
///   when: "Responding to user"
///   then: "Match casual tone"
///   threshold: 0.8
///   examples:
///     - input: "hey what's up"
///       expect: "casual_tone"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContractDecl {
    pub name: Ident,
    pub given: Option<StringLiteral>,
    pub when: Option<StringLiteral>,
    pub then: Option<StringLiteral>,
    pub threshold: Option<f64>,
    pub examples: Vec<ContractExample>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContractExample {
    pub input: Value,
    pub expect: Value,
    pub threshold: Option<f64>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// FUNCTION (typed signatures with docstrings)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// function classify_intent(message: string) -> string:
///   mode: deterministic
///   """Classify the user's intent from message content."""
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FunctionDecl {
    pub name: Ident,
    pub params: Vec<FieldDecl>,
    pub return_type: TypeExpr,
    pub mode: Option<FunctionMode>,
    pub docstring: Option<String>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FunctionMode {
    Deterministic,
    Probabilistic,
    Hybrid,
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRIGGER (legacy — compiles to single-input dataflow)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// trigger daily_consolidation:
///   on: timer
///   schedule: "0 3 * * *"
///   run: consolidate_memories
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TriggerDecl {
    pub name: Ident,
    pub event: TriggerEvent,
    pub schedule: Option<StringLiteral>,
    pub run: Ident,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum TriggerEvent {
    AfterStore,
    BeforeSearch,
    OnEvent(StringLiteral),
    Timer,
}

// ═══════════════════════════════════════════════════════════════════════════════
// SCENARIO (test specifications)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// scenario test_routing:
///   given: "User sends a question"
///   setup:
///     define $msg = "What time is it?"
///   run: classify_intent {input: $msg}
///   expect:
///     - result_contains {text: "question"}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioDecl {
    pub name: Ident,
    pub given: Option<StringLiteral>,
    pub setup: Vec<crate::procedures::Step>,
    pub run: Option<ScenarioRun>,
    pub expectations: Vec<Expectation>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioRun {
    pub procedure: Ident,
    pub args: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Expectation {
    pub negated: bool,
    pub name: Ident,
    pub args: Option<Value>,
}
