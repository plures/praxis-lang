# M5 — YAML surface + NAPI bindings (praxis-lang epic)

**Worker milestone file.** Tick checkboxes as you complete each. Main session reads this.

**Branch:** `m5-yaml-napi` (worktree `C:\Projects\praxis-lang-m5`). Base = `6ece8f4` (origin/main, M4 done).\n**Repo:** local clone `C:\Projects\praxis-lang`; origin `plures/praxis-lang`. Toolchain 1.96.1.\n**Autonomy:** full. Commit frequently `[praxis-lang epic]` milestone-coded. Do NOT merge to main / do NOT open a PR — commit + push the BRANCH; main session adjudicates.

## Objective (ADR pillars P4 + P5)
Two surfaces over the SAME canonical `px-ast` — no second source of truth.
1. **`px-yaml`** — YAML <-> px-ast round-trip.
2. **`px-napi`** — NAPI-RS bindings so Node can compile/evaluate `.px`.

## Design constraints (from M2 finding + ADR)
- YAML is a SURFACE, not a second truth. A `.px` file and its YAML equivalent must deserialize to the SAME px-ast (assert it). Reuse px-ast's serde (M4 stabilized kind-tagged enums, span dropped) — `serde_yaml` (or `serde_yml`) over the existing derives is the path of least resistance. For TypeExpr/Expr that have string concrete syntax in `.px`, YAML may carry them as structured serde OR as `.px`-string scalars sub-parsed via px-compiler — pick the cleaner one and document it. V2 procedure code-blocks stay as embedded `.px` block-scalars (NOT structural YAML) — see M2.
- NAPI: expose at minimum `parse(src: &str) -> AST-as-JSON` (via px-compiler) and an `evaluate`/`checkConstraints` entry (via px-eval). Return JSON-serializable results. Use napi-rs (napi + napi-derive, `#[napi]`).

## Gates (ALL must pass before reporting DONE — RUN them, do not claim)
- [ ] **px-yaml crate:** `px_yaml::to_yaml(&Program) -> String` + `px_yaml::from_yaml(&str) -> Result<Program>`; round-trip test: `from_yaml(to_yaml(p)) == p` for a representative Program.
- [ ] **YAML<->.px parity test:** take one of the `examples/*.px`, parse via px-compiler -> Program A; hand-write (or generate) its `.yaml`, from_yaml -> Program B; assert A == B (the "same AST, two surfaces" proof).
- [ ] **px-napi crate:** napi-rs bindings; `parse` + an eval/constraint-check fn; builds a native addon (`.node`). cdylib crate-type. Provide package.json + index.d.ts (napi build can generate the .d.ts).\n- [ ] **Node smoke test (build-the-binary-run-the-binary):** `napi build` (or `cargo build -p px-napi`), then a tiny Node script that loads the addon, parses a real `.px` string, and asserts it got a sane AST back. Must actually run under node. If a full `npm publish` is heavy, wire the build + smoke test and document publish as an HONEST follow-up (do NOT fake a publish).
- [ ] **No stubs (C-NOSTUB-001):** real impls or honest absence. No canned AST, no fake eval. If a construct doesn't round-trip through YAML yet, that's an honest documented gap, not a silent stub.
- [ ] **Local green:** `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` all zero-warning. **verify-grammar + verify-schema drift gates STILL green** (don't hand-edit generated artifacts; if px-ast changes shape, regenerate schema via `cargo run -p px-schema -- schema` and grammar via `cargo run -p px-grammar-gen -- crates/px-grammar/src/grammar.pest`, and re-commit).
- [ ] Push branch `m5-yaml-napi` to origin. (Main session opens the PR for CI + merges.)

## CI note
If you add CI lanes for napi build / node smoke, keep them additive and actionlint-clean; do NOT rename existing job display names (branch protection). A napi cross-build matrix is nice-to-have but a single-platform build+smoke that proves it works is sufficient for M5; note broader matrix as follow-up.

## Report
Write `C:\Users\kbristol\.openclaw\workspace\epic-praxis-lang\PXLANG-M5-RESULT.md`: crates added, public APIs, the two parity proofs (paste asserts/output), how the node smoke test runs + its output, gate results (pasted cargo summary), branch HEAD sha, honest gaps (esp. anything not round-tripping or publish-deferred).

## Anti-patterns
- ❌ A second AST/truth for YAML (must be the same px-ast).
- ❌ Faking the node addon or returning canned JSON from napi.
- ❌ Hand-editing schema/grammar generated files (regenerate instead).
- ❌ Pushing to main / opening+merging a PR yourself.
