//! # px-compiler — parse `.px` source text into the canonical [`px_ast`] tree.
//!
//! This crate lowers `.px` source into the canonical AST defined by `px-ast`
//! (the language spec). It uses the pest [`PxParser`](px_grammar::PxParser)
//! binding over the generated `grammar.pest` and walks the resulting parse
//! tree, constructing typed `px-ast` nodes.
//!
//! ## Public API
//!
//! - [`parse`] — `&str` → [`px_ast::PxDocument`] (the whole document).
//! - [`parse_statement`] — parse exactly one top-level statement.
//! - [`CompileError`] — parse / unsupported / internal errors.
//!
//! ## Coverage & honesty (C-NOSTUB-001)
//!
//! Every construct this crate *claims* to lower is really lowered into typed
//! AST nodes (verified by the parse tests + `examples/*.px`). Anything not yet
//! lowered returns a real [`CompileError::Unsupported`] naming the rule — never
//! a silent placeholder. See `PXLANG-M3W2-RESULT.md` for the exact coverage
//! matrix.

mod builder;
mod error;

pub use error::CompileError;

use pest::Parser;
use px_ast::{PxDocument, Statement};
use px_check::{
    check, check_source_contract, CheckReport, ContractCatalog, Diagnostic, ExecutionProfile,
};
use px_grammar::{PxParser, Rule};

/// A syntax or static-contract failure from [`parse_checked`].
#[derive(Debug, Clone)]
pub enum CheckedCompileError {
    Parse(CompileError),
    Contract(CheckReport),
}

impl std::fmt::Display for CheckedCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Contract(report) => {
                write!(f, "static contract check failed")?;
                for diagnostic in &report.diagnostics {
                    write!(
                        f,
                        "\n{} {} step {}: {}",
                        diagnostic.code, diagnostic.procedure, diagnostic.step, diagnostic.message
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CheckedCompileError {}

/// Complete checked-activation result for one `.px` source document.
///
/// The parser contributes a `PX1012` diagnostic when source is not syntactically
/// valid; when it can produce an AST, static call diagnostics are accumulated
/// alongside source/header diagnostics. Consumers should present the whole
/// report and only activate when [`Self::is_valid`] is true.
#[derive(Debug, Clone)]
pub struct CheckedParseResult {
    pub document: Option<PxDocument>,
    pub report: CheckReport,
}

impl CheckedParseResult {
    pub fn is_valid(&self) -> bool {
        self.document.is_some() && self.report.is_valid()
    }
}

/// Parse a complete `.px` document into the canonical AST.
///
/// # Errors
/// Returns [`CompileError::Parse`] on a syntax error, or
/// [`CompileError::Unsupported`] / [`CompileError::Internal`] if a
/// structurally-valid tree contains a shape the builder cannot lower.
///
/// # Example
/// ```
/// let doc = px_compiler::parse("import core::memory as mem\n").unwrap();
/// assert_eq!(doc.statements.len(), 1);
/// ```
pub fn parse(src: &str) -> Result<PxDocument, CompileError> {
    let mut pairs =
        PxParser::parse(Rule::document, src).map_err(|e| CompileError::Parse(e.to_string()))?;
    // `document = { SOI ~ NEWLINE* ~ (statement ~ NEWLINE*)* ~ EOI }`
    let document = pairs
        .next()
        .ok_or_else(|| CompileError::internal("empty parse: no `document` pair"))?;
    let mut statements = Vec::new();
    for pair in document.into_inner() {
        match pair.as_rule() {
            Rule::EOI => {}
            // `statement` is a silent rule, so its alternatives appear inline
            // as the concrete decl rules directly.
            _ => statements.push(builder::build_statement(pair)?),
        }
    }
    Ok(PxDocument { statements })
}

/// Validate every detectable source and static contract defect before a host
/// activates a `.px` document.
///
/// The report is intentionally non-fail-fast: malformed or missing schema pins,
/// unsupported parser-skip markers, host catalog mismatch, and all statically
/// reachable call defects are returned together. Syntax recovery is bounded by
/// the parser; a syntax failure is represented as `PX1012` and precludes AST
/// semantic checking.
pub fn validate_checked(
    src: &str,
    catalog: &ContractCatalog,
    profile: ExecutionProfile,
) -> CheckedParseResult {
    let mut report = check_source_contract(src, catalog);
    let document = match parse(src) {
        Ok(document) => {
            report
                .diagnostics
                .extend(check(&document, catalog, profile).diagnostics);
            Some(document)
        }
        Err(error) => {
            report.diagnostics.push(Diagnostic {
                code: "PX1012",
                procedure: "<document>".to_string(),
                step: 0,
                message: error.to_string(),
            });
            None
        }
    };
    CheckedParseResult { document, report }
}

/// Parse and fail closed against the target runtime's typed call contracts.
///
/// Use [`validate_checked`] when callers need every diagnostic for an activation
/// UI, API response, CI report, or Decision Ledger record.
pub fn parse_checked(
    src: &str,
    catalog: &ContractCatalog,
    profile: ExecutionProfile,
) -> Result<PxDocument, CheckedCompileError> {
    let result = validate_checked(src, catalog, profile);
    if result.is_valid() {
        // `is_valid` guarantees `document` is present.
        Ok(result.document.expect("checked document present"))
    } else {
        Err(CheckedCompileError::Contract(result.report))
    }
}

/// Parse a single top-level `.px` statement (e.g. one `entity`/`rule`/...).
///
/// Convenience for tools that operate on one construct at a time. The input
/// must contain exactly one statement.
///
/// # Errors
/// See [`parse`].
pub fn parse_statement(src: &str) -> Result<Statement, CompileError> {
    let doc = parse(src)?;
    let mut it = doc.statements.into_iter();
    let first = it
        .next()
        .ok_or_else(|| CompileError::internal("expected exactly one statement, found none"))?;
    if it.next().is_some() {
        return Err(CompileError::internal(
            "expected exactly one statement, found more than one",
        ));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use px_ast::{BaseType, TypeExpr};
    use px_check::{CallableContract, SchemaRef, StaticType};

    fn catalog() -> ContractCatalog {
        ContractCatalog::with_schema(SchemaRef::new("test.runtime", 1, "test-fingerprint"))
    }

    #[test]
    fn parses_import_with_alias() {
        let doc = parse("import core::memory as mem\n").unwrap();
        assert_eq!(doc.statements.len(), 1);
        match &doc.statements[0] {
            Statement::Import(i) => {
                assert_eq!(i.path.len(), 2);
                assert_eq!(i.path[0].name, "core");
                assert_eq!(i.path[1].name, "memory");
                assert_eq!(i.alias.as_ref().unwrap().name, "mem");
            }
            other => panic!("expected import, got {other:?}"),
        }
    }

    #[test]
    fn parses_entity_with_prefix_and_fields() {
        let src = "entity Player:\n  prefix: \"player\"\n  fields:\n    health: int\n    name: string\n    inventory: list[string]\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::Entity(e) => {
                assert_eq!(e.name.name, "Player");
                assert_eq!(e.prefix.as_ref().unwrap().value, "player");
                assert_eq!(e.fields.len(), 3);
                assert_eq!(e.fields[0].name.name, "health");
            }
            other => panic!("expected entity, got {other:?}"),
        }
    }

    #[test]
    fn parses_fact() {
        let src = "fact MemoryEntry:\n  content: string\n  category: string\n  timestamp: int\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::Fact(f) => {
                assert_eq!(f.name.name, "MemoryEntry");
                assert_eq!(f.fields.len(), 3);
            }
            other => panic!("expected fact, got {other:?}"),
        }
    }

    #[test]
    fn parses_rule_with_priority_when_then() {
        let src = "rule detect_urgency:\n  priority: 10\n  when:\n    - contains(message, \"urgent\")\n  then:\n    - action: flag_priority level: \"high\"\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::Rule(r) => {
                assert_eq!(r.name.name, "detect_urgency");
                assert_eq!(r.priority, Some(10));
                assert_eq!(r.conditions.len(), 1);
                assert_eq!(r.actions.len(), 1);
                assert_eq!(r.actions[0].action_name.name, "flag_priority");
            }
            other => panic!("expected rule, got {other:?}"),
        }
    }

    #[test]
    fn parses_constraint_with_require_and_severity() {
        let src = "constraint no_empty:\n  require: len > 0\n  severity: error\n  message: \"empty not allowed\"\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::Constraint(c) => {
                assert_eq!(c.name.name, "no_empty");
                assert_eq!(c.severity, px_ast::Severity::Error);
                assert!(c.require.is_some());
                assert_eq!(c.message.as_ref().unwrap().value, "empty not allowed");
            }
            other => panic!("expected constraint, got {other:?}"),
        }
    }

    #[test]
    fn parses_function_with_mode_and_docstring() {
        let src = "function classify(message: string) -> string:\n  mode: deterministic\n  \"\"\"Classify intent.\"\"\"\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::Function(f) => {
                assert_eq!(f.name.name, "classify");
                assert_eq!(f.params.len(), 1);
                assert_eq!(f.mode, Some(px_ast::FunctionMode::Deterministic));
                assert!(f.docstring.as_ref().unwrap().contains("Classify"));
            }
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn parses_dataflow_procedure_with_steps() {
        let src = "procedure classify_and_route(msg: string from \"inbound\") -> string into \"route\":\n  given: \"route it\"\n  classify_intent msg -> $intent\n  return $intent\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::DataflowProcedure(p) => {
                assert_eq!(p.name.name, "classify_and_route");
                assert_eq!(p.params.len(), 1);
                assert_eq!(p.params[0].source_queue.as_ref().unwrap().value, "inbound");
                assert!(p.return_type.is_some());
                match &p.body {
                    px_ast::ProcedureBody::Steps(steps) => assert_eq!(steps.len(), 2),
                    other => panic!("expected steps body, got {other:?}"),
                }
            }
            other => panic!("expected dataflow procedure, got {other:?}"),
        }
    }

    #[test]
    fn parses_code_block_procedure() {
        let src = "procedure compute() -> int:\n  {\n    let x = add(1, 2);\n    return x;\n  }\n";
        let doc = parse(src).unwrap();
        match &doc.statements[0] {
            Statement::DataflowProcedure(p) => match &p.body {
                px_ast::ProcedureBody::Code(block) => {
                    assert_eq!(block.statements.len(), 2);
                }
                other => panic!("expected code body, got {other:?}"),
            },
            other => panic!("expected procedure, got {other:?}"),
        }
    }

    #[test]
    fn parses_multiple_statements() {
        let src = "import core::a\n\nfact F:\n  x: int\n\nconfig C:\n  k: 1\n";
        let doc = parse(src).unwrap();
        assert_eq!(doc.statements.len(), 3);
    }

    #[test]
    fn syntax_error_is_reported() {
        // `entity` requires a body; a bare header must fail to parse.
        let err = parse("entity Broken\n").unwrap_err();
        matches!(err, CompileError::Parse(_));
    }

    #[test]
    fn parses_legacy_procedure_trigger_kinds() {
        use px_ast::ProcedureTrigger;
        // Compound trigger with a parenthesized pattern (no space before `(`).
        let on_write = "procedure p:\n  trigger: on_write(\"inbound\")\n  return 1\n";
        match &parse(on_write).unwrap().statements[0] {
            Statement::LegacyProcedure(p) => {
                assert!(matches!(p.trigger, Some(ProcedureTrigger::OnWrite { .. })));
            }
            other => panic!("expected legacy procedure, got {other:?}"),
        }
        // Bare keyword trigger.
        let manual = "procedure q:\n  trigger: manual\n  return 1\n";
        match &parse(manual).unwrap().statements[0] {
            Statement::LegacyProcedure(p) => {
                assert!(matches!(p.trigger, Some(ProcedureTrigger::Manual)));
            }
            other => panic!("expected legacy procedure, got {other:?}"),
        }
        // Compound trigger carrying a map_val (`cron { ... }`).
        let cron = "procedure r:\n  trigger: cron {expr: \"0 3 * * *\"}\n  return 1\n";
        match &parse(cron).unwrap().statements[0] {
            Statement::LegacyProcedure(p) => {
                assert!(matches!(p.trigger, Some(ProcedureTrigger::Cron { .. })));
            }
            other => panic!("expected legacy procedure, got {other:?}"),
        }
    }

    #[test]
    fn checked_parse_rejects_runtime_unsupported_accessor_parameters() {
        let source = concat!(
            "# px-schema: test.runtime@1#test-fingerprint\n",
            "fact Task:\n",
            "  id: string\n",
            "procedure dispatch(task: Task from \"inbound\") -> string:\n",
            "  send {task_id: $task.id}\n",
        );
        let mut catalog = catalog();
        catalog.insert(
            "send",
            CallableContract::new(StaticType::Known(TypeExpr::Base(BaseType::String)))
                .required("task_id", TypeExpr::Base(BaseType::String)),
        );
        let error = parse_checked(source, &catalog, ExecutionProfile::STRICT).unwrap_err();
        assert!(error.to_string().contains("PX1007"));
        assert!(parse_checked(source, &catalog, ExecutionProfile::ACCESSOR_VALUES).is_ok());
    }

    #[test]
    fn checked_parse_rejects_parser_skip_markers() {
        let error = parse_checked(
            "# px-schema: test.runtime@1#test-fingerprint\n# [parser-skip] config runtime:\nprocedure p() -> string:\n  return \"ok\"\n",
            &catalog(),
            ExecutionProfile::STRICT,
        )
        .unwrap_err();
        assert!(error.to_string().contains("PX1000"));
    }

    #[test]
    fn checked_parse_rejects_missing_or_mismatched_schema_pin() {
        let source = "procedure p() -> string:\n  return \"ok\"\n";
        assert!(parse_checked(source, &catalog(), ExecutionProfile::STRICT)
            .unwrap_err()
            .to_string()
            .contains("PX1009"));
        let source = "# px-schema: different.runtime@1#test-fingerprint\nprocedure p() -> string:\n  return \"ok\"\n";
        assert!(parse_checked(source, &catalog(), ExecutionProfile::STRICT)
            .unwrap_err()
            .to_string()
            .contains("PX1011"));
    }

    #[test]
    fn checked_validation_collects_source_and_call_defects_together() {
        let source = concat!(
            "# px-schema: other.runtime@1#different\n",
            "# [parser-skip] config unsupported\n",
            "procedure dispatch() -> string:\n",
            "  unknown_action {unexpected: 1}\n",
        );

        let result = validate_checked(&source, &catalog(), ExecutionProfile::STRICT);
        assert!(result.document.is_some(), "source itself should parse");
        assert!(!result.is_valid());
        let codes: Vec<_> = result
            .report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.contains(&"PX1000"));
        assert!(codes.contains(&"PX1001"));
        assert!(codes.contains(&"PX1011"));
    }

    #[test]
    fn checked_validation_retains_preflight_diagnostics_when_syntax_is_invalid() {
        let source = "# [parser-skip] unsupported\nprocedure broken(\n";
        let result = validate_checked(&source, &catalog(), ExecutionProfile::STRICT);
        assert!(result.document.is_none());
        let codes: Vec<_> = result
            .report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.contains(&"PX1000"));
        assert!(codes.contains(&"PX1009"));
        assert!(codes.contains(&"PX1012"));
    }
}
