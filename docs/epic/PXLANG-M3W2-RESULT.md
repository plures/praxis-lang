# PXLANG-M3W2-RESULT — px-compiler + px-eval fold

**Branch:** `m3-wave2-compiler-eval` (pushed to `origin/plures/praxis-lang`)
**HEAD:** `877b1a66d29868bb22acefb847041b64c772053c`
**Base:** `f787b4b` (origin/main, M3 Wave 1)
**Toolchain:** rustc/cargo **1.96.1** (31fca3adb 2026-06-26)
**Date:** 2026-07-01

## Status: ✅ DONE — all gates green

Turned the empty `px-compiler` and `px-eval` stubs into real crates over the
canonical `px-ast` + `px-grammar` already in the repo. Real `.px` source now
parses end-to-end into the canonical AST and evaluates.

---

## Gate results (run, not claimed)

| Gate | Result |
|---|---|
| `cargo build --workspace --all-targets` | ✅ Finished, 0 errors |
| `cargo test --workspace` | ✅ **0 failures** (see tally below) |
| `cargo fmt --all -- --check` | ✅ clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ **zero warnings** |
| grammar parity test (`grammar_matches_generator_output`) | ✅ passing (in px-grammar's 5) |
| verify-grammar drift (regenerate + diff) | ✅ **byte-identical** (C-DRIFT-001 clean) |
| examples parse-test (≥3 programs) | ✅ 3 programs parse green |

### Test tally (per crate)
- **px-compiler**: 11 unit + 3 integration (`tests/parse_examples.rs`) + 1 doctest
- **px-eval**: 16 unit + 1 doctest
- **px-grammar**: 5 unit + 1 doctest (incl. parity + pest-binding tests)
- **px-grammar-gen**: 2 unit
- px-schema 10, px-napi 1, px-schema-derive 1, px-yaml 1, px-ast 0 (+ doctests)
- **Total: 0 failed, 0 errors across the workspace.**

### verify-grammar
`cargo run -q -p px-grammar-gen` output is **byte-identical** to the committed
`crates/px-grammar/src/grammar.pest`. The generated grammar was **never
hand-edited**; the pest derive reads it read-only, so the drift gate is intact.

---

## What was built

### 1. px-grammar — real pest `Parser` binding
- Added `pest = "2.8"` + `pest_derive = "2.8"` (workspace + crate `Cargo.toml`).
- Added `#[derive(pest_derive::Parser)] #[grammar = "grammar.pest"] pub struct PxParser;`
  exposing `PxParser` + the generated `Rule` enum. The `#[grammar = ...]` path
  resolves to the **same** `grammar.pest` that `GRAMMAR_PEST` already
  `include_str!`s — no second copy, drift gate untouched.
- Kept `GRAMMAR_PEST` const + `REQUIRED_CONSTRUCT_RULES` + all parity tests.
- Confirmed the **entire** generated `grammar.pest` compiles as valid pest.
- Public API: `px_grammar::{PxParser, Rule, GRAMMAR_PEST, REQUIRED_CONSTRUCT_RULES}`.

### 2. px-compiler — `parse(&str) -> PxDocument`
- Deps: `px-ast`, `px-grammar`, `pest` (**no pluresdb**).
- `src/error.rs`: `CompileError { Parse(String), Unsupported { rule, detail }, Internal(String) }`
  — three honest variants; `Parse` = pest diagnostic, `Unsupported` names the
  offending grammar rule for any real gap, `Internal` = invariant/builder-shape
  mismatch. All three are actually constructed on real paths.
- `src/lib.rs`: `parse(src) -> Result<PxDocument, CompileError>` (walks
  `Rule::document`) + `parse_statement(src)` convenience + doctest.
- `src/builder.rs`: the full pest→AST tree-walk. **Written from the grammar +
  px-ast directly** (see "porting note" below), covering **all 12 statement
  constructs**: import, entity, config, fact, rule, constraint, contract,
  function, trigger, dataflow procedure (v3, queue-bound), legacy procedure
  (v1), scenario. Plus:
  - Type builders (base/named/list/optional/map/enum).
  - Value builders (string/int/float/bool/list/map/call/arith/var_ref w/
    `.field`/`["key"]` accessor chains/dotted-ident/paren/null).
  - **v1 expression** builders — full precedence chain
    (expr→comparison→additive→multiplicative→power→unary→atom), inline-if,
    match (wildcard + value patterns), calls, with left-assoc folding.
  - **v1 procedure step-list** control flow: define, call-with-output, if/else,
    for, loop, match, try/catch (retry/delay/backoff), parallel/branch, emit,
    return.
  - **v2 code blocks** (`CodeExpr`/`CodeBlock`) — parsed into the AST
    (statements + code expressions with their own precedence chain).
- Public API: `px_compiler::{parse, parse_statement, CompileError}`.

### 3. px-eval — expression / rule / constraint evaluator
- Deps: `px-ast`, `px-compiler`, `serde_json` (**no pluresdb**).
- Evaluates the **typed px-ast** produced by px-compiler (not a re-walk of the
  pest tree), using `serde_json::Value` as the runtime value type — same choice
  the canonical evaluator makes.
- `src/error.rs`: `EvalError { ParseError, DivisionByZero, UnknownFunction, FunctionError, TypeError }`
  — every variant constructed on a real failure path (see honesty note).
- `src/registry.rs`: **`FunctionRegistry` trait** (`call` / `contains`) — the
  single seam for the storage/effect/host boundary (this is how we avoid a
  pluresdb dep). Ships:
  - `EmptyRegistry` (rejects all calls),
  - `PureFunctionRegistry` with ~21 **real** pure builtins: `len upper lower
    trim abs min max floor ceil round sqrt contains starts_with ends_with
    concat coalesce not bool int float str`.
- `src/value.rs`: runtime semantics ported verbatim from praxis-native —
  `is_truthy`, `to_f64`, `f64_to_value`, `value_to_string`, `is_string_concat`,
  `values_equal` (cross-type number equality), `compare_ordered`,
  `resolve_accessor`.
- `src/eval.rs`: `eval_expr` / `eval_value` over `px_ast::Expr` / `Value` +
  `Env`. Short-circuit `&&`/`||`, `+` string-concat, numeric arithmetic,
  comparison, unary not/neg, inline-if, match, function dispatch through the
  registry, null-propagating var/path access.
- `src/lib.rs` public API:
  - `evaluate(expr: &str, vars) -> Result<Value, EvalError>` (parse via
    px-compiler + eval, default pure registry),
  - `evaluate_with_registry(...)`, `evaluate_expr(&Expr, ...)`,
  - `parse_expr(&str) -> Result<Expr, EvalError>`,
  - `eval_rule(&RuleDecl, vars, registry) -> RuleFiring { fired, actions }` —
    evaluates `when:` conditions + `let:` bindings (scoped) + action guards;
    reports which actions fire (does **not** execute effects — host concern),
  - `eval_constraint(&ConstraintDecl, vars, registry) -> ConstraintOutcome`
    (`NotApplicable` / `Satisfied` / `Violated { severity, message }`),
  - re-exports `FunctionRegistry`, `EmptyRegistry`, `PureFunctionRegistry`,
    `EvalError`, `Env`, `eval_ast_expr`, `eval_ast_value`.

### 4. examples/*.px + parse-test
Three real programs at repo-root `examples/`, all parsed green by
`crates/px-compiler/tests/parse_examples.rs`:
- **`memory_assistant.px`** — imports, entity, fact, config, function, a
  reactive `rule` (when/let/then/capture), two `constraint`s. The test also
  asserts the exact lowered construct counts.
- **`routing_pipeline.px`** — entity, two **dataflow procedures** (v3,
  queue-bound, one step-list + one v2 code block), a `trigger`, a `contract`
  with examples, a `scenario`.
- **`legacy_procedure.px`** — a v1 `procedure` exercising the full step-list
  control-flow surface: trigger clause, params, define, call-with-output,
  if/else, for, loop, match, try/catch (retry/delay/backoff), parallel/branch,
  emit, return.

The test enforces `>= 3` programs, parses each to a non-empty statement list,
and spot-checks `memory_assistant.px`'s construct breakdown.

---

## What evaluates vs. what is absent (C-NOSTUB-001)

**Really evaluated (px-eval):** all **v1** expression forms (literals, vars,
dotted + bracket access with null-propagation, arithmetic incl. `^`/`%`,
comparison, short-circuit logic, string-concat on `+`, unary not/neg, inline-if,
match, function calls through the registry), plus **rule firing** (when +
scoped let + action guards) and **constraint checking** (when-guard →
require → severity/message).

**Absent by design (not stubbed):**
- **v2 code-block *execution*** (`px_ast::CodeExpr` / `CodeBlock`). px-compiler
  *parses* v2 into the AST, but **px-eval exposes no entry point that accepts
  v2** — there is no function you can call that would need to fake it. This is
  the honest-absence form of C-NOSTUB-001 (#1: not declared), not a placeholder.
  A v2/procedure runtime is downstream (a later milestone), and would attach at
  the same `FunctionRegistry` seam.
- **Effect execution** (running a rule's actions, a procedure's side-effecting
  steps, storage reads/writes). `eval_rule` reports *which* actions fire; it does
  not run them. That boundary is the `FunctionRegistry` trait — deliberately a
  seam so the host (not the language core) owns effects. No pluresdb dep.

**Honesty note on `EvalError`:** an earlier draft carried an
`EvalError::Unsupported(String)` variant, but nothing in px-eval ever
*constructed* it (v1 `Expr`/`Value` are fully covered; v2 is absent, not
error-returning). A dead variant whose docs imply a return path is itself a
soft stub, so it was **removed** (commit `877b1a6`). Every remaining `EvalError`
variant is produced on a real failure path.

---

## Porting notes (ported vs. adapted vs. written fresh)

- **px-compiler builder**: the brief pointed at
  `pluresdb-px/src/px/compiler.rs` (and `builder.rs`). Those build into
  pluresdb-px's own **flatter** AST (`parse_value` → `serde_json::Value`), **not**
  the canonical `px-ast`. So they were used as a **reference for tree-walk
  mechanics only**; the actual builder was written **directly against the
  generated grammar + the canonical px-ast types** (cleaner and avoids importing
  a mismatched shape). No pluresdb code copied.
- **px-eval**: **ported from `praxis-native/src/px/eval.rs`** (the cleaner
  evaluator, as instructed) — specifically the numeric/comparison/truthiness/
  string-concat/accessor semantics (now in `value.rs`) and the expr-dispatch
  shape (now `eval.rs`, but over the AST instead of a pest tree). praxis-native's
  `NativeFunctionRegistry` is entangled with host storage, so it was **inverted
  into the `FunctionRegistry` trait seam** + a pure-only default impl. The 261KB
  `executor.rs` was **not** copied (only the language-eval concepts were taken).

---

## Bugs found & fixed by end-to-end testing

Building + running the binaries (not just `cargo test` on unit shapes) surfaced
**three real px-compiler bugs**, each fixed with a regression test:
1. **`power`/`code_power` folding** — `power = { unary ~ ("^" ~ unary)* }` uses a
   *literal* `^` (no pest pair), so the inner pairs are operands-only; the
   generic left-fold mis-read the second operand as the operator → "expected
   operand after op". Fixed with operand-only right-assoc Pow folding (v1 + v2).
2. **`conditional_action` keyword skip** — `kw_if` is atomic (`@{}`, emitted as a
   pair), so `if <expr>: action` fed `kw_if` where an `expr` was expected. Fixed
   by filtering `Rule::kw_if`.
3. **Compound trigger keyword extraction** — `on_write("inbound")` has no space
   before `(`, so `split_whitespace().next()` returned the whole string. Fixed to
   take the leading `[A-Za-z0-9_]` run. Regression test covers `on_write(...)`,
   bare `manual`, and `cron { ... }`.

---

## Commits (branch `m3-wave2-compiler-eval`)
```
877b1a6 M3w2: remove never-constructed EvalError::Unsupported; state v2 code-block eval as honest absence (C-NOSTUB-001)
1e7e958 M3w2: cargo fmt reflow (px-grammar test line)
7bea600 M3w2: add 3 real examples/*.px + parse-all integration test; fix compound-trigger keyword extraction
31f8be0 M3w2: implement px-eval (AST evaluator + rule/constraint eval + FunctionRegistry seam, no pluresdb) + fix compiler power/kw_if bugs
f1b9c67 M3w2: implement px-compiler parse(&str)->PxDocument for all 12 constructs + steps/code blocks
3fb38e4 M3w2: add pest Parser binding (PxParser/Rule) over grammar.pest in px-grammar
f787b4b (base) M3 wave 1
```

---

## Honest gaps / follow-ups (for the merge adjudicator)
- **v2 code-block runtime** is absent (parsed, not executed) — the next
  natural milestone; attaches at the `FunctionRegistry` seam.
- **`parse_expr`** parses a bare expression by wrapping it as a synthetic
  `constraint __expr__: require: <expr>` document (smallest well-formed carrier
  through the canonical front end, so there's no second/duplicate expression
  parser). It's correct and tested, but if a first-class `Rule::expr` entry is
  wanted later, it can call `PxParser::parse(Rule::expr, ..)` directly.
- **No PR opened, not pushed to main** — branch only, per instructions. Merge
  adjudication is the main session's call.
