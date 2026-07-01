//! Shared primitives used across all AST nodes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Source span — byte offsets into the original .px source.
/// Optional; not all AST nodes have spans (e.g., synthesized nodes).
///
/// Spans are an **editor-only** concern. They are excluded from the schema
/// projection and from serialized output (`#[serde(skip)]` on every `span`
/// field) so the generated `px.schema.json` describes the pure language shape,
/// not positional noise (ADR §M4 / PXLANG-M2 §3.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// An identifier (variable name, construct name, field name).
/// Invariant: ASCII_ALPHA | "_" followed by ASCII_ALPHANUMERIC | "_"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Ident {
    pub name: String,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

impl Ident {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            span: None,
        }
    }

    pub fn with_span(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span: Some(span),
        }
    }
}

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A string literal (single or double quoted in source).
/// The value is the content WITHOUT surrounding quotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StringLiteral {
    pub value: String,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

impl StringLiteral {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            span: None,
        }
    }
}

impl From<&str> for StringLiteral {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// A variable reference: $name or $name.field or $name["key"]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VarRef {
    pub name: Ident,
    pub accessors: Vec<Accessor>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

/// Access path segment on a variable or expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum Accessor {
    /// .field_name
    Dot(Ident),
    /// ["key"] or [0]
    Bracket(String),
}

/// A dotted identifier path (e.g., `module.submodule.name`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DottedIdent {
    pub segments: Vec<Ident>,
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Option<Span>,
}

impl DottedIdent {
    pub fn single(name: impl Into<String>) -> Self {
        Self {
            segments: vec![Ident::new(name)],
            span: None,
        }
    }

    pub fn from_parts(parts: Vec<&str>) -> Self {
        Self {
            segments: parts.into_iter().map(Ident::new).collect(),
            span: None,
        }
    }
}
