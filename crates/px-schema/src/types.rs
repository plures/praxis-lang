use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The complete schema document describing all Praxis constructs and types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PxSchemaDocument {
    pub schema_version: String,
    pub px_version: String,
    pub constructs: HashMap<String, SchemaConstruct>,
    pub types: HashMap<String, SchemaType>,
}

impl Default for PxSchemaDocument {
    fn default() -> Self {
        Self {
            schema_version: "1.0.0".to_string(),
            px_version: "2.0.0".to_string(),
            constructs: HashMap::new(),
            types: HashMap::new(),
        }
    }
}

/// Describes a single construct (e.g., entity, procedure, expectation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaConstruct {
    pub description: String,
    pub required: Vec<String>,
    pub fields: HashMap<String, SchemaField>,
}

/// Describes a field within a construct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub description: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_of: Option<Vec<String>>,
}

/// Describes a custom type that can be referenced by fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaType {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_of: Option<HashMap<String, SchemaVariant>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
}

/// Describes a variant within a sum type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVariant {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<HashMap<String, SchemaField>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_document_serialization() {
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

        doc.constructs.insert(
            "entity".to_string(),
            SchemaConstruct {
                description: "Defines a data shape".to_string(),
                required: vec!["name".to_string()],
                fields,
            },
        );

        let yaml = serde_yaml::to_string(&doc).unwrap();
        assert!(yaml.contains("schema_version"));
        assert!(yaml.contains("entity"));
        assert!(yaml.contains("Entity name"));

        // Round-trip test
        let deserialized: PxSchemaDocument = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.schema_version, doc.schema_version);
        assert!(deserialized.constructs.contains_key("entity"));
    }

    #[test]
    fn test_schema_field_optional_fields() {
        let field = SchemaField {
            description: "Test field".to_string(),
            field_type: "string".to_string(),
            required: false,
            default: None,
            example: None,
            one_of: None,
        };

        let json = serde_json::to_string(&field).unwrap();
        // Optional None fields should not appear in JSON
        assert!(!json.contains("default"));
        assert!(!json.contains("example"));
        assert!(!json.contains("one_of"));
    }
}
