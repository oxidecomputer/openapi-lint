// Copyright 2022 Oxide Computer Company

//! This is a simple crate to validate OpenAPI v3.0.3 content. It flags
//! constructs that we've determined are not "ergonomic" or "well-designed". In
//! particular we try to avoid constructs that lead to structures that SDK
//! generators would have a hard time turning into easy-to-use native
//! constructs.

use convert_case::{Case, Casing};
use indexmap::IndexMap;
use openapiv3::{Components, OpenAPI, Operation, Parameter, ReferenceOr, Schema, Type};

mod walker;

use walker::SchemaWalker;

pub fn validate(spec: &OpenAPI) -> Vec<String> {
    let schema = spec.walk().flat_map(|(name, schema)| {
        validate_subschemas(spec, schema)
            .map(|msg| {
                format!(
                    "problem with type {}: {}",
                    name.unwrap_or_else(|| "<unknown>".to_string()),
                    msg
                )
            })
            .into_iter()
            .chain(validate_object_camel_case(schema))
    });

    let operations = spec.operations().filter_map(validate_operation_id);
    let parameters = spec
        .operations()
        .flat_map(|(_, _, op)| validate_operation_parameters(spec, op));
    let named_schemas = spec.components.iter().flat_map(|components| {
        components
            .schemas
            .keys()
            .filter_map(|type_name| validate_named_schema(type_name))
    });

    schema
        .chain(operations)
        .chain(parameters)
        .chain(named_schemas)
        .collect()
}

fn validate_subschemas(spec: &OpenAPI, schema: &Schema) -> Option<String> {
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
                        "{}\nthis schema's type\n{:#?}\ndiffers from this\n{:#?}\n\n{}",
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
            .flat_map(|subschema| subschemas(spec, subschema.item(&spec.components).unwrap()))
            .collect(),
        openapiv3::SchemaKind::Not { .. } => todo!(),
        openapiv3::SchemaKind::Type(t) => vec![t],
        openapiv3::SchemaKind::Any(_) => todo!(),
    }
}

fn validate_object_camel_case(schema: &Schema) -> Vec<String> {
    let mut ret = Vec::new();

    if let openapiv3::SchemaKind::Type(Type::Object(obj)) = &schema.schema_kind {
        for prop_name in obj.properties.keys() {
            let camel = prop_name.to_case(Case::Camel);
            if prop_name.clone() != camel {
                ret.push(format!(
                    "an object contains a property '{}' which is not \
                    camelCase:\n{:#?}\n\
                    Add #[serde(rename = \"{}\")] to the member or \
                    #[serde(rename_all = \"camelCase\" to the object\n\
                    For more info see \
                    https://github.com/oxidecomputer/openapi-lint#naming",
                    prop_name, schema, camel
                ))
            }
        }
    }

    ret
}

fn validate_operation_id(path_method_op: (&str, &str, &Operation)) -> Option<String> {
    let (path, method, op) = path_method_op;

    const INFO: &str = "For more info, see \
    https://github.com/oxidecomputer/openapi-lint#naming";

    if let Some(operation_id) = &op.operation_id {
        let snake = operation_id.to_case(Case::Snake);
        if operation_id.as_str() == snake {
            return None;
        }
        Some(format!(
            "The operation for {} {} is named \"{}\" which is not snake_case\n{}",
            path, method, operation_id, INFO,
        ))
    } else {
        Some(format!(
            "The operation for {} {} does not have an operation_id\n{}",
            path, method, INFO,
        ))
    }
}

fn validate_operation_parameters(spec: &OpenAPI, op: &Operation) -> Vec<String> {
    const INFO: &str = "For more info, see \
    https://github.com/oxidecomputer/openapi-lint#naming";

    let operation_id = op.operation_id.as_deref().unwrap_or("<unknown>");
    op.parameters
        .iter()
        .filter_map(|ref_or_param| {
            let param = ref_or_param.item(&spec.components)?;

            let name = &param.parameter_data_ref().name;
            let camel = name.to_case(Case::Camel);

            if name.as_str() != camel {
                Some(format!(
                    "The parameter \"{}\" to {} should be camelCase.\n{}",
                    name, operation_id, INFO,
                ))
            } else {
                None
            }
        })
        .collect()
}

fn validate_named_schema(type_name: &str) -> Option<String> {
    const INFO: &str = "For more info, see \
    https://github.com/oxidecomputer/openapi-lint#naming";

    let pascal = type_name.to_case(Case::Pascal);
    if type_name == pascal {
        return None;
    }

    Some(format!(
        "The type \"{}\" has a name that is not PascalCase; to rename it add \
        #[serde(rename = \"{}\")]\n{}",
        type_name, pascal, INFO,
    ))
}

// fn resolve<'a>(ref_or_schema: &'a ReferenceOr<Schema>, spec: &'a OpenAPI) -> Option<&'a Schema> {
//     match ref_or_schema {
//         ReferenceOr::Reference { reference } => {
//             const PREFIX: &str = "#/components/schemas/";
//             if !reference.starts_with(PREFIX) {
//                 None
//             } else {
//                 spec.components
//                     .as_ref()?
//                     .schemas
//                     .get(&reference[PREFIX.len()..])
//                     .and_then(|ros| resolve(ros, spec))
//             }
//         }
//         ReferenceOr::Item(schema) => Some(schema),
//     }
// }

trait ReferenceOrExt<T: ComponentLookup> {
    fn item<'a>(&'a self, components: &'a Option<Components>) -> Option<&'a T>;
}
trait ComponentLookup: Sized {
    fn get_components(components: &Components) -> &IndexMap<String, ReferenceOr<Self>>;
}

impl<T: ComponentLookup> ReferenceOrExt<T> for openapiv3::ReferenceOr<T> {
    fn item<'a>(&'a self, components: &'a Option<Components>) -> Option<&'a T> {
        match self {
            ReferenceOr::Item(item) => Some(item),
            ReferenceOr::Reference { reference } => {
                let idx = reference.rfind('/').unwrap();
                let key = &reference[idx + 1..];
                let parameters = T::get_components(components.as_ref().unwrap());
                parameters.get(key).unwrap().item(components)
            }
        }
    }
}

impl ComponentLookup for Parameter {
    fn get_components(components: &Components) -> &IndexMap<String, ReferenceOr<Self>> {
        &components.parameters
    }
}

impl ComponentLookup for Schema {
    fn get_components(components: &Components) -> &IndexMap<String, ReferenceOr<Self>> {
        &components.schemas
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
