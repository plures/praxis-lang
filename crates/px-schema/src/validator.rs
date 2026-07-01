use crate::types::*;
use serde_json::Value;
use std::collections::HashSet;

/// Result of schema validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add_error(&mut self, path: String, message: String, suggestion: Option<String>) {
        self.errors.push(ValidationError {
            path,
            message,
            suggestion,
        });
    }

    pub fn add_warning(&mut self, path: String, message: String) {
        self.warnings.push(ValidationWarning { path, message });
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// A validation error indicating the document is invalid.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// A validation warning for non-fatal issues (e.g., unknown keys).
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub path: String,
    pub message: String,
}

/// Validate a parsed .px document against a schema.
pub fn validate(doc: &Value, schema: &PxSchemaDocument) -> ValidationResult {
    let mut result = ValidationResult::new();

    if let Some(obj) = doc.as_object() {
        validate_object(obj, "", schema, &mut result);
    } else {
        result.add_error(
            "".to_string(),
            "Document root must be an object".to_string(),
            None,
        );
    }

    result
}

fn validate_object(
    obj: &serde_json::Map<String, Value>,
    path: &str,
    schema: &PxSchemaDocument,
    result: &mut ValidationResult,
) {
    // Check each key in the object
    for (key, value) in obj {
        let current_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", path, key)
        };

        // Check if this key corresponds to a known construct
        if let Some(construct) = schema.constructs.get(key) {
            if let Some(construct_obj) = value.as_object() {
                validate_construct(construct_obj, &current_path, construct, result);
            } else {
                result.add_error(
                    current_path.clone(),
                    format!("Construct '{}' must be an object", key),
                    None,
                );
            }
        } else {
            // Unknown key at top level - warning, not error
            result.add_warning(
                current_path.clone(),
                format!("Unknown construct '{}' — will be ignored", key),
            );
        }
    }
}

fn validate_construct(
    obj: &serde_json::Map<String, Value>,
    path: &str,
    construct: &SchemaConstruct,
    result: &mut ValidationResult,
) {
    let present_keys: HashSet<String> = obj.keys().cloned().collect();
    let _schema_keys: HashSet<String> = construct.fields.keys().cloned().collect();

    // Check required fields
    for required in &construct.required {
        if !present_keys.contains(required) {
            let field_schema = construct.fields.get(required);
            let suggestion = field_schema
                .and_then(|f| f.example.clone())
                .or_else(|| Some(format!("Add '{}' field", required)));

            result.add_error(
                path.to_string(),
                format!("Missing required field '{}'", required),
                suggestion,
            );
        }
    }

    // Check each present field
    for (key, value) in obj {
        let field_path = format!("{}.{}", path, key);

        if let Some(field_schema) = construct.fields.get(key) {
            validate_field(value, &field_path, field_schema, result);
        } else {
            // Unknown field - warning, not error (per design decision)
            result.add_warning(
                field_path,
                format!("Unknown field '{}' — will be ignored", key),
            );
        }
    }
}

fn validate_field(
    value: &Value,
    path: &str,
    field_schema: &SchemaField,
    result: &mut ValidationResult,
) {
    let actual_type = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };

    let expected_type = field_schema.field_type.as_str();

    // Basic type checking (can be extended for more sophisticated validation)
    let types_match = match expected_type {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "any" => true,
        _ => true, // Custom types - assume valid for now
    };

    if !types_match {
        result.add_error(
            path.to_string(),
            format!(
                "Type mismatch: expected {}, got {}",
                expected_type, actual_type
            ),
            field_schema.example.clone(),
        );
    }

    // If one_of is specified, check that the value matches one of the options
    if let Some(one_of) = &field_schema.one_of {
        if let Some(string_value) = value.as_str() {
            if !one_of.contains(&string_value.to_string()) {
                result.add_error(
                    path.to_string(),
                    format!("Value must be one of: {}", one_of.join(", ")),
                    Some(format!("Valid options: {}", one_of.join(", "))),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_schema() -> PxSchemaDocument {
        let mut doc = PxSchemaDocument::default();

        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            SchemaField {
                description: "Entity name".to_string(),
                field_type: "string".to_string(),
                required: true,
                default: None,
                example: Some("\"Player\"".to_string()),
                one_of: None,
            },
        );
        fields.insert(
            "health".to_string(),
            SchemaField {
                description: "Health points".to_string(),
                field_type: "number".to_string(),
                required: false,
                default: Some(serde_json::json!(100)),
                example: None,
                one_of: None,
            },
        );

        doc.constructs.insert(
            "entity".to_string(),
            SchemaConstruct {
                description: "Game entity".to_string(),
                required: vec!["name".to_string()],
                fields,
            },
        );

        doc
    }

    #[test]
    fn test_valid_document() {
        let schema = create_test_schema();
        let doc = serde_json::json!({
            "entity": {
                "name": "Player",
                "health": 100
            }
        });

        let result = validate(&doc, &schema);
        assert!(result.is_valid());
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn test_missing_required_field() {
        let schema = create_test_schema();
        let doc = serde_json::json!({
            "entity": {
                "health": 100
            }
        });

        let result = validate(&doc, &schema);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0]
            .message
            .contains("Missing required field 'name'"));
    }

    #[test]
    fn test_type_mismatch() {
        let schema = create_test_schema();
        let doc = serde_json::json!({
            "entity": {
                "name": "Player",
                "health": "not a number"
            }
        });

        let result = validate(&doc, &schema);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.message.contains("Type mismatch")));
    }

    #[test]
    fn test_unknown_field_warning() {
        let schema = create_test_schema();
        let doc = serde_json::json!({
            "entity": {
                "name": "Player",
                "unknown_field": "value"
            }
        });

        let result = validate(&doc, &schema);
        assert!(result.is_valid()); // Unknown fields don't make it invalid
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0]
            .message
            .contains("Unknown field 'unknown_field'"));
    }

    #[test]
    fn test_unknown_construct_warning() {
        let schema = create_test_schema();
        let doc = serde_json::json!({
            "unknown_construct": {
                "foo": "bar"
            }
        });

        let result = validate(&doc, &schema);
        assert!(result.is_valid()); // Unknown constructs don't make it invalid
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0]
            .message
            .contains("Unknown construct 'unknown_construct'"));
    }

    #[test]
    fn test_one_of_validation() {
        let mut doc = PxSchemaDocument::default();
        let mut fields = HashMap::new();
        fields.insert(
            "status".to_string(),
            SchemaField {
                description: "Status".to_string(),
                field_type: "string".to_string(),
                required: true,
                default: None,
                example: None,
                one_of: Some(vec!["active".to_string(), "inactive".to_string()]),
            },
        );
        doc.constructs.insert(
            "test".to_string(),
            SchemaConstruct {
                description: "Test".to_string(),
                required: vec!["status".to_string()],
                fields,
            },
        );

        // Valid value
        let valid_doc = serde_json::json!({
            "test": {
                "status": "active"
            }
        });
        let result = validate(&valid_doc, &doc);
        assert!(result.is_valid());

        // Invalid value
        let invalid_doc = serde_json::json!({
            "test": {
                "status": "unknown"
            }
        });
        let result = validate(&invalid_doc, &doc);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.message.contains("must be one of")));
    }
}
