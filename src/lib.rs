// Copyright 2022 Oxide Computer Company

//! This is a simple crate to validate OpenAPI v3.0.3 content. It flags
//! constructs that we've determined are not "ergonomic" or "well-designed". In
//! particular we try to avoid constructs that lead to structures that SDK
//! generators would have a hard time turning into easy-to-use native
//! constructs.

use heck::{ToPascalCase, ToShoutySnakeCase, ToSnakeCase};
use indexmap::IndexMap;
use openapiv3::{
    AnySchema, Components, OpenAPI, Operation, Parameter, ReferenceOr, Schema, SchemaKind,
    StringType, Type, VariantOrUnknownOrEmpty,
};

mod walker;

use walker::SchemaWalker;

pub fn validate(spec: &OpenAPI) -> Vec<String> {
    Validator::default().validate(spec)
}

struct Validator;

impl Default for Validator {
    fn default() -> Self {
        Self
    }
}

impl Validator {
    pub fn validate(&self, spec: &OpenAPI) -> Vec<String> {
        let schema = spec.walk().flat_map(|(name, schema)| {
            let subs = self.validate_subschemas(spec, schema).map(|msg| {
                format!(
                    "Problem with type {}: {}",
                    name.unwrap_or_else(|| "<unknown>".to_string()),
                    msg
                )
            });
            let properties = self.validate_object(schema);
            let enum_values = self.validate_enumeration_value(schema);
            subs.into_iter().chain(properties).chain(enum_values)
        });

        let operations = spec
            .operations()
            .filter_map(|path_method_op| self.validate_operation_id(path_method_op));
        let parameters = spec
            .operations()
            .flat_map(|(_, _, op)| self.validate_operation_parameters(spec, op));
        let named_schemas = spec.components.iter().flat_map(|components| {
            components
                .schemas
                .keys()
                .filter_map(|type_name| self.validate_named_schema(type_name))
        });

        schema
            .chain(operations)
            .chain(parameters)
            .chain(named_schemas)
            .collect()
    }

    fn validate_subschemas(&self, spec: &OpenAPI, schema: &Schema) -> Option<String> {
        let subschemas = self.subschemas(spec, schema);
        let mut iter = subschemas.into_iter();

        const PRE: &str = "Mismatched types between subschemas; this is often \
            due to enums with different data payloads and can be resolved \
            using serde adjacent tagging.";
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

    fn subschemas<'a>(&self, spec: &'a OpenAPI, schema: &'a Schema) -> Vec<&'a Type> {
        match &schema.schema_kind {
            openapiv3::SchemaKind::OneOf { one_of: ofs }
            | openapiv3::SchemaKind::AllOf { all_of: ofs }
            | openapiv3::SchemaKind::AnyOf { any_of: ofs } => ofs
                .iter()
                .flat_map(|subschema| {
                    self.subschemas(spec, subschema.item(&spec.components).unwrap())
                })
                .collect(),
            openapiv3::SchemaKind::Not { .. } => todo!("'not' subschemas aren't handled"),
            openapiv3::SchemaKind::Type(t) => vec![t],
            openapiv3::SchemaKind::Any(any) if is_permissive(any) => vec![],
            openapiv3::SchemaKind::Any(_) => todo!("complex 'any' schema not handled"),
        }
    }

    fn validate_object(&self, schema: &Schema) -> Vec<String> {
        let mut ret = Vec::new();

        if let openapiv3::SchemaKind::Type(Type::Object(obj)) = &schema.schema_kind {
            for prop_name in obj.properties.keys() {
                let snake = prop_name.to_snake_case();
                if prop_name.clone() != snake {
                    ret.push(format!(
                        "An object contains a property '{}' which is not \
                        snake_case:\n{:#?}\n\
                        Add #[serde(rename = \"{}\")] to the member or \
                        #[serde(rename_all = \"snake_case\")] to the struct.\n\
                        For more info see \
                        https://github.com/oxidecomputer/openapi-lint#naming",
                        prop_name, schema, snake
                    ))
                }
            }

            for (prop_name, prop_schema) in obj.properties.iter() {
                if prop_name.ends_with("_uuid") {
                    match prop_schema.as_item().map(Box::as_ref) {
                        Some(Schema {
                            schema_kind:
                                SchemaKind::Type(Type::String(StringType {
                                    format: VariantOrUnknownOrEmpty::Unknown(format),
                                    pattern: None,
                                    enumeration,
                                    min_length: None,
                                    max_length: None,
                                })),
                            ..
                        }) if format == "uuid" && enumeration.is_empty() => ret.push(format!(
                            "An object contains a property '{}' that is a \
                            uuid and redundantly ends with `_uuid`'; rename \
                            this property to `{}_id`.\n\
                            For more info see \
                            https://github.com/oxidecomputer/openapi-lint#uuids",
                            prop_name,
                            prop_name.trim_end_matches("_uuid"),
                        )),
                        _ => (),
                    }
                }
            }
        }

        ret
    }

    fn validate_enumeration_value(&self, schema: &Schema) -> Vec<String> {
        let mut ret = Vec::new();

        if let openapiv3::SchemaKind::Type(Type::String(StringType { enumeration, .. })) =
            &schema.schema_kind
        {
            enumeration.iter().for_each(|enum_value| {
                if let Some(label) = enum_value {
                    let lower = label.to_snake_case();
                    let upper = label.to_shouty_snake_case();
                    if label != &lower && label != &upper {
                        ret.push(format!(
                            "An enumerated string contains a value '{}' that \
                            is neither snake_case nor \
                            SCREAMING_SNAKE_CASE:\n{:#?}\n\
                            Add #[serde(rename = \"{}\")] to the variant or \
                            #[serde(rename_all = \"snake_case\")] to the enum.\n\
                            For more info see \
                            https://github.com/oxidecomputer/openapi-lint#naming",
                            label, schema, lower
                        ));
                    }
                }
            });
        }
        ret
    }

    fn validate_operation_id(&self, path_method_op: (&str, &str, &Operation)) -> Option<String> {
        let (path, method, op) = path_method_op;

        const INFO: &str = "For more info, see \
            https://github.com/oxidecomputer/openapi-lint#naming";

        if let Some(operation_id) = &op.operation_id {
            let snake = operation_id.to_snake_case();
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

    fn validate_operation_parameters(&self, spec: &OpenAPI, op: &Operation) -> Vec<String> {
        const INFO: &str = "For more info, see \
            https://github.com/oxidecomputer/openapi-lint#naming";

        let operation_id = op.operation_id.as_deref().unwrap_or("<unknown>");
        op.parameters
            .iter()
            .filter_map(|ref_or_param| {
                let param = ref_or_param.item(&spec.components)?;

                let name = &param.parameter_data_ref().name;
                let snake = name.to_snake_case();

                if name.as_str() != snake {
                    Some(format!(
                        "The parameter \"{}\" to {} should be snake_case.\n{}",
                        name, operation_id, INFO,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    fn validate_named_schema(&self, type_name: &str) -> Option<String> {
        const INFO: &str = "For more info, see \
            https://github.com/oxidecomputer/openapi-lint#naming";

        let pascal = type_name.to_pascal_case();
        if type_name == pascal {
            return None;
        }

        Some(format!(
            "The type \"{}\" has a name that is not PascalCase; to rename it add \
            #[serde(rename = \"{}\")]\n{}",
            type_name, pascal, INFO,
        ))
    }
}

fn is_permissive(any: &AnySchema) -> bool {
    match any {
        AnySchema {
            typ: None,
            pattern: None,
            multiple_of: None,
            exclusive_minimum: None,
            exclusive_maximum: None,
            minimum: None,
            maximum: None,
            properties,
            required,
            additional_properties: None,
            min_properties: None,
            max_properties: None,
            items: None,
            min_items: None,
            max_items: None,
            unique_items: None,
            enumeration,
            format: None,
            min_length: None,
            max_length: None,
            one_of,
            all_of,
            any_of,
            not: None,
        } if properties.is_empty()
            && required.is_empty()
            && enumeration.is_empty()
            && one_of.is_empty()
            && all_of.is_empty()
            && any_of.is_empty() =>
        {
            true
        }
        _ => false,
    }
}

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
    use heck::ToSnakeCase;

    use crate::validate;

    #[test]
    fn bad_schema() {
        let openapi = serde_json::from_str(include_str!("tests/errors.json")).unwrap();

        let actual = validate(&openapi).join("\n\n");
        expectorate::assert_contents("src/tests/errors.out", &actual);
    }

    #[test]
    fn test_ipv6() {
        assert_eq!("ipv6".to_snake_case(), "ipv6");
        assert_eq!("the_ipv6_network".to_snake_case(), "the_ipv6_network");
    }
}
