# PXLANG-M5-RESULT — YAML surface + NAPI bindings

**Status:** COMPLETE (local gates green except clippy, which is deferred to CI — see Gaps).
**Branch:** `m5-yaml-napi` — HEAD `3005703`.
**Base:** `6ece8f4` (origin/main, M4 done).
**Author note:** The worker subagent implemented both crates and ran the node smoke test, but died before committing px-napi, writing this report, or pushing. The MAIN session independently verified all runnable gates on disk (not worker-trust), committed the px-napi work (`3005703`), wrote this report, and pushed. px-yaml was already committed by the worker (`872f36d`).

## Crates delivered

### px-yaml (commit 872f36d)
YAML ⇔ px-ast round-trip, reusing px-ast's existing serde derives (no second source of truth).
- `px_yaml::to_yaml(&PxDocument) -> Result<String>`
- `px_yaml::from_yaml(&str) -> Result<PxDocument>`
- YAML is a **surface** over the canonical px-ast — it deserializes to the *same* AST a `.px` file parses to.

### px-napi (commit 3005703)
NAPI-RS v3 bindings (cdylib) exposing the canonical Rust engine to Node/TS. Every entry point delegates to the real `px-compiler` / `px-eval` — no re-implementation, no canned data.
- `parse(src: String) -> String` — px-compiler::parse → PxDocument as JSON
- `evaluate(expr: String, vars_json: String) -> String` — px-eval::evaluate a v1 expr against a JSON var map
- `check_constraints(src: String, vars_json: String) -> String` — parse, then eval **every** constraint via px-eval::eval_constraint; returns JSON array of {name, status: satisfied|violated|not_applicable, severity?, message?}
- `px_ast_version() -> String` — px-ast crate version the addon was built against (engine/schema alignment assertion for JS callers)
- Package: `@plures/px-napi` (package.json + generated index.js loader + index.d.ts), build.rs = `napi_build::setup()`, targets win/linux/darwin.
- `#![forbid(unsafe_code)]` intentionally omitted (napi-derive generates the required `unsafe extern "C"` shims; all hand-written logic is safe Rust — documented in the crate header).

## The two parity proofs

**1. YAML round-trip** — `crates/px-yaml/tests/roundtrip.rs`: `from_yaml(to_yaml(p)) == p` for representative Programs.
```
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored
```

**2. `.px` ⇔ `.yaml` "same AST, two surfaces"** — `crates/px-yaml/tests/parity.rs`: parse an `examples/*.px` via px-compiler → Program A; deserialize its `.yaml` via from_yaml → Program B; assert A == B.
```
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored
```

## Node smoke test (build-the-binary-run-the-binary)

Built the real addon (`napi build`): `crates/px-napi/px-napi.win32-x64-msvc.node` (4.42 MB). Then `node test/smoke.mjs` loads the REAL `.node` addon (via the generated index.js loader) and exercises every entry point against real `.px` input — nothing mocked.
```
px-napi smoke test — loading real .node addon and exercising it

  ok   addon exports parse/evaluate/checkConstraints/pxAstVersion
  ok   pxAstVersion returns a version string
  ok   parse() returns a sane canonical AST for a real .px string
  ok   parse() throws on malformed .px
  ok   evaluate() computes a real expression result
  ok   checkConstraints() evaluates every constraint against the vars
  ok   checkConstraints() marks not_applicable when a when-guard is false

px-napi smoke: all checks passed ✔
```

## Gate results (verified on disk by main session, toolchain 1.96.1)

| Gate | Result |
|---|---|
| `cargo build --workspace` | ✅ Finished clean, 0 warnings |
| `cargo test --workspace` | ✅ all `ok`, 0 failures (yaml round-trip 3, parity 1, + rest) |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --workspace -- -D warnings` | ⛔ **blocked locally** — Windows Defender quarantined `clippy-driver.exe` as `Trojan:Win32/Wacatac.B!ml` (`!ml` = ML false-positive; same toolchain's rustc built+tested fine). Deferred to CI (Linux — heuristic absent there). |
| verify-grammar drift gate | ✅ regen == committed grammar.pest (21722 bytes), zero diff |
| verify-schema drift gate | ✅ regen == committed schema/px.schema.{json,px}, zero diff |
| Node smoke | ✅ 7/7 checks passed |

## Honest gaps (C-NOSTUB-001)
- **clippy not proven locally** — blocked by a Windows-Defender-only false-positive on clippy-driver.exe, not a code issue. Authoritative clippy verdict comes from CI on Linux. (Local exec cannot add a Defender exclusion from this session; owner can clear with `Add-MpPreference -ExclusionPath "$env:USERPROFILE\.rustup\toolchains"; Remove-MpThreat` if a local clippy run is desired.)
- **npm publish deferred** (honest follow-up, not faked): the addon is built + smoke-tested + package.json/index.d.ts wired; an actual `npm publish` + a cross-platform napi build matrix are follow-ups (single-platform build+smoke proves it works per the M5 brief). No fake publish performed.
- **`.node` binary not committed** — correctly gitignored (`*.node`); addons are built in CI/release, not checked in.

## Anti-patterns avoided
- No second AST/truth for YAML — it deserializes to the same px-ast (parity test proves it).
- No faked node addon / no canned JSON — napi fns delegate to real px-compiler + px-eval; smoke test loads the real compiled `.node`.
- No hand-edited generated files — grammar+schema regenerated, drift gates clean.
- Did NOT open+merge a PR or push to main from the worker — branch pushed; main session opens the PR for CI + adjudicates merge.
