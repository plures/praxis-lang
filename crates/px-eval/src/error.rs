//! Evaluation error type.
//!
//! Ported from `praxis-native::px::eval::EvalError` (the canonical, cleaner
//! evaluator), extended with a real [`EvalError::Unsupported`] variant so any
//! construct that is not yet evaluable fails *honestly* (C-NOSTUB-001) rather
//! than silently returning a placeholder.

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
    /// A type mismatch in an operation (e.g. adding a list to an int).
    TypeError(String),
    /// The construct/expression form is recognized but not yet evaluable.
    /// Names the form so gaps are locatable and honest — never a silent stub.
    Unsupported(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::ParseError(msg) => write!(f, "parse error: {msg}"),
            EvalError::DivisionByZero => write!(f, "division by zero"),
            EvalError::UnknownFunction(name) => write!(f, "unknown function: {name}"),
            EvalError::FunctionError(msg) => write!(f, "function error: {msg}"),
            EvalError::TypeError(msg) => write!(f, "type error: {msg}"),
            EvalError::Unsupported(what) => write!(f, "unsupported for evaluation: {what}"),
        }
    }
}

impl std::error::Error for EvalError {}
