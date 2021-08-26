// Copyright 2021 Oxide Computer Company

use openapiv3::{OpenAPI, ReferenceOr, Schema, Type};

mod walker;

use walker::SchemaWalker;

/// This is a simple crate to validate OpenAPI v3.0.3 content. It flags
/// constructs that we've determined are not "ergonomic" or "well-designed". In
/// particular we try to avoid constructs that lead to structures that SDK
/// generators would have a hard time turning into easy-to-use native
/// constructs.

pub fn validate(spec: &OpenAPI) -> Vec<String> {
    spec.walk()
        .filter_map(|(name, schema)| {
            validate_schema(spec, schema).map(|msg| {
                format!(
                    "problem with type {}: {}",
                    name.unwrap_or_else(|| "<unknown>".to_string()),
                    msg
                )
            })
        })
        .collect()
}

fn validate_schema(spec: &OpenAPI, schema: &Schema) -> Option<String> {
    let subschemas = subschemas(spec, schema);
    let mut iter = subschemas.into_iter();

    const PRE: &str = "mismatched types between subschemas; this is often \
    due to enums with different data payloads and can be resolved using serde \
    adjacent tagging.";
    const POST: &str = "For more info, see \
    https://github.com/oxidecomputer/openapi-lint#type-mismatch";

    if let Some(first) = iter.next() {
        for ty in iter {
            match (first, ty) {
                (Type::String(_), Type::String(_))
                | (Type::Number(_), Type::Number(_))
                | (Type::Integer(_), Type::Integer(_))
                | (Type::Object(_), Type::Object(_))
                | (Type::Array(_), Type::Array(_))
                | (Type::Boolean {}, Type::Boolean {}) => {}
                (a, b) => {
                    return Some(format!(
                        "{}\nthis schema's type\n{:?}\ndiffers from this\n{:?}\n\n{}",
                        PRE, a, b, POST,
                    ))
                }
            }
        }
    }

    None
}

fn subschemas<'a>(spec: &'a OpenAPI, schema: &'a Schema) -> Vec<&'a Type> {
    match &schema.schema_kind {
        openapiv3::SchemaKind::OneOf { one_of: ofs }
        | openapiv3::SchemaKind::AllOf { all_of: ofs }
        | openapiv3::SchemaKind::AnyOf { any_of: ofs } => ofs
            .iter()
            .flat_map(|subschema| subschemas(spec, resolve(subschema, spec).unwrap()))
            .collect(),
        openapiv3::SchemaKind::Type(t) => vec![t],
        openapiv3::SchemaKind::Any(_) => todo!(),
    }
}

fn resolve<'a>(ref_or_schema: &'a ReferenceOr<Schema>, spec: &'a OpenAPI) -> Option<&'a Schema> {
    match ref_or_schema {
        ReferenceOr::Reference { reference } => {
            const PREFIX: &str = "#/components/schemas/";
            if !reference.starts_with(PREFIX) {
                None
            } else {
                spec.components
                    .as_ref()?
                    .schemas
                    .get(&reference[PREFIX.len()..])
                    .and_then(|ros| resolve(ros, spec))
            }
        }
        ReferenceOr::Item(schema) => Some(schema),
    }
}

#[cfg(test)]
mod tests {
    use crate::validate;

    #[test]
    fn bad_schema() {
        let openapi = serde_json::from_str(include_str!("tests/errors.json")).unwrap();

        let actual = validate(&openapi).join("\n\n");
        expectorate::assert_contents("src/tests/errors.out", &actual);
    }
}
