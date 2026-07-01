//! The core AST evaluator: `px_ast::Expr` / `px_ast::Value` → runtime value.
//!
//! Ported from `praxis-native::px::eval::eval_expr`, but evaluating the typed
//! `px-ast` produced by `px-compiler` rather than re-walking a pest tree. The
//! numeric/comparison/logic/string semantics are preserved via [`crate::value`].

use serde_json::Value as J;

use px_ast::{
    Accessor, ArithOp, BinOp, DottedIdent, Expr, ExprMatchPattern, UnaryOp, Value as AstValue,
    VarRef,
};

use crate::error::EvalError;
use crate::registry::FunctionRegistry;
use crate::value::{
    compare_ordered, f64_to_value, is_string_concat, is_truthy, resolve_accessor, to_f64,
    value_to_string, values_equal,
};

/// The evaluation environment: variable bindings + the function boundary.
pub struct Env<'a> {
    vars: &'a std::collections::HashMap<String, J>,
    registry: &'a dyn FunctionRegistry,
}

impl<'a> Env<'a> {
    /// Build an environment from a variable map and a function registry.
    pub fn new(
        vars: &'a std::collections::HashMap<String, J>,
        registry: &'a dyn FunctionRegistry,
    ) -> Self {
        Self { vars, registry }
    }

    fn lookup(&self, name: &str) -> Option<&J> {
        self.vars.get(name)
    }
}

/// Evaluate a v1 [`Expr`] against `env`.
pub fn eval_expr(expr: &Expr, env: &Env<'_>) -> Result<J, EvalError> {
    match expr {
        Expr::Literal(v) => eval_value(v, env),
        Expr::Paren(inner) => eval_expr(inner, env),
        Expr::Var(v) => Ok(eval_var_ref(v, env)),
        Expr::Path(p) => Ok(eval_path(p, env)),
        Expr::Unary { op, operand } => {
            let val = eval_expr(operand, env)?;
            match op {
                UnaryOp::Not => Ok(J::Bool(!is_truthy(&val))),
                UnaryOp::Neg => {
                    let n = to_f64(&val).ok_or_else(|| {
                        EvalError::TypeError("negation requires numeric operand".into())
                    })?;
                    Ok(f64_to_value(-n))
                }
            }
        }
        Expr::Binary { left, op, right } => {
            // Short-circuit logic.
            match op {
                BinOp::And => {
                    let l = eval_expr(left, env)?;
                    if !is_truthy(&l) {
                        return Ok(J::Bool(false));
                    }
                    let r = eval_expr(right, env)?;
                    return Ok(J::Bool(is_truthy(&r)));
                }
                BinOp::Or => {
                    let l = eval_expr(left, env)?;
                    if is_truthy(&l) {
                        return Ok(J::Bool(true));
                    }
                    let r = eval_expr(right, env)?;
                    return Ok(J::Bool(is_truthy(&r)));
                }
                _ => {}
            }
            let l = eval_expr(left, env)?;
            let r = eval_expr(right, env)?;
            eval_binop(&l, *op, &r)
        }
        Expr::InlineIf {
            condition,
            then_val,
            else_val,
        } => {
            let c = eval_expr(condition, env)?;
            if is_truthy(&c) {
                eval_expr(then_val, env)
            } else {
                eval_expr(else_val, env)
            }
        }
        Expr::Call { name, args } => {
            let arg_vals = args
                .iter()
                .map(|a| eval_expr(a, env))
                .collect::<Result<Vec<_>, _>>()?;
            eval_call(&name.name, &arg_vals, env)
        }
        Expr::Match { subject, arms } => {
            let subj = eval_expr(subject, env)?;
            for arm in arms {
                let hit = match &arm.pattern {
                    ExprMatchPattern::Wildcard => true,
                    ExprMatchPattern::Values(vals) => {
                        let mut any = false;
                        for v in vals {
                            let pv = eval_value(v, env)?;
                            if values_equal(&subj, &pv) {
                                any = true;
                                break;
                            }
                        }
                        any
                    }
                };
                if hit {
                    return eval_expr(&arm.result, env);
                }
            }
            // No arm matched and no wildcard: null (matches JS-ish leniency).
            Ok(J::Null)
        }
    }
}

/// Evaluate a `px_ast::Value` (declaration/leaf value) against `env`.
pub fn eval_value(v: &AstValue, env: &Env<'_>) -> Result<J, EvalError> {
    match v {
        AstValue::String(s) => Ok(J::String(s.clone())),
        AstValue::Integer(i) => Ok(J::Number((*i).into())),
        AstValue::Float(f) => Ok(f64_to_value(*f)),
        AstValue::Boolean(b) => Ok(J::Bool(*b)),
        AstValue::Null => Ok(J::Null),
        AstValue::List(items) => {
            let out = items
                .iter()
                .map(|i| eval_value(i, env))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(J::Array(out))
        }
        AstValue::Map(entries) => {
            let mut map = serde_json::Map::new();
            for (k, val) in entries {
                map.insert(k.name.clone(), eval_value(val, env)?);
            }
            Ok(J::Object(map))
        }
        AstValue::Var(vr) => Ok(eval_var_ref(vr, env)),
        AstValue::Path(p) => Ok(eval_path(p, env)),
        AstValue::Ident(id) => {
            // Bare identifier: variable if bound, else its own name as a string
            // (enum variant / symbol), matching the canonical evaluator.
            Ok(env
                .lookup(&id.name)
                .cloned()
                .unwrap_or_else(|| J::String(id.name.clone())))
        }
        AstValue::Paren(inner) => eval_expr(inner, env),
        AstValue::Call { name, args } => {
            let arg_vals = args
                .iter()
                .map(|a| eval_expr(a, env))
                .collect::<Result<Vec<_>, _>>()?;
            eval_call(&name.name, &arg_vals, env)
        }
        AstValue::Arithmetic { left, op, right } => {
            let l = eval_value(left, env)?;
            let r = eval_value(right, env)?;
            let binop = match op {
                ArithOp::Add => BinOp::Add,
                ArithOp::Sub => BinOp::Sub,
                ArithOp::Mul => BinOp::Mul,
                ArithOp::Div => BinOp::Div,
                ArithOp::Mod => BinOp::Mod,
            };
            eval_binop(&l, binop, &r)
        }
    }
}

fn eval_call(name: &str, args: &[J], env: &Env<'_>) -> Result<J, EvalError> {
    if !env.registry.contains(name) {
        return Err(EvalError::UnknownFunction(name.to_string()));
    }
    env.registry
        .call(name, args)
        .map_err(EvalError::FunctionError)
}

fn eval_var_ref(vr: &VarRef, env: &Env<'_>) -> J {
    // `$name` missing → Null (null propagation, per canonical evaluator).
    let base = match env.lookup(&vr.name.name) {
        Some(v) => v.clone(),
        None => return J::Null,
    };
    let mut current = base;
    for acc in &vr.accessors {
        current = match acc {
            Accessor::Dot(id) => resolve_accessor(&current, false, &id.name),
            Accessor::Bracket(key) => resolve_accessor(&current, true, key),
        };
    }
    current
}

fn eval_path(p: &DottedIdent, env: &Env<'_>) -> J {
    // Keyword-ish leaves that the grammar may route through a path.
    if p.segments.len() == 1 {
        match p.segments[0].name.as_str() {
            "true" => return J::Bool(true),
            "false" => return J::Bool(false),
            "null" => return J::Null,
            _ => {}
        }
    }
    let mut segs = p.segments.iter();
    let Some(base_id) = segs.next() else {
        return J::Null;
    };
    match env.lookup(&base_id.name) {
        Some(base) => {
            let mut current = base.clone();
            for seg in segs {
                current = resolve_accessor(&current, false, &seg.name);
            }
            current
        }
        None => {
            if p.segments.len() == 1 {
                // Bare identifier not in vars → its own name (enum/symbol).
                J::String(base_id.name.clone())
            } else {
                J::Null
            }
        }
    }
}

fn eval_binop(left: &J, op: BinOp, right: &J) -> Result<J, EvalError> {
    match op {
        // Logic handled with short-circuit in caller; here for completeness.
        BinOp::And => Ok(J::Bool(is_truthy(left) && is_truthy(right))),
        BinOp::Or => Ok(J::Bool(is_truthy(left) || is_truthy(right))),
        // Comparison
        BinOp::Eq => Ok(J::Bool(values_equal(left, right))),
        BinOp::Neq => Ok(J::Bool(!values_equal(left, right))),
        BinOp::Gt => Ok(J::Bool(compare_ordered(left, ">", right))),
        BinOp::Lt => Ok(J::Bool(compare_ordered(left, "<", right))),
        BinOp::Gte => Ok(J::Bool(compare_ordered(left, ">=", right))),
        BinOp::Lte => Ok(J::Bool(compare_ordered(left, "<=", right))),
        // Additive (with string concat on +)
        BinOp::Add => {
            if is_string_concat(left, right) {
                Ok(J::String(format!(
                    "{}{}",
                    value_to_string(left),
                    value_to_string(right)
                )))
            } else {
                let a = num(left, "add")?;
                let b = num(right, "add")?;
                Ok(f64_to_value(a + b))
            }
        }
        BinOp::Sub => Ok(f64_to_value(num(left, "sub")? - num(right, "sub")?)),
        BinOp::Mul => Ok(f64_to_value(num(left, "mul")? * num(right, "mul")?)),
        BinOp::Div => {
            let b = num(right, "div")?;
            if b == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            Ok(f64_to_value(num(left, "div")? / b))
        }
        BinOp::Mod => {
            let b = num(right, "mod")?;
            if b == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            Ok(f64_to_value(num(left, "mod")? % b))
        }
        BinOp::Pow => Ok(f64_to_value(num(left, "pow")?.powf(num(right, "pow")?))),
    }
}

fn num(v: &J, op: &str) -> Result<f64, EvalError> {
    to_f64(v).ok_or_else(|| EvalError::TypeError(format!("{op}: non-numeric operand {v:?}")))
}
