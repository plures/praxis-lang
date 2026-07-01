//! # px-eval — evaluate `.px` expressions, rules, and constraints.
//!
//! Evaluates the canonical `px-ast` (produced by `px-compiler`) against a
//! variable context. Ported from the cleaner `praxis-native::px::eval`
//! evaluator, but AST-driven and free of any `pluresdb`/host dependency: the
//! only boundary to effects/host capabilities is the [`FunctionRegistry`] trait.
//!
//! ## Public API
//!
//! - [`evaluate`] — evaluate an expression *string* (parsed via `px-compiler`).
//! - [`evaluate_expr`] — evaluate an already-parsed [`px_ast::Expr`].
//! - [`eval_rule`] — evaluate a [`px_ast::RuleDecl`]'s `when` conditions →
//!   does the rule fire? (Returns the fired actions' names for convenience.)
//! - [`eval_constraint`] — evaluate a [`px_ast::ConstraintDecl`] →
//!   [`ConstraintOutcome`] (satisfied / violated / not-applicable).
//! - [`FunctionRegistry`] + [`PureFunctionRegistry`] — the function boundary.
//! - [`EvalError`] — evaluation errors (parse / type / division-by-zero /
//!   unknown-function / function-error).
//!
//! ## Coverage (C-NOSTUB-001)
//!
//! v1 expressions (literals, vars, dotted/bracket access, arithmetic,
//! comparison, logic, string-concat, inline-if, match, function calls) are
//! **really evaluated**. Rules and constraints are evaluated over those v1
//! expressions. v2 code blocks (`px_ast::CodeExpr` / `px_ast::CodeBlock`) are
//! **absent from this crate's API surface** — there is no entry point that
//! accepts them, so nothing pretends to evaluate them. A v2 procedure runtime
//! is out of scope for M3 wave 2 (documented in `PXLANG-M3W2-RESULT.md`).

mod error;
mod eval;
mod registry;
mod value;

pub use error::EvalError;
pub use eval::{eval_expr as eval_ast_expr, eval_value as eval_ast_value, Env};
pub use registry::{EmptyRegistry, FunctionRegistry, PureFunctionRegistry};

use std::collections::HashMap;

use px_ast::{ConstraintDecl, Expr, RuleDecl, Severity, Statement};
use serde_json::Value;

/// Evaluate a single `.px` expression string against `vars`, using the default
/// [`PureFunctionRegistry`].
///
/// The string is parsed by wrapping it as a one-line `.px` expression through
/// `px-compiler`, then evaluated. For repeated evaluation, parse once and use
/// [`evaluate_expr`].
///
/// # Errors
/// [`EvalError::ParseError`] on syntax error; other [`EvalError`]s on eval.
///
/// # Example
/// ```
/// use std::collections::HashMap;
/// use serde_json::json;
///
/// let mut vars = HashMap::new();
/// vars.insert("x".to_string(), json!(10));
/// vars.insert("y".to_string(), json!(20));
/// assert_eq!(px_eval::evaluate("$x + $y", &vars).unwrap(), json!(30));
/// ```
pub fn evaluate(expr: &str, vars: &HashMap<String, Value>) -> Result<Value, EvalError> {
    let registry = PureFunctionRegistry;
    evaluate_with_registry(expr, vars, &registry)
}

/// Evaluate an expression string with a caller-supplied function registry.
pub fn evaluate_with_registry(
    expr: &str,
    vars: &HashMap<String, Value>,
    registry: &dyn FunctionRegistry,
) -> Result<Value, EvalError> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    let ast = parse_expr(trimmed)?;
    let env = Env::new(vars, registry);
    eval::eval_expr(&ast, &env)
}

/// Evaluate an already-parsed [`Expr`] against `vars` + `registry`.
pub fn evaluate_expr(
    expr: &Expr,
    vars: &HashMap<String, Value>,
    registry: &dyn FunctionRegistry,
) -> Result<Value, EvalError> {
    let env = Env::new(vars, registry);
    eval::eval_expr(expr, &env)
}

/// Parse a bare expression string into a [`px_ast::Expr`].
///
/// Implemented by parsing a synthetic constraint whose `require:` clause is the
/// expression — the smallest well-formed `.px` document that carries a single
/// v1 expression through the canonical `px-compiler` front end (no bespoke
/// expression parser, no grammar duplication).
///
/// # Errors
/// [`EvalError::ParseError`] if the expression is not valid `.px`.
pub fn parse_expr(src: &str) -> Result<Expr, EvalError> {
    let one_line = src.replace('\n', " ");
    let doc_src = format!("constraint __expr__:\n  require: {one_line}\n  severity: info\n");
    let doc = px_compiler::parse(&doc_src).map_err(|e| EvalError::ParseError(e.to_string()))?;
    for stmt in doc.statements {
        if let Statement::Constraint(c) = stmt {
            if let Some(expr) = c.require {
                return Ok(expr);
            }
        }
    }
    Err(EvalError::ParseError(
        "expression did not parse into a require expression".into(),
    ))
}

/// The result of firing a rule against a context.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleFiring {
    /// Whether every `when:` condition held (the rule fires).
    pub fired: bool,
    /// The `action_name`s whose (optional) guard also held. Empty when the rule
    /// did not fire. Note: this reports *which* actions would run; it does not
    /// execute them (executing effects belongs to a host runtime).
    pub actions: Vec<String>,
}

/// Evaluate a [`RuleDecl`]: do all `when:` conditions hold, and which actions'
/// guards pass?
///
/// `let:` bindings are evaluated in order and added to a *scoped copy* of the
/// variable context before conditions/actions are checked.
///
/// This does **not** execute actions (that is a host concern) — it reports
/// whether the rule fires and which actions are selected.
pub fn eval_rule(
    rule: &RuleDecl,
    vars: &HashMap<String, Value>,
    registry: &dyn FunctionRegistry,
) -> Result<RuleFiring, EvalError> {
    // Scoped context = caller vars + let bindings.
    let mut scope = vars.clone();
    for binding in &rule.let_bindings {
        let env = Env::new(&scope, registry);
        let v = eval::eval_expr(&binding.value, &env)?;
        scope.insert(binding.name.name.clone(), v);
    }

    // All when-conditions must be truthy.
    for cond in &rule.conditions {
        let env = Env::new(&scope, registry);
        let v = eval::eval_expr(cond, &env)?;
        if !value::is_truthy(&v) {
            return Ok(RuleFiring {
                fired: false,
                actions: Vec::new(),
            });
        }
    }

    // Rule fires: collect actions whose guard (if any) holds.
    let mut actions = Vec::new();
    for act in &rule.actions {
        let guard_ok = match &act.condition {
            None => true,
            Some(guard) => {
                let env = Env::new(&scope, registry);
                value::is_truthy(&eval::eval_expr(guard, &env)?)
            }
        };
        if guard_ok {
            actions.push(act.action_name.name.clone());
        }
    }

    Ok(RuleFiring {
        fired: true,
        actions,
    })
}

/// The outcome of evaluating a constraint against a context.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintOutcome {
    /// The `when:` guard was present and false → the constraint does not apply.
    NotApplicable,
    /// `require:` held (or was absent) → the invariant is satisfied.
    Satisfied,
    /// `require:` was present and false → the invariant is violated. Carries the
    /// declared severity and (optional) message for reporting.
    Violated {
        severity: Severity,
        message: Option<String>,
    },
}

/// Evaluate a [`ConstraintDecl`] against `vars`.
///
/// Semantics:
/// - If `when:` is present and false → [`ConstraintOutcome::NotApplicable`].
/// - Else evaluate `require:`. Absent `require:` is treated as trivially
///   satisfied. Truthy → [`ConstraintOutcome::Satisfied`]; falsy →
///   [`ConstraintOutcome::Violated`] with the declared severity/message.
pub fn eval_constraint(
    constraint: &ConstraintDecl,
    vars: &HashMap<String, Value>,
    registry: &dyn FunctionRegistry,
) -> Result<ConstraintOutcome, EvalError> {
    let env = Env::new(vars, registry);

    if let Some(when) = &constraint.when {
        let guard = eval::eval_expr(when, &env)?;
        if !value::is_truthy(&guard) {
            return Ok(ConstraintOutcome::NotApplicable);
        }
    }

    match &constraint.require {
        None => Ok(ConstraintOutcome::Satisfied),
        Some(require) => {
            let held = value::is_truthy(&eval::eval_expr(require, &env)?);
            if held {
                Ok(ConstraintOutcome::Satisfied)
            } else {
                Ok(ConstraintOutcome::Violated {
                    severity: constraint.severity,
                    message: constraint.message.as_ref().map(|m| m.value.clone()),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vars(entries: &[(&str, Value)]) -> HashMap<String, Value> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    // ── literals ──
    #[test]
    fn eval_literals() {
        let v = vars(&[]);
        assert_eq!(evaluate("42", &v).unwrap(), json!(42));
        assert_eq!(evaluate("3.5", &v).unwrap(), json!(3.5));
        assert_eq!(evaluate("\"hello\"", &v).unwrap(), json!("hello"));
        assert_eq!(evaluate("'world'", &v).unwrap(), json!("world"));
        assert_eq!(evaluate("true", &v).unwrap(), json!(true));
        assert_eq!(evaluate("false", &v).unwrap(), json!(false));
    }

    // ── arithmetic ──
    #[test]
    fn eval_arithmetic() {
        let v = vars(&[("x", json!(10)), ("y", json!(20))]);
        assert_eq!(evaluate("$x + $y", &v).unwrap(), json!(30));
        assert_eq!(evaluate("$y - $x", &v).unwrap(), json!(10));
        assert_eq!(evaluate("$x * 2", &v).unwrap(), json!(20));
        assert_eq!(evaluate("$y / $x", &v).unwrap(), json!(2));
        assert_eq!(evaluate("2 ^ 3", &v).unwrap(), json!(8));
        assert_eq!(evaluate("10 % 3", &v).unwrap(), json!(1));
    }

    #[test]
    fn eval_division_by_zero() {
        let v = vars(&[]);
        assert_eq!(evaluate("1 / 0", &v), Err(EvalError::DivisionByZero));
    }

    // ── comparison & logic ──
    #[test]
    fn eval_comparison_logic() {
        let v = vars(&[("n", json!(5))]);
        assert_eq!(evaluate("$n > 3", &v).unwrap(), json!(true));
        assert_eq!(evaluate("$n == 5", &v).unwrap(), json!(true));
        assert_eq!(evaluate("$n != 5", &v).unwrap(), json!(false));
        assert_eq!(evaluate("$n > 3 && $n < 10", &v).unwrap(), json!(true));
        assert_eq!(evaluate("$n > 10 || $n < 6", &v).unwrap(), json!(true));
        assert_eq!(evaluate("!(($n) > 10)", &v).unwrap(), json!(true));
    }

    #[test]
    fn eval_cross_type_number_equality() {
        let v = vars(&[]);
        assert_eq!(evaluate("42 == 42.0", &v).unwrap(), json!(true));
    }

    // ── strings ──
    #[test]
    fn eval_string_concat() {
        let v = vars(&[("name", json!("Ada"))]);
        assert_eq!(evaluate("\"Hi \" + $name", &v).unwrap(), json!("Hi Ada"));
    }

    // ── var access / null propagation ──
    #[test]
    fn eval_dotted_and_bracket_access() {
        let v = vars(&[
            ("ship", json!({"x": 3, "crew": ["ann", "bo"]})),
            ("arr", json!([10, 20, 30])),
        ]);
        assert_eq!(evaluate("$ship.x", &v).unwrap(), json!(3));
        assert_eq!(evaluate("$ship.crew[1]", &v).unwrap(), json!("bo"));
        assert_eq!(evaluate("$arr[0]", &v).unwrap(), json!(10));
        // Missing var / field → null (not an error).
        assert_eq!(evaluate("$missing", &v).unwrap(), json!(null));
        assert_eq!(evaluate("$ship.nope", &v).unwrap(), json!(null));
    }

    // ── inline if & match ──
    #[test]
    fn eval_inline_if() {
        let v = vars(&[("n", json!(7))]);
        assert_eq!(
            evaluate("if $n > 5: \"big\" else: \"small\"", &v).unwrap(),
            json!("big")
        );
    }

    #[test]
    fn eval_match_expr() {
        let v = vars(&[("code", json!(2))]);
        let out = evaluate(
            "match $code { 1 => \"one\", 2 => \"two\", _ => \"other\" }",
            &v,
        )
        .unwrap();
        assert_eq!(out, json!("two"));
    }

    // ── function calls (pure registry) ──
    #[test]
    fn eval_pure_functions() {
        let v = vars(&[("s", json!("Hello")), ("xs", json!([1, 2, 3]))]);
        assert_eq!(evaluate("len($s)", &v).unwrap(), json!(5));
        assert_eq!(evaluate("len($xs)", &v).unwrap(), json!(3));
        assert_eq!(evaluate("upper($s)", &v).unwrap(), json!("HELLO"));
        assert_eq!(evaluate("abs(-4)", &v).unwrap(), json!(4));
        assert_eq!(evaluate("max(1, 9, 4)", &v).unwrap(), json!(9));
        assert_eq!(evaluate("contains($s, \"ell\")", &v).unwrap(), json!(true));
    }

    #[test]
    fn eval_unknown_function_is_error() {
        let v = vars(&[]);
        assert_eq!(
            evaluate("no_such_fn(1)", &v),
            Err(EvalError::UnknownFunction("no_such_fn".into()))
        );
    }

    #[test]
    fn empty_registry_rejects_calls() {
        let v = vars(&[]);
        let reg = EmptyRegistry;
        assert!(matches!(
            evaluate_with_registry("len(\"x\")", &v, &reg),
            Err(EvalError::UnknownFunction(_))
        ));
    }

    // ── rule evaluation ──
    #[test]
    fn eval_rule_fires_and_selects_actions() {
        let src = "rule urgent:\n  when:\n    - $score > 5\n  then:\n    - action: flag level: \"high\"\n    - if $score > 9: action: escalate\n";
        let doc = px_compiler::parse(src).unwrap();
        let rule = match &doc.statements[0] {
            Statement::Rule(r) => r,
            _ => panic!("expected rule"),
        };
        let reg = PureFunctionRegistry;

        let fired = eval_rule(rule, &vars(&[("score", json!(7))]), &reg).unwrap();
        assert!(fired.fired);
        assert_eq!(fired.actions, vec!["flag".to_string()]); // escalate guard false

        let big = eval_rule(rule, &vars(&[("score", json!(12))]), &reg).unwrap();
        assert_eq!(
            big.actions,
            vec!["flag".to_string(), "escalate".to_string()]
        );

        let quiet = eval_rule(rule, &vars(&[("score", json!(1))]), &reg).unwrap();
        assert!(!quiet.fired);
        assert!(quiet.actions.is_empty());
    }

    #[test]
    fn eval_rule_uses_let_bindings() {
        // Grammar places `let_clause*` after `when_clause`, so the let binding
        // is available to the `then:` action guards (evaluated in scope order).
        let src = "rule ok:\n  when:\n    - $base > 0\n  let total = $base + 5\n  then:\n    - if $total > 10: action: big\n";
        let doc = px_compiler::parse(src).unwrap();
        let rule = match &doc.statements[0] {
            Statement::Rule(r) => r,
            _ => panic!("expected rule"),
        };
        let reg = PureFunctionRegistry;
        let fired = eval_rule(rule, &vars(&[("base", json!(8))]), &reg).unwrap();
        assert!(fired.fired);
        assert_eq!(fired.actions, vec!["big".to_string()]); // total = 13 > 10
    }

    // ── constraint evaluation ──
    #[test]
    fn eval_constraint_satisfied_and_violated() {
        let src = "constraint non_empty:\n  require: $len > 0\n  severity: error\n  message: \"must be non-empty\"\n";
        let doc = px_compiler::parse(src).unwrap();
        let c = match &doc.statements[0] {
            Statement::Constraint(c) => c,
            _ => panic!("expected constraint"),
        };
        let reg = PureFunctionRegistry;

        assert_eq!(
            eval_constraint(c, &vars(&[("len", json!(3))]), &reg).unwrap(),
            ConstraintOutcome::Satisfied
        );
        assert_eq!(
            eval_constraint(c, &vars(&[("len", json!(0))]), &reg).unwrap(),
            ConstraintOutcome::Violated {
                severity: Severity::Error,
                message: Some("must be non-empty".to_string()),
            }
        );
    }

    #[test]
    fn eval_constraint_not_applicable_when_guard_false() {
        let src = "constraint guarded:\n  when: $active == true\n  require: $len > 0\n  severity: warning\n";
        let doc = px_compiler::parse(src).unwrap();
        let c = match &doc.statements[0] {
            Statement::Constraint(c) => c,
            _ => panic!("expected constraint"),
        };
        let reg = PureFunctionRegistry;
        assert_eq!(
            eval_constraint(
                c,
                &vars(&[("active", json!(false)), ("len", json!(0))]),
                &reg
            )
            .unwrap(),
            ConstraintOutcome::NotApplicable
        );
    }
}
