# M3 WAVE 2 — px-compiler + px-eval fold (praxis-lang epic)

**Worker milestone file.** Update the checkboxes as you complete each. Main session reads this to track you.

**Branch:** `m3-wave2-compiler-eval` (worktree `C:\Projects\praxis-lang-m3w2`). Base = `f787b4b` (origin/main, M3 Wave 1).\n**Repo:** local clone `C:\Projects\praxis-lang`; origin `plures/praxis-lang`.\n**Autonomy:** full. Commit frequently with `[praxis-lang epic]` milestone-coded messages. Do NOT open a PR / do NOT push to main — commit to the branch and push the BRANCH; main session adjudicates the merge.

## Objective
Turn `px-compiler` and `px-eval` from empty stubs into real crates by folding in the best of the existing implementations, over the canonical `px-ast` + `px-grammar` already in this repo. Get real `.px` source parsing end-to-end.

## Source material (READ these; port-and-adapt, do NOT blind-copy — they drag pluresdb-px-internal deps)
- Parser/compiler: `C:\Projects\pluresdb\crates\pluresdb-px\src\px\compiler.rs` (19KB) — pest-based parse → px-ast build.\n- Evaluator (praxis-native): `C:\Projects\praxis\crates\praxis-native\src\px\eval.rs` (42KB) — the cleaner evaluator; PREFER this for px-eval.\n- Executor (large): `C:\Projects\pluresdb\crates\pluresdb-px\src\px\executor.rs` (261KB) + `dataflow.rs` (37KB) — runtime semantics. Port SELECTIVELY: constraint eval, rule eval, expression eval, function/procedure dispatch. Leave PluresDB-storage-coupled effect execution OUT (that's downstream M6, not the language core). If a piece needs a PluresDB type, define a minimal trait seam in px-eval and leave the storage impl to the consumer — do NOT pull pluresdb crates in.

## Gates (ALL must pass before you report DONE)
- [ ] **px-grammar Parser binding:** a real `pest::Parser` derive (or equivalent) over `px_grammar::GRAMMAR_PEST`, exposed so px-compiler can parse. (px-grammar currently only embeds the string + parity-tests it.)
- [ ] **px-compiler:** parse `&str` → `px_ast` `Program`/`Statement` tree. Port compiler.rs's pest→AST construction. Public API e.g. `px_compiler::parse(src: &str) -> Result<Program, CompileError>`.
- [ ] **px-eval:** evaluate expressions + rules + constraints over an AST + a fact/context model. Port from praxis-native eval.rs (preferred) — expression evaluation, rule firing, constraint check with severity. Minimal trait seam for any storage/effect boundary; NO pluresdb dep.
- [ ] **Examples parse:** add `examples/*.px` (at least 3: an entity+fact+rule file, a constraint file, a procedure file) and a test that parses ALL of them green via px-compiler. Use REAL constructs from the grammar (see `crates/px-grammar-gen/src/fragments/*.pest` for exact syntax).
- [ ] **No stubs (C-NOSTUB-001):** no `todo!()`/`unimplemented!()`/canned returns in shipped paths. If a construct's eval isn't ported this wave, it must be ABSENT (not declared) or return a real `Unsupported` error the caller handles — and note it in the report as "not yet ported" honestly.
- [ ] **Local green:** `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` ALL pass with zero warnings (toolchain 1.96.1).
- [ ] **Drift gate intact:** `verify-grammar` still green (don't touch generated grammar.pest by hand; if you regenerate, use `cargo run -p px-grammar-gen -- <path>`).
- [ ] Push the branch `m3-wave2-compiler-eval` to origin. Report result to `WORKTASK`-style result file below.

## Report
Write `C:\Users\kbristol\.openclaw\workspace\epic-praxis-lang\PXLANG-M3W2-RESULT.md` with: what you ported vs. left out (and why), the public APIs, which constructs evaluate vs. return Unsupported, gate results (paste the cargo test summary), branch HEAD sha, and any honest gaps. Do NOT claim green you didn't run.

## Anti-patterns
- ❌ Pulling `pluresdb` / `pluresdb-px` as a dependency (that's the thing we're replacing).
- ❌ Copying executor.rs wholesale (it's 261KB of PluresDB-coupled runtime; take the language-eval parts only).
- ❌ Stubbing a construct's eval and calling it done.
- ❌ Pushing to main or opening+merging a PR yourself.
