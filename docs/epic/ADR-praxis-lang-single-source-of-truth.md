# ADR / DESIGN: `praxis-lang` — the single source of truth for the `.px` Praxis Intent Language

- **Status:** ACCEPTED (kbristol, 2026-06-30) — supersedes the 2026-06-29 "praxis repo is the .px home" ruling in PLURES-FOUNDATION.md.
- **Decision owner:** kbristol (Paradox)
- **Author:** mswork (assistant)
- **Date:** 2026-06-30
- **Type:** Foundational architecture decision + migration epic charter
- **Restart-proof:** YES. This doc + `PRAXIS-LANG-TRACKER.md` are the durable handoff. A fresh session (or post-gateway-restart) resumes from the tracker's checklist with zero re-derivation.

---

## 1. The decision (verbatim intent)

> "We need this resolved once and for all. .px is the foundation of everything we do. Create a **new repo** for the praxis intent language. Make it the **single source of truth for everything .px**. Select the **best concepts** that make the best and most expressive, consistent, and versatile language. **Rust-first**, **schema that auto-updates with every release**, **standard syntax**, **Rust and YAML**, and **broad support via NAPI**. Do it right, so when the gateway restarts we just continue, and we **never get confused about where .px lives again**." — kbristol, 2026-06-30

> "That was before I released. .px was spread across 4 repos. **Override the guide and update it.** We need a fresh clean new single source of truth for .px. **New repo.**" — kbristol, 2026-06-30

**This explicitly overrides** the 2026-06-29 PLURES-FOUNDATION ruling (which named the existing `praxis` repo as the `.px` home and said "do not re-litigate"). That ruling was made before the 4-repo sprawl was fully understood. It is now superseded by this ADR.

---

## 2. Why (the problem this kills forever)

`.px` — the Praxis Intent Language — is currently spread across **4+ locations**, with duplication and no single canonical home. Verified on disk 2026-06-30:

| # | Location | What's there | Role today |
|---|---|---|---|
| 1 | `praxis` repo — Rust crates | `px-ast` (canonical AST), `px-grammar-gen` (fragments → `grammar.pest`), `px-schema`, `px-schema-derive`, `praxis-native` | The Rust language core (ADR-0021) |
| 2 | `praxis` repo — TS cores | `core/schema-engine` (PSF — Praxis Schema Format), `core/logic-engine`, `core/codegen`, `core/db-adapter` (+ byte-identical copy in `packages/praxis-core/src/`) | TS schema-driven compiler (PR #51 `88ba653`) |
| 3 | `praxis` repo — cross-lang sprawl | `csharp/`, `powershell/`, `deno.json`, `jsr.json`, `ui/`, multiple `*_SYNC.md` / `STREAM_*.md` | Kitchen-sink monorepo baggage |
| 4 | `pluresdb/crates/pluresdb-px` (v3.0.1) | A **duplicate in-tree `.px` engine** — `compiler.rs`, `executor.rs`, generated `grammar.pest` (Grammar v4). Does NOT import the praxis crate (references it only as a keyword string). | The runnable engine radix/agens actually consume — the ADR-0017 drift |
| 5 | `pares-radix/crates/praxis` + `pares-agens/crates/praxis` | Wrapper/copy consumers | Consumers |

**Consequence:** "Where does `.px` live?" has had no single answer. The grammar is authored in `praxis`, generated into `pluresdb`, wrapped in `pares-radix`, copied in `pares-agens`, and a *separate* TS PSF schema engine sits in `praxis/core` unwired from the Rust grammar. This is the confusion to end.

---

## 3. The solution: `praxis-lang` (proposed name)

A **fresh, clean, new repo** — `plures/praxis-lang` (alt names considered: `px-lang`, `px`). The grammar header already literally calls the language **"Praxis Intent Language (.px)"**, so `praxis-lang` is the unmistakable canonical name. **(NAME PENDING kbristol final ✔ — everything else proceeds.)**

`praxis-lang` becomes the **single source of truth for everything `.px`**: the language spec, grammar, AST, schema, compiler, evaluator, constraint-engine primitives, the YAML surface, and the NAPI bindings. Nothing `.px`-language-shaped lives anywhere else after migration. Downstream repos (`pluresdb`, `pares-radix`, `pares-agens`, `inner-space`) **consume** it; they never re-fork it.

### 3.1 Design pillars (kbristol's "best concepts", made concrete)

**P1 — Rust-first (canonical implementation).**
- The language is *defined* in Rust. `px-ast` (canonical AST = "if it's not here, it doesn't exist in the language") is the spec. The compiler/executor/evaluator/constraint-engine are Rust.
- Best concept kept from the current Rust stack: **ADR-0021 grammar-as-generated-artifact** — `grammar.pest` is GENERATED from `px-ast` via `px-grammar-gen` fragments, never hand-edited. This stays and becomes law in the new repo.

> **⚠ M2 RECONCILIATION CORRECTION (verified on disk 2026-06-30 — supersedes the earlier PSF assumption below):** The M2 pass + a direct grep PROVED that **PSF is NOT a representation of the `.px` language** — it is a separate schema for a *visual app/canvas builder*. Evidence: `psf.ts` has **0** hits for `entity`/`contract`/`procedure`/`scenario`/`import`/`duration` (none of the core `.px` constructs) but **20 canvas / 17 component / 15 model / 12 flow / 13 event** hits. Meanwhile `px-ast ↔ grammar v4` are ALREADY the single codegen-linked representation of the whole language (grammar header: *"Source of truth: praxis/crates/px-ast/src/"*; the praxis `declarations.pest` fragment byte-matches `constructs.rs`; all 12 constructs + both expression layers + type system align).
>
> **Therefore the corrected design:**
> - **`praxis-lang`'s language core = `px-ast` (canonical) + generated grammar. Full stop.** PSF is NOT folded in.
> - The repo's **schema artifact (P2) is a pure JSON-Schema PROJECTION of `px-ast`** (via `schemars`/`JsonSchema` derives), NOT a re-rooted PSF. No TS PSF baggage enters the language SSOT.
> - **PSF stays a DOWNSTREAM concern** — the visual canvas/app layer — living with the app-framework (old `praxis` repo / canvas), expressed as an `x-app`/`x-canvas` EXTENSION that *annotates* the projected language constructs rather than redefining them. It never re-defines `.px` constructs and never becomes a second source of truth.
> - This makes `praxis-lang` **cleaner** than first assumed: a focused Rust-first language repo, no parallel-truth reconciliation needed for the language itself. Full detail: `PXLANG-M2-RECONCILIATION.md`.

**P2 — Schema that auto-updates with every release (C-DRIFT-001 made structural).**
- A machine-readable **schema artifact** (JSON Schema + a `.px` schema doc) is **regenerated from `px-ast` on every release**, in CI, and committed/published as part of the release. Never a manual sync step.
- CI gate: if `px-ast` changes and the regenerated schema/grammar would differ from what's committed, the build FAILS until regenerated. This is the existing `verify-grammar.sh` pattern, extended to the schema + every published surface.
- This is the literal mechanism that satisfies "schema that auto updates with every release" AND closes the PSF↔px-ast drift permanently.

**P3 — Standard syntax (one grammar, versioned).**
- One canonical grammar (current Grammar v4 is the starting point — it's stable since 2026-06-13). Construct set (from the live grammar): `import`, `entity`, `config`, `fact`, `rule`, `constraint`, `contract`, `function`, `trigger`, `procedure` (dataflow + legacy), `scenario`, plus V1 step-list + V2 code-block expression layers + the type system.
- "Best concepts" selection pass (tracked): reconcile any construct that exists in PSF but not the grammar (and vice-versa), settle ONE canonical form per concept, version the grammar (`PROTOCOL_VERSIONING.md` discipline carried over).

**P4 — Rust AND YAML surfaces.**
- Two authoring surfaces over the SAME AST: the native `.px` text syntax (Rust-parsed via pest) AND a **YAML surface** that deserializes to the identical `px-ast` types. YAML is for declarative/config-style authoring (ties directly to Thread-1's "YAML-declarative" directive). Both round-trip through `px-ast`; neither is a second source of truth.

**P5 — Broad support via NAPI.**
- First-class **NAPI-RS bindings** so Node/TS consumers load the Rust engine as a native addon (the TS→Rust→NAPI pattern from PLURES-FOUNDATION). The published TS package is the API surface; Rust is the runtime. Pure-Rust consumers depend on the crate directly.
- Keep the cross-language *capability* (C#/PowerShell/Deno bindings) ONLY if there's a live consumer; otherwise carve it out as legacy and do not carry the sprawl into the clean repo. (Default: NAPI + crate are the two supported surfaces; others are additive later, not foundational.)

### 3.2 Proposed `praxis-lang` repo layout (Rust-first workspace)
```
praxis-lang/
  Cargo.toml                  # workspace
  crates/
    px-ast/                   # canonical AST = the language spec (from praxis/crates/px-ast)
    px-grammar-gen/           # fragments -> grammar.pest generator (ADR-0021)
    px-grammar/               # the generated grammar.pest + pest parser binding
    px-schema/                # schema types + JSON-Schema emitter (PSF concept, re-rooted on px-ast)
    px-schema-derive/         # derive macros
    px-compiler/              # compiler (.px text -> AST -> IR)   [best of pluresdb-px compiler.rs]
    px-eval/                  # expression evaluator + constraint-engine primitives [praxis-native]
    px-yaml/                  # YAML surface <-> px-ast (P4)
    px-napi/                  # NAPI-RS bindings (P5)
  schema/                     # GENERATED, committed: px.schema.json + px.schema.px (P2, CI-enforced)
  docs/                       # language spec, ADRs, PROTOCOL_VERSIONING
  examples/                   # canonical .px + .yaml examples (round-trip tested)
  .github/workflows/          # build + verify-grammar + verify-schema + release(auto-schema) + napi publish
```

### 3.3 What each downstream repo does AFTER migration
- **`pluresdb`**: DELETE the in-tree `pluresdb-px` `.px` engine; `pluresdb-px` becomes a thin layer that *imports* `praxis-lang` crates and adds ONLY the PluresDB-specific procedure tooling (`pxCompileNl`/`pxLoadPxSource`/`pxInsertConstraint`). Closes ADR-0017 for real.
- **`pares-radix` / `pares-agens`**: their `crates/praxis` wrappers point at `praxis-lang` (via `pluresdb` for procedures, or `praxis-lang` directly for raw primitives).
- **`praxis` (old repo)**: the Rust `.px` crates + `core/schema-engine` are MOVED to `praxis-lang` (with history where feasible). What remains of `praxis` (the `@plures/praxis` app framework facade, Svelte canvas UI, decision-ledger app) either stays as an *app-framework* repo or is retired — decided in migration step M7. The `.px` LANGUAGE no longer lives in `praxis`.

---

## 4. Migration epic (gated stages — see PRAXIS-LANG-TRACKER.md for live status)

This is executed as a staged, gated epic (design → scaffold → migrate-core → schema-automation → YAML+NAPI → downstream-rewire → guide+cleanup → verify). Each stage gates the next. Full checklist + per-stage acceptance in the tracker.

**Hard rules carried in:** no stubs (C-NOSTUB-001); build-the-binary-run-the-binary; schema/grammar drift is CI-enforced (C-DRIFT-001); deprecate-not-delete only for *tested* code being replaced; verify every gate on disk.

---

## 5. Guide override (REQUIRED by this ADR)

`development-guide/design/PLURES-FOUNDATION.md` MUST be updated to:
- Add `praxis-lang` to the repo map as the **single source of truth for the `.px` language**.
- Change the "canonical-home ruling (2026-06-29)" block: the `.px` LANGUAGE (grammar/AST/schema/compiler/evaluator/constraint-engine primitives) now lives in **`praxis-lang`**, NOT the `praxis` repo. PluresDB imports `praxis-lang` (for primitives) and still owns procedures. The `praxis` repo is demoted to (at most) the TS app-framework/canvas, no longer the language home.
- Note this ADR as the superseding decision with date + rationale (4-repo sprawl discovered post-release).

This guide update is **Stage M7** but the override is RECORDED as accepted now so the ruling and reality never diverge again.
