use crate::types::*;

/// Builder for constructing a schema document from annotated types.
pub struct SchemaGenerator {
    document: PxSchemaDocument,
}

impl SchemaGenerator {
    pub fn new() -> Self {
        Self {
            document: PxSchemaDocument::default(),
        }
    }

    /// Register a construct in the schema.
    pub fn add_construct(&mut self, name: impl Into<String>, construct: SchemaConstruct) {
        self.document.constructs.insert(name.into(), construct);
    }

    /// Register a custom type in the schema.
    pub fn add_type(&mut self, name: impl Into<String>, schema_type: SchemaType) {
        self.document.types.insert(name.into(), schema_type);
    }

    /// Set the Praxis version.
    pub fn px_version(&mut self, version: impl Into<String>) {
        self.document.px_version = version.into();
    }

    /// Set the schema format version.
    pub fn schema_version(&mut self, version: impl Into<String>) {
        self.document.schema_version = version.into();
    }

    /// Build and return the final schema document.
    pub fn build(self) -> PxSchemaDocument {
        self.document
    }

    /// Build and serialize to YAML.
    pub fn build_yaml(self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.document)
    }

    /// Build and serialize to JSON.
    pub fn build_json(self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.document)
    }
}

impl Default for SchemaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_schema_generator() {
        let mut gen = SchemaGenerator::new();
        gen.px_version("2.0.0");
        gen.schema_version("1.0.0");

        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            SchemaField {
                description: "Entity name".to_string(),
                field_type: "string".to_string(),
                required: true,
                default: None,
                example: None,
                one_of: None,
            },
        );

        gen.add_construct(
            "entity",
            SchemaConstruct {
                description: "Defines a data shape".to_string(),
                required: vec!["name".to_string()],
                fields,
            },
        );

        let doc = gen.build();
        assert_eq!(doc.px_version, "2.0.0");
        assert_eq!(doc.constructs.len(), 1);
        assert!(doc.constructs.contains_key("entity"));
    }

    #[test]
    fn test_yaml_generation() {
        let mut gen = SchemaGenerator::new();

        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            SchemaField {
                description: "The name".to_string(),
                field_type: "string".to_string(),
                required: true,
                default: None,
                example: Some("\"example\"".to_string()),
                one_of: None,
            },
        );

        gen.add_construct(
            "test_construct",
            SchemaConstruct {
                description: "Test construct".to_string(),
                required: vec!["name".to_string()],
                fields,
            },
        );

        let yaml = gen.build_yaml().unwrap();
        assert!(yaml.contains("test_construct"));
        assert!(yaml.contains("The name"));
        assert!(yaml.contains("example"));
    }
}
