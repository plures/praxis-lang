//! Expression AST nodes.
//!
//! Two expression systems:
//! - **v1 Expr**: Used in declarative contexts (rule conditions, constraint requires, etc.)
//!   Simpler, supports YAML-friendly operators (AND, OR, NOT).
//! - **v2 CodeExpr**: Used inside code blocks. Full Rust-style operator precedence,
//!   closures, match expressions, parallel expressions.

use crate::common::*;
use crate::values::Value;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// V1 EXPRESSIONS (declarative contexts)
// Operator precedence (low to high):
//   logic (&&, ||, AND, OR) → comparison (==, !=, >, <, >=, <=) →
//   additive (+, -) → multiplicative (*, /, %) → power (^) → unary (!, -, NOT)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    /// `if cond: then_val else: else_val`
    InlineIf {
        condition: Box<Expr>,
        then_val: Box<Expr>,
        else_val: Box<Expr>,
    },
    /// Binary operation (logic, comparison, arithmetic)
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    /// Unary (!, NOT, -)
    Unary { op: UnaryOp, operand: Box<Expr> },
    /// `match subject { pattern => result, ... }`
    Match {
        subject: Box<Expr>,
        arms: Vec<ExprMatchArm>,
    },
    /// Function/action call: `name(arg1, arg2)`
    Call { name: Ident, args: Vec<Expr> },
    /// Dotted identifier: `foo.bar.baz`
    Path(DottedIdent),
    /// Variable reference: `$var` or `$var.field`
    Var(VarRef),
    /// Literal value
    Literal(Value),
    /// Parenthesized: `(expr)`
    Paren(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    // Logic (lowest precedence)
    And,
    Or,
    // Comparison
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    // Additive
    Add,
    Sub,
    // Multiplicative
    Mul,
    Div,
    Mod,
    // Power (highest binary precedence)
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExprMatchArm {
    pub pattern: ExprMatchPattern,
    pub result: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprMatchPattern {
    Wildcard,
    Values(Vec<Value>),
}

// ═══════════════════════════════════════════════════════════════════════════════
// V2 CODE EXPRESSIONS (Rust-style, inside code blocks)
// Operator precedence (low to high):
//   inline_if → logic (&&, ||) → comparison (==, !=, >, <, >=, <=) →
//   additive (+, -) → multiplicative (*, /, %) → power (^) → unary (!, -)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeExpr {
    /// `if cond { then_expr } else { else_expr }`
    InlineIf {
        condition: Box<CodeExpr>,
        then_val: Box<CodeExpr>,
        else_val: Box<CodeExpr>,
    },
    /// Binary operation
    Binary {
        left: Box<CodeExpr>,
        op: BinOp,
        right: Box<CodeExpr>,
    },
    /// Unary (!, -)
    Unary { op: UnaryOp, operand: Box<CodeExpr> },
    /// Function call: `name(args...)`
    Call {
        name: Ident,
        args: Vec<CodeExpr>,
        access_chain: Vec<CodeAccess>,
    },
    /// Variable/path access: `foo.bar[0]`
    Access {
        base: DottedIdent,
        chain: Vec<CodeAccess>,
    },
    /// Closure: `|x, y| expr`
    Closure {
        params: Vec<Ident>,
        body: Box<CodeExpr>,
    },
    /// Object literal: `{ key: value, ... }`
    Object(Vec<(Ident, CodeExpr)>),
    /// Parallel expression: `parallel { branch: { ... }, ... }`
    Parallel(Vec<(Ident, crate::procedures::CodeBlock)>),
    /// Literal value
    Literal(CodeLiteral),
    /// Parenthesized: `(expr)`
    Paren(Box<CodeExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeAccess {
    /// `.field`
    Dot(Ident),
    /// `[expr]`
    Bracket(Box<CodeExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeLiteral {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}
