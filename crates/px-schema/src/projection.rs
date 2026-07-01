//! The schema **projection** of `px-ast`.
//!
//! This module is the M4 single-source-of-truth mechanism (ADR §M4 /
//! PXLANG-M2 Part 3): the `.px` schema is not hand-authored — it is a pure,
//! deterministic **projection of the canonical `px-ast` types**, generated via
//! `schemars::JsonSchema` derives. Two artifacts are emitted:
//!
//! - `px.schema.json` — the JSON Schema (draft-07) for a whole `.px` document
//!   (`PxDocument`), emitted by `schemars` from the AST derives, with an
//!   `x-px-ast-version` marker so a document can be checked against the exact
//!   AST revision that produced it.
//! - `px.schema.px` — a `.px`-syntax schema description, generated **from the
//!   same JSON Schema** (i.e. from `px-ast`), listing every construct/type and
//!   its fields. It is a legible, `.px`-native view of the projection.
//!
//! Because both are regenerated from `px-ast` and a CI drift gate rejects any
//! mismatch against the committed files, the schema can never silently diverge
//! from the AST (C-DRIFT-001).

use px_ast::{PxDocument, PX_AST_VERSION};
use schemars::schema::{RootSchema, Schema, SchemaObject};
use schemars::schema_for;

/// Canonical file name for the generated JSON Schema artifact.
pub const JSON_SCHEMA_FILE: &str = "px.schema.json";
/// Canonical file name for the generated `.px`-syntax schema artifact.
pub const PX_SCHEMA_FILE: &str = "px.schema.px";

/// Build the JSON Schema (draft-07) for a whole `.px` document, projected from
/// `px-ast` via `schemars`. The `x-px-ast-version` extension records the AST
/// crate version that produced the schema.
///
/// `schemars` orders `definitions` in a `BTreeMap`, so this output is
/// deterministic across runs/platforms — a prerequisite for the drift gate.
pub fn build_json_schema() -> RootSchema {
    let mut root = schema_for!(PxDocument);
    // Stamp the producing AST version into the schema metadata so a `.px`
    // document can be validated against the exact revision that emitted it.
    root.schema.extensions.insert(
        "x-px-ast-version".to_string(),
        serde_json::Value::String(PX_AST_VERSION.to_string()),
    );
    // Give the root schema a stable title/id independent of struct renames.
    root.schema
        .metadata
        .get_or_insert_with(Default::default)
        .title
        .get_or_insert_with(|| "Praxis Intent Language (.px) — px-ast projection".to_string());
    root
}

/// Serialize the JSON Schema to a deterministic, pretty-printed UTF-8 string
/// with a trailing newline (LF). Written as raw bytes to a path by the bin.
pub fn json_schema_string() -> String {
    let root = build_json_schema();
    let mut s =
        serde_json::to_string_pretty(&root).expect("RootSchema is always JSON-serializable");
    s.push('\n');
    s
}

/// Generate the `.px`-syntax schema description **from the JSON Schema
/// projection** (hence from `px-ast`). This walks the schema's `definitions`
/// — every one of which is derived from a `px-ast` type — and emits a legible
/// `.px`-native listing of constructs, their fields, and field types.
///
/// This is a projection, not a re-authoring: if a construct/field is added to
/// `px-ast`, it appears here automatically; if one is removed, it disappears.
pub fn px_schema_string() -> String {
    let root = build_json_schema();
    let mut out = String::new();

    out.push_str("# Praxis Intent Language (.px) — schema\n");
    out.push_str("#\n");
    out.push_str("# GENERATED from px-ast via px-schema (do NOT edit by hand).\n");
    out.push_str(
        "# Regenerate: cargo run -p px-schema -- <out-dir>   (or scripts/regen-schema.ps1)\n",
    );
    out.push_str("# Source of truth: crates/px-ast/src/  (ADR §M4, C-DRIFT-001)\n");
    out.push_str(&format!("# px-ast version: {PX_AST_VERSION}\n"));
    out.push_str("#\n");
    out.push_str("# This is a .px-native projection of the JSON Schema (px.schema.json).\n");
    out.push_str("# Each `schema` block below mirrors one px-ast type; `f` lines are its\n");
    out.push_str("# fields with the projected JSON-Schema type. Sum types (enums) list their\n");
    out.push_str("# `kind` variants. The authoritative machine schema is px.schema.json.\n\n");

    out.push_str(&format!(
        "config schema_meta:\n  language: \"px\"\n  px_ast_version: \"{PX_AST_VERSION}\"\n  json_schema: \"{JSON_SCHEMA_FILE}\"\n  projection_of: \"px-ast\"\n\n",
    ));

    // Root document shape first.
    out.push_str("# ── root document ──────────────────────────────────────────────\n");
    emit_object("PxDocument", &root.schema, &mut out);

    // Then every definition, in deterministic (BTreeMap) order.
    out.push_str("\n# ── constructs & types (projected from px-ast) ─────────────────\n");
    for (name, schema) in &root.definitions {
        if let Schema::Object(obj) = schema {
            emit_object(name, obj, &mut out);
        }
    }

    out
}

/// Emit one `.px`-syntax `schema <Name>:` block for a schema object.
fn emit_object(name: &str, obj: &SchemaObject, out: &mut String) {
    // Adjacently-tagged enums surface as `oneOf`/`anyOf` of subschemas that
    // each pin `kind` to a const. Detect and render them as variant lists.
    if let Some(variants) = enum_variants(obj) {
        out.push_str(&format!("schema {name}:  # sum type (tagged on `kind`)\n"));
        if variants.is_empty() {
            out.push_str("  # (no variants)\n");
        }
        for v in variants {
            out.push_str(&format!("  variant: {v}\n"));
        }
        out.push('\n');
        return;
    }

    // Plain-string enums (unit-only, e.g. Severity/BaseType/BinOp).
    if let Some(values) = string_enum_values(obj) {
        out.push_str(&format!("schema {name}:  # enum\n"));
        for v in values {
            out.push_str(&format!("  value: {v}\n"));
        }
        out.push('\n');
        return;
    }

    // Object with properties.
    if let Some(ov) = &obj.object {
        out.push_str(&format!("schema {name}:\n"));
        if ov.properties.is_empty() {
            out.push_str("  # (no fields)\n");
        }
        let required = &ov.required;
        for (field, sub) in &ov.properties {
            let ty = type_label(sub);
            let req = if required.contains(field) {
                ""
            } else {
                " optional"
            };
            out.push_str(&format!("  f {field}: {ty}{req}\n"));
        }
        out.push('\n');
        return;
    }

    // Fallback: a leaf/aliased type (e.g. newtype over a scalar).
    out.push_str(&format!("schema {name}: {}\n\n", type_label_obj(obj)));
}

/// If this object is a tagged enum (`oneOf`/`anyOf` of variants each fixing
/// `kind` to a const), return the ordered list of variant names.
fn enum_variants(obj: &SchemaObject) -> Option<Vec<String>> {
    let sub = obj.subschemas.as_ref()?;
    let list = sub.one_of.as_ref().or(sub.any_of.as_ref())?;
    let mut names = Vec::new();
    for s in list {
        if let Schema::Object(o) = s {
            if let Some(k) = variant_kind_const(o) {
                names.push(k);
            } else if let Some(vals) = string_enum_values(o) {
                // A bare unit variant rendered as {"enum":["Name"]}.
                names.extend(vals);
            }
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

/// Extract the `const`/single-`enum` value of the `kind` property of a tagged
/// variant subschema.
fn variant_kind_const(obj: &SchemaObject) -> Option<String> {
    let ov = obj.object.as_ref()?;
    let kind = ov.properties.get("kind")?;
    if let Schema::Object(ko) = kind {
        if let Some(serde_json::Value::String(s)) = &ko.const_value {
            return Some(s.clone());
        }
        if let Some(first) = string_enum_values(ko).and_then(|v| v.into_iter().next()) {
            return Some(first);
        }
    }
    None
}

/// If this object is a plain string enum (`{"enum": ["A","B"]}` or a set of
/// string consts), return the values.
fn string_enum_values(obj: &SchemaObject) -> Option<Vec<String>> {
    if let Some(vals) = &obj.enum_values {
        let strs: Vec<String> = vals
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if strs.len() == vals.len() && !strs.is_empty() {
            return Some(strs);
        }
    }
    None
}

/// A short, legible type label for a field subschema (best-effort projection
/// of the JSON-Schema type into `.px`-ish notation).
fn type_label(schema: &Schema) -> String {
    match schema {
        Schema::Bool(_) => "any".to_string(),
        Schema::Object(o) => type_label_obj(o),
    }
}

fn type_label_obj(obj: &SchemaObject) -> String {
    // $ref to a named definition.
    if let Some(r) = &obj.reference {
        return ref_name(r);
    }
    // Nullable / Option<T> comes through as `anyOf: [T, null]` or a type array.
    if let Some(sub) = &obj.subschemas {
        if let Some(list) = sub.any_of.as_ref().or(sub.one_of.as_ref()) {
            let parts: Vec<String> = list
                .iter()
                .map(type_label)
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
    // Instance type(s).
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
        match it {
            SingleOrVec::Single(t) => {
                let base = name_of(t);
                if *base == *"array" {
                    if let Some(items) = array_item_label(obj) {
                        return format!("list[{items}]");
                    }
                }
                return base.to_string();
            }
            SingleOrVec::Vec(ts) => {
                let parts: Vec<String> = ts
                    .iter()
                    .map(|t| name_of(t).to_string())
                    .filter(|s| s != "null")
                    .collect();
                if parts.len() == 1 {
                    return format!("optional[{}]", parts[0]);
                }
                return parts.join(" | ");
            }
        }
    }
    "any".to_string()
}

/// Item type label for an array schema, if present.
fn array_item_label(obj: &SchemaObject) -> Option<String> {
    use schemars::schema::SingleOrVec;
    let arr = obj.array.as_ref()?;
    match arr.items.as_ref()? {
        SingleOrVec::Single(s) => Some(type_label(s)),
        SingleOrVec::Vec(v) => {
            let parts: Vec<String> = v.iter().map(type_label).collect();
            Some(format!("[{}]", parts.join(", ")))
        }
    }
}

/// Strip the `#/definitions/` prefix from a `$ref`.
fn ref_name(r: &str) -> String {
    r.rsplit('/').next().unwrap_or(r).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_schema_is_nonempty_and_versioned() {
        let s = json_schema_string();
        assert!(s.contains("\"x-px-ast-version\""));
        assert!(s.contains(PX_AST_VERSION));
        assert!(s.ends_with('\n'));
        // The root should reference the Statement definition.
        assert!(s.contains("Statement"));
    }

    #[test]
    fn json_schema_is_deterministic() {
        // Two independent generations must be byte-identical (drift-gate
        // prerequisite).
        assert_eq!(json_schema_string(), json_schema_string());
    }

    #[test]
    fn json_schema_drops_span_noise() {
        // span fields are #[schemars(skip)] — the projection must not carry
        // positional editor noise (PXLANG-M2 §3.1.3).
        let s = json_schema_string();
        assert!(!s.contains("\"span\""));
    }

    #[test]
    fn px_schema_lists_all_top_level_constructs() {
        let s = px_schema_string();
        for decl in [
            "ImportDecl",
            "EntityDecl",
            "ConfigDecl",
            "FactDecl",
            "RuleDecl",
            "ConstraintDecl",
            "ContractDecl",
            "FunctionDecl",
            "TriggerDecl",
            "DataflowProcedureDecl",
            "LegacyProcedureDecl",
            "ScenarioDecl",
        ] {
            assert!(s.contains(decl), "px.schema.px missing construct {decl}");
        }
        // Sum type variants (Statement is tagged on `kind`).
        assert!(s.contains("variant: Entity"));
        assert!(s.ends_with('\n'));
    }

    #[test]
    fn px_schema_is_deterministic() {
        assert_eq!(px_schema_string(), px_schema_string());
    }
}
