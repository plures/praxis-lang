# RFC-0001 — Semantic Core Vision

- **Status:** Draft
- **Workstream:** WS-1 (language evolution)
- **Author:** kbristol
- **Companion spec:** [`RFC-0001-semantic-core.px`](./RFC-0001-semantic-core.px) (self-hosting proof)
- **Governance:** `docs/epic/ADR-praxis-lang-single-source-of-truth.md`, `docs/epic/PRAXIS-LANG-TRACKER.md`

> **This RFC is specification-only.** No AST, grammar, evaluator, or projection code
> changes ship with it. Implementation begins only after this RFC is ratified.

---

## 0. Context

The praxis-lang consolidation epic (M0–M8, completed 2026-07-01) already delivered the
baseline the original design discussion asked for:

- One canonical Rust AST (`px-ast`, 12 constructs).
- One generated grammar (`px-grammar-gen` → `grammar.pest`, never hand-edited, ADR-0021).
- CI drift-gated projections (JSON Schema via `px-schema`, YAML via `px-yaml`, NAPI via `px-napi`).
- Downstream consumers rewired to import the canonical crates (`pluresdb-px` → `pares-radix`, `pares-agens`).

RFC-0001 does **not** re-litigate that finished work. It defines the *next* step: a
**semantic core** that the current 12 surface constructs **lower** into, enabling the later
RFCs (structural types, effects, relational inference, policy, durable workflows, bounded
verification) to be expressed against a small, stable set of primitives instead of a flat
peer list of constructs.

---

## 1. Semantic-Core Primitives

The current AST is flat: `import`, `entity`, `config`, `fact`, `rule`, `constraint`,
`contract`, `function`, `trigger`, `DataflowProcedure`, `LegacyProcedure`, `scenario` are
all peers. RFC-0001 proposes four irreducible core primitives that every surface construct
lowers into:

| Core primitive | Generalizes | Essence |
|---|---|---|
| **Declaration** | `entity`, `fact`, `config`, `function`, `import` | A named, typed, scoped binding of data or a callable signature. |
| **Assertion** | `constraint`, `rule`, `contract` | A predicate over state with a severity/outcome and optional reaction. |
| **Procedure** | `DataflowProcedure`, `LegacyProcedure`, `trigger` | An ordered/reactive set of steps with effects. |
| **Scenario** | `scenario` | A given/when/then executable expectation (already near-core). |

These are candidates, not final. Ratifying RFC-0001 means agreeing the core is *this small*
and that the lowering below is *mechanical and reversible*.

---

## 2. Lowering Model

Lowering is an **internal** transformation from surface AST → core AST. It must be:

1. **Mechanical** — a total function; every surface construct has exactly one lowering.
2. **Reversible** — `raise(lower(x)) == x` at the AST level (round-trip identity).
3. **Transparent** — no observable change to surface syntax, JSON Schema, YAML, or NAPI.

| Surface construct | Lowers to | Reversible |
|---|---|---|
| `import` | Declaration (module binding) | yes |
| `entity` | Declaration (record type + prefix) | yes |
| `fact` | Declaration (record instance) | yes |
| `config` | Declaration (config record) | yes |
| `function` | Declaration (callable signature + mode) | yes |
| `constraint` | Assertion (predicate + severity) | yes |
| `rule` | Assertion (reactive predicate + actions) | yes |
| `contract` | Assertion (given/when/then + threshold + examples) | yes |
| `DataflowProcedure` | Procedure (queue-driven steps) | yes |
| `LegacyProcedure` | Procedure (step list) | yes |
| `trigger` | Procedure (event-bound steps) | yes |
| `scenario` | Scenario | yes |

The round-trip identity is captured as a `contract` in the companion `.px` file
(`lowering_roundtrip`, threshold 1.0).

---

## 3. Compatibility Policy

- **Every existing `.px` file MUST continue to parse** — enforced by the `examples/*.px`
  parse suite and the RFC self-hosting test.
- **Surface syntax is preserved.** Lowering is internal; authors never see the core form.
- **Deprecation requires migration tooling** — no surface construct is removed until an
  automated migration exists.

Enforced in the companion `.px` by `constraint no_breaking_parse` and
`constraint surface_syntax_preserved` (both severity `error`).

---

## 4. Projection Model

The layered AST (surface → core) must keep every projection CI-drift-gated:

- **JSON Schema** (`px-schema`) — regenerate-and-diff gate.
- **YAML** (`px-yaml`) — round-trip parity gate.
- **NAPI** (`px-napi`) — surface stability gate.

Any core change flows through the ADR-0021 pipeline:
`px-ast` change → `px-grammar-gen` fragment → generated grammar → CI drift gate. The grammar
is never hand-edited. Enforced by `constraint projections_stay_gated`.

---

## 5. Non-Goals (Out of Scope for praxis-lang RFCs)

RFC-0001 draws a hard boundary. The following are **NOT** language concerns and do not
belong in praxis-lang:

- **Graph-native PluresDB development state** (semantic changesets, commit graph objects).
- **Git-compatible projection** of that state.
- **VCS / dev-environment tooling.**
- **Any implementation change** in this RFC (spec-only).

These live in a separate workstream (WS-2) owned by `pluresdb` and/or a dedicated tooling
repo, and depend on WS-1 — they must not be entangled in the language epic. Enforced by
`config non_goals` and `constraint no_implementation_in_rfc`.

---

## 6. Downstream-Transparency Constraint

`pluresdb-px`, `pares-radix`, and `pares-agens` all pin the current `Statement` enum
(pluresdb rev `d08f88b` appears in 6 dependency lines — the widest single-change blast
radius in the org). The lowering must be **transparent to these consumers**: they observe
no surface or schema change. Captured as `contract downstream_transparency` (threshold 1.0)
and enforced across the 5-phase rollout
(`praxis-lang → pluresdb → pares-radix/praxis-native → pares-agens → development-guide`).

---

## 7. Acceptance Criteria

RFC-0001 is **ratified** when:

1. The four core primitives and the 12-construct lowering table are agreed.
2. The companion `.px` file parses green (self-hosting proven — see
   `crates/px-compiler/tests/rfc_0001_self_hosts.rs`).
3. The compatibility, projection, and non-goals boundaries are accepted as binding.
4. The downstream-transparency contract is accepted as the gating invariant for any
   subsequent implementation RFC (RFC-0002+).

**No code changes ship under RFC-0001.** RFC-0002 (structural/refinement types) is the first
implementation RFC and gates on RFC-0001 acceptance.

---

## 8. Sequencing (WS-1)

1. **RFC-0001** — this document (spec only).
2. **RFC-0002** — structural/refinement types (first code change; smallest blast radius; litmus test for the core model).
3. **RFC-0003** — effects and capabilities.
4. **RFC-0004** — policy + relational inference (query engine may be a new `px-query` crate).
5. **RFC-0005** — durable workflow semantics (AST/semantics here; persistence engine is downstream).
6. **RFC-0006** — bounded verification (checker may be a new `px-check` crate; explorer is downstream).

RFCs are strictly sequential; each gates on the previous. No parallelization.
