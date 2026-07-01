# M4 — Schema auto-update on release + CI drift gate (praxis-lang epic)

**Worker milestone file.** Tick boxes as you finish. Main session reads this to track you.

**Branch:** `m4-schema-autoupdate` (worktree `C:\Projects\praxis-lang-m4`). Base = current `origin/main` (post-M3, `a72578c` or later).\n**Repo:** local clone `C:\Projects\praxis-lang`; origin `plures/praxis-lang`.\n**Autonomy:** full. Commit frequently, milestone-coded `[praxis-lang epic]`. Push the BRANCH only; do NOT merge/PR to main — main session adjudicates.

## Design (from ADR + M2 finding — READ `docs/epic/ADR-praxis-lang-single-source-of-truth.md` §M4 first)
The schema is a **pure projection of `px-ast`**, NOT a hand-maintained artifact and NOT a PSF re-root. Two generated, committed artifacts:
- `schema/px.schema.json` — JSON-Schema generated from px-ast via `schemars`/`JsonSchema` derives.
- `schema/px.schema.px` — the `.px`-syntax schema/description generated FROM px-ast.
Both regenerate deterministically; a CI drift gate fails the build if the committed files != freshly regenerated (this makes C-DRIFT-001 structural — the schema can never silently diverge from the AST).

## Gates (ALL pass before DONE — run them, don't claim)
- [ ] **px-ast gets `schemars::JsonSchema` derives** on the public AST types (Program/Statement + all construct structs/enums + TypeExpr/Value/Expr). Stabilize serde tagging on key enums so the schema is stable/legible. Drop or namespace `span`/positional noise from the projection (per M2 prereq). Keep existing serde behavior working (don't break px-yaml plans / existing tests).
- [ ] **px-schema emits both artifacts** via a lib fn + a bin: `cargo run -p px-schema -- <out-dir>` writes `schema/px.schema.json` + `schema/px.schema.px` as raw UTF-8/LF (same codepage-safe pattern px-grammar-gen uses — write bytes directly to a path, do NOT round-trip through PowerShell stdout).
- [ ] **Commit the generated `schema/px.schema.json` + `schema/px.schema.px`** into the repo.
- [ ] **CI `verify-schema` upgraded** from the M1 placeholder to a REAL regenerate-and-diff gate (mirror how `verify-grammar` works): regenerate into a temp dir, `git diff --exit-code` (or byte-compare) against committed; job display name kept identical so branch protection stays satisfied.
- [ ] **Release automation**: add/extend a release workflow (or a `release.yml` job) that regenerates schema + grammar and commits/publishes them as part of every release — NO manual step (C-DRIFT-001). If a full release pipeline is out of scope this wave, at minimum wire the regenerate step into the release path and document it; do not fake a release.
- [ ] **Proof test**: a test (or documented CI behavior) demonstrating that deliberately changing an AST construct makes the committed schema stale → drift gate goes red. Add `scripts/regen-schema.ps1` (like regen-grammar.ps1).
- [ ] **Local green**: `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` (toolchain 1.96.1, zero warnings). verify-grammar drift STILL green (don't touch grammar by hand).
- [ ] Push branch `m4-schema-autoupdate`. Write `C:\Users\kbristol\.openclaw\workspace\epic-praxis-lang\PXLANG-M4-RESULT.md` (what you added, the two artifact paths + sizes, the CI gate diff, gate output pasted, branch HEAD sha, honest gaps).

## Anti-patterns
- ❌ Hand-writing the schema or letting it drift from px-ast (the whole point is it's GENERATED).
- ❌ Re-introducing PSF as the schema (M2: PSF is a downstream canvas/app concern, not the `.px` schema).
- ❌ Faking a "release" that doesn't run. If release infra is heavy, wire the regen step + a working CI drift gate and note the release-publish as a documented follow-up — honestly.
- ❌ Generating schema files through PowerShell stdout (codepage corrupts multibyte) — write bytes to a path from Rust.
- ❌ Merging to main / opening a PR yourself.
