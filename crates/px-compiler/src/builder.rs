//! The parse-tree → `px-ast` lowering (tree walk).
//!
//! One `build_*` function per grammar rule (or rule family). Each mirrors the
//! exact inner-pair sequence declared in `grammar.pest`. Where the grammar
//! marks a rule silent (`_`), its children appear inline in the parent.

use px_ast::*;
use px_grammar::Rule;

type Pair<'i> = pest::iterators::Pair<'i, Rule>;

use crate::error::CompileError;

type R<T> = Result<T, CompileError>;

// ═══════════════════════════════════════════════════════════════════════════
// Small helpers
// ═══════════════════════════════════════════════════════════════════════════

fn span_of(pair: &Pair<'_>) -> Span {
    let s = pair.as_span();
    Span {
        start: s.start(),
        end: s.end(),
    }
}

/// Next inner pair from any pair-iterator, or an internal error naming the
/// parent rule + what we wanted. Generic so it works with both raw
/// [`Pairs`] and the keyword-filtered [`payload_pairs`] iterator.
fn next<'i, I>(inner: &mut I, parent: &str, want: &str) -> R<Pair<'i>>
where
    I: Iterator<Item = Pair<'i>>,
{
    inner
        .next()
        .ok_or_else(|| CompileError::internal(format!("{parent}: expected {want}")))
}

fn ident_of(pair: Pair<'_>) -> Ident {
    let span = span_of(&pair);
    Ident::with_span(pair.as_str().to_string(), span)
}

/// Strip the surrounding quotes from a `string` token (`"..."` or `'...'`).
fn unquote(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            // Strip the surrounding quotes, then process escape sequences (Rust/YAML
            // double-quoted-scalar faithful: \" \' \\ \n \t \r). The grammar now permits
            // backslash-escaped quotes inside strings, so unescaping happens here.
            let inner = &raw[1..raw.len() - 1];
            let mut out = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    match chars.next() {
                        Some('"') => out.push('"'),
                        Some('\'') => out.push('\''),
                        Some('\\') => out.push('\\'),
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some(other) => {
                            // Unknown escape: preserve backslash + char verbatim.
                            out.push('\\');
                            out.push(other);
                        }
                        None => out.push('\\'),
                    }
                } else {
                    out.push(c);
                }
            }
            return out;
        }
    }
    raw.to_string()
}

fn string_literal_of(pair: &Pair<'_>) -> StringLiteral {
    StringLiteral {
        value: unquote(pair.as_str()),
        span: Some(span_of(pair)),
    }
}

/// `""" ... """` docstring token → inner text.
fn docstring_of(raw: &str) -> String {
    let t = raw.trim();
    if let Some(inner) = t
        .strip_prefix("\"\"\"")
        .and_then(|s| s.strip_suffix("\"\"\""))
    {
        inner.to_string()
    } else {
        t.to_string()
    }
}

fn parse_i64(pair: &Pair<'_>) -> R<i64> {
    pair.as_str()
        .trim()
        .parse::<i64>()
        .map_err(|e| CompileError::internal(format!("invalid integer `{}`: {e}", pair.as_str())))
}

fn parse_f64(pair: &Pair<'_>) -> R<f64> {
    pair.as_str()
        .trim()
        .parse::<f64>()
        .map_err(|e| CompileError::internal(format!("invalid float `{}`: {e}", pair.as_str())))
}

/// True for the atomic `kw_*` keyword marker rules, which are emitted as pairs
/// (they are `@{}`, not silent `_{}`) but carry no payload — skip them when
/// walking a step's inner pairs.
fn is_keyword_rule(rule: Rule) -> bool {
    matches!(
        rule,
        Rule::kw_return
            | Rule::kw_abort
            | Rule::kw_define
            | Rule::kw_when
            | Rule::kw_loop
            | Rule::kw_emit
            | Rule::kw_try
            | Rule::kw_parallel
            | Rule::kw_if
            | Rule::kw_for
            | Rule::kw_in
            | Rule::kw_match
            | Rule::kw_end
    )
}

/// Iterator over a pair's inner pairs with keyword markers filtered out.
fn payload_pairs(pair: Pair<'_>) -> impl Iterator<Item = Pair<'_>> {
    pair.into_inner().filter(|p| !is_keyword_rule(p.as_rule()))
}

// ═══════════════════════════════════════════════════════════════════════════
// Statement dispatch
// ═══════════════════════════════════════════════════════════════════════════

pub(crate) fn build_statement(pair: Pair<'_>) -> R<Statement> {
    match pair.as_rule() {
        Rule::import_decl => Ok(Statement::Import(build_import(pair)?)),
        Rule::entity_decl => Ok(Statement::Entity(build_entity(pair)?)),
        Rule::config_decl => Ok(Statement::Config(build_config(pair)?)),
        Rule::fact_decl => Ok(Statement::Fact(build_fact(pair)?)),
        Rule::rule_decl => Ok(Statement::Rule(build_rule(pair)?)),
        Rule::constraint_decl => Ok(Statement::Constraint(build_constraint(pair)?)),
        Rule::contract_decl => Ok(Statement::Contract(build_contract(pair)?)),
        Rule::function_decl => Ok(Statement::Function(build_function(pair)?)),
        Rule::trigger_decl => Ok(Statement::Trigger(build_trigger(pair)?)),
        Rule::dataflow_procedure_decl => Ok(Statement::DataflowProcedure(
            build_dataflow_procedure(pair)?,
        )),
        Rule::procedure_decl => Ok(Statement::LegacyProcedure(build_legacy_procedure(pair)?)),
        Rule::scenario_decl => Ok(Statement::Scenario(build_scenario(pair)?)),
        other => Err(CompileError::unsupported(
            format!("{other:?}"),
            "not a recognized top-level statement rule",
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TYPES  (type_expr / entity_type_expr share sub-rules)
// ═══════════════════════════════════════════════════════════════════════════

fn build_type_expr(pair: Pair<'_>) -> R<TypeExpr> {
    // type_expr / entity_type_expr are wrappers around one alternative.
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| CompileError::internal("type_expr: missing inner"))?;
    build_type_inner(inner)
}

fn build_type_inner(pair: Pair<'_>) -> R<TypeExpr> {
    match pair.as_rule() {
        Rule::base_type => Ok(TypeExpr::Base(match pair.as_str() {
            "bool" => BaseType::Bool,
            "int" => BaseType::Int,
            "float" => BaseType::Float,
            "string" => BaseType::String,
            "duration" => BaseType::Duration,
            other => {
                return Err(CompileError::internal(format!(
                    "unknown base_type `{other}`"
                )))
            }
        })),
        Rule::ident_type => {
            let id = pair
                .into_inner()
                .next()
                .ok_or_else(|| CompileError::internal("ident_type: missing ident"))?;
            Ok(TypeExpr::Named(ident_of(id)))
        }
        Rule::list_type => {
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| CompileError::internal("list_type: missing element type"))?;
            Ok(TypeExpr::List(Box::new(build_type_expr(inner)?)))
        }
        Rule::optional_type => {
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| CompileError::internal("optional_type: missing inner type"))?;
            Ok(TypeExpr::Optional(Box::new(build_type_expr(inner)?)))
        }
        Rule::map_type => {
            let mut it = pair.into_inner();
            let k = next(&mut it, "map_type", "key type")?;
            let v = next(&mut it, "map_type", "value type")?;
            Ok(TypeExpr::Map(
                Box::new(build_type_expr(k)?),
                Box::new(build_type_expr(v)?),
            ))
        }
        Rule::enum_type => {
            let variants = pair.into_inner().map(ident_of).collect();
            Ok(TypeExpr::Enum(variants))
        }
        other => Err(CompileError::internal(format!(
            "unexpected type rule {other:?}"
        ))),
    }
}

// ═════════════════════════════════════════════════════════════════════════════════
// VALUES
// value = { string | float | integer | boolean | list_val | map_val
//         | call_expr | arith_val | var_ref | dotted_ident | paren_expr | ident }
// ═════════════════════════════════════════════════════════════════════════════════

fn build_value(pair: Pair<'_>) -> R<Value> {
    match pair.as_rule() {
        Rule::value => {
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| CompileError::internal("value: empty"))?;
            build_value(inner)
        }
        Rule::string => Ok(Value::String(unquote(pair.as_str()))),
        // YAML plain scalar (unquoted string, e.g. github-pr). Not quoted -> take verbatim.
        Rule::plain_scalar => Ok(Value::String(pair.as_str().trim().to_string())),
        Rule::float => Ok(Value::Float(parse_f64(&pair)?)),
        Rule::integer => Ok(Value::Integer(parse_i64(&pair)?)),
        Rule::boolean => Ok(Value::Boolean(pair.as_str() == "true")),
        Rule::list_val => {
            let items = pair.into_inner().map(build_value).collect::<R<Vec<_>>>()?;
            Ok(Value::List(items))
        }
        Rule::map_val => Ok(Value::Map(build_map_entries(pair)?)),
        Rule::call_expr => {
            let (name, args) = build_call_expr(pair)?;
            Ok(Value::Call { name, args })
        }
        Rule::arith_val => build_arith_val(pair),
        Rule::var_ref => Ok(Value::Var(build_var_ref(pair)?)),
        Rule::dotted_ident => Ok(Value::Path(build_dotted_ident(pair))),
        Rule::paren_expr => {
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| CompileError::internal("paren_expr: empty"))?;
            Ok(Value::Paren(Box::new(build_expr(inner)?)))
        }
        Rule::ident => Ok(Value::Ident(ident_of(pair))),
        other => Err(CompileError::internal(format!(
            "unexpected value rule {other:?}"
        ))),
    }
}

/// `map_val = { "{" ~ (ident ~ ":" ~ value ~ ...)? ~ "}" }` → `Vec<(Ident, Value)>`.
fn build_map_entries(pair: Pair<'_>) -> R<Vec<(Ident, Value)>> {
    let mut out = Vec::new();
    let mut it = pair.into_inner();
    while let Some(key) = it.next() {
        // Entries alternate ident, value.
        let key_id = ident_of(key);
        let val = next(&mut it, "map_val", "value after key")?;
        out.push((key_id, build_value(val)?));
    }
    Ok(out)
}

/// `arith_val = { (call_expr|var_ref|integer|float) ~ add_op ~ (... | paren_expr) }`.
fn build_arith_val(pair: Pair<'_>) -> R<Value> {
    let mut it = pair.into_inner();
    let left = next(&mut it, "arith_val", "left operand")?;
    let op_pair = next(&mut it, "arith_val", "add_op")?;
    let right = next(&mut it, "arith_val", "right operand")?;
    let op = match op_pair.as_str() {
        "+" => ArithOp::Add,
        "-" => ArithOp::Sub,
        other => {
            return Err(CompileError::internal(format!(
                "arith_val: unexpected op `{other}`"
            )))
        }
    };
    Ok(Value::Arithmetic {
        left: Box::new(build_value(left)?),
        op,
        right: Box::new(build_value(right)?),
    })
}

fn build_var_ref(pair: Pair<'_>) -> R<VarRef> {
    // var_ref is atomic (@) → its `.as_str()` is like `$name.field["k"]`.
    // Parse the accessor chain from the raw text since inner pairs are hidden.
    let span = span_of(&pair);
    let raw = pair.as_str();
    let body = raw.strip_prefix('$').unwrap_or(raw);
    let name_end = body.find(['.', '[']).unwrap_or(body.len());
    let name = &body[..name_end];
    let mut accessors = Vec::new();
    let mut rest = &body[name_end..];
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let seg_end = after.find(['.', '[']).unwrap_or(after.len());
            accessors.push(Accessor::Dot(Ident::new(&after[..seg_end])));
            rest = &after[seg_end..];
        } else if let Some(after) = rest.strip_prefix('[') {
            let close = after
                .find(']')
                .ok_or_else(|| CompileError::internal("var_ref: unterminated `[`"))?;
            let key = unquote(&after[..close]);
            accessors.push(Accessor::Bracket(key));
            rest = &after[close + 1..];
        } else {
            return Err(CompileError::internal(format!(
                "var_ref: unexpected accessor text `{rest}`"
            )));
        }
    }
    Ok(VarRef {
        name: Ident::new(name),
        accessors,
        span: Some(span),
    })
}

fn build_dotted_ident(pair: Pair<'_>) -> DottedIdent {
    // dotted_ident is atomic; split on '.' (bracket segments stay attached to
    // the preceding segment name, acceptable for a path identifier).
    let span = span_of(&pair);
    let segments = pair
        .as_str()
        .split('.')
        .map(|s| Ident::new(s.trim()))
        .collect();
    DottedIdent {
        segments,
        span: Some(span),
    }
}

// ══════════════════════════════════════════════════════════════════════════════════
// V1 EXPRESSIONS
// expr = { inline_if | comparison ~ (logic_op ~ comparison)* }
// comparison = { additive ~ (comp_op ~ additive)? }
// additive = { multiplicative ~ (add_op ~ multiplicative)* }
// multiplicative = { power ~ (mul_op ~ power)* }
// power = { unary ~ ("^" ~ unary)* }
// unary = { neg_expr | not_expr | atom }
// atom = { match_expr | call_expr | dotted_ident | value | "(" ~ expr ~ ")" }
// ══════════════════════════════════════════════════════════════════════════════════

fn build_expr(pair: Pair<'_>) -> R<Expr> {
    match pair.as_rule() {
        Rule::expr => {
            let mut it = pair.into_inner();
            let first = next(&mut it, "expr", "inline_if or comparison")?;
            if first.as_rule() == Rule::inline_if {
                return build_inline_if(first);
            }
            // comparison ~ (logic_op ~ comparison)*  (left-assoc)
            let mut left = build_expr(first)?;
            while let Some(op_pair) = it.next() {
                let op = logic_binop(op_pair.as_str())?;
                let rhs = next(&mut it, "expr", "comparison after logic op")?;
                let right = build_expr(rhs)?;
                left = Expr::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            }
            Ok(left)
        }
        Rule::comparison => {
            let mut it = pair.into_inner();
            let first = next(&mut it, "comparison", "additive")?;
            let mut left = build_expr(first)?;
            if let Some(op_pair) = it.next() {
                let op = comp_binop(op_pair.as_str())?;
                let rhs = next(&mut it, "comparison", "additive after comp op")?;
                left = Expr::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(build_expr(rhs)?),
                };
            }
            Ok(left)
        }
        Rule::additive => fold_left(pair, add_binop),
        Rule::multiplicative => fold_left(pair, mul_binop),
        // `power = { unary ~ ("^" ~ unary)* }` — the `^` is a LITERAL (no pair),
        // so inner pairs are operands only. Fold them with Pow (right-assoc).
        Rule::power => {
            let operands = pair.into_inner().collect::<Vec<_>>();
            if operands.is_empty() {
                return Err(CompileError::internal("power: no operands"));
            }
            // Right-associative: a ^ b ^ c == a ^ (b ^ c).
            let mut iter = operands.into_iter().rev();
            let mut acc = build_expr(iter.next().unwrap())?;
            for base in iter {
                acc = Expr::Binary {
                    left: Box::new(build_expr(base)?),
                    op: BinOp::Pow,
                    right: Box::new(acc),
                };
            }
            Ok(acc)
        }
        Rule::unary => {
            let inner = next(&mut pair.into_inner(), "unary", "neg/not/atom")?;
            build_expr(inner)
        }
        Rule::neg_expr => {
            let inner = next(&mut pair.into_inner(), "neg_expr", "atom")?;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(build_expr(inner)?),
            })
        }
        Rule::not_expr => {
            let inner = next(&mut pair.into_inner(), "not_expr", "atom")?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(build_expr(inner)?),
            })
        }
        Rule::atom => {
            let inner = next(&mut pair.into_inner(), "atom", "inner")?;
            build_expr(inner)
        }
        Rule::match_expr => build_match_expr(pair),
        Rule::call_expr => {
            let (name, args) = build_call_expr(pair)?;
            Ok(Expr::Call { name, args })
        }
        Rule::dotted_ident => Ok(Expr::Path(build_dotted_ident(pair))),
        Rule::paren_expr => {
            let inner = next(&mut pair.into_inner(), "paren_expr", "expr")?;
            Ok(Expr::Paren(Box::new(build_expr(inner)?)))
        }
        Rule::var_ref => Ok(Expr::Var(build_var_ref(pair)?)),
        Rule::value => {
            // A `value` in expression position: lower to an Expr. Prefer the
            // typed sub-forms so `$x`/`foo.bar`/`f()` become Var/Path/Call
            // rather than opaque literals.
            let inner = next(&mut pair.into_inner(), "value", "inner")?;
            match inner.as_rule() {
                Rule::var_ref => Ok(Expr::Var(build_var_ref(inner)?)),
                Rule::dotted_ident => Ok(Expr::Path(build_dotted_ident(inner))),
                Rule::call_expr => {
                    let (name, args) = build_call_expr(inner)?;
                    Ok(Expr::Call { name, args })
                }
                Rule::paren_expr => {
                    let e = next(&mut inner.into_inner(), "paren_expr", "expr")?;
                    Ok(Expr::Paren(Box::new(build_expr(e)?)))
                }
                _ => Ok(Expr::Literal(build_value(inner)?)),
            }
        }
        // Any remaining leaf value alternative (string/int/float/bool/list/map/ident/arith).
        Rule::string
        | Rule::float
        | Rule::integer
        | Rule::boolean
        | Rule::list_val
        | Rule::map_val
        | Rule::arith_val
        | Rule::ident => Ok(Expr::Literal(build_value(pair)?)),
        other => Err(CompileError::internal(format!(
            "unexpected expr rule {other:?}"
        ))),
    }
}

/// Fold a left-associative `child ~ (op ~ child)*` rule into `Expr::Binary`.
fn fold_left(pair: Pair<'_>, op_of: fn(&str) -> R<BinOp>) -> R<Expr> {
    let mut it = pair.into_inner();
    let first = it
        .next()
        .ok_or_else(|| CompileError::internal("binary chain: missing first operand"))?;
    let mut left = build_expr(first)?;
    while let Some(op_pair) = it.next() {
        let op = op_of(op_pair.as_str())?;
        let rhs = next(&mut it, "binary chain", "operand after op")?;
        left = Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(build_expr(rhs)?),
        };
    }
    Ok(left)
}

fn build_inline_if(pair: Pair<'_>) -> R<Expr> {
    // inline_if = { "if" ~ comparison ~ ":" ~ comparison ~ "else:" ~ comparison }
    let mut it = pair.into_inner();
    let cond = next(&mut it, "inline_if", "condition")?;
    let then_v = next(&mut it, "inline_if", "then value")?;
    let else_v = next(&mut it, "inline_if", "else value")?;
    Ok(Expr::InlineIf {
        condition: Box::new(build_expr(cond)?),
        then_val: Box::new(build_expr(then_v)?),
        else_val: Box::new(build_expr(else_v)?),
    })
}

fn build_match_expr(pair: Pair<'_>) -> R<Expr> {
    // match_expr = { "match" ~ match_subject ~ "{" ~ match_expr_arm_list ~ "}" }
    let mut it = pair.into_inner();
    let subject = next(&mut it, "match_expr", "subject")?;
    let subject_expr = build_match_atom(subject)?;
    let arm_list = next(&mut it, "match_expr", "arm list")?;
    let mut arms = Vec::new();
    for arm in arm_list.into_inner() {
        // match_expr_arm = { match_pattern ~ "=>" ~ match_result }
        let mut ai = arm.into_inner();
        let pat = next(&mut ai, "match_expr_arm", "pattern")?;
        let res = next(&mut ai, "match_expr_arm", "result")?;
        arms.push(ExprMatchArm {
            pattern: build_match_pattern(pat)?,
            result: build_match_atom(res)?,
        });
    }
    Ok(Expr::Match {
        subject: Box::new(subject_expr),
        arms,
    })
}

/// match_subject / match_result = { call_expr | dotted_ident | value }
fn build_match_atom(pair: Pair<'_>) -> R<Expr> {
    let inner = match pair.as_rule() {
        Rule::match_subject | Rule::match_result => {
            next(&mut pair.into_inner(), "match atom", "inner")?
        }
        _ => pair,
    };
    build_expr(inner)
}

fn build_match_pattern(pair: Pair<'_>) -> R<ExprMatchPattern> {
    // match_pattern = { "_" | multi_pattern }
    if pair.as_str().trim() == "_" {
        return Ok(ExprMatchPattern::Wildcard);
    }
    let mut values = Vec::new();
    for mp in pair.into_inner() {
        // multi_pattern → match_single_pattern → value
        match mp.as_rule() {
            Rule::multi_pattern => {
                for single in mp.into_inner() {
                    let v = next(&mut single.into_inner(), "match_single_pattern", "value")?;
                    values.push(build_value(v)?);
                }
            }
            Rule::match_single_pattern => {
                let v = next(&mut mp.into_inner(), "match_single_pattern", "value")?;
                values.push(build_value(v)?);
            }
            _ => values.push(build_value(mp)?),
        }
    }
    Ok(ExprMatchPattern::Values(values))
}

/// call_expr = { ident ~ "(" ~ (expr ~ ("," ~ expr)*)? ~ ")" }
fn build_call_expr(pair: Pair<'_>) -> R<(Ident, Vec<Expr>)> {
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "call_expr", "name")?);
    let mut args = Vec::new();
    for arg in it {
        args.push(build_expr(arg)?);
    }
    Ok((name, args))
}

fn logic_binop(s: &str) -> R<BinOp> {
    match s {
        "&&" | "AND" | "and" => Ok(BinOp::And),
        "||" | "OR" | "or" => Ok(BinOp::Or),
        other => Err(CompileError::internal(format!(
            "unknown logic op `{other}`"
        ))),
    }
}

fn comp_binop(s: &str) -> R<BinOp> {
    match s {
        "==" => Ok(BinOp::Eq),
        "!=" => Ok(BinOp::Neq),
        ">=" => Ok(BinOp::Gte),
        "<=" => Ok(BinOp::Lte),
        ">" => Ok(BinOp::Gt),
        "<" => Ok(BinOp::Lt),
        other => Err(CompileError::internal(format!("unknown comp op `{other}`"))),
    }
}

fn add_binop(s: &str) -> R<BinOp> {
    match s {
        "+" => Ok(BinOp::Add),
        "-" => Ok(BinOp::Sub),
        other => Err(CompileError::internal(format!("unknown add op `{other}`"))),
    }
}

fn mul_binop(s: &str) -> R<BinOp> {
    match s {
        "*" => Ok(BinOp::Mul),
        "/" => Ok(BinOp::Div),
        "%" => Ok(BinOp::Mod),
        other => Err(CompileError::internal(format!("unknown mul op `{other}`"))),
    }
}

// ══════════════════════════════════════════════════════════════════════════════════
// CONSTRUCT BUILDERS
// ══════════════════════════════════════════════════════════════════════════════════

// import_decl = { "import" ~ rust_path ~ ("as" ~ ident)? }
// rust_path = @{ ident ~ ("::" ~ ident)* }  (atomic)
fn build_import(pair: Pair<'_>) -> R<ImportDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let path_pair = next(&mut it, "import_decl", "rust_path")?;
    let path = path_pair
        .as_str()
        .split("::")
        .map(|s| Ident::new(s.trim()))
        .collect();
    let alias = it.next().map(ident_of);
    Ok(ImportDecl {
        path,
        alias,
        span: Some(span),
    })
}

// entity_decl = { "entity" ~ ident ~ ":" ~ NEWLINE ~ entity_body }
// entity_body = { entity_prefix_clause? ~ entity_fields_clause }
fn build_entity(pair: Pair<'_>) -> R<EntityDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "entity_decl", "name")?);
    let body = next(&mut it, "entity_decl", "entity_body")?;
    let mut prefix = None;
    let mut fields = Vec::new();
    for clause in body.into_inner() {
        match clause.as_rule() {
            Rule::entity_prefix_clause => {
                let s = next(
                    &mut clause.into_inner(),
                    "entity_prefix_clause",
                    "prefix string",
                )?;
                prefix = Some(string_literal_of(&s));
            }
            Rule::entity_fields_clause => {
                let list = next(
                    &mut clause.into_inner(),
                    "entity_fields_clause",
                    "entity_field_list",
                )?;
                for f in list.into_inner() {
                    fields.push(build_field(f, Rule::entity_type_expr)?);
                }
            }
            other => {
                return Err(CompileError::internal(format!(
                    "entity_body: unexpected {other:?}"
                )))
            }
        }
    }
    Ok(EntityDecl {
        name,
        prefix,
        fields,
        span: Some(span),
    })
}

// entity_field = { ident ~ ":" ~ entity_type_expr }
// field       = { ident ~ ":" ~ type_expr }
fn build_field(pair: Pair<'_>, _type_rule: Rule) -> R<FieldDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "field", "name")?);
    let ty = next(&mut it, "field", "type")?;
    Ok(FieldDecl {
        name,
        field_type: build_type_expr(ty)?,
        span: Some(span),
    })
}

// config_decl = { "config" ~ ident ~ ":" ~ NEWLINE ~ config_body }
// config_body = { (config_entry ~ NEWLINE?)+ }
fn build_config(pair: Pair<'_>) -> R<ConfigDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "config_decl", "name")?);
    let body = next(&mut it, "config_decl", "config_body")?;
    let mut entries = Vec::new();
    for entry in body.into_inner() {
        if entry.as_rule() == Rule::config_entry {
            entries.push(build_config_entry(entry)?);
        }
    }
    Ok(ConfigDecl {
        name,
        entries,
        span: Some(span),
    })
}

// config_entry = { config_key ~ ":" ~ (config_nested | config_value) ~ NEWLINE? }
// config_kv    = { config_key ~ ":" ~ (config_nested | config_value) }
fn build_config_entry(pair: Pair<'_>) -> R<ConfigEntry> {
    let mut it = pair.into_inner();
    let key = config_key_of(next(&mut it, "config_entry", "key")?);
    let val = next(&mut it, "config_entry", "value")?;
    let value = build_config_value(val)?;
    Ok(ConfigEntry { key, value })
}

// A config mapping key is `config_key = { ident | plain_scalar }`. Unwrap to the inner
// token and build an Ident from its text (plain scalars are unquoted; idents are bare),
// preserving the span for diagnostics. Mirrors `ident_of` for the wrapped case.
fn config_key_of(pair: Pair<'_>) -> Ident {
    let tok = if pair.as_rule() == Rule::config_key {
        let mut inner = pair.into_inner();
        next(&mut inner, "config_key", "key token")
            .unwrap_or_else(|_| unreachable!("config_key always wraps one token"))
    } else {
        pair
    };
    let span = span_of(&tok);
    Ident::with_span(tok.as_str().trim().to_string(), span)
}

fn build_config_value(pair: Pair<'_>) -> R<ConfigValue> {
    match pair.as_rule() {
        Rule::config_nested => {
            let mut nested = Vec::new();
            for kv in pair.into_inner() {
                if kv.as_rule() == Rule::config_kv {
                    nested.push(build_config_entry(kv)?);
                }
            }
            Ok(ConfigValue::Nested(nested))
        }
        Rule::config_value => {
            let inner = next(&mut pair.into_inner(), "config_value", "scalar")?;
            Ok(ConfigValue::Scalar(build_value(inner)?))
        }
        other => Err(CompileError::internal(format!(
            "config value: unexpected {other:?}"
        ))),
    }
}

// fact_decl = { "fact" ~ ident ~ ":" ~ NEWLINE ~ field_list }
fn build_fact(pair: Pair<'_>) -> R<FactDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "fact_decl", "name")?);
    let list = next(&mut it, "fact_decl", "field_list")?;
    let mut fields = Vec::new();
    for f in list.into_inner() {
        if f.as_rule() == Rule::field {
            fields.push(build_field(f, Rule::type_expr)?);
        }
    }
    Ok(FactDecl {
        name,
        fields,
        span: Some(span),
    })
}

// rule_decl = { "rule" ~ ident ~ ":" ~ NEWLINE ~ rule_body }
// rule_body = { priority_clause? ~ when_clause ~ let_clause* ~ then_clause ~ capture_clause? }
fn build_rule(pair: Pair<'_>) -> R<RuleDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "rule_decl", "name")?);
    let body = next(&mut it, "rule_decl", "rule_body")?;
    let mut priority = None;
    let mut conditions = Vec::new();
    let mut let_bindings = Vec::new();
    let mut actions = Vec::new();
    let mut captures = Vec::new();
    for clause in body.into_inner() {
        match clause.as_rule() {
            Rule::priority_clause => {
                let n = next(&mut clause.into_inner(), "priority_clause", "integer")?;
                priority = Some(parse_i64(&n)?);
            }
            Rule::when_clause => {
                let list = next(&mut clause.into_inner(), "when_clause", "condition_list")?;
                for c in list.into_inner() {
                    conditions.push(build_expr(c)?);
                }
            }
            Rule::let_clause => {
                let mut li = clause.into_inner();
                let lname = ident_of(next(&mut li, "let_clause", "name")?);
                let lval = next(&mut li, "let_clause", "expr")?;
                let_bindings.push(LetBinding {
                    name: lname,
                    value: build_expr(lval)?,
                });
            }
            Rule::then_clause => {
                let list = next(&mut clause.into_inner(), "then_clause", "action_list")?;
                for a in list.into_inner() {
                    if a.as_rule() == Rule::action_stmt {
                        actions.push(build_action_stmt(a)?);
                    }
                }
            }
            Rule::capture_clause => {
                for cap in clause.into_inner() {
                    if cap.as_rule() == Rule::capture_entry {
                        captures.push(build_capture_entry(cap)?);
                    }
                }
            }
            other => {
                return Err(CompileError::internal(format!(
                    "rule_body: unexpected {other:?}"
                )))
            }
        }
    }
    Ok(RuleDecl {
        name,
        priority,
        conditions,
        let_bindings,
        actions,
        captures,
        span: Some(span),
    })
}

// action_stmt = { conditional_action | simple_action }
// simple_action = { "action:" ~ ident ~ param_pair* }
// conditional_action = { kw_if ~ expr ~ ":" ~ simple_action }
fn build_action_stmt(pair: Pair<'_>) -> R<ActionStmt> {
    let inner = next(&mut pair.into_inner(), "action_stmt", "action")?;
    match inner.as_rule() {
        Rule::simple_action => build_simple_action(inner, None),
        Rule::conditional_action => {
            let mut it = inner.into_inner().filter(|p| p.as_rule() != Rule::kw_if);
            let cond = next(&mut it, "conditional_action", "expr")?;
            let cond_expr = build_expr(cond)?;
            let simple = next(&mut it, "conditional_action", "simple_action")?;
            build_simple_action(simple, Some(cond_expr))
        }
        other => Err(CompileError::internal(format!(
            "action_stmt: unexpected {other:?}"
        ))),
    }
}

fn build_simple_action(pair: Pair<'_>, condition: Option<Expr>) -> R<ActionStmt> {
    let mut it = pair.into_inner();
    let action_name = ident_of(next(&mut it, "simple_action", "name")?);
    let mut params = Vec::new();
    for pp in it {
        if pp.as_rule() == Rule::param_pair {
            params.push(build_param_pair(pp)?);
        }
    }
    Ok(ActionStmt {
        condition,
        action_name,
        params,
    })
}

// param_pair = { ident ~ ":" ~ value }
fn build_param_pair(pair: Pair<'_>) -> R<ParamPair> {
    let mut it = pair.into_inner();
    let key = ident_of(next(&mut it, "param_pair", "key")?);
    let value = build_value(next(&mut it, "param_pair", "value")?)?;
    Ok(ParamPair { key, value })
}

// capture_entry = { "fact:" ~ string ~ ("category:" ~ ident)? ~ ("tags:" ~ list_val)? }
fn build_capture_entry(pair: Pair<'_>) -> R<CaptureEntry> {
    let mut it = pair.into_inner();
    let fact = string_literal_of(&next(&mut it, "capture_entry", "fact string")?);
    let mut category = None;
    let mut tags = None;
    for rest in it {
        match rest.as_rule() {
            Rule::ident => category = Some(ident_of(rest)),
            Rule::list_val => {
                let items = rest.into_inner().map(build_value).collect::<R<Vec<_>>>()?;
                tags = Some(items);
            }
            other => {
                return Err(CompileError::internal(format!(
                    "capture_entry: unexpected {other:?}"
                )))
            }
        }
    }
    Ok(CaptureEntry {
        fact,
        category,
        tags,
    })
}

// constraint_decl = { "constraint" ~ ident ~ ":" ~ NEWLINE ~ constraint_body }
fn build_constraint(pair: Pair<'_>) -> R<ConstraintDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "constraint_decl", "name")?);
    let body = next(&mut it, "constraint_decl", "constraint_body")?;
    let mut scope = None;
    let mut phase = Vec::new();
    let mut trait_name = None;
    let mut weight = None;
    let mut prompt = None;
    let mut when = None;
    let mut require = None;
    let mut severity = None;
    let mut message = None;
    for clause in body.into_inner() {
        match clause.as_rule() {
            Rule::scope_clause => {
                scope = Some(ident_of(next(
                    &mut clause.into_inner(),
                    "scope_clause",
                    "ident",
                )?));
            }
            Rule::phase_clause => {
                let csv = next(&mut clause.into_inner(), "phase_clause", "ident_csv")?;
                phase = csv.into_inner().map(ident_of).collect();
            }
            Rule::trait_clause => {
                trait_name = Some(ident_of(next(
                    &mut clause.into_inner(),
                    "trait_clause",
                    "ident",
                )?));
            }
            Rule::weight_clause => {
                let f = next(&mut clause.into_inner(), "weight_clause", "float")?;
                weight = Some(parse_f64(&f)?);
            }
            Rule::prompt_clause => {
                prompt = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "prompt_clause",
                    "string",
                )?));
            }
            Rule::when_expr => {
                let e = next(&mut clause.into_inner(), "when_expr", "expr")?;
                when = Some(build_expr(e)?);
            }
            Rule::require_expr => {
                let e = next(&mut clause.into_inner(), "require_expr", "expr")?;
                require = Some(build_expr(e)?);
            }
            Rule::severity_clause => {
                let lvl = next(
                    &mut clause.into_inner(),
                    "severity_clause",
                    "severity_level",
                )?;
                severity = Some(match lvl.as_str().trim() {
                    "error" => Severity::Error,
                    "warning" => Severity::Warning,
                    "info" => Severity::Info,
                    other => {
                        return Err(CompileError::internal(format!(
                            "unknown severity `{other}`"
                        )))
                    }
                });
            }
            Rule::message_clause => {
                message = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "message_clause",
                    "string",
                )?));
            }
            other => {
                return Err(CompileError::internal(format!(
                    "constraint_body: unexpected {other:?}"
                )))
            }
        }
    }
    let severity = severity.ok_or_else(|| {
        CompileError::internal("constraint_body: severity is required by grammar but missing")
    })?;
    Ok(ConstraintDecl {
        name,
        scope,
        phase,
        trait_name,
        weight,
        prompt,
        when,
        require,
        severity,
        message,
        span: Some(span),
    })
}

// contract_decl = { "contract" ~ ident ~ ":" ~ NEWLINE ~ contract_body }
// contract_body = { given_clause? ~ when_desc? ~ then_desc? ~ threshold_clause? ~ examples_clause }
fn build_contract(pair: Pair<'_>) -> R<ContractDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "contract_decl", "name")?);
    let body = next(&mut it, "contract_decl", "contract_body")?;
    let mut given = None;
    let mut when = None;
    let mut then = None;
    let mut threshold = None;
    let mut examples = Vec::new();
    for clause in body.into_inner() {
        match clause.as_rule() {
            Rule::given_clause => {
                given = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "given_clause",
                    "string",
                )?));
            }
            Rule::when_desc => {
                when = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "when_desc",
                    "string",
                )?));
            }
            Rule::then_desc => {
                then = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "then_desc",
                    "string",
                )?));
            }
            Rule::threshold_clause => {
                let f = next(&mut clause.into_inner(), "threshold_clause", "float")?;
                threshold = Some(parse_f64(&f)?);
            }
            Rule::examples_clause => {
                let list = next(&mut clause.into_inner(), "examples_clause", "example_list")?;
                for ex in list.into_inner() {
                    if ex.as_rule() == Rule::example {
                        examples.push(build_example(ex)?);
                    }
                }
            }
            other => {
                return Err(CompileError::internal(format!(
                    "contract_body: unexpected {other:?}"
                )))
            }
        }
    }
    Ok(ContractDecl {
        name,
        given,
        when,
        then,
        threshold,
        examples,
        span: Some(span),
    })
}

// example = { "input:" ~ value ~ NEWLINE ~ "expect:" ~ value ~ (NEWLINE ~ "threshold:" ~ float)? }
fn build_example(pair: Pair<'_>) -> R<ContractExample> {
    let mut it = pair.into_inner();
    let input = build_value(next(&mut it, "example", "input value")?)?;
    let expect = build_value(next(&mut it, "example", "expect value")?)?;
    let threshold = match it.next() {
        Some(f) => Some(parse_f64(&f)?),
        None => None,
    };
    Ok(ContractExample {
        input,
        expect,
        threshold,
    })
}

// function_decl = { "function" ~ ident ~ "(" ~ param_list? ~ ")" ~ "->" ~ type_expr ~ ":" ~ NEWLINE ~ function_body }
// function_body = { mode_clause? ~ docstring }
fn build_function(pair: Pair<'_>) -> R<FunctionDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "function_decl", "name")?);
    let mut params = Vec::new();
    let mut return_type = None;
    let mut mode = None;
    let mut docstring = None;
    for p in it {
        match p.as_rule() {
            Rule::param_list => {
                for param in p.into_inner() {
                    params.push(build_param(param)?);
                }
            }
            Rule::type_expr => return_type = Some(build_type_expr(p)?),
            Rule::function_body => {
                for bc in p.into_inner() {
                    match bc.as_rule() {
                        Rule::mode_clause => {
                            let m = next(&mut bc.into_inner(), "mode_clause", "function_mode")?;
                            mode = Some(match m.as_str().trim() {
                                "deterministic" => FunctionMode::Deterministic,
                                "probabilistic" => FunctionMode::Probabilistic,
                                "hybrid" => FunctionMode::Hybrid,
                                other => {
                                    return Err(CompileError::internal(format!(
                                        "unknown function mode `{other}`"
                                    )))
                                }
                            });
                        }
                        Rule::docstring => docstring = Some(docstring_of(bc.as_str())),
                        other => {
                            return Err(CompileError::internal(format!(
                                "function_body: unexpected {other:?}"
                            )))
                        }
                    }
                }
            }
            other => {
                return Err(CompileError::internal(format!(
                    "function_decl: unexpected {other:?}"
                )))
            }
        }
    }
    let return_type = return_type.ok_or_else(|| {
        CompileError::internal("function_decl: return type required by grammar but missing")
    })?;
    Ok(FunctionDecl {
        name,
        params,
        return_type,
        mode,
        docstring,
        span: Some(span),
    })
}

// param = { ident ~ ":" ~ type_expr }
fn build_param(pair: Pair<'_>) -> R<FieldDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "param", "name")?);
    let ty = next(&mut it, "param", "type")?;
    Ok(FieldDecl {
        name,
        field_type: build_type_expr(ty)?,
        span: Some(span),
    })
}

// trigger_decl = { "trigger" ~ ident ~ ":" ~ NEWLINE ~ trigger_body }
// trigger_body = { on_clause ~ schedule_clause? ~ run_clause }
fn build_trigger(pair: Pair<'_>) -> R<TriggerDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "trigger_decl", "name")?);
    let body = next(&mut it, "trigger_decl", "trigger_body")?;
    let mut event = None;
    let mut schedule = None;
    let mut run = None;
    for clause in body.into_inner() {
        match clause.as_rule() {
            Rule::on_clause => {
                let ev = next(&mut clause.into_inner(), "on_clause", "trigger_event")?;
                event = Some(build_trigger_event(ev)?);
            }
            Rule::schedule_clause => {
                schedule = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "schedule_clause",
                    "string",
                )?));
            }
            Rule::run_clause => {
                run = Some(ident_of(next(
                    &mut clause.into_inner(),
                    "run_clause",
                    "ident",
                )?));
            }
            other => {
                return Err(CompileError::internal(format!(
                    "trigger_body: unexpected {other:?}"
                )))
            }
        }
    }
    let event = event
        .ok_or_else(|| CompileError::internal("trigger_body: on_clause required but missing"))?;
    let run =
        run.ok_or_else(|| CompileError::internal("trigger_body: run_clause required but missing"))?;
    Ok(TriggerDecl {
        name,
        event,
        schedule,
        run,
        span: Some(span),
    })
}

// trigger_event = { "after_store" | "before_search" | "on_event(" ~ string ~ ")" | "timer" }
fn build_trigger_event(pair: Pair<'_>) -> R<TriggerEvent> {
    let raw = pair.as_str().trim();
    if raw.starts_with("after_store") {
        Ok(TriggerEvent::AfterStore)
    } else if raw.starts_with("before_search") {
        Ok(TriggerEvent::BeforeSearch)
    } else if raw.starts_with("timer") {
        Ok(TriggerEvent::Timer)
    } else if raw.starts_with("on_event") {
        // inner `string` pair carries the event name
        let s = pair
            .into_inner()
            .find(|p| p.as_rule() == Rule::string)
            .ok_or_else(|| CompileError::internal("on_event: missing string"))?;
        Ok(TriggerEvent::OnEvent(string_literal_of(&s)))
    } else {
        Err(CompileError::internal(format!(
            "unknown trigger_event `{raw}`"
        )))
    }
}

// scenario_decl = { "scenario" ~ ident ~ ":" ~ NEWLINE ~ scenario_body }
// scenario_body = { given_clause ~ setup_clause? ~ scenario_run_clause? ~ expect_clause }
fn build_scenario(pair: Pair<'_>) -> R<ScenarioDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "scenario_decl", "name")?);
    let body = next(&mut it, "scenario_decl", "scenario_body")?;
    let mut given = None;
    let mut setup = Vec::new();
    let mut run = None;
    let mut expectations = Vec::new();
    for clause in body.into_inner() {
        match clause.as_rule() {
            Rule::given_clause => {
                given = Some(string_literal_of(&next(
                    &mut clause.into_inner(),
                    "given_clause",
                    "string",
                )?));
            }
            Rule::setup_clause => {
                let list = next(&mut clause.into_inner(), "setup_clause", "step_list")?;
                setup = build_step_list(list)?;
            }
            Rule::scenario_run_clause => {
                let mut ri = clause.into_inner();
                let proc = ident_of(next(&mut ri, "scenario_run_clause", "procedure")?);
                let args = match ri.next() {
                    Some(m) => Some(Value::Map(build_map_entries(m)?)),
                    None => None,
                };
                run = Some(ScenarioRun {
                    procedure: proc,
                    args,
                });
            }
            Rule::expect_clause => {
                let list = next(
                    &mut clause.into_inner(),
                    "expect_clause",
                    "expectation_list",
                )?;
                for exp in list.into_inner() {
                    if exp.as_rule() == Rule::expectation {
                        expectations.push(build_expectation(exp)?);
                    }
                }
            }
            other => {
                return Err(CompileError::internal(format!(
                    "scenario_body: unexpected {other:?}"
                )))
            }
        }
    }
    Ok(ScenarioDecl {
        name,
        given,
        setup,
        run,
        expectations,
        span: Some(span),
    })
}

// expectation = { not_expectation | positive_expectation }
// positive_expectation = { ident ~ map_val? }
fn build_expectation(pair: Pair<'_>) -> R<Expectation> {
    let inner = next(&mut pair.into_inner(), "expectation", "inner")?;
    let (negated, positive) = match inner.as_rule() {
        Rule::not_expectation => {
            let p = next(
                &mut inner.into_inner(),
                "not_expectation",
                "positive_expectation",
            )?;
            (true, p)
        }
        Rule::positive_expectation => (false, inner),
        other => {
            return Err(CompileError::internal(format!(
                "expectation: unexpected {other:?}"
            )))
        }
    };
    let mut it = positive.into_inner();
    let name = ident_of(next(&mut it, "positive_expectation", "name")?);
    let args = match it.next() {
        Some(m) => Some(Value::Map(build_map_entries(m)?)),
        None => None,
    };
    Ok(Expectation {
        negated,
        name,
        args,
    })
}

// ══════════════════════════════════════════════════════════════════════════════════
// PROCEDURES
// ══════════════════════════════════════════════════════════════════════════════════

// dataflow_procedure_decl = {
//   "procedure" ~ ident ~ "(" ~ dataflow_param_list? ~ ")" ~ dataflow_return_type? ~ ":" ~ NEWLINE
//   ~ given_clause? ~ (code_block | step_list) }
fn build_dataflow_procedure(pair: Pair<'_>) -> R<DataflowProcedureDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "dataflow_procedure_decl", "name")?);
    let mut params = Vec::new();
    let mut return_type = None;
    let mut given = None;
    let mut body = None;
    for p in it {
        match p.as_rule() {
            Rule::dataflow_param_list => {
                for param in p.into_inner() {
                    params.push(build_dataflow_param(param)?);
                }
            }
            Rule::dataflow_return_type => return_type = Some(build_dataflow_return(p)?),
            Rule::given_clause => {
                given = Some(string_literal_of(&next(
                    &mut p.into_inner(),
                    "given_clause",
                    "string",
                )?));
            }
            Rule::step_list => body = Some(ProcedureBody::Steps(build_step_list(p)?)),
            Rule::code_block => body = Some(ProcedureBody::Code(build_code_block(p)?)),
            other => {
                return Err(CompileError::internal(format!(
                    "dataflow_procedure_decl: unexpected {other:?}"
                )))
            }
        }
    }
    let body = body.ok_or_else(|| {
        CompileError::internal("dataflow_procedure_decl: body (steps or code) required but missing")
    })?;
    Ok(DataflowProcedureDecl {
        name,
        params,
        return_type,
        given,
        body,
        span: Some(span),
    })
}

// dataflow_param = { ident ~ ":" ~ type_expr ~ dataflow_source_binding? }
// dataflow_source_binding = { "from" ~ string }
fn build_dataflow_param(pair: Pair<'_>) -> R<DataflowParam> {
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "dataflow_param", "name")?);
    let ty = next(&mut it, "dataflow_param", "type")?;
    let param_type = build_type_expr(ty)?;
    let source_queue = match it.next() {
        Some(binding) => {
            let s = next(
                &mut binding.into_inner(),
                "dataflow_source_binding",
                "string",
            )?;
            Some(string_literal_of(&s))
        }
        None => None,
    };
    Ok(DataflowParam {
        name,
        param_type,
        source_queue,
    })
}

// dataflow_return_type = { "->" ~ type_expr ~ dataflow_dest_binding? }
// dataflow_dest_binding = { "into" ~ string }
fn build_dataflow_return(pair: Pair<'_>) -> R<DataflowReturn> {
    let mut it = pair.into_inner();
    let ty = next(&mut it, "dataflow_return_type", "type")?;
    let return_type = build_type_expr(ty)?;
    let dest_queue = match it.next() {
        Some(binding) => {
            let s = next(&mut binding.into_inner(), "dataflow_dest_binding", "string")?;
            Some(string_literal_of(&s))
        }
        None => None,
    };
    Ok(DataflowReturn {
        return_type,
        dest_queue,
    })
}

// procedure_decl = { "procedure" ~ ident ~ ":" ~ NEWLINE ~ procedure_body }
// procedure_body = { procedure_trigger_clause? ~ params_clause? ~ given_clause? ~ (code_block | step_list) }
fn build_legacy_procedure(pair: Pair<'_>) -> R<LegacyProcedureDecl> {
    let span = span_of(&pair);
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "procedure_decl", "name")?);
    let body_pair = next(&mut it, "procedure_decl", "procedure_body")?;
    let mut trigger = None;
    let mut params = Vec::new();
    let mut given = None;
    let mut body = None;
    for p in body_pair.into_inner() {
        match p.as_rule() {
            Rule::procedure_trigger_clause => trigger = Some(build_procedure_trigger(p)?),
            Rule::params_clause => {
                for pi in p.into_inner() {
                    if pi.as_rule() == Rule::param_ident {
                        params.push(param_ident_of(pi));
                    }
                }
            }
            Rule::given_clause => {
                given = Some(string_literal_of(&next(
                    &mut p.into_inner(),
                    "given_clause",
                    "string",
                )?));
            }
            Rule::step_list => body = Some(ProcedureBody::Steps(build_step_list(p)?)),
            Rule::code_block => body = Some(ProcedureBody::Code(build_code_block(p)?)),
            other => {
                return Err(CompileError::internal(format!(
                    "procedure_body: unexpected {other:?}"
                )))
            }
        }
    }
    let body = body.ok_or_else(|| {
        CompileError::internal("procedure_body: body (steps or code) required but missing")
    })?;
    Ok(LegacyProcedureDecl {
        name,
        trigger,
        params,
        given,
        body,
        span: Some(span),
    })
}

// param_ident = @{ "$"? ~ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }  (atomic)
fn param_ident_of(pair: Pair<'_>) -> Ident {
    let span = span_of(&pair);
    let raw = pair.as_str();
    let name = raw.strip_prefix('$').unwrap_or(raw);
    Ident::with_span(name.to_string(), span)
}

// procedure_trigger_clause = { "trigger:" ~ procedure_trigger_kind ~ NEWLINE ~ trigger_sub_keys? }
fn build_procedure_trigger(pair: Pair<'_>) -> R<ProcedureTrigger> {
    let kind = next(&mut pair.into_inner(), "procedure_trigger_clause", "kind")?;
    build_procedure_trigger_kind(kind)
}

// procedure_trigger_kind = {
//   "periodic" ~ map_val? | "on_write" ~ trigger_pattern? ~ map_val? | "on_event" ~ "(" ~ string ~ ")"
//   | "startup" | "before_response" | "after_response" | "cron" ~ map_val? | "manual" }
fn build_procedure_trigger_kind(pair: Pair<'_>) -> R<ProcedureTrigger> {
    // The bare keyword is the leading identifier of the match. Note that
    // compound forms like `on_write("q")` have NO whitespace before `(`, so we
    // take the leading `[a-z_]+` run rather than splitting on whitespace.
    let raw = pair.as_str().trim();
    let keyword: String = raw
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let mut inner = pair.into_inner();
    match keyword.as_str() {
        "periodic" => {
            let interval = match inner.next() {
                Some(m) => Some(Value::Map(build_map_entries(m)?)),
                None => None,
            };
            Ok(ProcedureTrigger::Periodic { interval })
        }
        "on_write" => {
            let mut pattern = None;
            let mut args = None;
            for p in inner {
                match p.as_rule() {
                    Rule::trigger_pattern => {
                        let s = next(&mut p.into_inner(), "trigger_pattern", "string")?;
                        pattern = Some(string_literal_of(&s));
                    }
                    Rule::map_val => args = Some(Value::Map(build_map_entries(p)?)),
                    _ => {}
                }
            }
            Ok(ProcedureTrigger::OnWrite { pattern, args })
        }
        "on_event" => {
            let s = inner
                .find(|p| p.as_rule() == Rule::string)
                .ok_or_else(|| CompileError::internal("on_event: missing string"))?;
            Ok(ProcedureTrigger::OnEvent(string_literal_of(&s)))
        }
        "startup" => Ok(ProcedureTrigger::Startup),
        "before_response" => Ok(ProcedureTrigger::BeforeResponse),
        "after_response" => Ok(ProcedureTrigger::AfterResponse),
        "cron" => {
            let schedule = match inner.next() {
                Some(m) => Some(Value::Map(build_map_entries(m)?)),
                None => None,
            };
            Ok(ProcedureTrigger::Cron { schedule })
        }
        "manual" => Ok(ProcedureTrigger::Manual),
        other => Err(CompileError::internal(format!(
            "unknown procedure trigger kind `{other}`"
        ))),
    }
}

// ══════════════════════════════════════════════════════════════════════════════════
// V1 STEPS
// ══════════════════════════════════════════════════════════════════════════════════

// step_list = { ((step_decl ~ NEWLINE) | NEWLINE)+ }
fn build_step_list(pair: Pair<'_>) -> R<Vec<Step>> {
    let mut steps = Vec::new();
    for p in pair.into_inner() {
        if p.as_rule() == Rule::step_decl {
            steps.push(build_step(p)?);
        }
    }
    Ok(steps)
}

fn build_step(pair: Pair<'_>) -> R<Step> {
    let inner = next(&mut pair.into_inner(), "step_decl", "concrete step")?;
    match inner.as_rule() {
        Rule::step_define => {
            // step_define = { kw_define ~ "$" ~ ident ~ "=" ~ value }
            let mut it = payload_pairs(inner);
            let var = ident_of(next(&mut it, "step_define", "var")?);
            let value = build_value(next(&mut it, "step_define", "value")?)?;
            Ok(Step::Define { var, value })
        }
        Rule::step_return => {
            let value = match payload_pairs(inner).next() {
                Some(v) => Some(build_value(v)?),
                None => None,
            };
            Ok(Step::Return { value })
        }
        Rule::step_abort => {
            let value = match payload_pairs(inner).next() {
                Some(v) => Some(build_value(v)?),
                None => None,
            };
            Ok(Step::Abort { value })
        }
        Rule::step_call => build_step_call(inner),
        Rule::step_assign => {
            // step_assign = { var_ref ~ "=" ~ assign_value }  (assign_value is atomic text)
            let mut it = inner.into_inner();
            let target = build_var_ref(next(&mut it, "step_assign", "var_ref")?)?;
            let value = next(&mut it, "step_assign", "assign_value")?
                .as_str()
                .trim()
                .to_string();
            Ok(Step::Assign { target, value })
        }
        Rule::step_if => {
            // step_if = { kw_if ~ expr ~ ":" ~ NEWLINE ~ block_step_list ~ step_else? ~ kw_end }
            let mut it = payload_pairs(inner);
            let cond = build_expr(next(&mut it, "step_if", "condition")?)?;
            let then_block = next(&mut it, "step_if", "then block")?;
            let then_steps = build_block_step_list(then_block)?;
            let else_steps = match it.next() {
                Some(else_clause) if else_clause.as_rule() == Rule::step_else => {
                    let inner_block = next(&mut payload_pairs(else_clause), "step_else", "block")?;
                    Some(build_block_step_list(inner_block)?)
                }
                _ => None,
            };
            Ok(Step::If {
                condition: cond,
                then_steps,
                else_steps,
            })
        }
        Rule::step_match => {
            // step_match = { kw_match ~ ":" ~ NEWLINE ~ match_arm_list ~ kw_end }
            let arm_list = next(&mut payload_pairs(inner), "step_match", "match_arm_list")?;
            let mut arms = Vec::new();
            for arm in arm_list.into_inner() {
                if arm.as_rule() == Rule::match_arm {
                    // match_arm = { expr ~ "->" ~ ident }
                    let mut ai = arm.into_inner();
                    let pattern = build_expr(next(&mut ai, "match_arm", "pattern")?)?;
                    let target = ident_of(next(&mut ai, "match_arm", "target")?);
                    arms.push(MatchArm { pattern, target });
                }
            }
            Ok(Step::Match { arms })
        }
        Rule::step_when => {
            // step_when = { kw_when ~ expr ~ ":" ~ NEWLINE ~ block_step_list ~ kw_end }
            let mut it = payload_pairs(inner);
            let condition = build_expr(next(&mut it, "step_when", "condition")?)?;
            let block = next(&mut it, "step_when", "block")?;
            Ok(Step::When {
                condition,
                steps: build_block_step_list(block)?,
            })
        }
        Rule::step_for => {
            // step_for = { kw_for ~ var_ref ~ kw_in ~ (call_expr|var_ref|dotted_ident) ~ ":" ~ NEWLINE ~ block_step_list ~ kw_end }
            let mut it = payload_pairs(inner);
            let var = build_var_ref(next(&mut it, "step_for", "var_ref")?)?;
            let coll = next(&mut it, "step_for", "collection")?;
            let collection = build_expr(coll)?;
            let block = next(&mut it, "step_for", "block")?;
            Ok(Step::For {
                var,
                collection,
                steps: build_block_step_list(block)?,
            })
        }
        Rule::step_loop => build_step_loop(inner),
        Rule::step_emit => {
            // step_emit = { kw_emit ~ (map_val | param_pair+) }
            let mut params = Vec::new();
            for p in payload_pairs(inner) {
                match p.as_rule() {
                    Rule::map_val => {
                        for (k, v) in build_map_entries(p)? {
                            params.push((k, v));
                        }
                    }
                    Rule::param_pair => {
                        let pp = build_param_pair(p)?;
                        params.push((pp.key, pp.value));
                    }
                    _ => {}
                }
            }
            Ok(Step::Emit { params })
        }
        Rule::step_try => build_step_try(inner),
        Rule::step_parallel => build_step_parallel(inner),
        other => Err(CompileError::internal(format!(
            "step_decl: unexpected {other:?}"
        ))),
    }
}

// block_step_list = { (!(kw_end | "catch:") ~ ((step_decl ~ NEWLINE) | NEWLINE))+ }
fn build_block_step_list(pair: Pair<'_>) -> R<Vec<Step>> {
    let mut steps = Vec::new();
    for p in pair.into_inner() {
        if p.as_rule() == Rule::step_decl {
            steps.push(build_step(p)?);
        }
    }
    Ok(steps)
}

// step_call = { ident ~ (call_args | map_val | param_pair+ | value+)? ~ ("->" ~ "$" ~ ident)? }
fn build_step_call(pair: Pair<'_>) -> R<Step> {
    let mut it = pair.into_inner();
    let action = ident_of(next(&mut it, "step_call", "action")?);
    let mut args = StepCallArgs::None;
    let mut output = None;
    // Collect the remaining pairs and classify.
    let rest: Vec<Pair<'_>> = it.collect();
    // The trailing `-> $ident` output ident is the LAST ident pair when preceded
    // by nothing else that consumes it; but idents also appear as `value`s.
    // Grammar guarantees the output ident (if present) is a bare `ident` pair
    // that is the final pair. Detect: last pair is Rule::ident AND there is an
    // arrow — but the arrow is literal (not a pair). We treat a final bare
    // `ident` pair as the output target.
    let mut rest_iter = rest.into_iter().peekable();
    let mut positional: Vec<Expr> = Vec::new();
    let mut param_pairs: Vec<(Ident, Value)> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    let mut trailing_ident: Option<Pair<'_>> = None;
    let mut arg_kind = 0u8; // 0 none, 1 positional(call_args), 2 map, 3 params, 4 values
    while let Some(p) = rest_iter.next() {
        match p.as_rule() {
            Rule::call_args => {
                arg_kind = 1;
                for e in p.into_inner() {
                    positional.push(build_expr(e)?);
                }
            }
            Rule::map_val => {
                arg_kind = 2;
                args = StepCallArgs::Map(Value::Map(build_map_entries(p)?));
            }
            Rule::param_pair => {
                arg_kind = 3;
                let pp = build_param_pair(p)?;
                param_pairs.push((pp.key, pp.value));
            }
            Rule::ident => {
                // Could be a bare value OR the trailing output ident. If it is
                // the last pair, treat as output; otherwise a value.
                if rest_iter.peek().is_none() {
                    trailing_ident = Some(p);
                } else {
                    arg_kind = 4;
                    values.push(Value::Ident(ident_of(p)));
                }
            }
            // value alternatives
            _ => {
                if rest_iter.peek().is_none() && p.as_rule() == Rule::ident {
                    trailing_ident = Some(p);
                } else {
                    arg_kind = 4;
                    values.push(build_value(p)?);
                }
            }
        }
    }
    if let Some(id) = trailing_ident {
        output = Some(ident_of(id));
    }
    args = match arg_kind {
        1 => StepCallArgs::Positional(positional),
        2 => args,
        3 => StepCallArgs::Params(param_pairs),
        4 => StepCallArgs::Values(values),
        _ => StepCallArgs::None,
    };
    Ok(Step::Call(StepCall {
        action,
        args,
        output,
    }))
}

// step_loop = { kw_loop ~ loop_source ~ ("as" ~ ident)? ~ key_as_clause? ~ ("->" ~ "$" ~ ident)? ~ ":" ~ NEWLINE ~ block_step_list ~ kw_end }
// loop_source = { "over" ~ "$" ~ ident | "times" ~ integer }
fn build_step_loop(pair: Pair<'_>) -> R<Step> {
    let mut it = payload_pairs(pair);
    let source_pair = next(&mut it, "step_loop", "loop_source")?;
    let source = build_loop_source(source_pair)?;
    let mut item_name = None;
    let mut key_name = None;
    let mut output = None;
    let mut steps = Vec::new();
    for p in it {
        match p.as_rule() {
            Rule::key_as_clause => {
                let id = next(&mut p.into_inner(), "key_as_clause", "ident")?;
                key_name = Some(ident_of(id));
            }
            Rule::ident => {
                // Either the `as <item>` ident or the `-> $ident` output.
                if item_name.is_none() {
                    item_name = Some(ident_of(p));
                } else {
                    output = Some(ident_of(p));
                }
            }
            Rule::block_step_list => steps = build_block_step_list(p)?,
            _ => {}
        }
    }
    Ok(Step::Loop(LoopStep {
        source,
        item_name,
        key_name,
        output,
        steps,
    }))
}

fn build_loop_source(pair: Pair<'_>) -> R<LoopSource> {
    let raw = pair.as_str().trim();
    if raw.starts_with("over") {
        let id = pair
            .into_inner()
            .find(|p| p.as_rule() == Rule::ident)
            .ok_or_else(|| CompileError::internal("loop_source over: missing ident"))?;
        Ok(LoopSource::Over(ident_of(id)))
    } else if raw.starts_with("times") {
        let n = pair
            .into_inner()
            .find(|p| p.as_rule() == Rule::integer)
            .ok_or_else(|| CompileError::internal("loop_source times: missing integer"))?;
        Ok(LoopSource::Times(parse_i64(&n)?))
    } else {
        Err(CompileError::internal(format!(
            "unknown loop_source `{raw}`"
        )))
    }
}

// step_try = { kw_try ~ try_retry_clause? ~ ":" ~ NEWLINE ~ try_step_list ~ catch_clause? ~ kw_end }
fn build_step_try(pair: Pair<'_>) -> R<Step> {
    let mut retries = None;
    let mut retry_opts = Vec::new();
    let mut steps = Vec::new();
    let mut catch = None;
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::try_retry_clause => {
                let (n, opts) = build_retry_clause(p)?;
                retries = Some(n);
                retry_opts = opts;
            }
            Rule::try_step_list => {
                for s in p.into_inner() {
                    if s.as_rule() == Rule::step_decl {
                        steps.push(build_step(s)?);
                    }
                }
            }
            Rule::catch_clause => {
                let mut cs = Vec::new();
                for s in p.into_inner() {
                    if s.as_rule() == Rule::step_decl {
                        cs.push(build_step(s)?);
                    }
                }
                catch = Some(cs);
            }
            _ => {}
        }
    }
    Ok(Step::Try(TryStep {
        retries,
        retry_opts,
        steps,
        catch,
    }))
}

// try_retry_clause = { "retry" ~ integer ~ branch_retry_opt* }
// branch_retry_clause = { "retry" ~ integer ~ branch_retry_opt* }
fn build_retry_clause(pair: Pair<'_>) -> R<(i64, Vec<RetryOpt>)> {
    let mut it = pair.into_inner();
    let n = parse_i64(&next(&mut it, "retry clause", "integer")?)?;
    let mut opts = Vec::new();
    for opt in it {
        if opt.as_rule() == Rule::branch_retry_opt {
            opts.push(build_retry_opt(opt)?);
        }
    }
    Ok((n, opts))
}

// branch_retry_opt = { retry_delay_opt | retry_backoff_opt | retry_max_delay_opt | retry_jitter_opt }
fn build_retry_opt(pair: Pair<'_>) -> R<RetryOpt> {
    let inner = next(&mut pair.into_inner(), "branch_retry_opt", "opt")?;
    match inner.as_rule() {
        Rule::retry_delay_opt => {
            let n = inner
                .into_inner()
                .find(|p| p.as_rule() == Rule::integer)
                .ok_or_else(|| CompileError::internal("retry_delay_opt: missing integer"))?;
            Ok(RetryOpt::Delay(parse_i64(&n)?))
        }
        Rule::retry_backoff_opt => {
            let strat = next(
                &mut inner.into_inner(),
                "retry_backoff_opt",
                "backoff_strategy",
            )?;
            Ok(RetryOpt::Backoff(match strat.as_str().trim() {
                "exponential" => BackoffStrategy::Exponential,
                "fixed" => BackoffStrategy::Fixed,
                other => {
                    return Err(CompileError::internal(format!(
                        "unknown backoff strategy `{other}`"
                    )))
                }
            }))
        }
        Rule::retry_max_delay_opt => {
            let n = inner
                .into_inner()
                .find(|p| p.as_rule() == Rule::integer)
                .ok_or_else(|| CompileError::internal("retry_max_delay_opt: missing integer"))?;
            Ok(RetryOpt::MaxDelay(parse_i64(&n)?))
        }
        Rule::retry_jitter_opt => Ok(RetryOpt::Jitter),
        other => Err(CompileError::internal(format!(
            "branch_retry_opt: unexpected {other:?}"
        ))),
    }
}

// step_parallel = { kw_parallel ~ ("->" ~ "$" ~ ident)? ~ ":" ~ NEWLINE ~ parallel_branch_list ~ kw_end }
fn build_step_parallel(pair: Pair<'_>) -> R<Step> {
    let mut output = None;
    let mut branches = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::ident => output = Some(ident_of(p)),
            Rule::parallel_branch_list => {
                for b in p.into_inner() {
                    if b.as_rule() == Rule::parallel_branch {
                        branches.push(build_parallel_branch(b)?);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(Step::Parallel(ParallelStep { output, branches }))
}

// parallel_branch = { "branch" ~ ident ~ branch_retry_clause? ~ ":" ~ NEWLINE ~ block_step_list ~ kw_end }
fn build_parallel_branch(pair: Pair<'_>) -> R<ParallelBranch> {
    let mut it = pair.into_inner();
    let name = ident_of(next(&mut it, "parallel_branch", "name")?);
    let mut retries = None;
    let mut retry_opts = Vec::new();
    let mut steps = Vec::new();
    for p in it {
        match p.as_rule() {
            Rule::branch_retry_clause => {
                let (n, opts) = build_retry_clause(p)?;
                retries = Some(n);
                retry_opts = opts;
            }
            Rule::block_step_list => steps = build_block_step_list(p)?,
            _ => {}
        }
    }
    Ok(ParallelBranch {
        name,
        retries,
        retry_opts,
        steps,
    })
}

// ══════════════════════════════════════════════════════════════════════════════════
// V2 CODE BLOCK (Rust-style imperative body)
// ══════════════════════════════════════════════════════════════════════════════════

// code_block = { "{" ~ NEWLINE* ~ (code_stmt ~ NEWLINE*)* ~ "}" }
fn build_code_block(pair: Pair<'_>) -> R<CodeBlock> {
    let mut statements = Vec::new();
    for p in pair.into_inner() {
        if p.as_rule() == Rule::code_stmt {
            statements.push(build_code_stmt(p)?);
        }
    }
    Ok(CodeBlock { statements })
}

fn build_code_stmt(pair: Pair<'_>) -> R<CodeStmt> {
    let inner = next(&mut pair.into_inner(), "code_stmt", "concrete stmt")?;
    match inner.as_rule() {
        Rule::code_let_stmt => {
            // code_let_stmt = { "let" ~ ident ~ "=" ~ code_expr ~ ";" }
            let mut it = inner.into_inner();
            let name = ident_of(next(&mut it, "code_let_stmt", "name")?);
            let value = build_code_expr(next(&mut it, "code_let_stmt", "value")?)?;
            Ok(CodeStmt::Let { name, value })
        }
        Rule::code_assign_stmt => {
            // code_assign_stmt = { code_lvalue ~ code_assign_op ~ code_expr ~ ";" }
            let mut it = inner.into_inner();
            let target = next(&mut it, "code_assign_stmt", "lvalue")?
                .as_str()
                .to_string();
            let op_pair = next(&mut it, "code_assign_stmt", "assign_op")?;
            let op = match op_pair.as_str() {
                "=" => AssignOp::Set,
                "+=" => AssignOp::AddAssign,
                "-=" => AssignOp::SubAssign,
                other => {
                    return Err(CompileError::internal(format!(
                        "unknown assign op `{other}`"
                    )))
                }
            };
            let value = build_code_expr(next(&mut it, "code_assign_stmt", "value")?)?;
            Ok(CodeStmt::Assign { target, op, value })
        }
        Rule::code_if_stmt => build_code_if(inner),
        Rule::code_for_stmt => {
            // code_for_stmt = { "for" ~ ident ~ "in" ~ code_expr ~ code_block }
            let mut it = inner.into_inner();
            let var = ident_of(next(&mut it, "code_for_stmt", "var")?);
            let iter = build_code_expr(next(&mut it, "code_for_stmt", "iter")?)?;
            let body = build_code_block(next(&mut it, "code_for_stmt", "body")?)?;
            Ok(CodeStmt::For { var, iter, body })
        }
        Rule::code_match_stmt => {
            // code_match_stmt = { "match" ~ code_expr ~ "{" ~ (code_match_arm)+ ~ "}" }
            let mut it = inner.into_inner();
            let subject = build_code_expr(next(&mut it, "code_match_stmt", "subject")?)?;
            let mut arms = Vec::new();
            for arm in it {
                if arm.as_rule() == Rule::code_match_arm {
                    arms.push(build_code_match_arm(arm)?);
                }
            }
            Ok(CodeStmt::Match { subject, arms })
        }
        Rule::code_try_stmt => {
            // code_try_stmt = { "try" ~ code_block ~ ("catch" ~ ident? ~ code_block)? }
            let mut it = inner.into_inner();
            let body = build_code_block(next(&mut it, "code_try_stmt", "body")?)?;
            let mut catch = None;
            let mut catch_name = None;
            let mut catch_block = None;
            for p in it {
                match p.as_rule() {
                    Rule::ident => catch_name = Some(ident_of(p)),
                    Rule::code_block => catch_block = Some(build_code_block(p)?),
                    _ => {}
                }
            }
            if let Some(block) = catch_block {
                catch = Some((catch_name, block));
            }
            Ok(CodeStmt::Try { body, catch })
        }
        Rule::code_parallel_stmt => {
            let branches = build_code_parallel_branches(inner)?;
            Ok(CodeStmt::Parallel { branches })
        }
        Rule::code_return_stmt => {
            let value = match inner.into_inner().next() {
                Some(e) => Some(build_code_expr(e)?),
                None => None,
            };
            Ok(CodeStmt::Return { value })
        }
        Rule::code_emit_stmt => {
            // code_emit_stmt = { "emit" ~ "(" ~ code_expr ~ ("," ~ code_expr)? ~ ")" ~ ";" }
            let mut it = inner.into_inner();
            let queue = build_code_expr(next(&mut it, "code_emit_stmt", "queue")?)?;
            let value = match it.next() {
                Some(e) => Some(build_code_expr(e)?),
                None => None,
            };
            Ok(CodeStmt::Emit { queue, value })
        }
        Rule::code_expr_stmt => {
            let e = next(&mut inner.into_inner(), "code_expr_stmt", "expr")?;
            Ok(CodeStmt::Expr(build_code_expr(e)?))
        }
        other => Err(CompileError::internal(format!(
            "code_stmt: unexpected {other:?}"
        ))),
    }
}

// code_if_stmt = { "if" ~ code_expr ~ code_block ~ ("else" ~ (code_if_stmt | code_block))? }
fn build_code_if(pair: Pair<'_>) -> R<CodeStmt> {
    let mut it = pair.into_inner();
    let condition = build_code_expr(next(&mut it, "code_if_stmt", "condition")?)?;
    let then_block = build_code_block(next(&mut it, "code_if_stmt", "then block")?)?;
    let else_clause = match it.next() {
        Some(p) if p.as_rule() == Rule::code_if_stmt => {
            Some(ElseClause::ElseIf(Box::new(build_code_if(p)?)))
        }
        Some(p) if p.as_rule() == Rule::code_block => Some(ElseClause::Else(build_code_block(p)?)),
        _ => None,
    };
    Ok(CodeStmt::If {
        condition,
        then_block,
        else_clause,
    })
}

// code_match_arm = { code_match_pattern ~ "=>" ~ (code_block | code_expr ~ ","?) }
fn build_code_match_arm(pair: Pair<'_>) -> R<CodeMatchArm> {
    let mut it = pair.into_inner();
    let pat_pair = next(&mut it, "code_match_arm", "pattern")?;
    let pattern = if pat_pair.as_str().trim() == "_" {
        CodePattern::Wildcard
    } else {
        // code_match_pattern = { "_" | code_expr }
        let inner = pat_pair.into_inner().next();
        match inner {
            Some(e) => CodePattern::Expr(build_code_expr(e)?),
            None => CodePattern::Wildcard,
        }
    };
    let body_pair = next(&mut it, "code_match_arm", "body")?;
    let body = match body_pair.as_rule() {
        Rule::code_block => CodeMatchBody::Block(build_code_block(body_pair)?),
        _ => CodeMatchBody::Expr(build_code_expr(body_pair)?),
    };
    Ok(CodeMatchArm { pattern, body })
}

// code_parallel_stmt / code_parallel_expr share `code_parallel_branch`.
fn build_code_parallel_branches(pair: Pair<'_>) -> R<Vec<(Ident, CodeBlock)>> {
    let mut branches = Vec::new();
    for b in pair.into_inner() {
        if b.as_rule() == Rule::code_parallel_branch {
            // code_parallel_branch = { ident ~ ":" ~ code_block }
            let mut it = b.into_inner();
            let name = ident_of(next(&mut it, "code_parallel_branch", "name")?);
            let block = build_code_block(next(&mut it, "code_parallel_branch", "block")?)?;
            branches.push((name, block));
        }
    }
    Ok(branches)
}

// ─── V2 code expressions ───
fn build_code_expr(pair: Pair<'_>) -> R<CodeExpr> {
    match pair.as_rule() {
        Rule::code_expr => {
            let inner = next(&mut pair.into_inner(), "code_expr", "inner")?;
            build_code_expr(inner)
        }
        Rule::code_inline_if => {
            // code_inline_if = { "if" ~ code_logic ~ "{" ~ code_expr ~ "}" ~ "else" ~ "{" ~ code_expr ~ "}" }
            let mut it = pair.into_inner();
            let condition = build_code_expr(next(&mut it, "code_inline_if", "cond")?)?;
            let then_val = build_code_expr(next(&mut it, "code_inline_if", "then")?)?;
            let else_val = build_code_expr(next(&mut it, "code_inline_if", "else")?)?;
            Ok(CodeExpr::InlineIf {
                condition: Box::new(condition),
                then_val: Box::new(then_val),
                else_val: Box::new(else_val),
            })
        }
        Rule::code_logic => code_fold(pair, code_logic_binop),
        Rule::code_comparison => {
            let mut it = pair.into_inner();
            let first = next(&mut it, "code_comparison", "additive")?;
            let mut left = build_code_expr(first)?;
            if let Some(op_pair) = it.next() {
                let op = code_comp_binop(op_pair.as_str())?;
                let rhs = next(&mut it, "code_comparison", "rhs")?;
                left = CodeExpr::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(build_code_expr(rhs)?),
                };
            }
            Ok(left)
        }
        Rule::code_additive => code_fold(pair, add_binop),
        Rule::code_multiplicative => code_fold(pair, mul_binop),
        // `code_power = { code_unary ~ ("^" ~ code_unary)* }` — `^` is literal
        // (no pair); fold operands with Pow (right-associative).
        Rule::code_power => {
            let operands = pair.into_inner().collect::<Vec<_>>();
            if operands.is_empty() {
                return Err(CompileError::internal("code_power: no operands"));
            }
            let mut iter = operands.into_iter().rev();
            let mut acc = build_code_expr(iter.next().unwrap())?;
            for base in iter {
                acc = CodeExpr::Binary {
                    left: Box::new(build_code_expr(base)?),
                    op: BinOp::Pow,
                    right: Box::new(acc),
                };
            }
            Ok(acc)
        }
        Rule::code_unary => {
            let inner = next(&mut pair.into_inner(), "code_unary", "inner")?;
            build_code_expr(inner)
        }
        Rule::code_neg => {
            let inner = next(&mut pair.into_inner(), "code_neg", "atom")?;
            Ok(CodeExpr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(build_code_expr(inner)?),
            })
        }
        Rule::code_not => {
            let inner = next(&mut pair.into_inner(), "code_not", "atom")?;
            Ok(CodeExpr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(build_code_expr(inner)?),
            })
        }
        Rule::code_atom => {
            let inner = next(&mut pair.into_inner(), "code_atom", "inner")?;
            build_code_expr(inner)
        }
        Rule::code_parallel_expr => Ok(CodeExpr::Parallel(build_code_parallel_branches(pair)?)),
        Rule::code_object_literal => {
            let mut fields = Vec::new();
            for f in pair.into_inner() {
                if f.as_rule() == Rule::code_obj_field {
                    // code_obj_field = { ident ~ ":" ~ code_expr }
                    let mut it = f.into_inner();
                    let key = ident_of(next(&mut it, "code_obj_field", "key")?);
                    let val = build_code_expr(next(&mut it, "code_obj_field", "value")?)?;
                    fields.push((key, val));
                }
            }
            Ok(CodeExpr::Object(fields))
        }
        Rule::code_closure => {
            // code_closure = { "|" ~ ident ~ ("," ~ ident)* ~ "|" ~ code_expr }
            let mut params = Vec::new();
            let mut body = None;
            for p in pair.into_inner() {
                match p.as_rule() {
                    Rule::ident => params.push(ident_of(p)),
                    Rule::code_expr => body = Some(build_code_expr(p)?),
                    _ => {}
                }
            }
            let body = body.ok_or_else(|| CompileError::internal("code_closure: missing body"))?;
            Ok(CodeExpr::Closure {
                params,
                body: Box::new(body),
            })
        }
        Rule::code_call_expr => {
            // code_call_expr = { ident ~ "(" ~ (code_expr ~ ...)* ~ ")" ~ code_access_chain? }
            let mut it = pair.into_inner();
            let name = ident_of(next(&mut it, "code_call_expr", "name")?);
            let mut args = Vec::new();
            let mut access_chain = Vec::new();
            for p in it {
                match p.as_rule() {
                    Rule::code_expr => args.push(build_code_expr(p)?),
                    Rule::code_access_chain => access_chain = build_code_access_chain(p)?,
                    _ => {}
                }
            }
            Ok(CodeExpr::Call {
                name,
                args,
                access_chain,
            })
        }
        Rule::code_access_expr => {
            // code_access_expr = { code_dotted_ident ~ code_access_chain? }
            let mut it = pair.into_inner();
            let base_pair = next(&mut it, "code_access_expr", "dotted_ident")?;
            let base = build_code_dotted(base_pair);
            let chain = match it.next() {
                Some(c) => build_code_access_chain(c)?,
                None => Vec::new(),
            };
            Ok(CodeExpr::Access { base, chain })
        }
        Rule::code_string => Ok(CodeExpr::Literal(CodeLiteral::String(unquote(
            pair.as_str(),
        )))),
        Rule::code_float => Ok(CodeExpr::Literal(CodeLiteral::Float(parse_f64(&pair)?))),
        Rule::code_integer => Ok(CodeExpr::Literal(CodeLiteral::Integer(parse_i64(&pair)?))),
        Rule::code_boolean => Ok(CodeExpr::Literal(CodeLiteral::Boolean(
            pair.as_str() == "true",
        ))),
        Rule::code_null => Ok(CodeExpr::Literal(CodeLiteral::Null)),
        other => Err(CompileError::internal(format!(
            "code_expr: unexpected {other:?}"
        ))),
    }
}

fn code_fold(pair: Pair<'_>, op_of: fn(&str) -> R<BinOp>) -> R<CodeExpr> {
    let mut it = pair.into_inner();
    let first = it
        .next()
        .ok_or_else(|| CompileError::internal("code binary chain: missing first operand"))?;
    let mut left = build_code_expr(first)?;
    while let Some(op_pair) = it.next() {
        let op = op_of(op_pair.as_str())?;
        let rhs = next(&mut it, "code binary chain", "operand")?;
        left = CodeExpr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(build_code_expr(rhs)?),
        };
    }
    Ok(left)
}

fn build_code_access_chain(pair: Pair<'_>) -> R<Vec<CodeAccess>> {
    let mut chain = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::code_dot_access => {
                let id = next(&mut p.into_inner(), "code_dot_access", "ident")?;
                chain.push(CodeAccess::Dot(ident_of(id)));
            }
            Rule::code_bracket_access => {
                let e = next(&mut p.into_inner(), "code_bracket_access", "expr")?;
                chain.push(CodeAccess::Bracket(Box::new(build_code_expr(e)?)));
            }
            _ => {}
        }
    }
    Ok(chain)
}

fn build_code_dotted(pair: Pair<'_>) -> DottedIdent {
    let span = span_of(&pair);
    let segments = pair.as_str().split('.').map(Ident::new).collect();
    DottedIdent {
        segments,
        span: Some(span),
    }
}

fn code_logic_binop(s: &str) -> R<BinOp> {
    match s {
        "&&" => Ok(BinOp::And),
        "||" => Ok(BinOp::Or),
        other => Err(CompileError::internal(format!(
            "unknown code logic op `{other}`"
        ))),
    }
}

fn code_comp_binop(s: &str) -> R<BinOp> {
    comp_binop(s)
}
