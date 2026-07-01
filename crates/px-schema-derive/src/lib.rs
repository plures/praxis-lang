use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derive macro for generating Praxis schema metadata.
///
/// # Usage
///
/// ```ignore
/// use px_schema_derive::PxSchema;
///
/// #[derive(PxSchema)]
/// #[px_schema(construct = "entity", description = "Defines a data shape")]
/// pub struct PxEntity {
///     #[px_schema(description = "Entity name", required = true)]
///     pub name: String,
///     
///     #[px_schema(description = "Optional prefix")]
///     pub prefix: Option<String>,
/// }
/// ```
#[proc_macro_derive(PxSchema, attributes(px_schema))]
pub fn derive_px_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let attrs = &input.attrs;

    // Parse struct-level attributes
    let mut construct_name = None;
    let mut construct_desc = String::new();

    for attr in attrs {
        if attr.path().is_ident("px_schema") {
            if let syn::Meta::List(_meta_list) = &attr.meta {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("construct") {
                        let value: syn::LitStr = meta.value()?.parse()?;
                        construct_name = Some(value.value());
                    } else if meta.path.is_ident("description") {
                        let value: syn::LitStr = meta.value()?.parse()?;
                        construct_desc = value.value();
                    }
                    Ok(())
                })
                .ok();
            }
        }
    }

    let construct_name = construct_name.unwrap_or_else(|| name.to_string().to_lowercase());

    // Parse field-level attributes
    let mut field_schemas = Vec::new();
    let mut required_fields = Vec::new();

    if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields) = &data_struct.fields {
            for field in &fields.named {
                let field_name = field.ident.as_ref().unwrap().to_string();
                let mut field_desc = String::new();
                let mut field_required = false;
                let mut schema_type = None;
                let default_value: Option<String> = None;
                let mut example = None;
                let one_of: Option<Vec<String>> = None;

                for attr in &field.attrs {
                    if attr.path().is_ident("px_schema") {
                        attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("description") {
                                let value: syn::LitStr = meta.value()?.parse()?;
                                field_desc = value.value();
                            } else if meta.path.is_ident("required") {
                                let value: syn::LitBool = meta.value()?.parse()?;
                                field_required = value.value();
                            } else if meta.path.is_ident("schema_type") {
                                let value: syn::LitStr = meta.value()?.parse()?;
                                schema_type = Some(value.value());
                            } else if meta.path.is_ident("example") {
                                let value: syn::LitStr = meta.value()?.parse()?;
                                example = Some(value.value());
                            }
                            Ok(())
                        })
                        .ok();
                    }
                }

                // Infer type from field type if not explicitly specified
                let inferred_type = schema_type.unwrap_or_else(|| infer_type(&field.ty));

                if field_required {
                    required_fields.push(field_name.clone());
                }

                field_schemas.push((
                    field_name,
                    field_desc,
                    inferred_type,
                    field_required,
                    default_value,
                    example,
                    one_of,
                ));
            }
        }
    }

    // Generate the implementation
    let field_entries = field_schemas
        .iter()
        .map(|(name, desc, ty, req, def, ex, _one_of)| {
            let default_code = if let Some(d) = def {
                quote! { Some(serde_json::json!(#d)) }
            } else {
                quote! { None }
            };

            let example_code = if let Some(e) = ex {
                quote! { Some(#e.to_string()) }
            } else {
                quote! { None }
            };

            let one_of_code = quote! { None }; // TODO: support one_of

            quote! {
                (#name.to_string(), px_schema::SchemaField {
                    description: #desc.to_string(),
                    field_type: #ty.to_string(),
                    required: #req,
                    default: #default_code,
                    example: #example_code,
                    one_of: #one_of_code,
                })
            }
        });

    let expanded = quote! {
        impl #name {
            pub fn px_schema_entry() -> px_schema::SchemaConstruct {
                use std::collections::HashMap;

                let mut fields = HashMap::new();
                #(fields.insert #field_entries);*;

                px_schema::SchemaConstruct {
                    description: #construct_desc.to_string(),
                    required: vec![#(#required_fields.to_string()),*],
                    fields,
                }
            }

            pub fn px_schema_construct_name() -> &'static str {
                #construct_name
            }
        }
    };

    TokenStream::from(expanded)
}

fn infer_type(ty: &syn::Type) -> String {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();

            match ident.as_str() {
                "String" => return "string".to_string(),
                "bool" => return "boolean".to_string(),
                "i32" | "i64" | "u32" | "u64" | "usize" => return "number".to_string(),
                "f32" | "f64" => return "number".to_string(),
                "Vec" => return "array".to_string(),
                "Option" => {
                    // Try to infer the inner type
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            return infer_type(inner_ty);
                        }
                    }
                    return "any".to_string();
                }
                "HashMap" | "BTreeMap" => return "object".to_string(),
                _ => return ident.to_lowercase(),
            }
        }
    }

    "any".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_type() {
        // Basic smoke test - actual macro tests need integration testing
        assert_eq!(infer_type(&syn::parse_str("String").unwrap()), "string");
        assert_eq!(infer_type(&syn::parse_str("bool").unwrap()), "boolean");
        assert_eq!(infer_type(&syn::parse_str("i32").unwrap()), "number");
    }
}
