# RFC-0004 — Policy + Relational Inference

- **Status:** Draft
- **Workstream:** WS-1 (language evolution)
- **Author:** kbristol (design drafted by delegated agent session, 2026-08-03)
- **Companion spec:** [`RFC-0004-policy-relational-inference.px`](./RFC-0004-policy-relational-inference.px) (self-hosting proof, current syntax only)
- **Gates on:** RFC-0003 (`docs/rfcs/RFC-0003-effects-and-capabilities.md`, merged `plures/praxis-lang#21`)
- **Governance:** `docs/epic/ADR-praxis-lang-single-source-of-truth.md`, `docs/epic/PRAXIS-LANG-TRACKER.md`, issue `plures/praxis-lang#3` (WS-1 epic)

> **This RFC is specification-only.** No AST, grammar, evaluator, or projection code
> changes ship with it. RFC-0001 (spec-only) and RFC-0003 (spec-only) are the direct
> precedent for a design RFC that follows a first-code-change RFC (RFC-0002); RFC-0004
> follows the same discipline. Implementation is separate, follow-on work gated on
> this RFC being ratified.

---

## 0. Context

RFC-0001 ratified four semantic-core primitives (Declaration, Assertion, Procedure,
Scenario). RFC-0002 added the first non-core-widening surface construct (`type`).
RFC-0003 added a declared **effect**/**capability** boundary but explicitly deferred
one question to this RFC (RFC-0003 §1 Non-Goals, §8): *deciding whether* a capability
grant is policy-permitted — org rules, RBAC, time-of-day, environment — is **not**
RFC-0003's job. RFC-0003 defines the mechanical, fail-closed check that a call's
declared effect is covered by a granted capability set; it does not define how that
capability set gets decided in the first place, nor does it give `.px` any way to
express or query relationships between the facts/entities a document declares.

RFC-0004 is that missing piece: a **policy** capability (declarative rules that decide
grants/outcomes from state) and a **relational-inference** capability (a query
mechanism over the `Declaration`/`Assertion` state a `.px` document already
establishes). Both are grounded directly in the semantic core RFC-0001 ratified — this
RFC does not invent a parallel design; it is the next lowering target for the same
four primitives.

### 0.1 What already exists (grounded, not invented)

This RFC is scoped by what already exists in three places, so it augments rather than
duplicates:

- **`.px`'s own `constraint`/`rule`/`contract` surface constructs** (RFC-0001's
  Assertion primitive) already express predicates over state with severity and
  reaction — `constraint-eval.px`-style evaluation (`given`, `check`, `severity`) is a
  live pattern in `pares-radix`'s `praxis/procedures/constraint-eval.px`. RFC-0004 does
  not replace this; it generalizes it from a flat, single-namespace set of named
  predicates into a **scoped, queryable, relational** one.
- **`pares-radix`'s ADR-0038 (`radix-policy-settings-engine`, design stage, dated
  2026-07-29)** independently arrived at a scoped policy model (`policy:global:*` /
  `policy:plugin:<id>:*` / `policy:host:<id>:*`, most-specific-wins resolution,
  `overridable`/floor semantics, narrowing-only override for constraint-kind
  policies) built entirely from existing `.px`/PluresDB primitives (`db_get`,
  `db_get_prefix`, `pluresdb_write`, the `constraint:` keyspace). That ADR is a
  **downstream consumer's own design for using today's `.px`**, not a language
  feature — it does not touch `px-ast`/grammar/evaluator. RFC-0004 must not duplicate
  ADR-0038's scope-resolution mechanism at the language level; instead this RFC defines
  what a **relation** and a **policy rule** mean as language-level constructs so that a
  host-side engine like ADR-0038's could (optionally, later) be re-expressed on top of
  them, rather than needing its own ad hoc key-prefix convention. This RFC does not
  require ADR-0038 to change and does not depend on it — the two are complementary,
  language vs. host-application layers.
- **RFC-0003's deferred capability-grant decision** (§8 "Policy-driven grant
  decisions") is explicitly named as this RFC's job. RFC-0004 defines the *rule* shape
  that could decide a grant; it does not modify RFC-0003's Effect/CapabilityGrant
  shapes or its fail-closed runtime check.

---

## 1. Non-Goals for This Slice

Per the sequential-RFC discipline (RFC-0001 §8, RFC-0002 §7, RFC-0003 §1), this RFC
keeps scope to the smallest slice that gives `.px` a checkable policy/relational-query
boundary:

- **No new expression syntax in `Expr`.** Relational queries and policy rules attach to
  new top-level declaration forms (§2, §3), not new operators or literal forms.
- **No capability-grant wiring change.** RFC-0003's `CapabilityGrant`/`Effect`
  shapes and its `FunctionRegistry`-seam runtime check are unmodified. This RFC defines
  a rule shape a host *could* use to decide grants; it does not rewire how grants are
  threaded through evaluation.
- **No storage/query engine implementation.** Whether relational queries execute via a
  new `px-query` crate, an in-memory Datalog-style fixpoint evaluator, or delegation to
  a host-provided store (e.g. PluresDB) is an **open implementation decision** (§7),
  not fixed here.
- **No general-purpose recursive query language.** This slice scopes relational
  inference to **non-recursive, stratified, finite** rule evaluation (no unbounded
  recursion, no negation-as-failure across strata) — full Datalog-with-recursion
  semantics are explicitly deferred (§8) pending a concrete need.
- **No durable/persisted policy state.** Whether policy rule evaluations persist across
  a durable workflow run is RFC-0005's concern (durable workflow semantics).
- **No static verification that policy rules terminate or are consistent.** Proving a
  rule set has no contradictions or infinite loops is RFC-0006's concern (bounded
  verification).
- **No change to `constraint`/`rule`/`contract` surface syntax or their existing
  lowering to Assertion (RFC-0001 §2).** This RFC adds new constructs (`relation`,
  `policy`, §2-§3) alongside the existing Assertion-lowering surface forms; it does not
  redefine them.

---

## 2. Relational Inference: the `relation` Construct

### 2.1 What "relational inference" means here

Grounded in RFC-0001's core primitives: a `.px` document already declares typed,
scoped bindings (Declaration: `entity`, `fact`, `config`, `function`, `import`) and
predicates over them (Assertion: `constraint`, `rule`, `contract`). What is missing is
a way to **derive new facts from existing facts** via named, declarative rules — the
classic relational-inference capability (Datalog-style rule/fact/query separation),
scoped to what `.px`'s existing `entity`/`fact` declarations already provide as a
schema and instance store.

A **relation** is a named, typed predicate over one or more `entity`-shaped tuples,
either:

1. **Extensional** — backed directly by `fact` declarations already matching that
   entity's shape (no new storage; this is a *view* over facts that already exist), or
2. **Intensional** — derived by one or more `infer` rules (§2.3) from other relations
   (extensional or intensional).

This directly generalizes RFC-0001's Declaration/Assertion split: an extensional
relation is a query-shaped **view** over Declarations; an intensional relation is
itself closer to an Assertion (a predicate that produces new derived state) but one
that, unlike `constraint`/`rule`, is not itself a pass/fail check — it's a fact
generator. RFC-0004 therefore proposes `relation`/`infer` as declaration forms that
**lower to Declaration** (like `entity`/`fact`) since they define named, typed bindings
(the relation's schema and its derived tuples), while the `infer` **rule body**
(the derivation logic) lowers to Procedure (an ordered set of steps producing output),
matching how RFC-0001 §2 already treats `rule` as lowering to Assertion for a
predicate-with-reaction, distinct from a plain derivation step.

### 2.2 Declaring a relation (illustrative, not shipped by this RFC)

```rust
// crates/px-ast/src/constructs.rs (illustrative — not shipped by this RFC)
pub struct RelationDecl {
    pub name: Ident,
    /// Field names + types, matching an existing entity's field shape or a
    /// projection/join of several.
    pub fields: Vec<Field>,
    pub docstring: Option<String>,
    pub span: Span,
}
```

Surface syntax sketch (illustrative, for the follow-on RFC to finalize against the
grammar-gen pipeline; not proposed as final grammar text here):

```
relation eligible_reviewer:
  fields:
    reviewer_id: string
    repo: string
  """A reviewer eligible to approve a PR in a given repo."""
```

An extensional relation with no `infer` rule attached is simply a typed view: every
`fact` matching `eligible_reviewer`'s field shape is a member. This is the trivial
case and requires no new evaluation mechanism beyond entity/fact matching that already
exists.

### 2.3 Deriving a relation with `infer`

```
infer eligible_reviewer(reviewer_id, repo):
  given: "A reviewer is eligible if they are an org member with no open conflict-of-interest flag for that repo"
  from: org_member(reviewer_id), NOT conflict_of_interest(reviewer_id, repo)
```

- `from:` lists a conjunction of relation references (extensional or intensional) and
  optionally-negated ones (`NOT <relation>(...)`), matching the "non-recursive,
  stratified" restriction in §1: a negated relation reference must not appear in its
  own derivation chain (no `infer` may derive a relation it also negates, directly or
  transitively) — this is the standard stratified-Datalog safety condition, stated
  here as a **requirement to enforce**, not a new algorithm this RFC specifies.
- Multiple `infer` blocks for the same relation name are unioned (standard Datalog
  multi-rule-per-predicate semantics): a tuple is a member of the relation if *any*
  `infer` rule derives it.
- Variables (`reviewer_id`, `repo`) are bound by position/name across the `from:`
  conjuncts; this RFC does not fix the exact unification algorithm (unification over
  named fields vs. positional — an implementation decision, §7).

### 2.4 Querying a relation

A relation, once declared (extensional or intensional), is queried the same way an
existing `fact` collection would be inspected by a procedure step — this RFC does not
add new query syntax to `Expr`; querying is a **procedure-level operation** (a new
step kind, illustrative below, not fixed grammar):

```
procedure list_eligible_reviewers(repo: string) -> reviewers:
  query eligible_reviewer {repo: $repo} -> $result
  return $result
```

The `query` step kind is illustrative; whether it is a new `Step` variant in
`px-ast::procedures` or expressed via the existing function-call step form calling a
relation-as-function is an open implementation decision (§7).

---

## 3. Policy: the `policy` Construct

### 3.1 What "policy" means here

Grounded directly in RFC-0003's own deferred scope (§8: "Policy-driven grant
decisions — RFC-0004 ... is the natural home for *deciding* what capabilities a given
principal/context should receive"): a **policy** is a named, declarative rule that
maps a **context** (a tuple of relevant facts — principal, resource, scope, time, etc.)
to an **outcome** (allow/deny, a capability grant, or an arbitrary decision value),
evaluated over relations (§2). Where a `constraint` (RFC-0001 Assertion) is a
pass/fail predicate with a severity, a `policy` is a **decision function**: given a
context, it produces an outcome, using the same relational substrate.

This is the direct generalization of `pares-radix`'s ADR-0038 scope/precedence model
(§0.1) into a language-level shape, without adopting ADR-0038's specific PluresDB key
convention: ADR-0038's `policy:global:<name>` / `policy:plugin:<id>:<name>` /
`policy:host:<id>:<name>` scope-path is one possible **host-side storage
convention** for the `policy` construct's declared rules; this RFC's `policy`
construct itself is scope-agnostic — scope is a field a policy rule can inspect via
relations (§2), not something this RFC bakes into the AST as host/plugin/global.

### 3.2 Declaring a policy (illustrative, not shipped by this RFC)

```rust
// crates/px-ast/src/constructs.rs (illustrative — not shipped by this RFC)
pub struct PolicyDecl {
    pub name: Ident,
    pub context_fields: Vec<Field>,
    pub outcome_type: TypeExpr,
    pub docstring: Option<String>,
    pub span: Span,
}
```

Surface syntax sketch (illustrative; not proposed as final grammar text):

```
policy grant_network_capability:
  context:
    principal: string
    host: string
    effect: string
  outcome: bool
  """Decide whether principal may be granted the network effect for a given host."""

decide grant_network_capability(principal, host, effect):
  given: "Deny by default; allow only for an org member with an explicit network-allow relation for that host"
  when: effect == "network"
  from: org_member(principal), network_allow(principal, host)
  outcome: true

decide grant_network_capability(principal, host, effect):
  given: "Fallback: deny everything not explicitly allowed above (fail-closed default, matching RFC-0003 §5 property 1)"
  outcome: false
```

- `decide` blocks for a `policy` are evaluated **in declaration order**; the first
  matching `decide` (whose `when:`/`from:` conditions hold) wins — this is a
  deliberate departure from `infer`'s union-of-all-matches semantics (§2.3), because a
  policy decision must be a single deterministic outcome, not a set. This mirrors
  ADR-0038's own override-precedence model (most-specific/first-match wins), restated
  at the language level rather than as a storage-key convention.
- **A `policy` construct with no matching `decide` block is a hard requirement, not
  optional**: every `policy` declaration MUST end with a `decide` clause whose
  `from:`/`when:` are both absent (an unconditional catch-all), enforced as a
  compile/lint-time check. This mechanically enforces the same fail-closed-by-default
  posture RFC-0003 §5 established for capability enforcement, applied here to policy
  *decisions* rather than capability *checks* — the two are complementary layers
  (RFC-0003: is an attempted call's effect covered by a granted capability;
  RFC-0004: how a capability grant itself gets decided).

### 3.3 Relationship to RFC-0003's capability model

RFC-0004 does not change RFC-0003's `Effect`/`CapabilityGrant` AST shapes or its
runtime fail-closed check (§1). What it adds is a **language-level way to express the
decision logic** that a host could use to *construct* the `CapabilitySet` RFC-0003 §3.3
says is threaded in "outside the `.px` source, by whatever host embeds the evaluator."
Concretely: a host MAY evaluate a `policy` declaration to decide whether to grant a
capability, but RFC-0004 does not require this — a host remains free to decide grants
by any other mechanism (e.g. ADR-0038's PluresDB-key-based engine) as RFC-0003 already
allows. This RFC only ensures `.px` itself has a construct capable of expressing such a
decision declaratively, should an author or host want to author it in `.px` rather than
host-native code.

---

## 4. Compatibility

- **Every existing `.px` file MUST continue to parse** — `relation`/`infer`/`policy`/
  `decide` are new, additive top-level construct kinds; no existing construct's syntax
  changes. Enforced by the `examples/*.px` parse suite and this RFC's own self-hosting
  test (companion `.px`, current syntax only — see §6).
- **No change to the 12-construct lowering table (RFC-0001 §2).** `relation` lowers to
  Declaration; `policy` lowers to Declaration; `infer`/`decide` rule bodies lower to
  Procedure (per §2.1/§3.2's reasoning) — these are *additions* to the lowering table,
  not modifications of any existing row.
- **No change to JSON Schema/YAML/NAPI projection surfaces for existing constructs.**
  Any implementation adding `relation`/`policy` flows through the ADR-0021 pipeline
  (`px-ast` → `px-grammar-gen` fragment → generated grammar → CI drift gate); the
  grammar is never hand-edited, per the epic's binding constraint.

---

## 5. Downstream Transparency

`pluresdb-px`, `pares-radix`, and `pares-agens` pin the current `Statement` enum
(RFC-0001 §6). This RFC's proposed `RelationDecl`/`PolicyDecl` additions are new
`Statement` variants, not modifications of existing ones — matching RFC-0002's own
precedent (RFC-0002 §5: new variant, exhaustive-match audit required in all three
downstream repos before any implementation PR merges) and RFC-0003's identical
requirement (RFC-0003 §7 item 4).

**This RFC's scope does not require any change in `pares-radix` or `pares-agens`.**
Specifically:

- **ADR-0038 (`radix-policy-settings-engine`) needs no change.** It is a design for how
  `pares-radix` itself organizes policy data in PluresDB using *today's* `.px`
  procedures (`constraint-eval.px`-style, key-prefix convention) — it does not consume
  a `RelationDecl`/`PolicyDecl` AST node, because those nodes do not exist until a
  follow-on implementation RFC ships them. If a future implementation RFC for
  `relation`/`policy` lands, `pares-radix` *could* choose to re-express ADR-0038's
  policy engine on top of the new constructs, but that is optional, later work, not a
  requirement flowing from this spec RFC.
- **`pluresdb-px`/`pares-agens` need no change** for the same reason: no AST/grammar
  change ships under this RFC.

If a future implementation RFC decides to add the `Effect`-set-informed policy
evaluation to `pares-radix`'s spine (`praxis/spine/*.px`), that is explicitly flagged
here as **out of this RFC's scope** and left for that follow-on work's own downstream
audit, per the standing exhaustive-match-audit requirement (RFC-0002 §5, RFC-0003 §7
item 4).

---

## 6. Self-Hosting Proof

Companion file: [`RFC-0004-policy-relational-inference.px`](./RFC-0004-policy-relational-inference.px).
Per RFC-0002 §6 item 3 / RFC-0003 §9 item 5 precedent: since `relation`/`infer`/
`policy`/`decide` do not exist in the shipping grammar until a follow-on
implementation RFC, the companion `.px` file uses **only current (RFC-0001/0002/0003-era)
syntax** (`config`, `entity`, `fact`, `function`, `constraint`, `contract`, `scenario`)
to model this RFC's own scope, boundaries, and acceptance criteria — the same pattern
RFC-0001/0002/0003's companion files used. Gated by
`crates/px-compiler/tests/rfc_0004_self_hosts.rs` (parses via `px_compiler::parse`,
non-empty statement list required).

---

## 7. Open Implementation Decisions (deferred to the follow-on PR, not fixed here)

Per the RFC-0001/0002/0003 precedent of separating spec from implementation:

1. **Whether `relation`/`policy`/`infer`/`decide` are four new `Statement` variants or
   whether `infer`/`decide` are sub-forms nested under `relation`/`policy` (a single
   variant each with an internal rule-list field)** — an AST-shape decision left to the
   implementation PR.
2. **Unification algorithm for `from:` conjuncts** (§2.3) — named-field unification vs.
   positional, and how variables are scoped across a rule body.
3. **Stratification-safety check enforcement** (§2.3's non-recursive/stratified
   requirement) — whether enforced at parse time, a separate lint pass, or left to
   effect/well-formedness checking analogous to RFC-0003 §4's effect-inference pass.
4. **The concrete `query`/`decide` step-kind shape** in `px-ast::procedures` (§2.4,
   §3.2) — new `Step` variants vs. reuse of the existing function-call step form
   treating relations/policies as callables.
5. **Whether relational query execution is a new `px-query` crate** (as speculatively
   named in the epic issue, `plures/praxis-lang#3`) **or folds into `px-eval`** as an
   additional evaluation mode — this RFC does not decide crate boundaries, only the
   language-level shape of `relation`/`policy`/`infer`/`decide`.
6. **Exhaustive-match audit** of any existing `match` over `Statement`-like enums in
   `pluresdb-px`, `pares-radix`, `pares-agens` that would need a new arm for
   `RelationDecl`/`PolicyDecl`, per the RFC-0002 §5 / RFC-0003 §7 item 4 precedent —
   must be grepped and either patched or converted to non-exhaustive handling before
   the implementation PR merges.
7. **Whether/how a host-side engine like ADR-0038 adopts `policy`/`relation`
   constructs** once implemented — explicitly optional, deferred to whoever owns that
   follow-on decision in `pares-radix` (§5).

---

## 8. Deferred / Future-RFC Candidates

Not proposed here, but flagged so they are not silently forgotten:

- **Recursive (non-stratified) relational inference** — full fixpoint/recursive
  Datalog semantics, deferred until a concrete need appears (§1).
- **Negation-as-failure across strata beyond the single-level `NOT` in §2.3** — a
  fuller multi-stratum stratification model, useful once relation chains grow deep
  enough that single-level negation safety is insufficient.
- **Persisted/durable policy decisions** — interacts with RFC-0005 (durable workflow
  semantics): whether a `decide` outcome, once computed for a given context, is cached
  or must be re-evaluated on every durable-workflow resume.
- **Bounded verification of policy totality/determinism** — proving a `policy`'s
  `decide` chain always terminates in exactly one outcome for any context is a natural
  RFC-0006 (bounded verification) candidate, not fixed here.
- **Host-side re-basing of ADR-0038 onto `relation`/`policy`** (§0.1, §7 item 7) — an
  optional follow-on for `pares-radix` once the language constructs exist; not
  proposed or required by this RFC.

---

## 9. Acceptance Criteria

RFC-0004 (this design) is **ratified** when:

1. The `relation`/`infer` shape (§2) — extensional relations as views over `fact`
   declarations, intensional relations derived by `infer` with union-of-matching-rules
   semantics, and the stratified/non-recursive restriction (§1, §2.3) — is agreed as
   the entire scope of this slice's relational-inference capability.
2. The `policy`/`decide` shape (§3) — context/outcome declaration, first-match-wins
   `decide` ordering, and the mandatory unconditional catch-all requirement (§3.2) — is
   agreed as the entire scope of this slice's policy capability.
3. §3.3's boundary is accepted: RFC-0004 does not modify RFC-0003's
   `Effect`/`CapabilityGrant` shapes or runtime check; it only adds a declarative way
   to express a grant-decision that a host *may* use.
4. §5's downstream-transparency conclusion is accepted: no change required in
   `pluresdb-px`, `pares-radix` (including ADR-0038), or `pares-agens` as part of this
   RFC; any future re-basing of ADR-0038 onto these constructs is separate, optional,
   later work.
5. The companion `.px` file parses green against the **current** grammar (self-hosting
   proof — it does not and cannot yet use `relation`/`policy`/`infer`/`decide`, exactly
   as RFC-0002 §6 item 3 / RFC-0003 §9 item 5 required for their own companion files).
6. The open-implementation-decisions list (§7) is accepted as binding scope for the
   follow-on implementation PR(s), out of scope for this RFC's own diff.

**No code changes ship under RFC-0004.**

---

## 10. Sequencing

Per RFC-0001 §8 / RFC-0002 §7 / RFC-0003 §10, RFCs are strictly sequential:

1. RFC-0001 — semantic core (ratified, merged, spec-only, PR #4).
2. RFC-0002 — structural/refinement types (ratified, merged, PR #5 + #8).
3. RFC-0003 — effects and capabilities (ratified, merged, spec-only, PR #21).
4. **RFC-0004 — this document.** Design-only. On ratification, a separate
   implementation PR (or PR series) lands the `relation`/`policy`/`infer`/`decide`
   AST nodes, grammar fragments, and evaluation mechanism — gated on this design being
   accepted, tracked as its own follow-on work item, not part of this RFC's diff.
5. RFC-0005 — durable workflow semantics — decides how policy decisions and relational
   query results persist across durable execution/resume; unaffected by this slice.
6. RFC-0006 — bounded verification — may statically prove policy totality/determinism
   and relational-inference termination properties; unaffected by this slice's design.