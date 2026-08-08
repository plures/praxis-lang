# RFC-0005 — Durable Workflow Semantics

- **Status:** Draft
- **Workstream:** WS-1 (language evolution)
- **Author:** kbristol (design drafted by delegated agent session, 2026-08-08)
- **Companion spec:** [`RFC-0005-durable-workflow-semantics.px`](./RFC-0005-durable-workflow-semantics.px) (self-hosting proof, current syntax only)
- **Gates on:** RFC-0004 (`docs/rfcs/RFC-0004-policy-relational-inference.md`, merged `plures/praxis-lang#29`)
- **Governance:** `docs/epic/ADR-praxis-lang-single-source-of-truth.md`, `docs/epic/PRAXIS-LANG-TRACKER.md`, issue `plures/praxis-lang#3` (WS-1 epic)

> **This RFC is specification-only.** No AST, grammar, evaluator, or projection code
> changes ship with it. RFC-0001 (spec-only), RFC-0003 (spec-only), and RFC-0004
> (spec-only) are the direct precedents. Implementation is separate, follow-on work
> gated on this RFC being ratified.

---

## 0. Context

RFC-0001 ratified four semantic-core primitives (Declaration, Assertion, Procedure,
Scenario). RFC-0002 added structural/refinement types (Declaration). RFC-0003 added
effects and capabilities (boundary between pure and effectful evaluation). RFC-0004
added policy and relational inference (declarative rules that derive facts and decide
outcomes from state). None of these RFCs address **durability**: what happens when a
procedure's execution is interrupted — by a crash, a deploy, a human-approval gate, a
timer, or an explicit `await` on an external event — and must be **resumed** later with
its intermediate state intact.

Today `.px` procedures (`DataflowProcedure`, `LegacyProcedure`) are implicitly
ephemeral: `px-eval` runs them to completion in a single invocation. There is no
language-level concept of:

- **Checkpointing** intermediate state so a procedure can survive process death.
- **Suspending** on an external event (timer, human approval, webhook) and resuming
  when the event arrives.
- **Compensating** already-completed steps when a later step fails (saga pattern).
- **Idempotency** guarantees for steps that may execute more than once after a resume.

These are the defining properties of a *durable workflow* runtime (temporal.io, Azure
Durable Functions, Restate, etc.). RFC-0005 defines what these concepts mean **at the
language level** — as constructs that lower into the existing semantic core — so that a
host-side persistence engine (PluresDB, an event log, or an external service) can
provide durability without the language or its evaluator needing to embed a specific
runtime.

### 0.1 What already exists (grounded, not invented)

- **`px-eval`'s `FunctionRegistry` seam** (RFC-0003 §0): effectful operations are
  already gated behind host-provided function implementations. A `db_write` or
  `http_post` is not built into the evaluator; it's a registry-provided capability.
  Durable-workflow operations (checkpoint, suspend, compensate) are the same kind of
  host-provided capability — this RFC defines their **language-level declaration shape**,
  not their host-side implementation.
- **`pares-radix`'s orchestration model** (`praxis/procedures/`): durable reactive
  `.px` files already describe multi-stage workflows with `orch:stage:` key prefixes,
  subagent lanes, completion gates, and human-approval checkpoints — all expressed in
  today's procedure/config/entity syntax without language support. RFC-0005 does not
  replace this; it gives the pattern first-class language semantics so the compiler can
  verify compensation coverage, idempotency annotations, and suspend-point legality at
  compile time rather than relying on host-side convention.
- **RFC-0004's policy decisions** (§8 "Persisted/durable policy decisions"): whether a
  `decide` outcome is cached across a durable-workflow resume is this RFC's concern.
  RFC-0005 answers that question by defining how **all** intermediate results —
  including policy decisions — interact with checkpoint/resume semantics.

---

## 1. Non-Goals for This Slice

- **No specific persistence engine.** Whether checkpoints are stored in PluresDB, a
  Kafka topic, a local file, or an external durable-execution service is a host
  decision. This RFC defines the language-level checkpoint/suspend/compensate semantics;
  the storage backend is pluggable via `FunctionRegistry` (RFC-0003).
- **No new expression syntax in `Expr`.** Durable-workflow constructs attach to
  procedure-level step annotations and new procedure modifiers (§2, §3), not new
  operators or literal forms.
- **No change to existing procedure evaluation.** A procedure without durable
  annotations continues to evaluate ephemerally exactly as today — durability is
  opt-in per-procedure.
- **No distributed-transaction coordination.** This RFC covers single-workflow
  durability (one procedure's execution across suspends/resumes). Cross-workflow
  coordination (distributed sagas spanning multiple independent workflow instances) is
  explicitly deferred (§8).
- **No change to RFC-0003's effect/capability model.** Durable-workflow operations
  (checkpoint, suspend, compensate) are host-provided capabilities; they use the
  RFC-0003 mechanism, they do not replace it.
- **No static verification that compensation is total or idempotency is correct.**
  Proving these properties is RFC-0006's concern (bounded verification).

---

## 2. Durable Procedure: the `workflow` Modifier

### 2.1 What "durable workflow" means here

A **durable workflow** is a procedure whose execution state survives beyond the
lifetime of a single evaluator invocation. It can:

1. **Checkpoint** — persist intermediate state at defined points so that after a
   failure/restart, execution resumes from the last checkpoint rather than re-running
   from the start.
2. **Suspend** — yield control and wait for an external event (timer expiry, human
   approval, webhook receipt, message arrival) without holding resources; on event
   arrival, resume from the suspend point.
3. **Compensate** — if a step N fails after steps 1…N-1 succeeded (and some had
   side effects), run declared compensation actions for the completed steps in reverse
   order (saga pattern).

These map to RFC-0001's core primitives:

| Durable concept | Core primitive | Rationale |
|---|---|---|
| `workflow` modifier | Procedure | A workflow is a procedure with durability annotations. |
| `checkpoint` step | Procedure (step) | Persisting state is an effectful step within the procedure. |
| `suspend` step | Procedure (step) | Yielding and awaiting is an effectful step. |
| `compensate` block | Procedure (step group) | Compensation is an ordered sub-procedure triggered on failure. |
| Idempotency annotation | Declaration (metadata) | A property declared on a step, like a type annotation. |

### 2.2 Declaring a workflow (illustrative, not shipped by this RFC)

```rust
// crates/px-ast/src/constructs.rs (illustrative — not shipped by this RFC)
pub struct WorkflowDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub steps: Vec<WorkflowStep>,
    pub compensations: Vec<CompensationBlock>,
    pub docstring: Option<String>,
    pub span: Span,
}
```

Surface syntax sketch (illustrative, for the follow-on RFC to finalize against the
grammar-gen pipeline; not proposed as final grammar text here):

```
workflow onboard_new_user(user_id: string):
  effects: [db_write, email_send]
  step create_account:
    idempotent: true
    call: db_write {collection: "users", id: $user_id, data: {status: "pending"}}
    compensate: db_write {collection: "users", id: $user_id, data: {status: "deleted"}}
  checkpoint after_account_created
  step send_welcome_email:
    idempotent: true
    call: email_send {to: $user_id, template: "welcome"}
    compensate: noop
  suspend await_email_verification:
    event: "email_verified"
    timeout: "72h"
    on_timeout: cancel_onboarding
  step activate_account:
    idempotent: true
    call: db_write {collection: "users", id: $user_id, data: {status: "active"}}
    compensate: db_write {collection: "users", id: $user_id, data: {status: "pending"}}
  checkpoint completed
```

### 2.3 Checkpoint semantics

A `checkpoint` is a named point in a workflow's step sequence where the runtime
persists the workflow's accumulated state (completed step results, variable bindings,
position in the step list). After a crash/restart, the runtime resumes from the most
recent checkpoint rather than re-executing completed steps.

Properties:
- Checkpoints are **sequencing barriers** — all steps before a checkpoint must complete
  before the checkpoint is reached.
- A checkpoint's persisted state includes the results of all completed steps and the
  current variable environment.
- Checkpoint persistence is an **effect** (requires a host-provided capability, per
  RFC-0003); the `workflow` modifier implicitly declares this effect requirement.

### 2.4 Suspend semantics

A `suspend` is a step that yields the workflow's execution thread and registers interest
in an external event. The runtime:

1. Persists the workflow state (implicit checkpoint).
2. Releases all held resources (no thread/connection blocked).
3. On event arrival (or timeout), resumes execution at the step following the suspend.

A suspend has:
- **`event`**: the external signal name to await.
- **`timeout`** (optional): maximum duration to wait; if exceeded, the `on_timeout`
  handler is invoked (which may be a compensation/cancel path or a specific step).
- **`on_timeout`** (optional): procedure/step to invoke on timeout.

### 2.5 Compensation semantics (saga pattern)

Each step in a workflow may declare a `compensate` action — the inverse operation to
undo the step's side effect. If a step fails after prior steps have committed side
effects, the runtime invokes compensation actions for all completed steps in **reverse
declaration order** (backward recovery, saga pattern).

Properties:
- Compensation actions are themselves effectful steps and must be idempotent (they may
  run more than once if compensation itself is interrupted).
- A step with `compensate: noop` explicitly declares that no compensation is needed
  (e.g., a read-only or truly idempotent operation).
- The compensation chain is a **language-level declaration**, not a runtime convention —
  this enables static analysis (RFC-0006) to check compensation coverage.

### 2.6 Idempotency annotation

A step annotated `idempotent: true` declares that re-executing it with the same inputs
produces the same observable outcome. This is a **contract** between the workflow author
and the runtime:
- The runtime *may* re-execute idempotent steps after a resume without additional
  deduplication.
- The runtime *must not* re-execute non-idempotent steps after a resume without
  deduplication (e.g., using a step execution ID persisted at the prior checkpoint).

This annotation is a Declaration-level metadata property (lowering to Declaration per
RFC-0001's table), not a runtime enforcement mechanism — enforcement is the host's
responsibility; the language provides the annotation for tooling/verification.

---

## 3. Interaction with Prior RFCs

### 3.1 Effects (RFC-0003)

Durable-workflow operations (checkpoint persistence, suspend registration, compensation
dispatch) are **effects** in RFC-0003's model. A `workflow` declaration implicitly
requires the following capabilities:

- `checkpoint_persist` — write workflow state to durable storage.
- `checkpoint_restore` — read workflow state on resume.
- `suspend_register` — register interest in an external event with the runtime.
- `compensate_dispatch` — invoke compensation actions on failure.

These are host-provided capabilities injected through `FunctionRegistry` exactly as
`db_write` or `http_post` are today. The `workflow` modifier's `effects:` list (§2.2)
makes this explicit at the language level.

### 3.2 Policy (RFC-0004)

RFC-0004 §8 asked: "whether a `decide` outcome, once computed for a given context, is
cached or must be re-evaluated on every durable-workflow resume." RFC-0005's answer:

- Policy decisions computed **before** a checkpoint are part of the checkpointed state
  and are **not re-evaluated** on resume (they are treated as completed step results).
- Policy decisions computed **after** the last checkpoint (in a step that has not yet
  been checkpointed) **are re-evaluated** on resume, because their computation may have
  side effects or depend on state that changed during the failure window.

This is consistent with checkpoint semantics generally: everything before a checkpoint
is committed; everything after is re-executed.

### 3.3 Relational inference (RFC-0004)

Relational query results follow the same rule as policy decisions: results computed
before a checkpoint are checkpointed; results computed after the last checkpoint are
re-derived on resume. Since `infer` rules are pure derivations over declared facts
(RFC-0004 §2.3), re-derivation is safe — it will produce the same results given the
same fact base (which is itself checkpointed).

---

## 4. Compatibility

- **Every existing `.px` file continues to parse.** The `workflow` modifier is a new
  top-level construct kind, not a change to existing procedure/function/entity syntax.
- **Existing procedures remain ephemeral.** Only procedures explicitly declared as
  `workflow` gain durable semantics; existing `DataflowProcedure` and
  `LegacyProcedure` are unaffected.
- **Lowering table additions (illustrative):**

| Surface construct | Core primitive | Notes |
|---|---|---|
| `workflow` | Procedure | A procedure with durability annotations |
| `checkpoint` (step) | Procedure | An effectful step within the procedure |
| `suspend` (step) | Procedure | An effectful yield step |
| `compensate` (step annotation) | Procedure | Sub-procedure for backward recovery |
| `idempotent` (step annotation) | Declaration | Metadata property on a step |

No existing row in RFC-0001's lowering table is modified.

---

## 5. Downstream Transparency

Per the WS-1 downstream-transparency invariant (RFC-0001 §hard-constraints):

- **`pluresdb-px`**: no change required. `WorkflowDecl` (when implemented) would be a
  new `Statement` enum variant; the same exhaustive-match-audit process applies as for
  RFC-0004's `RelationDecl`/`PolicyDecl`. As a spec-only RFC, no enum change ships now.
- **`pares-radix`**: no change required. Existing orchestration procedures
  (`orch:stage:` key-prefix patterns) continue to work; they are a host-side convention
  that `workflow` constructs could optionally formalize later but are not required to
  adopt.
- **`pares-agens`**: no change required.

---

## 6. Self-Hosting Proof

The companion file `RFC-0005-durable-workflow-semantics.px` uses **only** current `.px`
syntax (entity, fact, config, function, constraint, contract, scenario) to model this
RFC's design boundaries and acceptance criteria. It parses green via `px_compiler::parse`
(gated by `crates/px-compiler/tests/rfc_0005_self_hosts.rs`).

It does **not** use `workflow`, `checkpoint`, `suspend`, or `compensate` syntax — those
constructs do not exist until the follow-on implementation RFC lands them. This matches
RFC-0002/0003/0004's discipline for spec-only companion files.

---

## 7. Open Implementation Decisions (deferred to the follow-on PR, not fixed here)

1. **AST representation:** whether `WorkflowDecl` is a new top-level `Statement`
   variant or a modifier/annotation on existing `DataflowProcedure` with additional
   fields.
2. **Grammar-gen integration:** how `workflow`, `checkpoint`, `suspend`, `compensate`,
   `idempotent` keywords are added to `px-grammar-gen` fragment generation.
3. **Checkpoint storage interface:** the trait/capability shape for the host-provided
   checkpoint persistence — whether it's a single `checkpoint_persist(state: bytes)`
   call or a structured interface with named fields.
4. **Suspend/resume wire protocol:** how the host runtime signals event arrival to the
   evaluator — push (callback) vs. pull (poll on resume) — and how event payloads are
   threaded into the workflow's variable environment.
5. **Compensation ordering guarantees:** whether compensation is strictly reverse-order
   or allows host-defined ordering (e.g., parallel compensation for independent steps).
6. **Idempotency key generation:** whether the runtime auto-generates step execution IDs
   or the workflow author provides explicit idempotency keys.
7. **Interaction with `px-eval`'s current single-pass model:** whether durable workflows
   require a new evaluator mode (coroutine/continuation-style) or can be expressed as
   repeated invocations of the existing step-by-step evaluator with externally managed
   state.

---

## 8. Deferred / Future-RFC Candidates

Not proposed here, but flagged so they are not silently forgotten:

- **Distributed workflow coordination** — cross-workflow sagas, two-phase commit across
  independent workflow instances, eventual-consistency protocols between workflows.
- **Workflow versioning** — how a running durable workflow handles a code change
  (new step added, step removed, compensation logic changed) between checkpoint and
  resume.
- **Workflow observability** — standard tracing/metrics integration for checkpoint
  frequency, suspend duration, compensation invocation counts.
- **Nested workflows** — a workflow step that invokes another workflow as a child,
  with parent-child lifecycle coupling (cancel parent → cancel children).
- **Static verification of compensation totality** — proving every effectful step has
  a declared compensation action, and that compensation actions are themselves
  idempotent; this is RFC-0006's concern.

---

## 9. Acceptance Criteria

RFC-0005 (this design) is **ratified** when:

1. The `workflow` modifier shape (§2) — checkpoints, suspends, compensation, and
   idempotency annotations as procedure-level constructs that lower to Procedure — is
   agreed as the entire scope of this slice's durable-workflow capability.
2. §3's interactions with RFC-0003 (effects), RFC-0004 (policy/relational re-evaluation
   on resume) are accepted as the defining semantics for how prior-RFC constructs behave
   across checkpoint/resume boundaries.
3. §4's compatibility guarantee is accepted: existing procedures remain ephemeral;
   `workflow` is purely additive; no existing lowering-table row changes.
4. §5's downstream-transparency conclusion is accepted: no change required in
   `pluresdb-px`, `pares-radix`, or `pares-agens` as part of this RFC.
5. The companion `.px` file parses green against the **current** grammar (self-hosting
   proof — it does not and cannot yet use `workflow`/`checkpoint`/`suspend`/`compensate`,
   matching RFC-0002/0003/0004 discipline).
6. The open-implementation-decisions list (§7) is accepted as binding scope for the
   follow-on implementation PR(s).

**No code changes ship under RFC-0005.**

---

## 10. Sequencing

Per RFC-0001 §8 / RFC-0002 §7 / RFC-0003 §10 / RFC-0004 §10, RFCs are strictly
sequential:

1. RFC-0001 — semantic core (ratified, merged, spec-only, PR #4).
2. RFC-0002 — structural/refinement types (ratified, merged, PR #5 + #8).
3. RFC-0003 — effects and capabilities (ratified, merged, spec-only, PR #21).
4. RFC-0004 — policy + relational inference (ratified, merged, spec-only, PR #29).
5. **RFC-0005 — this document.** Design-only. On ratification, a separate
   implementation PR (or PR series) lands the `workflow`/`checkpoint`/`suspend`/
   `compensate` AST nodes, grammar fragments, and evaluation mechanism — gated on this
   design being accepted.
6. RFC-0006 — bounded verification — may statically prove compensation totality,
   idempotency correctness, policy determinism, and relational-inference termination;
   unaffected by this slice's design.
