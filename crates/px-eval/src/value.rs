//! Runtime value helpers, ported from `praxis-native::px::eval`.
//!
//! The runtime value type is [`serde_json::Value`] (same as the canonical
//! evaluator). These helpers carry the exact numeric/comparison/truthiness/
//! string-concat/path-resolution semantics of the original so behavior is
//! preserved across the migration.

use serde_json::Value;

/// JavaScript-ish truthiness used by logic ops and conditionals.
pub fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => !s.is_empty() && s != "false",
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
    }
}

/// Coerce a value to `f64` for arithmetic/comparison (bools→0/1, numeric
/// strings parse). Non-coercible values yield `None`.
pub fn to_f64(val: &Value) -> Option<f64> {
    match val {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Convert an `f64` back to a JSON value, preferring an integer when whole.
pub fn f64_to_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
        Value::Number(serde_json::Number::from(n as i64))
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// Stringify a value for `+` concatenation / `str()` / `concat()`.
pub fn value_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(val).unwrap_or_default(),
    }
}

fn value_to_compare_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(val).unwrap_or_default(),
    }
}

fn both_numeric(left: &Value, right: &Value) -> bool {
    matches!(left, Value::Number(_)) && matches!(right, Value::Number(_))
}

/// Determine if a `+` operation should be string concatenation: true when
/// either operand is a non-numeric string.
pub fn is_string_concat(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(s), _) => s.parse::<f64>().is_err(),
        (_, Value::String(s)) => s.parse::<f64>().is_err(),
        _ => false,
    }
}

/// Structural/loose equality: exact JSON eq, cross-type numeric eq, else
/// string-form eq (so `42 == "42"` holds, matching the canonical evaluator).
pub fn values_equal(left: &Value, right: &Value) -> bool {
    if left == right {
        return true;
    }
    if both_numeric(left, right) {
        if let (Some(a), Some(b)) = (to_f64(left), to_f64(right)) {
            return (a - b).abs() < f64::EPSILON;
        }
    }
    value_to_compare_string(left) == value_to_compare_string(right)
}

/// Ordered comparison (`>`, `<`, `>=`, `<=`): numeric when both coerce, else
/// lexicographic on their compare-strings.
pub fn compare_ordered(left: &Value, op: &str, right: &Value) -> bool {
    if let (Some(a), Some(b)) = (to_f64(left), to_f64(right)) {
        return match op {
            ">=" => a >= b,
            "<=" => a <= b,
            ">" => a > b,
            "<" => a < b,
            _ => false,
        };
    }
    let l = value_to_compare_string(left);
    let r = value_to_compare_string(right);
    match op {
        ">=" => l >= r,
        "<=" => l <= r,
        ">" => l > r,
        "<" => l < r,
        _ => false,
    }
}

/// Resolve a JSON value along a `foo.bar[0]["k"]` accessor path (dot fields +
/// bracket indices/keys). Missing segments propagate as `Null`.
pub fn resolve_accessor(val: &Value, seg_is_bracket: bool, key: &str) -> Value {
    if seg_is_bracket {
        if let Ok(idx) = key.parse::<usize>() {
            val.get(idx).cloned().unwrap_or(Value::Null)
        } else {
            let k = key.trim_matches('"').trim_matches('\'');
            val.get(k).cloned().unwrap_or(Value::Null)
        }
    } else {
        val.get(key).cloned().unwrap_or(Value::Null)
    }
}
