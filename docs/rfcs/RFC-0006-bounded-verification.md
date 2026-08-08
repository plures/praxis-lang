# RFC-0006 — Bounded Verification

- **Status:** Draft
- **Workstream:** WS-1 (language evolution)
- **Author:** kbristol (design drafted by delegated agent session, 2026-08-08)
- **Companion spec:** [`RFC-0006-bounded-verification.px`](./RFC-0006-bounded-verification.px) (self-hosting proof, current syntax only)
- **Gates on:** RFC-0005 (`docs/rfcs/RFC-0005-durable-workflow-semantics.md`)
- **Governance:** `docs/epic/ADR-praxis-lang-single-source-of-truth.md`, `docs/epic/PRAXIS-LANG-TRACKER.md`, issue `plures/praxis-lang#3` (WS-1 epic)

> **This RFC is specification-only.** No AST, grammar, evaluator, or projection code
> changes ship with it. Implementation is separate, follow-on work gated on this RFC
> being ratified.

---

## 0. Context

RFC-0001 through RFC-0005 have progressively enriched `.px` with structural types
(RFC-0002), effects and capabilities (RFC-0003), policy and relational inference
(RFC-0004), and durable workflow semantics (RFC-0005). Each of these RFCs explicitly
deferred one class of question to this RFC: **can we statically prove properties about
a `.px` document without executing it?**

Specifically:

- **RFC-0002** (structural types): can we verify that a refinement type's predicate is
  satisfiable? That a type alias is well-founded (no infinite recursion)?
- **RFC-0003** (effects): can we verify that every call site's declared effects are
  covered by the enclosing scope's capability grants? (RFC-0003 already defines a
  fail-closed runtime check; RFC-0006 asks whether a *static* version is feasible.)
- **RFC-0004** (policy/relational inference): can we verify that a policy's `decide`
  chain is total (every possible context produces exactly one outcome)? That relational
  `infer` rules terminate (no unbounded derivation)?
- **RFC-0005** (durable workflows): can we verify that every effectful step has a
  declared compensation action (compensation totality)? That idempotency annotations
  are consistent with step semantics?

RFC-0006 defines what "bounded verification" means at the language level — a
**checker** that operates on the AST (after lowering to semantic-core primitives) and
reports violations as diagnostics, without executing the document.

### 0.1 What "bounded" means

The word "bounded" is intentional: this RFC does not propose a general-purpose theorem
prover or an undecidable static analysis. It defines a **finite, decidable** set of
verification properties that can be checked in time polynomial (or at worst exponential
in a bounded parameter, e.g., number of `decide` clauses) in the size of the document.
The checker is a **lint-like tool**, not a proof assistant.

### 0.2 What already exists (grounded, not invented)

- **`px-compiler`'s parse phase** already rejects syntactically invalid documents.
- **`px-eval`'s type-checking** (when present) validates fact/entity field-type
  compatibility at evaluation time.
- **RFC-0003's fail-closed capability check** is a runtime verification: at each
  effectful call site, the evaluator checks that the call's declared effect is covered.
  RFC-0006 proposes a **static** version of the same check that runs at compile time
  (before evaluation).
- **RFC-0004's stratification requirement** (§2.3): `infer` rules must be
  non-recursive and stratified. This is a property that can be checked statically by
  analyzing the dependency graph of relation references.

---

## 1. Non-Goals for This Slice

- **No general theorem proving.** The checker does not prove arbitrary user-supplied
  assertions. It checks a fixed set of language-defined properties (§2).
- **No runtime enforcement changes.** RFC-0006 adds a static checker (a new analysis
  pass); it does not modify how `px-eval` executes documents at runtime.
- **No new surface syntax.** The checker operates on existing AST constructs; it does
  not add keywords or declaration forms to the grammar.
- **No mandatory verification gate.** Whether verification is run as a CI check, an
  editor lint, or not at all is a host/tooling decision. The checker is opt-in.
- **No cross-document analysis.** The checker operates on a single `.px` document (or a
  resolved import set). Cross-repository or cross-service verification is out of scope.
- **No performance guarantees on checker execution.** While properties are decidable,
  this RFC does not commit to specific time/space bounds for the checker
  implementation.

---

## 2. Verification Properties

The checker validates the following properties, organized by the RFC that introduced
the construct being checked:

### 2.1 Type well-formedness (RFC-0002)

- **No cyclic type aliases.** If `type A = B` and `type B = A`, the checker reports an
  error. Checked via cycle detection in the type-alias dependency graph.
- **Refinement predicate satisfiability (optional, best-effort).** If a refinement
  type's predicate is trivially unsatisfiable (e.g., `x > 5 and x < 3`), the checker
  reports a warning. This is best-effort — complex predicates may be reported as
  "unchecked" rather than wrongly flagged.

### 2.2 Effect coverage (RFC-0003)

- **Static capability coverage.** For every call site that declares an effect, the
  checker verifies that the enclosing scope (function/procedure/workflow) declares a
  capability that covers that effect. Missing coverage is an error.
- **Unused capability warning.** If a scope declares a capability that no call site
  within it uses, the checker reports a warning (potential over-granting).

### 2.3 Policy totality and relational termination (RFC-0004)

- **Policy totality.** Every `policy` declaration's `decide` chain must cover all
  possible contexts — guaranteed by the mandatory unconditional catch-all requirement
  (RFC-0004 §3.2). The checker verifies syntactically that the final `decide` clause is
  unconditional.
- **Relational stratification.** The dependency graph of `relation`/`infer`
  declarations must be acyclic (no relation derives from itself, directly or
  transitively). Checked via topological sort; cycles are errors.
- **No negation in derivation chain.** A relation must not appear negated (`NOT`) in
  its own derivation chain. Checked alongside stratification.

### 2.4 Workflow compensation and idempotency (RFC-0005)

- **Compensation coverage.** Every step in a `workflow` that declares a non-trivial
  effect must have an associated `compensate` action (or explicit `compensate: noop`).
  Missing compensation for an effectful step is an error.
- **Idempotency consistency.** A step marked `idempotent: false` (or without the
  annotation, defaulting to non-idempotent) that appears after a `suspend` point must
  have deduplication support (the checker flags a warning if such steps lack an explicit
  idempotency-key annotation, since they may re-execute on resume).
- **Checkpoint reachability.** Every `suspend` point must be preceded by (or implicitly
  include) a checkpoint. The checker verifies this structural property.

### 2.5 General structural checks

- **Unreachable code.** Steps in a procedure/workflow after an unconditional `return` or
  terminal `suspend` with no resume path are flagged as warnings.
- **Duplicate declarations.** Two declarations with the same name in the same scope are
  errors.
- **Import resolution.** Imported names that cannot be resolved (when the import source
  is available) are errors.

---

## 3. Checker Architecture (illustrative)

```rust
// crates/px-check/src/lib.rs (illustrative — not shipped by this RFC)
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
}

pub struct Diagnostic {
    pub severity: Severity,  // Error, Warning, Info
    pub property: Property,  // Which §2 property was violated
    pub span: Span,          // Location in source
    pub message: String,
}

pub fn check(doc: &px_ast::Document) -> CheckResult { ... }
```

The checker takes a parsed `Document` (from `px_compiler::parse`) and returns
diagnostics. It does not modify the AST or produce a new output — it is a pure
read-only analysis pass.

---

## 4. Compatibility

- **Every existing `.px` file continues to parse.** The checker is a separate analysis
  pass; it does not change the parser or grammar.
- **Existing documents may produce new diagnostics.** A document that parsed and
  evaluated correctly before RFC-0006 may now receive warnings or errors from the
  checker. This is informational — the checker does not block parsing or evaluation
  unless the host opts in to treating checker errors as fatal.
- **No lowering-table changes.** The checker operates on already-lowered AST; it does
  not add constructs to the lowering table.

---

## 5. Downstream Transparency

- **`pluresdb-px`**: no change required. The checker is a new optional analysis pass in
  `praxis-lang`; downstream consumers are unaffected.
- **`pares-radix`**: no change required.
- **`pares-agens`**: no change required.

The checker may be exposed as a library (`px-check` crate) that downstream tools can
optionally invoke, but no downstream repo is required to adopt it.

---

## 6. Self-Hosting Proof

The companion file `RFC-0006-bounded-verification.px` uses **only** current `.px`
syntax to model this RFC's design boundaries and acceptance criteria. It parses green
via `px_compiler::parse` (gated by `crates/px-compiler/tests/rfc_0006_self_hosts.rs`).

---

## 7. Open Implementation Decisions (deferred to the follow-on PR, not fixed here)

1. **Crate structure:** whether the checker lives in a new `px-check` crate or as a
   module within `px-compiler`.
2. **Incremental checking:** whether the checker supports incremental analysis (only
   re-checking changed portions of a document) or always operates on the full document.
3. **Diagnostic output format:** whether diagnostics are emitted as structured JSON,
   LSP-compatible diagnostics, or plain text.
4. **Severity configuration:** whether users can suppress specific properties or
   promote warnings to errors via configuration.
5. **Best-effort vs. sound properties:** which §2 properties are sound (no false
   negatives) vs. best-effort (may miss violations) — the default is sound for
   structural/syntactic checks (§2.3, §2.4, §2.5) and best-effort for semantic checks
   (§2.1 refinement satisfiability).
6. **Integration with `px-eval`:** whether the checker runs as a pre-evaluation pass
   (fail-fast before evaluation) or as a standalone tool independent of the evaluator.

---

## 8. Deferred / Future-RFC Candidates

Not proposed here, but flagged so they are not silently forgotten:

- **Cross-document verification** — checking properties across import boundaries when
  multiple `.px` files form a module graph.
- **Formal semantics** — a mechanized specification of `.px`'s evaluation semantics
  that the checker's properties can be proven correct against.
- **User-defined verification properties** — allowing `.px` authors to declare custom
  invariants that the checker should verify (beyond the fixed set in §2).
- **Verification-guided refactoring** — using checker results to suggest automated
  fixes (e.g., "add missing compensation for step X").
- **Performance bounds** — proving that a procedure/workflow terminates within a
  declared time/step budget.

---

## 9. Acceptance Criteria

RFC-0006 (this design) is **ratified** when:

1. The verification properties (§2) — type well-formedness, effect coverage, policy
   totality, relational stratification, compensation coverage, idempotency consistency,
   checkpoint reachability, and general structural checks — are agreed as the initial
   scope of the bounded-verification checker.
2. The "bounded" constraint (§0.1) is accepted: the checker is a decidable, finite
   analysis tool, not a general theorem prover.
3. §4's compatibility guarantee is accepted: existing documents continue to parse and
   evaluate; the checker produces diagnostics but does not gate parsing or evaluation
   unless the host opts in.
4. §5's downstream-transparency conclusion is accepted: no downstream repo requires
   changes.
5. The companion `.px` file parses green against the **current** grammar (self-hosting
   proof).
6. The open-implementation-decisions list (§7) is accepted as binding scope for the
   follow-on implementation PR(s).

**No code changes ship under RFC-0006.**

---

## 10. Sequencing

Per the sequential-RFC discipline:

1. RFC-0001 — semantic core (ratified, merged, spec-only, PR #4).
2. RFC-0002 — structural/refinement types (ratified, merged, PR #5 + #8).
3. RFC-0003 — effects and capabilities (ratified, merged, spec-only, PR #21).
4. RFC-0004 — policy + relational inference (ratified, merged, spec-only, PR #29).
5. RFC-0005 — durable workflow semantics (this epic, spec-only).
6. **RFC-0006 — this document.** Design-only. On ratification, a separate
   implementation PR (or PR series) lands the `px-check` crate with the verification
   properties defined in §2 — gated on this design being accepted.

This is the final RFC in the WS-1 (semantic-core evolution) epic. On ratification of
RFC-0006, the language's semantic core is complete as specified in RFC-0001's vision:
structural types, effects, relational inference, policy, durable workflows, and bounded
verification are all defined at the language level, with implementation following as
sequential work items.
