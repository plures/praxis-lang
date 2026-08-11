//! Static, fail-closed contracts for `.px` procedure calls.
//!
//! Parsing proves syntax. This crate proves that a procedure can be lowered by
//! its target runtime: every call is declared, named arguments are complete and
//! typed, and every variable-reference form is supported by that runtime.
//! Hosts provide the action catalog; the language core never imports a host.

use std::collections::{BTreeMap, HashMap};

use px_ast::{
    Accessor, BaseType, DataflowProcedureDecl, FieldDecl, ProcedureBody, PxDocument, Statement,
    Step, StepCall, StepCallArgs, TypeExpr, Value, VarRef,
};

/// Contract for one named parameter of a callable action or procedure.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterContract {
    pub value_type: TypeExpr,
    pub required: bool,
}

/// Typed, named call contract exported by a runtime or `.px` module.
#[derive(Debug, Clone, PartialEq)]
pub struct CallableContract {
    pub params: BTreeMap<String, ParameterContract>,
    pub result: StaticType,
}

/// Identity of a generated callable/schema surface.
///
/// A checked `.px` file pins this value in a `# px-schema:` header so a host
/// cannot silently load it against a different action surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRef {
    pub id: String,
    pub version: u32,
    pub fingerprint: String,
}

impl SchemaRef {
    pub fn new(id: impl Into<String>, version: u32, fingerprint: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version,
            fingerprint: fingerprint.into(),
        }
    }
}

impl CallableContract {
    pub fn new(result: StaticType) -> Self {
        Self {
            params: BTreeMap::new(),
            result,
        }
    }

    pub fn required(mut self, name: impl Into<String>, value_type: TypeExpr) -> Self {
        self.params.insert(
            name.into(),
            ParameterContract {
                value_type,
                required: true,
            },
        );
        self
    }

    pub fn optional(mut self, name: impl Into<String>, value_type: TypeExpr) -> Self {
        self.params.insert(
            name.into(),
            ParameterContract {
                value_type,
                required: false,
            },
        );
        self
    }
}

/// Runtime-provided callable contracts. Unknown calls are errors by design.
#[derive(Debug, Clone, Default)]
pub struct ContractCatalog {
    callables: BTreeMap<String, CallableContract>,
    schema: Option<SchemaRef>,
}

impl ContractCatalog {
    pub fn with_schema(schema: SchemaRef) -> Self {
        Self {
            callables: BTreeMap::new(),
            schema: Some(schema),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, contract: CallableContract) {
        self.callables.insert(name.into(), contract);
    }

    pub fn get(&self, name: &str) -> Option<&CallableContract> {
        self.callables.get(name)
    }

    pub fn schema(&self) -> Option<&SchemaRef> {
        self.schema.as_ref()
    }

    /// Add typed `function` declarations from a contract module.
    pub fn add_functions_from(&mut self, document: &PxDocument) {
        for statement in &document.statements {
            if let Statement::Function(function) = statement {
                self.insert(
                    function.name.name.clone(),
                    contract_from_fields(
                        &function.params,
                        StaticType::Known(function.return_type.clone()),
                    ),
                );
            }
        }
    }
}

/// Parse the schema pin from source. The value is deliberately source-local:
/// `# px-schema: owner.surface@1#fingerprint`.
pub fn schema_ref_from_source(source: &str) -> Result<SchemaRef, String> {
    let Some(line) = source
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("# px-schema:"))
    else {
        return Err(
            "missing required `# px-schema: <id>@<version>#<fingerprint>` header".to_string(),
        );
    };
    let (identity, fingerprint) = line
        .trim()
        .split_once('#')
        .ok_or_else(|| "schema header is missing `#<fingerprint>`".to_string())?;
    let (id, version) = identity
        .rsplit_once('@')
        .ok_or_else(|| "schema header is missing `@<version>`".to_string())?;
    let version = version
        .parse::<u32>()
        .map_err(|_| "schema header version must be an unsigned integer".to_string())?;
    if id.is_empty() || fingerprint.is_empty() {
        return Err("schema header id and fingerprint must be non-empty".to_string());
    }
    Ok(SchemaRef::new(id, version, fingerprint))
}

/// The static type tracked for variables and call results.
#[derive(Debug, Clone, PartialEq)]
pub enum StaticType {
    Known(TypeExpr),
    Any,
    Null,
}

/// Target-runtime lowering capabilities. The conservative default rejects
/// compound variable references in call parameters until a runtime advertises
/// that it preserves them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionProfile {
    pub supports_accessor_values_in_call_params: bool,
}

impl ExecutionProfile {
    pub const STRICT: Self = Self {
        supports_accessor_values_in_call_params: false,
    };

    pub const ACCESSOR_VALUES: Self = Self {
        supports_accessor_values_in_call_params: true,
    };
}

/// A source-location-independent compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub procedure: String,
    pub step: usize,
    pub message: String,
}

/// Result of one whole-document static check.
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckReport {
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Check all dataflow procedures against the catalog and target profile.
pub fn check(
    document: &PxDocument,
    catalog: &ContractCatalog,
    profile: ExecutionProfile,
) -> CheckReport {
    let records = record_types(document);
    let mut effective_catalog = catalog.clone();
    add_procedure_contracts(document, &mut effective_catalog);

    let mut report = CheckReport::default();
    for statement in &document.statements {
        if let Statement::DataflowProcedure(procedure) = statement {
            check_procedure(
                procedure,
                &effective_catalog,
                &records,
                profile,
                &mut report,
            );
        }
    }
    report
}

fn contract_from_fields(fields: &[FieldDecl], result: StaticType) -> CallableContract {
    let mut contract = CallableContract::new(result);
    for field in fields {
        contract = contract.required(field.name.name.clone(), field.field_type.clone());
    }
    contract
}

fn add_procedure_contracts(document: &PxDocument, catalog: &mut ContractCatalog) {
    for statement in &document.statements {
        if let Statement::DataflowProcedure(procedure) = statement {
            let result = procedure
                .return_type
                .as_ref()
                .map(|return_type| StaticType::Known(return_type.return_type.clone()))
                .unwrap_or(StaticType::Any);
            let fields: Vec<FieldDecl> = procedure
                .params
                .iter()
                .map(|param| FieldDecl {
                    name: param.name.clone(),
                    field_type: param.param_type.clone(),
                    span: None,
                })
                .collect();
            catalog.insert(
                procedure.name.name.clone(),
                contract_from_fields(&fields, result),
            );
        }
    }
}

fn record_types(document: &PxDocument) -> HashMap<String, HashMap<String, TypeExpr>> {
    let mut records = HashMap::new();
    for statement in &document.statements {
        let (name, fields) = match statement {
            Statement::Entity(entity) => (&entity.name.name, &entity.fields),
            Statement::Fact(fact) => (&fact.name.name, &fact.fields),
            _ => continue,
        };
        records.insert(
            name.clone(),
            fields
                .iter()
                .map(|field| (field.name.name.clone(), field.field_type.clone()))
                .collect(),
        );
    }
    records
}

fn check_procedure(
    procedure: &DataflowProcedureDecl,
    catalog: &ContractCatalog,
    records: &HashMap<String, HashMap<String, TypeExpr>>,
    profile: ExecutionProfile,
    report: &mut CheckReport,
) {
    let mut variables = HashMap::new();
    for parameter in &procedure.params {
        variables.insert(
            parameter.name.name.clone(),
            StaticType::Known(parameter.param_type.clone()),
        );
    }
    if let ProcedureBody::Steps(steps) = &procedure.body {
        check_steps(
            steps,
            &procedure.name.name,
            catalog,
            records,
            profile,
            &mut variables,
            report,
        );
    }
}

fn check_steps(
    steps: &[Step],
    procedure: &str,
    catalog: &ContractCatalog,
    records: &HashMap<String, HashMap<String, TypeExpr>>,
    profile: ExecutionProfile,
    variables: &mut HashMap<String, StaticType>,
    report: &mut CheckReport,
) {
    for (step, node) in steps.iter().enumerate() {
        match node {
            Step::Define { var, value } => {
                let value_type = infer_value(
                    value, variables, records, profile, procedure, step, report, false,
                );
                variables.insert(var.name.clone(), value_type);
            }
            Step::Call(call) => check_call(
                call, procedure, step, catalog, records, profile, variables, report,
            ),
            Step::When { steps, .. }
            | Step::If {
                then_steps: steps,
                else_steps: None,
                ..
            } => {
                check_steps(
                    steps, procedure, catalog, records, profile, variables, report,
                );
            }
            Step::If {
                then_steps,
                else_steps: Some(else_steps),
                ..
            } => {
                let base = variables.clone();
                let mut then_vars = base.clone();
                check_steps(
                    then_steps,
                    procedure,
                    catalog,
                    records,
                    profile,
                    &mut then_vars,
                    report,
                );
                let mut else_vars = base;
                check_steps(
                    else_steps,
                    procedure,
                    catalog,
                    records,
                    profile,
                    &mut else_vars,
                    report,
                );
            }
            Step::Loop(loop_step) => {
                if let Some(item) = &loop_step.item_name {
                    variables.insert(item.name.clone(), StaticType::Any);
                }
                check_steps(
                    &loop_step.steps,
                    procedure,
                    catalog,
                    records,
                    profile,
                    variables,
                    report,
                );
            }
            Step::Try(try_step) => {
                check_steps(
                    &try_step.steps,
                    procedure,
                    catalog,
                    records,
                    profile,
                    variables,
                    report,
                );
                if let Some(catch) = &try_step.catch {
                    check_steps(
                        catch, procedure, catalog, records, profile, variables, report,
                    );
                }
            }
            Step::Parallel(parallel) => {
                for branch in &parallel.branches {
                    check_steps(
                        &branch.steps,
                        procedure,
                        catalog,
                        records,
                        profile,
                        variables,
                        report,
                    );
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)] // Call checking needs the complete lexical typing context.
fn check_call(
    call: &StepCall,
    procedure: &str,
    step: usize,
    catalog: &ContractCatalog,
    records: &HashMap<String, HashMap<String, TypeExpr>>,
    profile: ExecutionProfile,
    variables: &mut HashMap<String, StaticType>,
    report: &mut CheckReport,
) {
    let action = &call.action.name;
    let Some(contract) = catalog.get(action) else {
        push(report, "PX1001", procedure, step, format!("unknown callable `{action}`; declare it in the host action catalog or a typed .px module"));
        return;
    };

    let arguments = named_arguments(&call.args);
    let Some(arguments) = arguments else {
        push(report, "PX1002", procedure, step, format!("call `{action}` uses positional arguments; the checked runtime contract requires named arguments"));
        return;
    };

    for (name, parameter) in &contract.params {
        if parameter.required && !arguments.contains_key(name) {
            push(
                report,
                "PX1003",
                procedure,
                step,
                format!("call `{action}` is missing required argument `{name}`"),
            );
        }
    }
    for (name, value) in &arguments {
        let Some(parameter) = contract.params.get(name) else {
            push(
                report,
                "PX1004",
                procedure,
                step,
                format!("call `{action}` passes unknown argument `{name}`"),
            );
            continue;
        };
        let actual = infer_value(
            value, variables, records, profile, procedure, step, report, true,
        );
        if !assignable(&actual, &parameter.value_type) {
            push(
                report,
                "PX1005",
                procedure,
                step,
                format!(
                    "call `{action}` argument `{name}` expects `{}`, got `{}`",
                    parameter.value_type,
                    display_static_type(&actual)
                ),
            );
        }
    }

    if let Some(output) = &call.output {
        variables.insert(output.name.clone(), contract.result.clone());
    }
}

fn named_arguments(args: &StepCallArgs) -> Option<BTreeMap<String, &Value>> {
    match args {
        StepCallArgs::Params(pairs) => Some(
            pairs
                .iter()
                .map(|(name, value)| (name.name.clone(), value))
                .collect(),
        ),
        StepCallArgs::Map(Value::Map(pairs)) => Some(
            pairs
                .iter()
                .map(|(name, value)| (name.name.clone(), value))
                .collect(),
        ),
        StepCallArgs::None => Some(BTreeMap::new()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)] // Value inference shares that same lexical context.
fn infer_value(
    value: &Value,
    variables: &HashMap<String, StaticType>,
    records: &HashMap<String, HashMap<String, TypeExpr>>,
    profile: ExecutionProfile,
    procedure: &str,
    step: usize,
    report: &mut CheckReport,
    call_parameter: bool,
) -> StaticType {
    match value {
        Value::String(_) => StaticType::Known(TypeExpr::Base(BaseType::String)),
        Value::Integer(_) => StaticType::Known(TypeExpr::Base(BaseType::Int)),
        Value::Float(_) => StaticType::Known(TypeExpr::Base(BaseType::Float)),
        Value::Boolean(_) => StaticType::Known(TypeExpr::Base(BaseType::Bool)),
        Value::Null => StaticType::Null,
        Value::List(_) | Value::Map(_) => StaticType::Any,
        Value::Var(reference) => resolve_variable(
            reference,
            variables,
            records,
            profile,
            procedure,
            step,
            report,
            call_parameter,
        ),
        _ => StaticType::Any,
    }
}

#[allow(clippy::too_many_arguments)] // Accessor diagnostics need source and runtime context.
fn resolve_variable(
    reference: &VarRef,
    variables: &HashMap<String, StaticType>,
    records: &HashMap<String, HashMap<String, TypeExpr>>,
    profile: ExecutionProfile,
    procedure: &str,
    step: usize,
    report: &mut CheckReport,
    call_parameter: bool,
) -> StaticType {
    let Some(mut value_type) = variables.get(&reference.name.name).cloned() else {
        push(
            report,
            "PX1006",
            procedure,
            step,
            format!("unknown variable `${}`", reference.name.name),
        );
        return StaticType::Any;
    };
    if call_parameter
        && !reference.accessors.is_empty()
        && !profile.supports_accessor_values_in_call_params
    {
        push(
            report,
            "PX1007",
            procedure,
            step,
            format!("runtime profile cannot lower accessor reference `{reference}` in a call parameter; extract it with a declared callable or enable accessor lowering"),
        );
    }
    for accessor in &reference.accessors {
        value_type = match (value_type, accessor) {
            (StaticType::Known(TypeExpr::Named(name)), Accessor::Dot(field)) => records
                .get(&name.name)
                .and_then(|fields| fields.get(&field.name))
                .cloned()
                .map(StaticType::Known)
                .unwrap_or_else(|| {
                    push(
                        report,
                        "PX1008",
                        procedure,
                        step,
                        format!("unknown field `{}` on `{}`", field.name, name.name),
                    );
                    StaticType::Any
                }),
            (StaticType::Known(TypeExpr::List(inner)), Accessor::Bracket(_)) => {
                StaticType::Known(*inner)
            }
            (StaticType::Any, _) => StaticType::Any,
            (other, _) => {
                push(
                    report,
                    "PX1008",
                    procedure,
                    step,
                    format!(
                        "cannot access `{accessor}` on `{}`",
                        display_static_type(&other)
                    ),
                );
                StaticType::Any
            }
        };
    }
    value_type
}

fn assignable(actual: &StaticType, expected: &TypeExpr) -> bool {
    match expected {
        TypeExpr::Optional(inner) => match actual {
            StaticType::Null | StaticType::Any => true,
            StaticType::Known(actual) => actual == inner.as_ref(),
        },
        _ => match actual {
            StaticType::Any => true,
            StaticType::Null => false,
            StaticType::Known(actual) => actual == expected,
        },
    }
}

fn display_static_type(value_type: &StaticType) -> String {
    match value_type {
        StaticType::Known(value_type) => value_type.to_string(),
        StaticType::Any => "any".to_string(),
        StaticType::Null => "null".to_string(),
    }
}

fn push(
    report: &mut CheckReport,
    code: &'static str,
    procedure: &str,
    step: usize,
    message: String,
) {
    report.diagnostics.push(Diagnostic {
        code,
        procedure: procedure.to_string(),
        step,
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use px_ast::{DataflowParam, DataflowReturn, Ident, ProcedureBody, StepCall, StepCallArgs};

    fn id(name: &str) -> Ident {
        Ident::new(name)
    }
    fn int() -> TypeExpr {
        TypeExpr::Base(BaseType::Int)
    }
    fn string() -> TypeExpr {
        TypeExpr::Base(BaseType::String)
    }

    fn procedure(call: StepCall) -> PxDocument {
        PxDocument {
            statements: vec![
                Statement::Fact(px_ast::FactDecl {
                    name: id("Task"),
                    fields: vec![FieldDecl {
                        name: id("id"),
                        field_type: string(),
                        span: None,
                    }],
                    span: None,
                }),
                Statement::DataflowProcedure(DataflowProcedureDecl {
                    name: id("dispatch"),
                    params: vec![DataflowParam {
                        name: id("task"),
                        param_type: TypeExpr::Named(id("Task")),
                        source_queue: None,
                    }],
                    return_type: Some(DataflowReturn {
                        return_type: string(),
                        dest_queue: None,
                    }),
                    given: None,
                    body: ProcedureBody::Steps(vec![Step::Call(call)]),
                    span: None,
                }),
            ],
        }
    }

    #[test]
    fn rejects_unknown_callable_and_argument_shape() {
        let doc = procedure(StepCall {
            action: id("missing"),
            args: StepCallArgs::Params(vec![(id("count"), Value::String("wrong".into()))]),
            output: None,
        });
        let report = check(
            &doc,
            &ContractCatalog::default(),
            ExecutionProfile::ACCESSOR_VALUES,
        );
        assert_eq!(report.diagnostics[0].code, "PX1001");
    }

    #[test]
    fn rejects_missing_unknown_and_mistyped_arguments() {
        let doc = procedure(StepCall {
            action: id("dispatch_task"),
            args: StepCallArgs::Params(vec![
                (id("task_id"), Value::Integer(1)),
                (id("extra"), Value::String("x".into())),
            ]),
            output: None,
        });
        let mut catalog = ContractCatalog::default();
        catalog.insert(
            "dispatch_task",
            CallableContract::new(StaticType::Null)
                .required("task_id", string())
                .required("prompt", string()),
        );
        let report = check(&doc, &catalog, ExecutionProfile::ACCESSOR_VALUES);
        let codes: Vec<_> = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.contains(&"PX1003"));
        assert!(codes.contains(&"PX1004"));
        assert!(codes.contains(&"PX1005"));
    }

    #[test]
    fn rejects_accessor_when_runtime_cannot_lower_it() {
        let doc = procedure(StepCall {
            action: id("dispatch_task"),
            args: StepCallArgs::Params(vec![(
                id("task_id"),
                Value::Var(VarRef {
                    name: id("task"),
                    accessors: vec![Accessor::Dot(id("id"))],
                    span: None,
                }),
            )]),
            output: None,
        });
        let mut catalog = ContractCatalog::default();
        catalog.insert(
            "dispatch_task",
            CallableContract::new(StaticType::Null).required("task_id", string()),
        );
        let report = check(&doc, &catalog, ExecutionProfile::STRICT);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PX1007"));
    }

    #[test]
    fn accepts_typed_accessor_when_runtime_advertises_support() {
        let doc = procedure(StepCall {
            action: id("dispatch_task"),
            args: StepCallArgs::Params(vec![(
                id("task_id"),
                Value::Var(VarRef {
                    name: id("task"),
                    accessors: vec![Accessor::Dot(id("id"))],
                    span: None,
                }),
            )]),
            output: None,
        });
        let mut catalog = ContractCatalog::default();
        catalog.insert(
            "dispatch_task",
            CallableContract::new(StaticType::Null).required("task_id", string()),
        );
        assert!(check(&doc, &catalog, ExecutionProfile::ACCESSOR_VALUES).is_valid());
    }

    #[test]
    fn supports_typed_function_contract_modules() {
        let mut catalog = ContractCatalog::default();
        let contracts = PxDocument {
            statements: vec![Statement::Function(px_ast::FunctionDecl {
                name: id("timestamp_now"),
                params: vec![],
                return_type: int(),
                mode: None,
                docstring: None,
                span: None,
            })],
        };
        catalog.add_functions_from(&contracts);
        let doc = procedure(StepCall {
            action: id("timestamp_now"),
            args: StepCallArgs::None,
            output: Some(id("now")),
        });
        assert!(check(&doc, &catalog, ExecutionProfile::STRICT).is_valid());
    }

    #[test]
    fn reads_a_source_local_schema_pin() {
        assert_eq!(
            schema_ref_from_source("# px-schema: plures.agens.spine@2#abc123\nprocedure p() -> string:\n  return \"ok\"\n").unwrap(),
            SchemaRef::new("plures.agens.spine", 2, "abc123")
        );
        assert!(schema_ref_from_source("procedure p() -> string:\n  return \"ok\"\n").is_err());
    }
}
