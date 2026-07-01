//! Procedure declarations — the executable logic layer.
//!
//! Two forms:
//! - **DataflowProcedureDecl** (v3, PRIMARY): Queue-driven, typed args, no triggers.
//! - **LegacyProcedureDecl** (v1): Step-list with optional trigger. Kept for migration.
//!
//! Procedure bodies can be either:
//! - **StepList** (v1): Declarative step-by-step `define`, `call`, `if`, `for`, etc.
//! - **CodeBlock** (v2): Rust-style `{ let x = f(); if x { ... } }`

use crate::common::*;
use crate::expressions::{CodeExpr, Expr};
use crate::types::TypeExpr;
use crate::values::Value;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// DATAFLOW PROCEDURE (v3 — PRIMARY FORM)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// procedure classify_and_route(msg: InboundMessage from "inbound") -> RouteDecision into "route_decision":
///   given: "Classify message intent and determine routing"
///   classify_intent msg.content -> $intent
///   route_by_intent $intent -> $destination
///   return {intent: $intent, destination: $destination}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowProcedureDecl {
    pub name: Ident,
    pub params: Vec<DataflowParam>,
    pub return_type: Option<DataflowReturn>,
    pub given: Option<StringLiteral>,
    pub body: ProcedureBody,
    pub span: Option<Span>,
}

/// A typed parameter with optional queue binding.
/// `name: Type from "queue_name"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowParam {
    pub name: Ident,
    pub param_type: TypeExpr,
    /// The queue this parameter reads from.
    pub source_queue: Option<StringLiteral>,
}

/// Return type with optional queue binding.
/// `-> Type into "queue_name"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowReturn {
    pub return_type: TypeExpr,
    /// The queue output is pushed to.
    pub dest_queue: Option<StringLiteral>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// LEGACY PROCEDURE (v1 — kept for backward compatibility)
// ═══════════════════════════════════════════════════════════════════════════════

/// ```px
/// procedure handle_message:
///   trigger: on_write("inbound")
///   params: [$message]
///   given: "Process an incoming message"
///   steps:
///     classify_intent $message -> $intent
///     ...
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProcedureDecl {
    pub name: Ident,
    pub trigger: Option<ProcedureTrigger>,
    pub params: Vec<Ident>,
    pub given: Option<StringLiteral>,
    pub body: ProcedureBody,
    pub span: Option<Span>,
}

/// Legacy trigger (deprecated — prefer dataflow queue bindings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcedureTrigger {
    Periodic {
        interval: Option<Value>,
    },
    OnWrite {
        pattern: Option<StringLiteral>,
        args: Option<Value>,
    },
    OnEvent(StringLiteral),
    Startup,
    BeforeResponse,
    AfterResponse,
    Cron {
        schedule: Option<Value>,
    },
    Manual,
}

// ═══════════════════════════════════════════════════════════════════════════════
// PROCEDURE BODY (v1 step-list OR v2 code block)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcedureBody {
    /// v1: Declarative step list
    Steps(Vec<Step>),
    /// v2: Rust-style code block with braces
    Code(CodeBlock),
}

// ═══════════════════════════════════════════════════════════════════════════════
// V1 STEPS
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Step {
    /// `define $var = value`
    Define { var: Ident, value: Value },
    /// `return value`
    Return { value: Option<Value> },
    /// `abort value`
    Abort { value: Option<Value> },
    /// `action_name arg1 arg2` or `action_name key: value`
    Call(StepCall),
    /// `$var = value`
    Assign { target: VarRef, value: String },
    /// `if expr:\n  steps...\nelse:\n  steps...\nend`
    If {
        condition: Expr,
        then_steps: Vec<Step>,
        else_steps: Option<Vec<Step>>,
    },
    /// `match:\n  pattern -> action\nend`
    Match { arms: Vec<MatchArm> },
    /// `when expr:\n  steps...\nend`
    When { condition: Expr, steps: Vec<Step> },
    /// `for $var in collection:\n  steps...\nend`
    For {
        var: VarRef,
        collection: Expr,
        steps: Vec<Step>,
    },
    /// `loop over $list as item:\n  steps...\nend`
    Loop(LoopStep),
    /// `emit key: value ...`
    Emit { params: Vec<(Ident, Value)> },
    /// `try [retry N]:\n  steps...\ncatch:\n  steps...\nend`
    Try(TryStep),
    /// `parallel:\n  branch name:\n    steps...\nend`
    Parallel(ParallelStep),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepCall {
    pub action: Ident,
    pub args: StepCallArgs,
    /// Optional output variable: `action -> $result`
    pub output: Option<Ident>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepCallArgs {
    /// `action(expr, expr, ...)`
    Positional(Vec<Expr>),
    /// `action {key: value, ...}`
    Map(Value),
    /// `action key: value key: value`
    Params(Vec<(Ident, Value)>),
    /// `action value value value`
    Values(Vec<Value>),
    /// No arguments
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Expr,
    pub target: Ident,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopStep {
    pub source: LoopSource,
    pub item_name: Option<Ident>,
    pub key_name: Option<Ident>,
    pub output: Option<Ident>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopSource {
    Over(Ident),
    Times(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryStep {
    pub retries: Option<i64>,
    pub retry_opts: Vec<RetryOpt>,
    pub steps: Vec<Step>,
    pub catch: Option<Vec<Step>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryOpt {
    Delay(i64),
    Backoff(BackoffStrategy),
    MaxDelay(i64),
    Jitter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Exponential,
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelStep {
    pub output: Option<Ident>,
    pub branches: Vec<ParallelBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelBranch {
    pub name: Ident,
    pub retries: Option<i64>,
    pub retry_opts: Vec<RetryOpt>,
    pub steps: Vec<Step>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// V2 CODE BLOCK (Rust-style imperative body)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub statements: Vec<CodeStmt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeStmt {
    /// `let name = expr;`
    Let { name: Ident, value: CodeExpr },
    /// `lvalue = expr;` or `lvalue += expr;`
    Assign {
        target: String,
        op: AssignOp,
        value: CodeExpr,
    },
    /// `if expr { ... } else { ... }`
    If {
        condition: CodeExpr,
        then_block: CodeBlock,
        else_clause: Option<ElseClause>,
    },
    /// `for name in expr { ... }`
    For {
        var: Ident,
        iter: CodeExpr,
        body: CodeBlock,
    },
    /// `match expr { pattern => result, ... }`
    Match {
        subject: CodeExpr,
        arms: Vec<CodeMatchArm>,
    },
    /// `try { ... } catch name { ... }`
    Try {
        body: CodeBlock,
        catch: Option<(Option<Ident>, CodeBlock)>,
    },
    /// `parallel { branch_name: { ... }, ... }`
    Parallel { branches: Vec<(Ident, CodeBlock)> },
    /// `return expr;`
    Return { value: Option<CodeExpr> },
    /// `emit(queue, value);`
    Emit {
        queue: CodeExpr,
        value: Option<CodeExpr>,
    },
    /// Expression statement: `expr;`
    Expr(CodeExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignOp {
    Set,       // =
    AddAssign, // +=
    SubAssign, // -=
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElseClause {
    ElseIf(Box<CodeStmt>), // else if { ... }
    Else(CodeBlock),       // else { ... }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMatchArm {
    pub pattern: CodePattern,
    pub body: CodeMatchBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodePattern {
    Wildcard,
    Expr(CodeExpr),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeMatchBody {
    Block(CodeBlock),
    Expr(CodeExpr),
}
