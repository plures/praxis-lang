//! # px-ast — Canonical AST for the Praxis Intent Language (.px)
//!
//! This crate defines the **single source of truth** for all .px language constructs.
//! Every .px file, parser, validator, and code generator must agree with these types.
//!
//! ## Design Principles
//!
//! 1. **These types ARE the language spec.** If it's not here, it doesn't exist in .px.
//! 2. **Grammar is derived from these types** (ADR-0021). Never edit grammar.pest manually.
//! 3. **Two expression systems**: v1 (declarative, YAML contexts) and v2 (Rust-style code blocks).
//! 4. **Procedures are queue-driven** (v3): typed args from named queues, no triggers.
//! 5. **Declarations are YAML-native**: keyword name:\n  fields...
//!
//! ## Module Organization
//!
//! - `constructs` — Top-level declarations (entity, config, fact, rule, constraint, etc.)
//! - `procedures` — Procedure variants (dataflow v3, legacy v1, code blocks v2)
//! - `expressions` — v1 declarative expressions + v2 code expressions
//! - `types` — Type system (base types, generics, user-defined)
//! - `values` — Literal values, identifiers, references
//! - `common` — Shared primitives (Ident, StringLiteral, Span)

pub mod common;
pub mod constructs;
pub mod expressions;
pub mod procedures;
pub mod types;
pub mod values;

pub use common::*;
pub use constructs::*;
pub use expressions::*;
pub use procedures::*;
pub use types::*;
pub use values::*;

/// A complete .px document — the root AST node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PxDocument {
    pub statements: Vec<Statement>,
}

/// A top-level statement in a .px file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Statement {
    Import(ImportDecl),
    Entity(EntityDecl),
    Config(ConfigDecl),
    Fact(FactDecl),
    Rule(RuleDecl),
    Constraint(ConstraintDecl),
    Contract(ContractDecl),
    Function(FunctionDecl),
    Trigger(TriggerDecl),
    DataflowProcedure(DataflowProcedureDecl),
    LegacyProcedure(LegacyProcedureDecl),
    Scenario(ScenarioDecl),
}
