//! Literal values and references.
//!
//! These are the leaf nodes of expressions — the actual data that appears in .px files.

use crate::common::{DottedIdent, Ident, VarRef};
use crate::expressions::Expr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A value that can appear in declarations, step arguments, or config entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum Value {
    /// `"hello"` or `'hello'`
    String(String),
    /// `42` or `-7`
    Integer(i64),
    /// `3.14` or `-0.5`
    Float(f64),
    /// `true` or `false`
    Boolean(bool),
    /// `[val1, val2, ...]`
    List(Vec<Value>),
    /// `{key: value, key: value, ...}`
    Map(Vec<(Ident, Value)>),
    /// Function call used as value: `func(args)`
    Call { name: Ident, args: Vec<Expr> },
    /// Arithmetic expression: `a + b` (simple binary in value position)
    Arithmetic {
        left: Box<Value>,
        op: ArithOp,
        right: Box<Value>,
    },
    /// Variable reference: `$var` or `$var.field`
    Var(VarRef),
    /// Dotted identifier path: `foo.bar.baz`
    Path(DottedIdent),
    /// Bare identifier used as value (e.g., enum variant name)
    Ident(Ident),
    /// Parenthesized expression
    Paren(Box<Expr>),
    /// Null (explicit absence)
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl std::fmt::Display for ArithOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
            ArithOp::Mod => "%",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(v) => write!(f, "{}", v),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Call { name, args } => {
                write!(f, "{}(", name)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
            Value::Arithmetic { left, op, right } => write!(f, "{} {} {}", left, op, right),
            Value::Var(v) => write!(f, "{}", v),
            Value::Path(p) => write!(f, "{}", p),
            Value::Ident(id) => write!(f, "{}", id),
            Value::Paren(inner) => write!(f, "({})", inner),
            Value::Null => write!(f, "null"),
        }
    }
}
