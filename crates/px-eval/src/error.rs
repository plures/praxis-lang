//! Evaluation error type.
//!
//! Ported from `praxis-native::px::eval::EvalError` (the canonical, cleaner
//! evaluator). Every variant is really constructed on a real failure path —
//! there is no catch-all placeholder. Constructs this crate does not evaluate
//! this wave (v2 code blocks) are *absent from the API surface* rather than
//! represented by a dead "unsupported" error (C-NOSTUB-001, honest-absence form).

use std::fmt;

/// Errors that can occur during `.px` evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    /// Source expression failed to parse (via `px-compiler`).
    ParseError(String),
    /// Division (or modulo) by zero.
    DivisionByZero,
    /// A called function is not present in the active [`crate::FunctionRegistry`].
    UnknownFunction(String),
    /// A registered function returned an error.
    FunctionError(String),
    /// A type mismatch in an operation (e.g. negating a list).
    TypeError(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::ParseError(msg) => write!(f, "parse error: {msg}"),
            EvalError::DivisionByZero => write!(f, "division by zero"),
            EvalError::UnknownFunction(name) => write!(f, "unknown function: {name}"),
            EvalError::FunctionError(msg) => write!(f, "function error: {msg}"),
            EvalError::TypeError(msg) => write!(f, "type error: {msg}"),
        }
    }
}

impl std::error::Error for EvalError {}
