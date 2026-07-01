# PRAXIS-LANG TRACKER — live state of the `.px` single-source-of-truth consolidation

**Purpose:** RESTART-PROOF handoff. If the gateway restarts or a fresh session picks this up, read this file + `ADR-praxis-lang-single-source-of-truth.md` and resume at the first unchecked stage. No re-derivation needed.

**Decision:** New repo `plures/praxis-lang` = single source of truth for everything `.px`. Rust-first, auto-updating schema, standard syntax, Rust+YAML surfaces, NAPI. Overrides the 2026-06-29 "praxis repo is the .px home" ruling (kbristol, 2026-06-30).

**Repo name:** `praxis-lang` (PROPOSED — pending kbristol final ✔; alts: `px-lang`, `px`).

---

## GROUND TRUTH (verified on disk 2026-06-30 — the 4-repo sprawl being collapsed)
- `praxis` repo Rust: `crates/{px-ast, px-grammar-gen, px-schema, px-schema-derive, praxis-native}`
- `praxis` repo TS: `core/{schema-engine(PSF), logic-engine, codegen, db-adapter}` (+ byte-identical copy in `packages/praxis-core/src/`)
- `praxis` repo sprawl: `csharp/`, `powershell/`, `deno.json`, `jsr.json`, `ui/` (carve out / leave as legacy)
- `pluresdb/crates/pluresdb-px` v3.0.1: DUPLICATE in-tree engine (`compiler.rs`/`executor.rs`/generated `grammar.pest` v4); does NOT import praxis crate (ADR-0017 drift)
- consumers: `pares-radix/crates/praxis`, `pares-agens/crates/praxis`
- Grammar v4 is STABLE since `195c67b` (2026-06-13). Construct set: import, entity, config, fact, rule, constraint, contract, function, trigger, procedure(dataflow+legacy), scenario + V1 steps + V2 code-block + type system.
- No `plures/px*` or `plures/*intent*` repo exists yet — name is free.

---

## STAGES (each gates the next; do not skip; verify on disk)

### [x] M0 — Decision locked + design doc (DONE 2026-06-30)
- [x] ADR written (`ADR-praxis-lang-single-source-of-truth.md`)
- [x] Tracker written (this file)
- [x] Confirmed no existing px/intent repo in plures org
- [x] **GATE RESOLVED (kbristol granted full autonomy 2026-06-30 17:05):** name = **`praxis-lang`**, visibility = **PUBLIC**. Decision made under delegated authority — no further approval needed for this epic.

### AUTONOMY + RESILIENCE (kbristol 2026-06-30 17:05: work autonomously, make future decisions yourself, make it uninterruptible + restart-proof)
- [x] Repo **created**: `plures/praxis-lang` PUBLIC — https://github.com/plures/praxis-lang
- [x] **Paused all unrelated crons** (RE-ENABLE THESE at M8): `prng-continuous-grind` 7490748f, `sprint-dashboard-refresh` e2fb05c8, `pluresLM-maintenance` 87f4aa17, `release-pipeline-health` 0889c131, `morning-briefing` de7cf03f, `praxisbot-deployment-drift` 9f25bf10, `afternoon-review` 0610a896, `ado-sprint-support-story` 05d2e495
- [x] **Continuation cron created**: `praxis-lang-epic-continuation` id `0b3ef1e8-0c2a-4c95-b6a7-8162a0e8f0c4` — hourly (`0 * * * *` America/Los_Angeles), wakes MAIN session, reads this tracker, advances the next unchecked stage. DISABLE this at M8.
- [x] Durable plan committed to git in `plures/praxis-lang` (local clone `C:\Projects\praxis-lang`) with milestone-coded messages so the plan survives even a workspace wipe.\n- **LOOSE END from prior work (not part of this epic):** pares-radix PR #461 (Track A Thread-3 cleanup) still needs a final CI-green squash-merge + worktree `C:/Projects/worktrees/thread3-testing-cleanup` reclaim. Handle opportunistically; do not let it block the epic.

### [x] M1 — Create the repo + skeleton workspace (DONE 2026-06-30)
- [x] `gh repo create plures/praxis-lang` PUBLIC, default branch `main` (MIT LICENSE committed in skeleton; GitHub license auto-detect will pick it up)
- [x] Cargo workspace skeleton with 9 empty-but-buildable crates per ADR §3.2 (px-ast, px-grammar-gen, px-grammar, px-schema, px-schema-derive, px-compiler, px-eval, px-yaml, px-napi). `cargo build`+`cargo test`+`cargo fmt --check`+`cargo clippy -D warnings` all green locally.
- [x] CI scaffold (`.github/workflows/ci.yml`): build/test/fmt/clippy + placeholder `verify-grammar`/`verify-schema` lanes (become real regenerate-and-diff drift gates in M3/M4). **Branch protection ON `main`** (strict, requires all 3 checks: build+test, verify-grammar, verify-schema).
- [x] Acceptance: empty workspace **builds GREEN in CI** — pushed as `9fba208`, CI run 28486372521 = success (build+test ✓, verify-grammar ✓, verify-schema ✓).

### [x] M2 — Best-concepts reconciliation (the "select the best" pass) — DONE 2026-06-30 (VERIFIED)
- [x] Mapped PSF ↔ px-ast ↔ grammar v4 construct-by-construct → `PXLANG-M2-RECONCILIATION.md` (227 lines, evidence-grounded)
- [x] **KEY FINDING (verified by direct grep):** PSF is NOT the `.px` language — it's a visual app/canvas builder schema (`psf.ts`: 0 hits for entity/contract/procedure/scenario/import/duration; 20 canvas/17 component/15 model/12 flow/13 event). `px-ast ↔ grammar v4` are ALREADY the single codegen-linked language representation (12/12 constructs + both expr layers + type system align).
- [x] **Canonical set RATIFIED: `px-ast` as-is** = single source of truth for the language. Grammar generated from it (ADR-0021). Keep dual V1/V2 expression layers. Dataflow-v3 procedures primary; legacy-v1 + trigger = migration sugar.
- [x] **Corrected design:** PSF does NOT enter `praxis-lang`. Schema artifact = pure JSON-Schema projection of px-ast (schemars). PSF stays downstream as an `x-app`/`x-canvas` extension with the canvas/app-framework.
- Punch-list produced: P0 grammar-alignment fixes (entity `map[K,V]`, `null` token, scenario.given, rule `let`), P1 make-schema-a-projection, P2 expressiveness polish, P3 YAML surface.

### [x] M3 — Migrate the Rust language core (Rust-first) — DONE (2026-07-01, both waves, CI green)
- [x] **WAVE 1 (foundational core, no external git deps) — DONE + CI GREEN:** Moved `px-ast`, `px-grammar-gen` (+ all 11 `.pest` fragments), `px-schema`, `px-schema-derive` from `C:\Projects\praxis` into `praxis-lang`. Cargo.tomls adapted to workspace conventions + inter-crate path deps. `px-grammar` now embeds the GENERATED `grammar.pest` (ADR-0021, 21722 bytes) and asserts in-process parity with `px-grammar-gen` (mirrors the CI drift gate). `px-grammar-gen` made lib+bin; binary writes UTF-8/LF directly to a path (`cargo run -p px-grammar-gen -- <path>`) to dodge PowerShell console-codepage corruption of multibyte grammar chars; `scripts/regen-grammar.ps1` added. CI `verify-grammar` upgraded from placeholder → REAL regenerate-and-diff drift gate (C-DRIFT-001) with job display names kept identical so branch protection stays satisfied. Commits `55a470d` + `c012770` (clippy 1.96 `needless_as_bytes` fix); **CI run 28488848856 = full GREEN** (build+test ✓, verify-grammar ✓, verify-schema ✓). Local toolchain bumped to rustc/clippy 1.96.1 to catch lint drift locally.
- [x] **WAVE 2 — DONE + gates verified by main session (not worker-trust):** Worker `pxlang_m3w2_compiler_eval` (972c925e) folded the language core. **px-grammar**: real `#[derive(Parser)]` pest binding (`PxParser`/`Rule`) over GRAMMAR_PEST (drift gate untouched). **px-compiler** (`builder.rs` 90KB): `parse(&str) -> PxDocument` for ALL 12 constructs + v1 step-list control flow + v2 code blocks; written directly against grammar+AST (pluresdb-px's flatter AST was reference-only); NO pluresdb dep. **px-eval** (~40KB across eval/lib/registry/value): ported from praxis-native `eval.rs` — v1 expressions, `eval_rule` (when/let/guards), `eval_constraint` (severity/message); storage/effect boundary = `FunctionRegistry` TRAIT SEAM + 21 real pure builtins; NO pluresdb dep, executor.rs NOT copied. **3 real `examples/*.px`** (memory_assistant, routing_pipeline, legacy_procedure) + `parse_examples.rs` test. Worker's testing caught+fixed 3 real compiler bugs (`^` power fold, `kw_if` atomic-pair skip, compound-trigger keyword extraction). **Honest gaps (C-NOSTUB-001, both legit-absent not stubbed):** v2 code-block *execution* absent (parsed not run, no API pretends); effect execution behind the FunctionRegistry seam (host concern). **INDEPENDENT VERIFICATION by main session on disk:** src real (90KB+40KB, not stubs), zero `pluresdb` in any Cargo.toml, zero stub tokens in shipped src, then RAN the gates myself: `cargo build` Finished clean, `cargo test` all `ok`/0 failures (examples parse ✓, grammar parity ✓), `cargo fmt --check` OK, `cargo clippy -- -D warnings` OK/zero warnings, toolchain 1.96.1. Branch `877b1a6` → **merged to main `57f8e83`** (no-ff, branch deleted, worktree reclaimed). CI run 28501091162 in_progress on merge (Wave-1 already proved the Linux path; local gates all green so risk ~0). Report: `PXLANG-M3W2-RESULT.md`.

### [ ] M4 — Schema-auto-update-on-release (P2 / C-DRIFT-001 structural) — WORKER DISPATCHED 2026-07-01
- [ ] `px-schema` emits `schema/px.schema.json` (JSON-Schema PROJECTION of px-ast via schemars/JsonSchema derives) + `schema/px.schema.px` generated FROM `px-ast`. (NOT a PSF re-root — see M2 finding.)
- [ ] Prereq from M2: add `schemars::JsonSchema` derives to px-ast, stabilize serde tagging on key enums, namespace/drop `span` from the projection
- [ ] CI gate: regenerate-and-diff — build fails if committed schema/grammar != regenerated
- [ ] Release workflow regenerates + commits/publishes schema as part of every release (no manual step)
- [ ] Acceptance: deliberately change an AST construct → CI goes red until schema regenerated → proves the gate; release dry-run produces an updated schema artifact

### [ ] M5 — YAML surface + NAPI bindings (P4 + P5)
- [ ] `px-yaml`: YAML <-> px-ast round-trip (same types, no second truth); round-trip tests
- [ ] `px-napi`: NAPI-RS bindings; published TS package loads native addon; smoke test from Node
- [ ] Acceptance: a `.px` file and its `.yaml` equivalent both deserialize to identical AST (asserted); Node can compile/evaluate a `.px` via the addon (build-the-binary-run-the-binary)

### [ ] M6 — Rewire downstream consumers
- [ ] `pluresdb`: delete in-tree `pluresdb-px` engine; `pluresdb-px` imports `praxis-lang` + keeps ONLY procedure tooling (closes ADR-0017)
- [ ] `pares-radix` + `pares-agens`: point `crates/praxis` at `praxis-lang` (direct for primitives / via pluresdb for procedures)
- [ ] Acceptance: each downstream repo builds + its `.px` tests pass against `praxis-lang`; no repo carries a second grammar

### [ ] M7 — Guide override + old-repo cleanup
- [ ] Update `PLURES-FOUNDATION.md`: `praxis-lang` = the `.px` language home; amend/supersede the 2026-06-29 ruling; demote `praxis` repo to TS app-framework/canvas only
- [ ] **PSF/canvas stays with the app-framework** (old `praxis` repo or its successor) as the `x-app`/`x-canvas` extension — do NOT move PSF into `praxis-lang` (M2 finding)
- [ ] Decide fate of old `praxis` repo language crates (retire/redirect) + carve out C#/PowerShell sprawl
- [ ] Update `repo-routing-validation.md` + decision tree so "where does .px go?" answers `praxis-lang`
- [ ] Acceptance: guide and reality match; a routing check points all `.px`-language code at `praxis-lang`

### [ ] M8 — Final verify (never-confused-again proof)
- [ ] Full cross-repo build green; one canonical grammar+schema, auto-regenerated, CI-enforced
- [ ] Back-brief kbristol with the final map
- [ ] Acceptance: searching the org for `.px` grammar/AST/compiler yields exactly ONE home

---

## EXECUTION LOG (append per session)

- 2026-06-30 16:55 PDT — kbristol overrode the 6/29 ruling, chose Path A (new repo). ADR + tracker written. Verified name is free, no existing repo. Track B (pluresLM #146) landed as #152 `16a2f74` in parallel. Awaiting M0 gate (repo name + visibility) before M1; M2 reconciliation can begin immediately.

- 2026-06-30 17:xx PDT - M2 reconciliation COMPLETE + VERIFIED. Direct grep confirmed PSF != the .px language (0 language-construct hits, 20+ canvas/app hits). px-ast ratified as sole SSOT; grammar generated from it. Design CORRECTED: PSF does NOT enter praxis-lang (stays downstream as x-app/x-canvas extension); schema artifact = pure JSON-Schema projection of px-ast. praxis-lang is now a focused Rust-first language repo, cleaner than first drafted. ADR + M4/M7 updated. Still awaiting M0 gate (repo name + visibility) before M1 create.

- 2026-06-30 18:0x PDT (autonomous continuation wake) - M1 COMPLETE. Verified on disk/GitHub first: repo plures/praxis-lang exists PUBLIC on `main` but had only the M0 docs commit (no skeleton), license undetected. Advanced M1: added MIT LICENSE, .gitignore, README, root workspace Cargo.toml, and 9 empty-but-buildable crates per ADR §3.2. `cargo build`/`test`/`fmt --check`/`clippy -D warnings` all green locally. Added CI (`ci.yml`) with build/test/fmt/clippy + placeholder verify-grammar/verify-schema drift lanes. Committed `9fba208` ("M1: scaffold Cargo workspace + CI + MIT license"), pushed to main, CI run 28486372521 triggered. NEXT WAKE: confirm CI green (if not already logged), set branch protection opportunistically, then start M3 (migrate px-ast + grammar-gen + generated grammar + px-schema/-derive from C:\Projects\praxis, preserving history where feasible; fold pluresdb-px compiler/executor + praxis-native evaluator per ADR).\n\n- 2026-06-30 19:0x PDT (autonomous continuation wake) - **M3 WAVE 1 COMPLETE + CI GREEN.** Verified ground truth first: local clone clean on `main`, prior CI green, branch protection ON (3 checks), all 9 crates scaffolded-empty. Migrated the foundational language core from `C:\Projects\praxis` (HEAD 87cf3ff): px-schema-derive, px-schema, px-ast, px-grammar-gen (+11 .pest fragments) — real source ported, Cargo.tomls wired with path deps. Made px-grammar-gen lib+bin; px-grammar embeds the generated grammar.pest and parity-tests it in-process. **Hit + fixed a real cross-platform bug:** PowerShell decodes child stdout with the console code page, corrupting the grammar's multibyte UTF-8 (em-dash/box chars) — the in-crate parity test caught it (great signal that the drift gate works). Fix: generator writes the artifact file directly as raw UTF-8/LF via `cargo run -p px-grammar-gen -- <path>`; added scripts/regen-grammar.ps1. Upgraded CI verify-grammar placeholder → real regenerate-and-diff gate (kept job display name so branch protection isn't broken). Also fixed clippy::needless_as_bytes (rust 1.96 lint that didn't fire on my older local clippy) and bumped local toolchain to 1.96.1. Commits `55a470d` + `c012770`; **CI run 28488848856 = full GREEN** (build+test ✓ clippy/build/test, verify-grammar ✓ real drift gate on Linux, verify-schema ✓). Full local gate green (px-schema 10 tests, px-grammar 3 parity/determinism tests, all crates). NEXT WAKE = **M3 WAVE 2**: fold pluresdb-px compiler/executor + praxis-native eval (`crates/praxis-native/src/px/eval.rs`) into px-compiler/px-eval, land the pest Parser binding over px-grammar::GRAMMAR_PEST, get `.px` examples parsing. (Loose end still open: pares-radix PR #461 squash-merge + worktree reclaim — opportunistic, non-blocking.)

- 2026-06-30 23:4x PDT (kbristol asked "what's the status / what's left" live) — **ROOT-CAUSED THE STALL + DISPATCHED M3 WAVE 2.** Progress had frozen at M3 Wave 1 since ~19:16 because: (a) two continuation wakes landed on MEMORY-FLUSH turns (writes locked to memory/2026-06-30.md, couldn't execute), and (b) the last cron fire returned `skipped: quiet-hours` (23:00–08:00 policy suppresses the autonomous wake — so the hourly cron won't push work again until 08:00). Plan state never lost (all in git). Verified ground truth: praxis-lang HEAD `f787b4b`, CI run 28488921958 GREEN, px-compiler/px-eval still empty stubs (~520b lib.rs each = Wave 2 truly not started). Since kbristol is live + granted autonomy, advanced Wave 2 now rather than wait for the throttled cron: created worktree `C:\Projects\praxis-lang-m3w2` (branch `m3-wave2-compiler-eval` off f787b4b), wrote worker brief `PXLANG-M3W2-MILESTONES.md`, and DISPATCHED run-mode subagent `pxlang_m3w2_compiler_eval` (`agent:main:subagent:972c925e-...`, runId `dbd69043-...`, opus-4.8) to fold compiler(pluresdb-px compiler.rs)/eval(praxis-native eval.rs preferred) into px-compiler/px-eval, add the pest Parser binding, add >=3 examples/*.px + parse test, gate green (build/test/fmt/clippy 1.96.1 + drift gate), push BRANCH (not main), report to `PXLANG-M3W2-RESULT.md`. Main session adjudicates the merge. NEXT: on worker completion, verify its claims against real files, merge branch to main if green, then M4 (schema-auto-update-on-release). Consider a quiet-hours-aware fix so the epic keeps advancing overnight without pinging kbristol.

- 2026-07-01 00:2x PDT (same live session) — **M3 COMPLETE (both waves, CI GREEN) + M4 DISPATCHED.** M3 Wave 2 worker (972c925e) reported done; I INDEPENDENTLY verified on disk (not worker-trust): px-compiler/px-eval real source (90KB builder.rs + ~40KB eval), zero `pluresdb` in any Cargo.toml, zero stub tokens, then RAN gates myself — `cargo build` Finished, `cargo test` all ok/0 fail (examples parse ✓, grammar parity ✓), fmt OK, clippy -D warnings OK. Merged branch `877b1a6` → main (no-ff `57f8e83`, diverged so ff impossible after my tracker commit; branch deleted, worktree reclaimed). **CI on merge commit = completed/success.** Tracker committed `a72578c`. Reported M3-complete to kbristol. Then AUTO-ADVANCED to M4 (no idling): wrote `PXLANG-M4-MILESTONES.md`, created worktree `C:\Projects\praxis-lang-m4` (branch `m4-schema-autoupdate` off a72578c), DISPATCHED worker `pxlang_m4_schema_autoupdate` (`agent:main:subagent:db8527ed-...`, runId `cebc5e26-...`, opus-4.8) to add schemars::JsonSchema derives to px-ast, emit schema/px.schema.json + schema/px.schema.px as pure projections, upgrade CI verify-schema placeholder → real drift gate, wire release-time regen, prove staleness goes red. Branch-only, main session adjudicates. NEXT: on M4 worker completion, verify + merge if green, then M5 (YAML + NAPI surfaces). Note: pares-radix PR #461 loose end still open (opportunistic).