//! Type system for .px declarations.
//!
//! Base types: bool, int, float, string, duration
//! Generics: list[T], optional[T], map[K, V]
//! Enums: enum(variant1, variant2, ...)
//! User-defined: any Ident that isn't a base type

use crate::common::Ident;
use serde::{Deserialize, Serialize};

/// A type expression used in field declarations, params, and return types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    /// Primitive: `bool`, `int`, `float`, `string`, `duration`
    Base(BaseType),
    /// User-defined type name (refers to an entity or fact)
    Named(Ident),
    /// `list[T]`
    List(Box<TypeExpr>),
    /// `optional[T]`
    Optional(Box<TypeExpr>),
    /// `map[K, V]`
    Map(Box<TypeExpr>, Box<TypeExpr>),
    /// `enum(a, b, c)`
    Enum(Vec<Ident>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BaseType {
    Bool,
    Int,
    Float,
    String,
    Duration,
}

impl std::fmt::Display for BaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaseType::Bool => write!(f, "bool"),
            BaseType::Int => write!(f, "int"),
            BaseType::Float => write!(f, "float"),
            BaseType::String => write!(f, "string"),
            BaseType::Duration => write!(f, "duration"),
        }
    }
}

impl std::fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeExpr::Base(b) => write!(f, "{}", b),
            TypeExpr::Named(id) => write!(f, "{}", id),
            TypeExpr::List(inner) => write!(f, "list[{}]", inner),
            TypeExpr::Optional(inner) => write!(f, "optional[{}]", inner),
            TypeExpr::Map(k, v) => write!(f, "map[{}, {}]", k, v),
            TypeExpr::Enum(variants) => {
                write!(f, "enum(")?;
                for (i, v) in variants.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }
        }
    }
}
