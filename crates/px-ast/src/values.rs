//! Literal values and references.
//!
//! These are the leaf nodes of expressions — the actual data that appears in .px files.

use crate::common::{DottedIdent, Ident, VarRef};
use crate::expressions::Expr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A value that can appear in declarations, step arguments, or config entries.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
