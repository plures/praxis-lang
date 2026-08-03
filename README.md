# praxis-lang

**The single source of truth for the `.px` Praxis Intent Language.**

Rust-first. One canonical grammar + AST. Schema auto-regenerated from the AST on
every release (CI-enforced). Rust and YAML authoring surfaces over the same AST.
Broad language support via NAPI.

> Status: **v0.1.0 released** ([releases](https://github.com/plures/praxis-lang/releases)).
> Consolidation epic (M0-M8) is complete: this repo is the single source of
> truth for the `.px` language, replacing the sprawl previously spread across
> four repos (`praxis`, `pluresdb`, `pares-radix`, `pares-agens`). See
> [`docs/epic/ADR-praxis-lang-single-source-of-truth.md`](docs/epic/ADR-praxis-lang-single-source-of-truth.md)
> for the design and [`docs/epic/PRAXIS-LANG-TRACKER.md`](docs/epic/PRAXIS-LANG-TRACKER.md)
> for full migration history.
>
> Rust crates are release-validated in CI with `cargo publish --dry-run` for
> the dependency root and `cargo package --workspace` for the coordinated
> workspace release, pending a `CARGO_REGISTRY_TOKEN` repo secret; the
> NAPI/npm package (`@plures/px-napi`) is likewise `npm publish --dry-run`-
> verified pending an `NPM_TOKEN` secret. See `.github/workflows/release.yml`
> for exact status.

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
