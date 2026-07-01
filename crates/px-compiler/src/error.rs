//! Compiler error type.

use std::fmt;

/// An error produced while compiling `.px` source text into the canonical AST.
#[derive(Debug, Clone)]
pub enum CompileError {
    /// The pest grammar rejected the source (syntax error). Carries the
    /// human-readable pest diagnostic (line/column + expected rules).
    Parse(String),
    /// The parse tree was structurally valid per the grammar, but a builder
    /// encountered a shape it does not know how to lower into the AST. This is
    /// a *real* error surfaced to the caller (never a silent stub): it names
    /// the offending grammar rule so gaps are honest and locatable.
    Unsupported {
        /// The grammar rule (or construct) that is not yet lowered.
        rule: String,
        /// Why / what was expected.
        detail: String,
    },
    /// An internal invariant was violated while walking the tree (e.g. a rule
    /// that the grammar guarantees to be present was missing). Indicates a
    /// grammar/builder mismatch, not user error.
    Internal(String),
}

impl CompileError {
    pub(crate) fn unsupported(rule: impl Into<String>, detail: impl Into<String>) -> Self {
        CompileError::Unsupported {
            rule: rule.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn internal(msg: impl Into<String>) -> Self {
        CompileError::Internal(msg.into())
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Parse(msg) => write!(f, "parse error: {msg}"),
            CompileError::Unsupported { rule, detail } => {
                write!(f, "unsupported construct `{rule}`: {detail}")
            }
            CompileError::Internal(msg) => write!(f, "internal compiler error: {msg}"),
        }
    }
}

impl std::error::Error for CompileError {}
