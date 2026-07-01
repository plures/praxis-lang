# praxis-lang

**The single source of truth for the `.px` Praxis Intent Language.**

Rust-first. One canonical grammar + AST. Schema auto-regenerated from the AST on
every release (CI-enforced). Rust and YAML authoring surfaces over the same AST.
Broad language support via NAPI.

> Status: **early skeleton (epic M1)**. This repo is being assembled by
> consolidating the `.px` language that was previously spread across four repos
> (`praxis`, `pluresdb`, `pares-radix`, `pares-agens`). See
> [`docs/epic/ADR-praxis-lang-single-source-of-truth.md`](docs/epic/ADR-praxis-lang-single-source-of-truth.md)
> for the design and [`docs/epic/PRAXIS-LANG-TRACKER.md`](docs/epic/PRAXIS-LANG-TRACKER.md)
> for live migration status.

## Crate layout

| Crate | Role |
|-------|------|
| `px-ast` | Canonical AST — the language spec ("if it's not here, it isn't in the language") |
| `px-grammar-gen` | Fragments → `grammar.pest` generator (grammar is generated, never hand-edited) |
| `px-grammar` | The generated grammar + pest parser binding |
| `px-schema` | Schema types + JSON-Schema emitter (projection of `px-ast`) |
| `px-schema-derive` | Derive macros for the schema layer |
| `px-compiler` | Compiler: `.px` text → AST → IR |
| `px-eval` | Expression evaluator + constraint-engine primitives |
| `px-yaml` | YAML surface ↔ `px-ast` (round-trip, no second source of truth) |
| `px-napi` | NAPI-RS bindings for Node/TS consumers |

## Build

```
cargo build
cargo test
```

## License

MIT — see [LICENSE](LICENSE).
