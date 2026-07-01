//! # px-napi — NAPI-RS bindings for the `.px` Praxis Intent Language
//!
//! Node/TS consumers load this crate as a native `.node` addon and drive the
//! **canonical Rust engine** directly (ADR pillar P5 — "broad support via
//! NAPI"). There is no re-implementation and no canned data here: every entry
//! point delegates to `px-compiler` (the real parser) and `px-eval` (the real
//! evaluator), and returns their genuine results as JSON strings.
//!
//! ## Exposed functions (`#[napi]`)
//!
//! - [`parse`] — parse a `.px` source string via `px-compiler` and return the
//!   canonical AST ([`px_ast::PxDocument`]) as a JSON string.
//! - [`evaluate`] — evaluate a single v1 `.px` expression string against a JSON
//!   variable map, via `px-eval`; returns the result value as JSON.
//! - [`check_constraints`] — parse a `.px` source, evaluate **every**
//!   `constraint` in it against a JSON variable map via `px-eval`, and return a
//!   JSON array of per-constraint outcomes (satisfied / violated / not
//!   applicable, with severity + message on violation).
//! - [`px_ast_version`] — the `px-ast` crate version the addon was built
//!   against (so JS callers can assert engine/schema alignment).
//!
//! JSON is used as the boundary type because the AST and evaluation results are
//! already `serde`-serializable and JSON is the natural N-API interchange for
//! structured data; TS callers `JSON.parse` the returned strings.
//!
//! `#![forbid(unsafe_code)]` is intentionally NOT set here: `napi-derive`
//! generates the required `unsafe extern "C"` N-API shims. All hand-written
//! logic in this crate is safe Rust; the only unsafe is macro-generated glue.

use std::collections::HashMap;

use napi_derive::napi;
use serde_json::Value as Json;

/// Parse a `.px` source string into the canonical AST, returned as JSON.
///
/// Delegates to `px_compiler::parse` (the real front end) and serializes the
/// resulting [`px_ast::PxDocument`] with `serde_json`. The JSON uses the same
/// kind-tagged encoding as the rest of the toolchain, so a TS consumer sees the
/// exact canonical AST shape.
///
/// Throws (JS `Error`) if the source does not parse.
#[napi]
pub fn parse(src: String) -> napi::Result<String> {
    let doc = px_compiler::parse(&src)
        .map_err(|e| napi::Error::from_reason(format!("px parse error: {e}")))?;
    serde_json::to_string(&doc)
        .map_err(|e| napi::Error::from_reason(format!("AST serialize error: {e}")))
}

/// Evaluate a single v1 `.px` expression against a JSON variable map.
///
/// `vars_json` must be a JSON object (`{"x": 10, "name": "abc"}`); its entries
/// become the evaluation context. Delegates to `px_eval::evaluate` (the real
/// evaluator) and returns the result value as a JSON string.
///
/// Throws if `vars_json` is not a JSON object, or if the expression fails to
/// parse or evaluate.
#[napi]
pub fn evaluate(expr: String, vars_json: String) -> napi::Result<String> {
    let vars = parse_vars(&vars_json)?;
    let result = px_eval::evaluate(&expr, &vars)
        .map_err(|e| napi::Error::from_reason(format!("px eval error: {e}")))?;
    serde_json::to_string(&result)
        .map_err(|e| napi::Error::from_reason(format!("result serialize error: {e}")))
}

/// Parse a `.px` source and check **every** `constraint` in it against a JSON
/// variable map. Returns a JSON array of outcomes.
///
/// Each element is `{ "name": <constraint name>, "status": "satisfied" |
/// "violated" | "not_applicable", "severity"?: "error"|"warning"|"info",
/// "message"?: <string> }`. Delegates to `px_eval::eval_constraint` (the real
/// constraint evaluator) using the pure function registry.
///
/// Throws if the source fails to parse or `vars_json` is not a JSON object.
#[napi]
pub fn check_constraints(src: String, vars_json: String) -> napi::Result<String> {
    use px_ast::Statement;
    use px_eval::{ConstraintOutcome, PureFunctionRegistry};

    let doc = px_compiler::parse(&src)
        .map_err(|e| napi::Error::from_reason(format!("px parse error: {e}")))?;
    let vars = parse_vars(&vars_json)?;
    let registry = PureFunctionRegistry;

    let mut out: Vec<Json> = Vec::new();
    for stmt in &doc.statements {
        if let Statement::Constraint(c) = stmt {
            let outcome = px_eval::eval_constraint(c, &vars, &registry)
                .map_err(|e| napi::Error::from_reason(format!("px constraint eval error: {e}")))?;
            let name = c.name.name.clone();
            let entry = match outcome {
                ConstraintOutcome::Satisfied => serde_json::json!({
                    "name": name,
                    "status": "satisfied",
                }),
                ConstraintOutcome::NotApplicable => serde_json::json!({
                    "name": name,
                    "status": "not_applicable",
                }),
                ConstraintOutcome::Violated { severity, message } => serde_json::json!({
                    "name": name,
                    "status": "violated",
                    "severity": severity_str(severity),
                    "message": message,
                }),
            };
            out.push(entry);
        }
    }

    serde_json::to_string(&Json::Array(out))
        .map_err(|e| napi::Error::from_reason(format!("outcomes serialize error: {e}")))
}

/// The `px-ast` crate version this addon was built against.
#[napi]
pub fn px_ast_version() -> String {
    px_ast::PX_AST_VERSION.to_string()
}

/// Parse the JSON variable map into the `HashMap<String, serde_json::Value>`
/// that `px-eval` expects.
fn parse_vars(vars_json: &str) -> napi::Result<HashMap<String, Json>> {
    let trimmed = vars_json.trim();
    if trimmed.is_empty() {
        return Ok(HashMap::new());
    }
    let value: Json = serde_json::from_str(trimmed)
        .map_err(|e| napi::Error::from_reason(format!("vars JSON parse error: {e}")))?;
    match value {
        Json::Object(map) => Ok(map.into_iter().collect()),
        _ => Err(napi::Error::from_reason(
            "vars must be a JSON object, e.g. {\"x\": 1}".to_string(),
        )),
    }
}

/// Map an `px_ast::Severity` to the lowercase string used in JSON outcomes.
fn severity_str(sev: px_ast::Severity) -> &'static str {
    match sev {
        px_ast::Severity::Error => "error",
        px_ast::Severity::Warning => "warning",
        px_ast::Severity::Info => "info",
    }
}
