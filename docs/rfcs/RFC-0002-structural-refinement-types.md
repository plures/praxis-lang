# RFC-0002 — Structural/Refinement Types (Smallest Slice)

- **Status:** Draft
- **Workstream:** WS-1 (language evolution)
- **Author:** kbristol
- **Companion spec:** [`RFC-0002-structural-refinement-types.px`](./RFC-0002-structural-refinement-types.px) (self-hosting proof, current syntax only)
- **Gates on:** RFC-0001 (`docs/rfcs/RFC-0001-semantic-core.md`), ratified, merged `plures/praxis-lang#4`
- **Governance:** `docs/epic/ADR-praxis-lang-single-source-of-truth.md`, `docs/epic/PRAXIS-LANG-TRACKER.md`

> **This RFC is specification-only.** No AST, grammar, evaluator, or projection code
> changes ship with it. It is the *design* for the first implementation RFC named in
> RFC-0001 §8 ("RFC-0002 — structural/refinement types (first code change; smallest
> blast radius; litmus test for the core model)"). Implementation is a separate,
> follow-on piece of work gated on this RFC being ratified.

---

## 0. Context

RFC-0001 ratified four semantic-core primitives — **Declaration**, **Assertion**,
**Procedure**, **Scenario** — and a mechanical, reversible lowering for all 12 current
surface constructs. It deliberately shipped no code. RFC-0002 is the "litmus test": the
first RFC that actually changes `px-ast`/`px-grammar`/`px-compiler`, chosen specifically
because it is the **smallest possible slice** that exercises the lowering model end to
end without touching effects, policy, workflows, or verification (those are RFC-0003+).

Today `TypeExpr` (`crates/px-ast/src/types.rs`) has six variants: `Base`, `Named`,
`List`, `Optional`, `Map`, `Enum`. Every field, param, and return type in the language is
one of these. There is no way to say "an `int` that must be `> 0`" or "a `string` that
matches a set of allowed prefixes" without pushing that check into a separate
`constraint` block disconnected from the field declaration itself. Authors currently
duplicate intent: the field says `amount: int`, and somewhere else a constraint says
`amount > 0`. There is no structural link between the two, and no reuse of a validated
shape across multiple entities/facts/functions.

**Structural typing**, in the narrow sense this RFC scopes, means: a named type alias
whose identity is defined by its *shape* (base type + refinement), not by where it's
declared — so `PositiveInt` used in `entity Order` and `entity Payment` is the same type,
compared structurally, not nominally-per-entity.

**Refinement typing**, in the narrow sense this RFC scopes, means: attaching a
boolean predicate (reusing the existing v1 `Expr` grammar — no new expression syntax)
to a base type, such that a value of that type is only valid if the predicate holds.

## 1. Non-Goals for This Slice

To keep blast radius minimal, RFC-0002 explicitly does **not** attempt:

- **Anonymous/inline structural record types** (e.g. `{name: string, age: int}` as a
  type position). Only a *named alias* over an existing `TypeExpr` gets a refinement.
  Anonymous structural records are deferred to a later RFC if ever needed.
- **Refinements on `Named` (user entity/fact) types.** Only the five existing scalar/
  collection `TypeExpr` variants (`Base`, `List`, `Optional`, `Map`, `Enum`) may be
  refined in this slice. Refining a `Named` type recursively is deferred.
- **Dependent types / refinements referencing other fields.** The refinement predicate
  in this slice may only reference the bound value itself (conventionally `value`, see
  §3). Cross-field refinements (`end_date > start_date`) stay in `constraint`/`contract`
  as today.
- **New expression syntax.** The refinement predicate reuses v1 `Expr` verbatim
  (already used by `rule`/`constraint`/`contract`). No new operators, no new literal
  forms.
- **Runtime/evaluator behavior change.** This RFC defines the AST/grammar/schema shape.
  Whether/when refinements are *checked* (parse-time constant-folding vs. eval-time
  guard vs. purely advisory) is an implementation decision for the follow-on code RFC's
  own PR, not fixed here — see §6.

## 2. Proposed AST Change (design, not code)

One new `TypeExpr` variant and one new top-level declaration:

```rust
// crates/px-ast/src/types.rs (illustrative — not shipped by this RFC)
pub enum TypeExpr {
    Base(BaseType),
    Named(Ident),
    List(Box<TypeExpr>),
    Optional(Box<TypeExpr>),
    Map(Box<TypeExpr>, Box<TypeExpr>),
    Enum(Vec<Ident>),
    /// NEW: a base/collection type narrowed by a boolean predicate over `value`.
    Refined { base: Box<TypeExpr>, predicate: Box<Expr> },
}
```

```rust
// crates/px-ast/src/constructs.rs (illustrative — not shipped by this RFC)
/// `type <Name> = <TypeExpr> [where <Expr>]`
pub struct TypeAliasDecl {
    pub name: Ident,
    pub aliased: TypeExpr,
    pub span: Span,
}
```

Surface syntax sketch (illustrative, for the follow-on RFC to finalize against the
grammar-gen pipeline — not proposed as final grammar text here):

```
type PositiveInt = int where value > 0
type NonEmptyString = string where length(value) > 0
type Percentage = float where value >= 0.0 and value <= 100.0

entity Payment:
  prefix: "pay"
  fields:
    amount: PositiveInt
    note: NonEmptyString
```

`TypeAliasDecl` is a 13th surface construct. Per RFC-0001's table, it **lowers to
Declaration** — same primitive as `entity`/`fact`/`config`/`function`/`import` — with no
change to the core-primitive count or the lowering model. This is precisely what makes
it the right litmus test: it proves a *new* surface construct can be added without
widening the semantic core.

## 3. Refinement Predicate Binding

The refinement predicate is an ordinary v1 `Expr` with exactly one free variable, bound
by convention to the identifier `value` (parsed as `Expr::Var(VarRef)` already supported
today, no grammar change to `Expr` itself). Scoping rule: `value` refers to the value
being checked against the type; no other variables are in scope. This keeps the
refinement a pure, single-argument predicate — deliberately weaker than general guards
in `rule`, so it can be reused as a *type*, not just a one-off check.

## 4. Lowering (extends the RFC-0001 table)

| Surface construct | Lowers to | Reversible |
|---|---|---|
| `type` (`TypeAliasDecl`) | Declaration (named type binding, refinement carried as an attached predicate) | yes |

No existing row in the RFC-0001 table changes. `Refined` is a `TypeExpr` variant, not a
new declaration kind on its own — in this slice it becomes observable at the surface only via
`type` (`TypeAliasDecl`), and it still lowers through the `Declaration` primitive that owns type
annotations today.

## 5. Compatibility, Projection, Downstream-Transparency

Same discipline RFC-0001 established, restated for this slice:

- **Every existing `.px` file MUST continue to parse unchanged.** `Refined` and
  `TypeAliasDecl` are additive grammar productions; no existing production's syntax
  changes. Enforced by the existing `examples/*.px` parse suite plus a new
  `no_breaking_parse` assertion in the companion `.px` (§7).
- **JSON Schema / YAML / NAPI projections stay CI drift-gated** (`px-schema`, `px-yaml`,
  `px-napi`, ADR-0021 pipeline). A `TypeExpr::Refined` variant is a new tagged-enum case;
  the schema projection gate will simply pick it up on regeneration — no manual schema
  edits, per existing discipline.
- **Downstream-transparency invariant (RFC-0001 §6) still binds.** `pluresdb-px`,
  `pares-radix`, `pares-agens` must observe no *forced* change: they already pattern-match
  on `TypeExpr`, and adding a variant is source-compatible for consumers using `..`/
  non-exhaustive matches, but is a **breaking match arm** for anyone doing an exhaustive
  `match` without a wildcard. The follow-on implementation RFC/PR must grep all three
  downstream repos for exhaustive `TypeExpr` matches before landing, and either add the
  new arm or convert to non-exhaustive handling. This is called out explicitly here so it
  is not missed when code is written — see §6, item 4.

## 6. Acceptance Criteria

RFC-0002 (this design) is **ratified** when:

1. The single new `TypeExpr::Refined` variant and single new `TypeAliasDecl` construct
   above are agreed as the entire scope of the first structural/refinement-type slice —
   no anonymous records, no cross-field/dependent refinements, no new operators.
2. The lowering (`type` → Declaration) is agreed to extend, not widen, the RFC-0001
   core-primitive set.
3. The companion `.px` file parses green against the **current** grammar (self-hosting
   proof that the RFC document itself is expressible in today's syntax — it does not
   and cannot yet use `type`/`where`, since those don't exist until implementation).
4. The following are accepted as binding scope for the follow-on implementation PR(s),
   which are **out of scope for this RFC**:
   - Grammar fragment addition via `px-grammar-gen` only (ADR-0021: grammar is never
     hand-edited).
   - Exhaustive-match audit of `TypeExpr` across `pluresdb-px`, `pares-radix`,
     `pares-agens` before merge (per §5).
   - A decision on *when* refinement predicates are checked (parse-time constant-fold
     for literal-only predicates vs. eval-time guard) — deferred to the implementation
     PR's own design note, not fixed by this RFC.
   - Migration guidance: existing fields keep working unchanged; `type` aliases are
     opt-in sugar, not a required rewrite.

## 7. Sequencing

Per RFC-0001 §8, RFCs are strictly sequential:

1. RFC-0001 — semantic core (ratified, merged, spec-only).
2. **RFC-0002 — this document.** Design-only. On ratification, a separate implementation
   PR (or PR series) lands the `TypeExpr::Refined` variant, `TypeAliasDecl` construct,
   generated grammar fragment, schema/YAML/NAPI projection updates, and the downstream
   exhaustive-match audit — gated on this design being accepted, tracked as its own
   follow-on work item, not part of this RFC's diff.
3. RFC-0003 — effects and capabilities (unaffected by this slice).
4. RFC-0004 through RFC-0006 — unchanged, per RFC-0001 §8.
