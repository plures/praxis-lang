//! px-cheatsheet — a compact, auto-generated `.px` grammar/AST reference.
//!
//! This is a **projection**, not a hand-authored doc (C-DRIFT-001, same
//! pattern as `px-schema`): it walks the JSON Schema produced by
//! `px_schema::projection::build_json_schema()` (itself a pure projection of
//! `px-ast` via `schemars`) to list every top-level `.px` construct and its
//! fields, and pairs each construct with a REAL usage snippet extracted from
//! this repo's own committed `.px` corpus (`examples/*.px`,
//! `docs/rfcs/*.px`) — never an invented example.
//!
//! ## Honest limitations (documented, not hidden)
//!
//! - Only **top-level statement constructs** (the `Statement` enum variants:
//!   entity, config, fact, rule, constraint, contract, function, trigger,
//!   dataflow/legacy procedure, scenario) get a dedicated section. Nested
//!   expression grammar (v1 `expr` / v2 `code_expr` operator precedence,
//!   step-list step kinds) is NOT walkable from the `schemars` JSON-Schema
//!   projection the same way, because `px-ast`'s expression/step enums are
//!   deeply recursive sum types whose JSON Schema shape does not carry a
//!   short display label per variant the way top-level decls do. Those are
//!   summarized from `grammar.pest` rule names directly (still real source,
//!   just a different, coarser projection) rather than from the AST schema.
//! - The example snippet per construct is the **first real match** found by
//!   scanning the corpus for a line starting with that construct's keyword.
//!   If a construct has no real usage anywhere in the corpus yet, the
//!   generator says so explicitly (`(no example found in corpus)`) rather
//!   than fabricating one.
//! - The corpus is limited to files vendored in *this* repo
//!   (`examples/*.px`, `docs/rfcs/*.px`). Cross-repo corpora (e.g.
//!   `development-guide`'s `.px` procedures) were considered but deliberately
//!   excluded from the automated generator: CI here only checks out
//!   `praxis-lang`, so a generator depending on another repo's working copy
//!   could not be regenerated deterministically in this repo's own gate.

use std::collections::BTreeMap;

use px_schema::projection::build_json_schema;
use schemars::schema::{Schema, SchemaObject};

/// One top-level `.px` statement construct, in a fixed, curated display order
/// matching `px_ast::Statement`'s variant order (the actual sum type).
const STATEMENT_CONSTRUCTS: &[(&str, &str, &str)] = &[
    // (Statement variant name, px-ast schema definition name, keyword as it appears in source)
    ("Import", "ImportDecl", "import"),
    ("Entity", "EntityDecl", "entity"),
    ("Config", "ConfigDecl", "config"),
    ("Fact", "FactDecl", "fact"),
    ("Rule", "RuleDecl", "rule"),
    ("Constraint", "ConstraintDecl", "constraint"),
    ("Contract", "ContractDecl", "contract"),
    ("Function", "FunctionDecl", "function"),
    ("Trigger", "TriggerDecl", "trigger"),
    ("DataflowProcedure", "DataflowProcedureDecl", "procedure"),
    ("LegacyProcedure", "LegacyProcedureDecl", "procedure"),
    ("Scenario", "ScenarioDecl", "scenario"),
];

/// A pinned, hand-curated summary of the grammar-level (not AST-level) forms
/// that `grammar.pest` documents but the AST JSON-Schema projection cannot
/// enumerate cleanly (see module docs). Sourced directly from
/// `crates/px-grammar/src/grammar.pest` rule names — real source, coarser
/// projection than the AST walk above.
const STEP_AND_EXPR_FORMS: &[(&str, &str)] = &[
    ("step_define", "define $x = value"),
    ("step_return", "return value?"),
    ("step_abort", "abort value?"),
    ("step_call", "ident (call_args | map_val | params | values)? (-> $x)?"),
    ("step_assign", "$var = expr"),
    ("step_if", "if expr: ... else: ... end"),
    ("step_match", "match: (expr -> ident)+ end"),
    ("step_when", "when expr: ... end"),
    ("step_for", "for $x in <expr>: ... end"),
    ("step_loop", "loop over $x (as ident)? (key_as ident)? (-> $y)?: ... end"),
    ("step_emit", "emit {..} | emit k: v ..."),
    ("step_try", "try (retry N ...)?: ... catch: ... end"),
    ("step_parallel", "parallel (-> $x)?: branch name (retry N ...)?: ... end ... end"),
    ("code_block", "{ code_stmt* }  (Rust-style v2 body)"),
    ("code_if_stmt", "if code_expr { .. } else { .. }"),
    ("code_for_stmt", "for ident in code_expr { .. }"),
    ("code_match_stmt", "match code_expr { pattern => .., _ => .. }"),
    ("code_try_stmt", "try { .. } catch ident? { .. }"),
    ("code_parallel_stmt", "parallel { name: { .. } ... }"),
    ("match_expr", "match subject { pattern|pattern => result, _ => default }"),
];

/// One extracted, real example usage of a construct's keyword.
pub struct Example<'a> {
    pub construct: &'static str,
    pub source_file: &'a str,
    pub snippet: String,
}

/// A minimal in-memory corpus entry: (repo-relative path, file contents).
pub type CorpusFile<'a> = (&'a str, &'a str);

/// Extract the first real usage block for `keyword` (a line starting with
/// `keyword ` or `keyword\t`, e.g. `entity Foo:`) from the corpus. Returns the
/// declaration line plus up to `max_lines` of its indented body, so the
/// snippet is a genuine, syntactically-recognizable fragment of a real file.
fn find_example<'a>(keyword: &str, corpus: &[CorpusFile<'a>], max_lines: usize) -> Option<Example<'a>> {
    for (path, text) in corpus {
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            // Only match TOP-LEVEL declarations (column 0), never an indented
            // clause of the same name nested inside another construct (e.g. a
            // legacy procedure's `  trigger: on_write(...)` clause must not be
            // mistaken for a top-level `trigger name:` declaration).
            if line.starts_with(' ') || line.starts_with('\t') {
                continue;
            }
            let trimmed = *line;
            if trimmed.starts_with(keyword)
                && trimmed[keyword.len()..]
                    .chars()
                    .next()
                    .map(|c| c == ' ' || c == ':' || c == '(')
                    .unwrap_or(false)
            {
                let mut end = i + 1;
                while end < lines.len()
                    && end < i + max_lines
                    && (lines[end].is_empty() || lines[end].starts_with(' ') || lines[end].starts_with('\t'))
                {
                    end += 1;
                }
                let snippet = lines[i..end].join("\n");
                return Some(Example {
                    construct: keyword_static(keyword),
                    source_file: path,
                    snippet,
                });
            }
        }
    }
    None
}

/// Leak-free static-str lookup so `Example.construct` can hold a `&'static
/// str` without unsafe; keyword set is small and fixed.
fn keyword_static(keyword: &str) -> &'static str {
    STATEMENT_CONSTRUCTS
        .iter()
        .find(|(_, _, kw)| *kw == keyword)
        .map(|(_, _, kw)| *kw)
        .unwrap_or("")
}

/// Render one construct's schema fields as `- field: type (optional)` lines.
fn render_fields(def_name: &str, definitions: &BTreeMap<String, Schema>) -> String {
    let mut out = String::new();
    let Some(Schema::Object(obj)) = definitions.get(def_name) else {
        out.push_str("  (schema definition not found — px-ast/px-schema drift)\n");
        return out;
    };
    let Some(ov) = &obj.object else {
        out.push_str("  (no object shape — leaf/alias type)\n");
        return out;
    };
    if ov.properties.is_empty() {
        out.push_str("  (no fields)\n");
        return out;
    }
    for (field, sub) in &ov.properties {
        let ty = field_type_label(sub);
        let req = if ov.required.contains(field) { "" } else { " (optional)" };
        out.push_str(&format!("  - {field}: {ty}{req}\n"));
    }
    out
}

fn field_type_label(schema: &Schema) -> String {
    match schema {
        Schema::Bool(_) => "any".to_string(),
        Schema::Object(o) => field_type_label_obj(o),
    }
}

fn field_type_label_obj(obj: &SchemaObject) -> String {
    if let Some(r) = &obj.reference {
        return r.rsplit('/').next().unwrap_or(r).to_string();
    }
    if let Some(sub) = &obj.subschemas {
        if let Some(list) = sub.any_of.as_ref().or(sub.one_of.as_ref()) {
            let parts: Vec<String> = list
                .iter()
                .map(field_type_label)
                .filter(|s| s != "null")
                .collect();
            if parts.len() == 1 {
                return format!("optional[{}]", parts[0]);
            }
            if !parts.is_empty() {
                return parts.join(" | ");
            }
        }
    }
    if let Some(it) = &obj.instance_type {
        use schemars::schema::{InstanceType, SingleOrVec};
        let name_of = |t: &InstanceType| -> &'static str {
            match t {
                InstanceType::Null => "null",
                InstanceType::Boolean => "bool",
                InstanceType::Object => "object",
                InstanceType::Array => "array",
                InstanceType::Number => "float",
                InstanceType::String => "string",
                InstanceType::Integer => "int",
            }
        };
        return match it {
            SingleOrVec::Single(t) => name_of(t).to_string(),
            SingleOrVec::Vec(ts) => ts
                .iter()
                .map(|t| name_of(t).to_string())
                .filter(|s| s != "null")
                .collect::<Vec<_>>()
                .join(" | "),
        };
    }
    "any".to_string()
}

/// Build the full cheatsheet markdown from the given real `.px` corpus.
///
/// This is the single function both the generator binary and the drift test
/// call, so "generated" always means the same deterministic projection.
pub fn build_cheatsheet(corpus: &[CorpusFile<'_>]) -> String {
    let root = build_json_schema();
    let mut out = String::new();

    out.push_str("# .px Grammar & AST Cheatsheet\n\n");
    out.push_str("<!-- GENERATED by px-cheatsheet (crates/px-cheatsheet) — DO NOT EDIT BY HAND. -->\n");
    out.push_str("<!-- Regenerate: cargo run -p px-cheatsheet -- docs/px-grammar-cheatsheet.md (or scripts/regen-cheatsheet.ps1) -->\n");
    out.push_str(
        "<!-- Source of truth: crates/px-ast/src/ (AST fields, via px-schema projection) -->\n",
    );
    out.push_str("<!-- and crates/px-grammar/src/grammar.pest (step/expression forms). -->\n\n");
    out.push_str(
        "This is a compact reference for the Praxis Intent Language (`.px`), \
         mechanically derived from this repo's actual parser/AST — not hand-guessed \
         syntax. Every field list below is a live projection of `px-ast`; every \
         example snippet is copied verbatim from a real, committed `.px` file in \
         this repo.\n\n",
    );

    out.push_str("## Top-level constructs\n\n");
    out.push_str(
        "Every `.px` document is a sequence of these statements \
         (`px_ast::Statement`, one variant each):\n\n",
    );

    let mut seen_keywords = std::collections::HashSet::new();
    for (variant, def_name, keyword) in STATEMENT_CONSTRUCTS {
        out.push_str(&format!("### `{keyword}` — {variant} ({def_name})\n\n"));
        out.push_str("**Fields (from px-ast, live projection):**\n\n");
        out.push_str(&render_fields(def_name, &root.definitions));
        out.push('\n');

        // Only look up one example per distinct keyword text (DataflowProcedure
        // and LegacyProcedure share the `procedure` keyword in source).
        if seen_keywords.insert(*keyword) {
            match find_example(keyword, corpus, 12) {
                Some(ex) => {
                    out.push_str(&format!("**Real example** (`{}`):\n\n", ex.source_file));
                    out.push_str("```px\n");
                    out.push_str(&ex.snippet);
                    out.push_str("\n```\n\n");
                }
                None => {
                    out.push_str("**Real example:** (no example found in corpus)\n\n");
                }
            }
        } else {
            out.push_str("**Real example:** see the shared `procedure` example above (dataflow vs legacy form is disambiguated by whether `from`/`into` queue bindings are present).\n\n");
        }
    }

    out.push_str("## Step-list & expression forms (grammar-level)\n\n");
    out.push_str(
        "These are recursive step/expression rules in `crates/px-grammar/src/grammar.pest` \
         that are not walked from the AST schema projection above (see crate docs for why); \
         listed directly from the grammar's own rule names so this section stays honest \
         about its narrower derivation:\n\n",
    );
    for (rule, form) in STEP_AND_EXPR_FORMS {
        out.push_str(&format!("- `{rule}`: `{form}`\n"));
    }
    out.push('\n');

    out
}
