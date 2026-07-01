//! The function-registry trait seam + a pure built-in registry.
//!
//! `.px` expressions can call functions (`len($x)`, `upper($s)`, ...). The
//! canonical evaluator in `praxis-native` reaches into a `NativeFunctionRegistry`
//! that is entangled with runtime/storage effects. To keep `px-eval` free of any
//! `pluresdb`/host dependency (a hard rule for this crate), we invert that: the
//! evaluator depends only on the [`FunctionRegistry`] **trait**, and callers plug
//! in whatever implementation they need (host effects, storage, etc.).
//!
//! [`PureFunctionRegistry`] is a real, dependency-free implementation of the
//! side-effect-free builtins — every function here genuinely computes its result
//! (no stubs). Effectful/host functions are intentionally *absent* here; a host
//! provides them via its own `FunctionRegistry` impl.

use serde_json::Value;

use crate::value::{to_f64, value_to_string};

/// The pluggable function boundary for evaluation.
///
/// Implementors expose named functions callable from `.px` expressions. This is
/// the single seam through which effects/host capabilities enter evaluation —
/// `px-eval` itself never links a host or storage layer.
pub trait FunctionRegistry {
    /// Call `name` with already-evaluated `args`. Return the result value, or a
    /// human-readable error string (surfaced as [`crate::EvalError::FunctionError`]).
    ///
    /// Return `Err` with a not-found marker only if the function is unknown; the
    /// evaluator distinguishes unknown-vs-failed via [`FunctionRegistry::contains`].
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, String>;

    /// Whether `name` is provided by this registry.
    fn contains(&self, name: &str) -> bool;
}

/// A registry with no functions. Any call is an unknown-function error.
///
/// Useful for evaluating pure-arithmetic/logic expressions that must not call
/// out to anything.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyRegistry;

impl FunctionRegistry for EmptyRegistry {
    fn call(&self, name: &str, _args: &[Value]) -> Result<Value, String> {
        Err(format!("no functions registered (called `{name}`)"))
    }
    fn contains(&self, _name: &str) -> bool {
        false
    }
}

/// The default registry of **pure** (side-effect-free) built-in functions.
///
/// Every function is really implemented over its arguments. Nothing here touches
/// I/O, storage, time, or randomness — those belong in a host-provided registry.
#[derive(Debug, Default, Clone, Copy)]
pub struct PureFunctionRegistry;

impl PureFunctionRegistry {
    /// The set of function names this registry provides.
    pub const NAMES: &'static [&'static str] = &[
        "len",
        "upper",
        "lower",
        "trim",
        "abs",
        "min",
        "max",
        "floor",
        "ceil",
        "round",
        "sqrt",
        "contains",
        "starts_with",
        "ends_with",
        "concat",
        "coalesce",
        "not",
        "bool",
        "int",
        "float",
        "str",
    ];
}

impl FunctionRegistry for PureFunctionRegistry {
    fn contains(&self, name: &str) -> bool {
        Self::NAMES.contains(&name)
    }

    fn call(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        match name {
            // ── sequence / string length ──
            "len" => {
                let a = arg(args, 0, name)?;
                let n = match a {
                    Value::String(s) => s.chars().count(),
                    Value::Array(v) => v.len(),
                    Value::Object(m) => m.len(),
                    Value::Null => 0,
                    other => return Err(format!("len: unsupported type {other:?}")),
                };
                Ok(Value::from(n as i64))
            }
            // ── string case / trim ──
            "upper" => Ok(Value::String(
                value_to_string(arg(args, 0, name)?).to_uppercase(),
            )),
            "lower" => Ok(Value::String(
                value_to_string(arg(args, 0, name)?).to_lowercase(),
            )),
            "trim" => Ok(Value::String(
                value_to_string(arg(args, 0, name)?).trim().to_string(),
            )),
            // ── numeric ──
            "abs" => Ok(num(num_arg(args, 0, name)?.abs())),
            "floor" => Ok(num(num_arg(args, 0, name)?.floor())),
            "ceil" => Ok(num(num_arg(args, 0, name)?.ceil())),
            "round" => Ok(num(num_arg(args, 0, name)?.round())),
            "sqrt" => {
                let x = num_arg(args, 0, name)?;
                if x < 0.0 {
                    return Err("sqrt: negative argument".to_string());
                }
                Ok(num(x.sqrt()))
            }
            "min" => fold_numeric(args, name, f64::min),
            "max" => fold_numeric(args, name, f64::max),
            // ── containment / predicates ──
            "contains" => {
                let hay = arg(args, 0, name)?;
                let needle = arg(args, 1, name)?;
                let result = match hay {
                    Value::String(s) => s.contains(&value_to_string(needle)),
                    Value::Array(v) => v.iter().any(|e| e == needle),
                    _ => false,
                };
                Ok(Value::Bool(result))
            }
            "starts_with" => Ok(Value::Bool(
                value_to_string(arg(args, 0, name)?)
                    .starts_with(&value_to_string(arg(args, 1, name)?)),
            )),
            "ends_with" => Ok(Value::Bool(
                value_to_string(arg(args, 0, name)?)
                    .ends_with(&value_to_string(arg(args, 1, name)?)),
            )),
            // ── combinators ──
            "concat" => {
                let mut s = String::new();
                for a in args {
                    s.push_str(&value_to_string(a));
                }
                Ok(Value::String(s))
            }
            "coalesce" => Ok(args
                .iter()
                .find(|v| !v.is_null())
                .cloned()
                .unwrap_or(Value::Null)),
            "not" => Ok(Value::Bool(!crate::value::is_truthy(arg(args, 0, name)?))),
            "bool" => Ok(Value::Bool(crate::value::is_truthy(arg(args, 0, name)?))),
            // ── casts ──
            "int" => {
                let x = num_arg(args, 0, name)?;
                Ok(Value::from(x.trunc() as i64))
            }
            "float" => Ok(num(num_arg(args, 0, name)?)),
            "str" => Ok(Value::String(value_to_string(arg(args, 0, name)?))),
            other => Err(format!("unknown pure function `{other}`")),
        }
    }
}

fn arg<'a>(args: &'a [Value], i: usize, name: &str) -> Result<&'a Value, String> {
    args.get(i)
        .ok_or_else(|| format!("{name}: missing argument {}", i + 1))
}

fn num_arg(args: &[Value], i: usize, name: &str) -> Result<f64, String> {
    let a = arg(args, i, name)?;
    to_f64(a).ok_or_else(|| format!("{name}: argument {} is not numeric ({a:?})", i + 1))
}

fn num(n: f64) -> Value {
    crate::value::f64_to_value(n)
}

fn fold_numeric(args: &[Value], name: &str, f: fn(f64, f64) -> f64) -> Result<Value, String> {
    if args.is_empty() {
        return Err(format!("{name}: requires at least one argument"));
    }
    let mut acc = num_arg(args, 0, name)?;
    for i in 1..args.len() {
        acc = f(acc, num_arg(args, i, name)?);
    }
    Ok(num(acc))
}
