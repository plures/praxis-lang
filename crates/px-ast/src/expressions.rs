//! Expression AST nodes.
//!
//! Two expression systems:
//! - **v1 Expr**: Used in declarative contexts (rule conditions, constraint requires, etc.)
//!   Simpler, supports YAML-friendly operators (AND, OR, NOT).
//! - **v2 CodeExpr**: Used inside code blocks. Full Rust-style operator precedence,
//!   closures, match expressions, parallel expressions.

use crate::common::*;
use crate::values::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// V1 EXPRESSIONS (declarative contexts)
// Operator precedence (low to high):
//   logic (&&, ||, AND, OR) → comparison (==, !=, >, <, >=, <=) →
//   additive (+, -) → multiplicative (*, /, %) → power (^) → unary (!, -, NOT)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Eq => "==",
            BinOp::Neq => "!=",
            BinOp::Gt => ">",
            BinOp::Lt => "<",
            BinOp::Gte => ">=",
            BinOp::Lte => "<=",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Pow => "^",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum UnaryOp {
    Not,
    Neg,
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            UnaryOp::Not => "!",
            UnaryOp::Neg => "-",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExprMatchArm {
    pub pattern: ExprMatchPattern,
    pub result: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum ExprMatchPattern {
    Wildcard,
    Values(Vec<Value>),
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::InlineIf {
                condition,
                then_val,
                else_val,
            } => write!(f, "if {}: {} else: {}", condition, then_val, else_val),
            Expr::Binary { left, op, right } => write!(f, "{} {} {}", left, op, right),
            Expr::Unary { op, operand } => write!(f, "{}{}", op, operand),
            Expr::Match { subject, arms } => {
                write!(f, "match {} {{", subject)?;
                for (i, arm) in arms.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} => {}", arm.pattern, arm.result)?;
                }
                write!(f, "}}")
            }
            Expr::Call { name, args } => {
                write!(f, "{}(", name)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
            Expr::Path(p) => write!(f, "{}", p),
            Expr::Var(v) => write!(f, "{}", v),
            Expr::Literal(v) => write!(f, "{}", v),
            Expr::Paren(inner) => write!(f, "({})", inner),
        }
    }
}

impl std::fmt::Display for ExprMatchPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExprMatchPattern::Wildcard => write!(f, "_"),
            ExprMatchPattern::Values(values) => {
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", v)?;
                }
                Ok(())
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// V2 CODE EXPRESSIONS (Rust-style, inside code blocks)
// Operator precedence (low to high):
//   inline_if → logic (&&, ||) → comparison (==, !=, >, <, >=, <=) →
//   additive (+, -) → multiplicative (*, /, %) → power (^) → unary (!, -)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum CodeAccess {
    /// `.field`
    Dot(Ident),
    /// `[expr]`
    Bracket(Box<CodeExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum CodeLiteral {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}
