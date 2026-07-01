# PXLANG-M4 Result — Schema auto-update from px-ast + CI drift gate

**Branch:** `m4-schema-autoupdate` (pushed to `origin`, NOT merged, no PR — per hard rules)
**HEAD:** `bce1baf832c60e72aa20641c8fab65274d6fc4db`
**Base:** `a72578c` (post-M3 `origin/main`)
**Worktree:** `C:\Projects\praxis-lang-m4`\n**Date:** 2026-07-01

## Objective (met)
Make the `.px` schema a **pure, deterministic projection of `px-ast`** that regenerates automatically and is protected by a CI drift gate, so the schema can never silently diverge from the AST (C-DRIFT-001). No hand-written schema, no PSF re-root, no PowerShell-stdout generation, no stubs.

## What was added / changed

### 1. `px-ast` — `schemars::JsonSchema` derives + stable serde tagging + span dropped
- Added `#[derive(JsonSchema)]` to **every** public AST type across `common.rs`, `lib.rs` (`PxDocument`, `Statement`), `constructs.rs` (all 12 construct decls + `ConfigValue`/`TriggerEvent`), `types.rs` (`TypeExpr`, `BaseType`), `values.rs` (`Value`, `ArithOp`), `expressions.rs` (`Expr`/`CodeExpr`/`ExprMatchPattern`/`CodeAccess`/`CodeLiteral`), `procedures.rs` (all procedure/step/loop/retry/code types).
- **Serde tagging stabilized:** all data-carrying enums use **adjacent tagging** `#[serde(tag = "kind", content = "value")]` — the only representation that handles every variant shape (scalar newtypes, struct variants, tuple variants) without runtime errors. Unit-only enums (`BaseType`, `BinOp`, `UnaryOp`, `ArithOp`, `Severity`, `FunctionMode`, `BackoffStrategy`, `AssignOp`) stay plain string enums.
- **Span noise dropped from the projection:** every `span` field is `#[serde(skip)]` + `#[schemars(skip)]`, so positional editor data never appears in the language schema (PXLANG-M2 §3.1.3).
- Added `PX_AST_VERSION` const (`env!("CARGO_PKG_VERSION")`), stamped into the schema as `x-px-ast-version`.
- Removed the unused `px-schema` / `px-schema-derive` deps from `px-ast` (grep-confirmed the derive macros were never used; also avoids a circular dep now that px-schema depends on px-ast).
- Existing serde behavior/tests preserved (no test asserted span serialization; no code used `serde(tag=…)`/`to_value` on AST types; px-yaml is an empty skeleton).

### 2. `px-schema` — projection lib fn + codepage-safe generator bin
- New module `crates/px-schema/src/projection.rs`:
  - `build_json_schema()` / `json_schema_string()` — emits the JSON Schema (draft-07) for `PxDocument` via `schema_for!`, injects `x-px-ast-version`, deterministic (schemars orders `definitions` in a `BTreeMap`), trailing LF.
  - `px_schema_string()` — generates the `.px`-syntax schema **from the same JSON Schema** (i.e. from px-ast): walks every definition and emits `schema <Name>:` blocks with `f <field>: <type>` lines, tagged-enum `variant:` lists, and plain-enum `value:` lists. A true projection — add/remove a construct in px-ast and it appears/disappears here automatically.
- New bin `crates/px-schema/src/bin/px_schema_gen.rs` (`cargo run -p px-schema -- <out-dir>`): writes both artifacts as **raw UTF-8/LF bytes directly to files** via `std::fs::write` — never through stdout, so no console code page can corrupt the multibyte characters (em dash, box-drawing). Mirrors the `px-grammar-gen` pattern exactly.
- The M1 hand-authored `types.rs`/`generator.rs`/`validator.rs` (a runtime validation utility with passing tests) were **kept intact** — the projection is additive, not a rewrite, so nothing regressed.

### 3. Generated + committed artifacts
| Artifact | Size | Source |
|----------|------|--------|
| `schema/px.schema.json` | **90,220 bytes** | schemars projection of `PxDocument` (draft-07 JSON Schema) |
| `schema/px.schema.px` | **8,897 bytes** | `.px`-native projection generated from the JSON Schema |

Both are pure LF, no BOM, valid UTF-8 (verified: first bytes `# P` / `{\n`, em dash round-trips, `has_CR=False`). Both are byte-deterministic across runs.

### 4. CI `verify-schema` — real regenerate-and-diff drift gate
`.github/workflows/ci.yml`: the M1 placeholder (`cargo build -p px-schema`) was replaced with a real gate that regenerates both artifacts into a temp dir and `diff -u`s them against the committed files, failing with an actionable `::error::` if either drifts. **Job display name kept identical (`verify-schema (placeholder)`)** so branch protection stays satisfied (same convention verify-grammar uses). Validated with `actionlint` (exit 0).

### 5. Release path — honest, no fake publish
New `.github/workflows/release.yml` (triggers on `v*` tags + `workflow_dispatch`):
- **`regenerate-artifacts` (REAL, ENFORCED):** regenerates schema **and** grammar from px-ast and **blocks the release** if either committed artifact drifts. This is the structural mechanism behind "schema auto-updates with every release" — a release literally cannot ship a stale schema.
- **`verify`:** fmt + clippy + build + test must pass.
- **`publish` (HONEST PLACEHOLDER):** gated on the two above; confirms artifacts are present but performs **NO** `cargo publish` / `npm publish`. Crate versions are `0.0.0` and NAPI publish wiring is explicitly scheduled for **M5** (per `docs/epic/PRAXIS-LANG-TRACKER.md` + `px-napi/Cargo.toml`). Documented as a follow-up, not faked.

### 6. Proof of drift detection + regen script
- `scripts/regen-schema.ps1` — regenerates both artifacts into `schema/` by passing the out-dir to the bin (never captures stdout; matches `regen-grammar.ps1`).
- `crates/px-schema/tests/schema_drift.rs` — the **executable form of the drift gate**: reads the actual committed `schema/*` from disk and asserts byte-equality with fresh projection output. Includes a negative-control test (`drift_gate_detects_a_stale_schema`) proving the check is not a tautology.
- **Empirically verified (run, not claimed):** temporarily added a `drift_probe` field to `EntityDecl` → `committed_json_schema_matches_px_ast_projection` went **RED (exit 101)**, the diff showed the new field. Reverted → **GREEN**. This proves changing an AST construct makes the committed schema stale and the gate blocks it.

## Gate output (all run locally, all green)

```
cargo clippy --workspace --all-targets -- -D warnings   →  CLIPPY_EXIT=0   (zero warnings)
cargo fmt --all -- --check                               →  FMT_EXIT=0
cargo build --workspace --all-targets                    →  BUILD_EXIT=0
cargo test --workspace                                   →  TEST_EXIT=0
    px-schema lib: 15 passed; schema_drift: 3 passed; all crates: 0 failed
```

Drift-gate simulations against the committed artifacts:
```
verify-schema:  px.schema.json match=True (90220 bytes);  px.schema.px match=True (8897 bytes)
verify-grammar: grammar.pest match=True        (untouched by M4 — still green)
OVERALL = ALL_GREEN
```

Drift-detection proof:
```
+ added EntityDecl.drift_probe (no regen)  → committed_json_schema_matches_px_ast_projection FAILED (exit 101)
+ reverted                                 → 3 passed; 0 failed
```

## Commits (6, milestone-coded)
```
bce1baf M4: collapse nested if-let in projection to satisfy clippy -D warnings
0cb95ca M4: add executable schema-drift proof (empirically verified RED on AST change)
825b9e1 M4: add release.yml — regenerate schema+grammar from px-ast and block release on drift
87acc89 M4: upgrade CI verify-schema to real regenerate-and-diff drift gate; add scripts/regen-schema.ps1
0226e37 M4: px-schema emits px.schema.json + px.schema.px projections from px-ast via bin
69d7074 M4: add schemars::JsonSchema derives to px-ast (stable kind-tagged enums, span dropped)
```
Diffstat vs base: **20 files, +5233 / -80** (the bulk is the two generated artifacts).

## Honest gaps / follow-ups
- **Crate/NAPI publish is deferred to M5** and intentionally not implemented — versions are `0.0.0`, NAPI build wiring is an M5 item. The release workflow's `publish` job is a clearly-labelled non-executing placeholder (no fake publish), while the regenerate-and-block-on-drift mechanism is fully real and enforced now.
- **Grammar regen is enforced in the release path but grammar.pest itself was not touched** by M4 (correctly — no hand edits; verify-grammar stayed green).
- **A cargo incremental-compilation staleness quirk** was observed during the drift experiment (reverting a source file with an unchanged mtime left a stale rlib linked into the test binary, causing a spurious failure until `touch` forced a rebuild). This is a local dev-loop caveat only — CI checks out fresh, so it is not a correctness issue for the gate. Noted here for the next worker.
- **`px.schema.px` is a legible projection**, not a formally-parseable `.px` program (it uses `schema`/`f`/`variant`/`value` descriptive syntax). The authoritative machine schema is `px.schema.json`; the `.px` file is the `.px`-native human view of the same projection, as the ADR intends. If a future milestone wants `px.schema.px` to be a real parseable `.px` document, that's a clean extension point in `projection.rs::px_schema_string()`.

## Where things live
- Projection logic: `crates/px-schema/src/projection.rs`
- Generator bin: `crates/px-schema/src/bin/px_schema_gen.rs`
- Artifacts: `schema/px.schema.json`, `schema/px.schema.px`
- Regen script: `scripts/regen-schema.ps1`
- Drift gate (CI): `.github/workflows/ci.yml` → `verify-schema`
- Release gate: `.github/workflows/release.yml` → `regenerate-artifacts`
- Drift proof test: `crates/px-schema/tests/schema_drift.rs`
