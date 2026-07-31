# RFC-0003 — Effects and Capabilities

- **Status:** Draft
- **Workstream:** WS-1 (language evolution)
- **Author:** kbristol (design drafted by delegated agent session, 2026-07-31)
- **Companion spec:** [`RFC-0003-effects-and-capabilities.px`](./RFC-0003-effects-and-capabilities.px) (self-hosting proof, current syntax only)
- **Gates on:** RFC-0002 (`docs/rfcs/RFC-0002-structural-refinement-types.md`), Draft status in-repo — **not yet merged** at time of writing this document (only RFC-0001 is confirmed merged, `plures/praxis-lang#4`; RFC-0002 exists as a Draft doc in-tree but this RFC could not confirm its PR/merge state independently — see §0.1 honesty note).
- **Governance:** `docs/epic/ADR-praxis-lang-single-source-of-truth.md`, `docs/epic/PRAXIS-LANG-TRACKER.md`, issue `plures/praxis-lang#3` (WS-1 epic)

> **This RFC is specification-only.** No AST, grammar, evaluator, or projection code
> changes ship with it. It is the design for RFC-0003 as named in RFC-0001 §8
> ("RFC-0003 — effects and capabilities") and reaffirmed in RFC-0002 §7 item 3
> ("RFC-0003 — effects and capabilities (unaffected by this slice)"). Implementation
> is a separate, follow-on piece of work gated on this RFC being ratified.

---

## 0. Context

RFC-0001 ratified four semantic-core primitives — **Declaration**, **Assertion**,
**Procedure**, **Scenario** — and RFC-0002 added the first non-core-widening surface
construct (`type` / `TypeAliasDecl`, lowering to Declaration). Neither RFC touches
*effects*: whether evaluating a `.px` procedure or calling a function can touch the
outside world, and if so, which part of it.

Today that boundary already exists informally, at the crate level: `px-eval`'s
[`FunctionRegistry`] trait (`crates/px-eval/src/registry.rs`) is documented as "the
single seam through which effects/host capabilities enter evaluation" — `px-eval`
itself ships only [`PureFunctionRegistry`] (genuinely side-effect-free builtins:
string/math/collection functions) and [`EmptyRegistry`] (unknown-function error for
everything). Any I/O, storage, network, or process capability is injected by a **host**
implementing `FunctionRegistry` itself — `px-eval` is deliberately kept free of any
`pluresdb`/host dependency. This is a real, working effect boundary already in the
codebase; it is just **untyped and unenforced at the language level**. A `.px` author
writing `function do_thing(...)` has no way to declare "this calls into storage" or
"this needs network," and the evaluator has no way to reject an undeclared effectful
call before it happens — the boundary lives entirely in *how the host wires its
registry*, not in anything the language can check statically.

RFC-0003 proposes making that boundary a first-class, checkable part of the language:
an **effect type** on `function`/procedure declarations, and a **capability** model
that says which effects a given execution context is actually permitted to perform,
enforced **fail-closed** (undeclared or unauthorized effects are rejected, not
silently allowed) by the runtime that plugs into the existing `FunctionRegistry` seam.

### 0.1 Honesty note on requested source material (C-NOSTUB-001)

This RFC was asked to also read pares-radix's "ADR-0038 (radix-policy-settings-engine)"
and "procedure-graph-repository-substrate design notes" for capability requirements.
Both were checked directly against the `pares-radix` working tree at
`C:\Projects\pares-radix`:

- **No `ADR-0038` exists in `pares-radix`.** `git grep -l "ADR-0038"` and a search for
  `radix-policy-settings` returned nothing. This document does not fabricate an ADR
  that isn't there; if that ADR exists elsewhere (a branch not fetched, a different
  repo, or not yet written), the capability model below should be reconciled against
  it once it's available, but it is **not a citation this RFC can honestly make**.
- **The procedure-graph-repository-substrate design notes ARE accessible** — the
  OpenClaw workspace memory file
  `memory/design-procedure-graph-repository-substrate-2026-07-24.md` was read in full.
  §13 ("Security model") and §4.3 ("Identity types") of that document are used directly
  below (§3 and §5 of this RFC) as the concrete requirements this effect/capability
  system must satisfy. This is the substrate's own author-approved design spec, not
  RFC-0001/0002 alone, so this document goes beyond the "otherwise proceed from
  RFC-0001/0002 + issue #3 alone" fallback the task allowed — the richer source was
  available and was used.

Where the substrate design says something praxis-lang doesn't yet have a mechanism
for, this RFC says so explicitly rather than inventing a resolution.

---

## 1. Non-Goals for This Slice

Per the sequential-RFC discipline (RFC-0001 §8, RFC-0002 §7), this RFC keeps its scope
to the smallest slice that gives `.px` a checkable effect/capability boundary:

- **No new expression syntax.** Effect declarations attach to `function`/procedure
  headers; they do not introduce new operators or literal forms in `Expr`.
- **No policy/relational inference.** Deciding *whether* a capability grant is
  policy-permitted (org rules, RBAC, time-of-day, environment) is RFC-0004's job
  (policy + relational inference). RFC-0003 defines the *shape* of effects and
  capabilities and the *mechanical* fail-closed check that a call's declared effect
  is covered by the context's granted capability set — not the business rules for
  *deciding* grants.
- **No durable-workflow semantics.** How a long-running procedure's granted
  capabilities persist/resume across a durable execution is RFC-0005's concern.
- **No bounded-verification/model-checking of effect reachability.** Proving that a
  procedure graph *cannot* reach an undeclared effect under all inputs is RFC-0006's
  concern (bounded verification). RFC-0003 defines the runtime check, not a static
  prover.
- **No change to `FunctionRegistry`'s Rust signature.** The existing trait
  (`crates/px-eval/src/registry.rs`) already IS the effect seam at the host-integration
  level; this RFC types and enforces what crosses that seam from the `.px` side, it
  does not redesign the trait itself. (A capability-aware registry wrapper is an
  implementation detail for the follow-on PR, not fixed here — see §7.)
- **No capability delegation/attenuation algebra** (e.g. "grant a strictly narrower
  capability to a callee") beyond the simple lexical-inheritance model in §4. Anything
  more expressive (partial capabilities, time-boxed grants, revocation) is deferred;
  §8 lists it as a candidate for a later RFC or an amendment.

---

## 2. Effect Type Syntax

### 2.1 The `Effect` set

A new closed enum, `Effect`, naming the same capability classes already required by the
procedure-graph substrate's security model (§13.1 of the substrate design doc) and
narrowed to what a `.px` procedure/function can itself request:

```rust
// crates/px-ast/src/effects.rs (illustrative — not shipped by this RFC)
pub enum Effect {
    DbRead,
    DbWrite,
    Network,
    Shell,      // process.spawn — matches substrate's process.spawn:<tool>
    FileRead,
    FileWrite,
    EnvRead,
    Clock,      // matches substrate's clock.read
    Random,     // matches substrate's random.read
}
```

This is a deliberately small, closed set for this slice — matching the substrate's
`13.1 Capability classes` list (`graph.read`, `graph.write`, `projection.read`,
`projection.write:<root>`, `blob.read:<hash>`, `process.spawn:<tool>`,
`network.connect:<host>`, `environment.read:<var>`, `secret.read:<secret>`,
`clock.read`, `random.read`) collapsed to the coarser classes `.px` itself needs to
express at the language level (`db-read`/`db-write`/`network`/`shell`/`file`, per the
task's own framing, plus `env-read`/`clock`/`random` since the substrate's model
requires them and `px-eval`'s `PureFunctionRegistry` already treats time/randomness as
host-injected, not pure). **Parameterized capabilities** (a specific host, a specific
secret name, a specific filesystem root) are explicitly out of scope for this slice —
see §8; `Effect` names a *class*, not an instance-scoped grant. `secret.read` itself is
deferred: it is capability-shaped but not yet exercised by any `.px` construct, and
adding it here without a concrete consumer would be scope creep for this slice.

### 2.2 Declaring effects on a function or procedure

New optional `effects:` field on `FunctionDecl` and `DataflowProcedureDecl` /
`LegacyProcedureDecl` (illustrative, not shipped by this RFC):

```rust
// crates/px-ast/src/constructs.rs (illustrative — not shipped by this RFC)
pub struct FunctionDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub mode: Option<FunctionMode>,
    pub docstring: Option<String>,
    /// NEW: the effects this function may perform. Absent/empty = pure.
    pub effects: Vec<Effect>,
    pub span: Span,
}
```

Surface syntax sketch (illustrative, for the follow-on RFC to finalize against the
grammar-gen pipeline; not proposed as final grammar text here):

```
function fetch_user(id: string) -> UserRecord:
  mode: deterministic
  effects: [db_read]
  """Fetch a user record by id."""

function notify_slack(channel: string, body: string) -> bool:
  effects: [network]
  """Post a message to a Slack channel."""

procedure sync_inventory:
  effects: [db_read, db_write, network]
  params: [$warehouse_id]
  given: "Reconcile local inventory against the supplier API"
  ...
```

A declaration with **no `effects:` field, or `effects: []`, is pure** — the same
meaning `px-eval`'s `PureFunctionRegistry` already gives to its builtins today. This
default is deliberate and matches substrate §3.4 ("Procedures are declarative by
default... Default: pure, deterministic... capability-free... Exceptional (must be
declared): network, process execution, environment access, secrets, writes outside
projection root").

### 2.3 Effects on calls, not just declarations

A `step_call` (`crates/px-grammar/src/grammar.pest`) that invokes a function/procedure
inherits that callee's declared effect set into its own caller's **inferred effect
set** — see §4. This RFC does not add new call-site syntax; effect propagation is
computed, not annotated per call-site, keeping this slice's grammar diff to the two
declaration-header additions above.

---

## 3. Capability Declaration and Inheritance Model

### 3.1 Capability vs. Effect

An **Effect** (§2) is a static, declared property of a `.px` construct: "this function
may perform a DB write." A **Capability** is a *runtime* grant: "this execution context
is allowed to actually perform DB writes, right now." The relationship is exactly the
one WASI and the substrate's §13 already establish: declaring an effect is not
performing it, and performing it requires an active, granted capability — no ambient
authority. `Effect` is checked at what this RFC calls **effect-inference time**
(structural, before execution); `Capability` is checked at **call time** (dynamic, per
invocation) — see §5.

### 3.2 Capability grant shape

A capability grant names one `Effect` variant plus an optional scope qualifier (a
string, opaque to this RFC — e.g. a table name for `db_write`, a host pattern for
`network`, a path prefix for `file_write`). This directly maps to the substrate's own
`CapabilityGrant` graph node (§4.1/§4.2 of the substrate design: `Procedure REQUIRES
CapabilityGrant`) and its qualified capability classes (`projection.write:<root>`,
`process.spawn:<tool>`, `network.connect:<host>`):

```rust
// crates/px-ast/src/effects.rs (illustrative — not shipped by this RFC)
pub struct CapabilityGrant {
    pub effect: Effect,
    /// Opaque scope qualifier, e.g. "orders_table", "api.example.com", "/data/imports".
    /// None = unscoped (grants the whole Effect class).
    pub scope: Option<String>,
}
```

This RFC does not define scope-qualifier syntax/matching rules (glob vs. exact vs.
prefix) — that is left to the follow-on implementation PR as a design decision, flagged
explicitly in §7 so it isn't silently decided by whoever writes the code.

### 3.3 Where capabilities are granted

Capabilities are granted **outside** the `.px` source, by whatever host embeds the
evaluator — this matches both `px-eval`'s existing `FunctionRegistry` seam (the host
already decides what functions exist and what they do) and the substrate's §13.2
default-checkout policy (workspace/checkout-scoped grants, not source-scoped grants).
Concretely, this RFC proposes the grant set is threaded through evaluation as a new
`CapabilitySet` value the host constructs and passes alongside the existing
`FunctionRegistry` — not embedded in the `.px` document itself. A `.px` file can
*declare* what it needs (§2); it cannot *grant itself* the right to do it. This is the
core fail-closed property (§5).

### 3.4 Inheritance model

Within a single evaluation:

1. A **procedure's granted capability set** is fixed for the whole procedure execution
   (matches substrate §13.3's per-workspace/per-extension trust-level granularity —
   capabilities are not renegotiated mid-procedure in this slice).
2. A procedure step that calls another `.px` function/procedure does **not** get a
   wider capability set than its caller — capabilities only ever narrow or stay equal
   down a call chain, never widen. This is a strict lexical-inheritance rule: callee's
   effective capability set = caller's granted set (no per-callee escalation
   mechanism in this slice — see §8 for attenuation/delegation as future work).
3. A function/procedure's **declared effects** (§2) must be a subset of what its
   deepest possible call chain could need — i.e., declared effects are meant to be an
   honest upper bound a reviewer can read off the header, not a hint. Whether this is
   enforced statically (walking the call graph at compile time) or left to
   effect-inference (§4) is decided by whichever mechanism proves cheaper to implement
   in the follow-on PR; this RFC requires only that *some* mechanism catches a
   declared-effects/actual-call mismatch, not which one.

This inheritance rule is exactly the substrate's "no ambient authority" principle
(§13.2/§13.3) restated for nested `.px` calls rather than nested filesystem/process
scopes — the substrate's checkout-level policy and this RFC's call-chain-level policy
are the same shape at two different levels of the stack.

---

## 4. How a `.px` Procedure Declares Required Effects

A procedure or function declares required effects via the `effects:` field (§2.2). The
**effect-inference** step (performed once, structurally, not per-execution) computes,
for every declaration in a `.px` document:

```
inferred_effects(D) = declared_effects(D) ∪ ⋃ { inferred_effects(callee) : D calls callee }
```

- If `D` declares `effects: [db_read]` but structurally calls another function declared
  `effects: [network]`, `inferred_effects(D)` includes `network` even though `D`'s own
  header only listed `db_read`. This is a **compatibility problem to surface at
  compile/lint time** (an under-declared effect), not a runtime capability failure —
  the follow-on PR decides whether this is a hard error or a warning; this RFC requires
  it be *detectable*.
- Declaring effects the callee never actually needs (over-declaration) is not an error
  in this slice — it is conservative and safe, matching the substrate's principle that
  more explicit is better than less (§3.7 "Fidelity loss is explicit" / §13.3
  "Capability changes must appear in review diffs").
- A pure declaration (`effects: []` or absent) that structurally calls anything with a
  non-empty declared-effect set is always an under-declaration and MUST be flagged —
  this is the mechanical enforcement of substrate §3.4's default-pure rule.

This computation is purely structural (walks the call graph in the AST/lowered form);
it requires no execution and no host. It is the RFC-0003 equivalent of RFC-0001's
"lowering must be mechanical" requirement (RFC-0001 §2) — effect inference here is
likewise a total, deterministic function over the AST.

---

## 5. Runtime Fail-Closed Capability Enforcement

At call time (actual evaluation, not effect-inference), enforcement is a single
mechanical check inserted at the existing `FunctionRegistry` seam:

```
for each host-effectful call (name, args) reaching FunctionRegistry::call:
  effect = effect_of(name)               // looked up from the callee's declared effects
  if effect is Some(e) and e not in current_capability_set:
      reject with CapabilityDenied(e)    // fail CLOSED — the call never executes
  else:
      proceed to FunctionRegistry::call as today
```

Three properties this RFC requires of that check, directly answering the task's
"fail-closed by default" requirement:

1. **Default deny.** If the evaluation context supplies no `CapabilitySet` at all
   (e.g. an embedder that hasn't opted in to this RFC's mechanism yet), **every**
   non-empty-declared-effect call is denied, not allowed. This matches substrate §13.2's
   explicit default-denied list (network, arbitrary subprocesses, writes outside
   workspace, ambient env reads, secret access) and `px-eval`'s existing behavior where
   `EmptyRegistry` denies everything by construction — this RFC generalizes that same
   "nothing works until explicitly wired" posture from "no functions registered" to
   "no capabilities granted."
2. **Deny before side effect, not after.** The check happens *before*
   `FunctionRegistry::call` executes the underlying host function — an unauthorized
   `network` call must never reach the network, even partially. This is why the check
   is described as sitting at the seam (wrapping `FunctionRegistry`), not inside
   individual host function implementations, which would require every host author to
   remember to self-check.
3. **Undeclared effects are also denied, not merely under-declared-and-flagged.** §4's
   effect-inference is a *compile/lint-time* signal for authors; §5's runtime check is
   independent and does not trust that signal — even if effect-inference was skipped or
   its warning ignored, an actual call to something requiring `Effect::Network` at
   runtime, with no `network` capability granted, is rejected regardless of what the
   declaration said. This double-layer (structural warning + runtime hard deny) is
   deliberate: it is the same "residual/defense-in-depth" posture substrate §21 uses for
   its own risk list ("Checkout executes malicious procedures — mitigation: pure
   materialization subset, explicit capabilities, isolated workspaces").

This RFC does not specify the Rust error type/wrapper shape for `CapabilityDenied` (that
is an implementation decision for the follow-on PR, consistent with RFC-0002 §6 item 4's
precedent of deferring an implementation-shape decision explicitly rather than fixing it
in the spec RFC).

---

## 6. How This Satisfies the Procedure-Graph Substrate's Needs

Mapping directly to the substrate design doc sections named in the task:

- **§4.3 (Identity types) — `ExactArtifactProcedure`:** the substrate's exact-artifact
  import path (§5.2 procedure taxonomy) is explicitly required to be
  **capability-free/pure by default** per §3.4 — an imported byte-for-byte artifact
  procedure has no business performing network/shell/db-write effects just to exist as
  a graph node. RFC-0003's default-pure rule (§2.2, §5 property 1) gives the substrate
  exactly this for free: an `ExactArtifactProcedure`'s `.px` representation, if/when one
  is generated, declares no `effects:` field and is therefore capability-free by
  construction, matching §3.4/§13.2's checkout allow-list ("read canonical procedures,
  read declared blobs, construct in-memory graph entities, write declared artifacts
  inside isolated workspace, run pure transformations" — all effect-free or narrowly
  `db_read`/scoped `file_write` under this RFC's model).
- **§13.1 (Capability classes)** maps near-1:1 onto this RFC's `Effect` enum (§2.1),
  collapsed to the coarser set `.px`-level declarations need; the substrate's more
  granular instance-scoped classes (`network.connect:<host>`, `process.spawn:<tool>`,
  `blob.read:<hash>`) are exactly what this RFC's `CapabilityGrant.scope` qualifier
  (§3.2) is designed to carry — the substrate can express `network.connect:api.x.com`
  as `CapabilityGrant { effect: Network, scope: Some("api.x.com") }` without any change
  to this RFC's shape.
- **§13.2 (Default checkout policy)** is exactly RFC-0003 §5 property 1 (default deny)
  applied at the workspace/checkout granularity rather than the per-call granularity —
  the same fail-closed posture, two altitudes of the same stack.
- **§13.3 (Extension trust levels / capability changes visible in review diffs)** is
  satisfied structurally: because effects are declared in the `.px` header text itself
  (§2.2), any diff that adds/widens an `effects:` list is visible in the same
  **procedure diff** and **semantic diff** views the substrate already defines (§11) —
  no separate capability-review tooling is needed; it falls out of `.px` being
  human-readable canonical text (RFC-0001's whole premise) plus this RFC's choice to put
  effects in the surface syntax rather than in a side-channel metadata file.
- **C-PLURES-004 spine architecture (pure logic in PluresDB/.px, side effects only at
  declared boundaries):** this is precisely §5's runtime enforcement model — `.px`
  procedures are pure by default (§2.2/§5) and the ONLY place a side effect can occur
  is a call that both (a) declares its effect in a header and (b) is granted the
  matching capability by the host at the `FunctionRegistry` seam (§3.3, §5). There is
  no path for a side effect to occur outside a declared boundary — the boundary IS the
  capability check, and it is fail-closed (§5 property 1).

---

## 7. Open Implementation Decisions (deferred to the follow-on PR, not fixed here)

Per the RFC-0001/RFC-0002 precedent of separating spec from implementation, the
following are explicitly **not** decided by this RFC and are binding scope for whoever
writes the implementation PR:

1. **Scope-qualifier syntax and matching** (§3.2) — exact string match, glob, or
   structured sub-fields (host + port, path prefix depth) is an implementation choice.
2. **Whether effect-inference under-declaration (§4) is a hard compile error or a
   lint warning** — this RFC requires it be *detectable*, not which severity it gets.
3. **The concrete `CapabilitySet`/`CapabilityDenied` Rust types and how they thread
   through the existing `FunctionRegistry` trait** (§5) — whether via a wrapping
   registry, a new trait method, or a separate checked-call entry point ahead of
   `FunctionRegistry::call`.
4. **Exhaustive-match audit** of any existing `match` over `TypeExpr`/`Statement`-like
   enums in `pluresdb-px`, `pares-radix`, `pares-agens` that would need a new arm for
   `Effect`/`CapabilityGrant`, per the RFC-0002 §5 precedent — must be grepped and
   either patched or converted to non-exhaustive handling before the implementation PR
   merges.
5. **Whether `secret.read` (substrate §13.1) is added to the `Effect` enum now or in a
   later amendment** — deferred in §2.1 because no `.px` construct exercises it yet;
   the follow-on PR should revisit once a concrete consumer exists.
6. **Reconciliation with `ADR-0038` in `pares-radix`, if/when it is located** — §0.1
   notes this RFC could not find that ADR in the `pares-radix` working tree at the time
   of writing. If it exists elsewhere and specifies a capability model in conflict with
   §2–§5 here, that reconciliation is follow-on work, not resolved by this document.

---

## 8. Deferred / Future-RFC Candidates

Not proposed here, but flagged so they are not silently forgotten:

- **Capability attenuation/delegation** — granting a callee a strictly narrower
  capability than the caller holds (rather than this slice's flat lexical-inheritance
  rule, §3.4 item 2). Useful once `.px` procedures start composing untrusted
  third-party procedures (substrate §9.4 external-contribution mode).
- **Time-boxed / revocable grants** — a capability valid only for the duration of one
  call or one durable-workflow step (interacts with RFC-0005, durable workflow
  semantics).
- **Policy-driven grant decisions** — RFC-0004 (policy + relational inference) is the
  natural home for *deciding* what capabilities a given principal/context should
  receive; this RFC only defines the shape of a grant and how it's checked once made.
- **Parameterized/instance-scoped `Effect` variants as first-class AST, not just an
  opaque `scope` string** — e.g. `Effect::NetworkConnect(HostPattern)` instead of
  `Effect::Network` + `scope: Option<String>`. Kept opaque in this slice to match
  RFC-0002's own "smallest blast radius" discipline; can be tightened later without
  breaking this RFC's core shape.

---

## 9. Acceptance Criteria

RFC-0003 (this design) is **ratified** when:

1. The `Effect` enum (§2.1, nine variants: `db_read`, `db_write`, `network`, `shell`,
   `file_read`, `file_write`, `env_read`, `clock`, `random`) is agreed as the entire
   scope of this slice's effect classes — no parameterized/instance-scoped variants,
   no `secret.read` yet (§7 item 5).
2. The `effects:` field addition to `FunctionDecl`/`DataflowProcedureDecl`/
   `LegacyProcedureDecl` (§2.2) and the `CapabilityGrant` shape (§3.2) are agreed as
   the entire AST surface this RFC proposes.
3. The default-pure, fail-closed, deny-before-side-effect, and
   undeclared-effects-always-denied properties (§5) are accepted as binding invariants
   for the follow-on implementation.
4. The lexical-inheritance-only capability-narrowing rule (§3.4) is accepted, with
   attenuation/delegation explicitly deferred (§8).
5. The companion `.px` file parses green against the **current** grammar (self-hosting
   proof — it does not and cannot yet use `effects:`, since that doesn't exist until
   implementation, exactly as RFC-0002 §6 item 3 required for its own companion file).
6. The open-implementation-decisions list (§7) is accepted as binding scope for the
   follow-on implementation PR(s), out of scope for this RFC's own diff.

**No code changes ship under RFC-0003.**

---

## 10. Sequencing

Per RFC-0001 §8 / RFC-0002 §7, RFCs are strictly sequential:

1. RFC-0001 — semantic core (ratified, merged, spec-only).
2. RFC-0002 — structural/refinement types (Draft in-tree; implementation status not
   independently confirmed by this document — see §0.1).
3. **RFC-0003 — this document.** Design-only. On ratification, a separate
   implementation PR (or PR series) lands the `Effect` enum, `effects:` field,
   `CapabilityGrant` type, effect-inference pass, and the `FunctionRegistry`-seam
   fail-closed check — gated on this design being accepted, tracked as its own
   follow-on work item, not part of this RFC's diff.
4. RFC-0004 — policy + relational inference (query engine may be a new `px-query`
   crate) — decides *how* capability grants are policy-derived; unaffected by this
   slice's mechanics.
5. RFC-0005 — durable workflow semantics — decides how granted capability sets persist
   across durable execution/resume; unaffected by this slice.
6. RFC-0006 — bounded verification — may statically prove effect-inference (§4)
   reachability properties; unaffected by this slice's runtime mechanism.
