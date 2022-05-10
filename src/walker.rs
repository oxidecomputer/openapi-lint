// Copyright 2022 Oxide Computer Company

use indexmap::IndexMap;
use openapiv3::{
    AdditionalProperties, AnySchema, ArrayType, Components, MediaType, ObjectType, OpenAPI,
    Operation, Parameter, ParameterSchemaOrContent, PathItem, ReferenceOr, RequestBody, Response,
    Schema, Type,
};

pub(crate) trait SchemaWalker<'a> {
    type SchemaIterator: Iterator<Item = (Option<String>, &'a Schema)>;
    fn walk(&'a self) -> Self::SchemaIterator;
}

impl<'a> SchemaWalker<'a> for OpenAPI {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.paths
            .iter()
            .flat_map(|(_, path)| path.walk())
            .chain(self.components.walk())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a, T> SchemaWalker<'a> for ReferenceOr<T>
where
    T: SchemaWalker<'a>,
{
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        match self {
            ReferenceOr::Reference { .. } => vec![].into_iter(),
            ReferenceOr::Item(walker) => walker.walk().collect::<Vec<_>>().into_iter(),
        }
    }
}

impl<'a, T> SchemaWalker<'a> for Box<T>
where
    T: SchemaWalker<'a>,
{
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.as_ref().walk().collect::<Vec<_>>().into_iter()
    }
}

impl<'a, T> SchemaWalker<'a> for Option<T>
where
    T: SchemaWalker<'a>,
{
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        match self {
            None => Vec::<(Option<String>, &Schema)>::new().into_iter(),
            Some(walker) => walker.walk().collect::<Vec<_>>().into_iter(),
        }
    }
}

impl<'a> SchemaWalker<'a> for IndexMap<String, ReferenceOr<Schema>> {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.iter()
            .flat_map(|(key, value)| {
                value
                    .walk()
                    .map(|(_, schema)| (Some(key.clone()), schema))
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a, K> SchemaWalker<'a> for IndexMap<K, MediaType> {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.iter()
            .flat_map(|(_key, value)| value.walk())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a, K> SchemaWalker<'a> for IndexMap<K, ReferenceOr<Response>> {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.iter()
            .flat_map(|(_key, value)| value.walk())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a> SchemaWalker<'a> for PathItem {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.iter()
            .flat_map(|(_, op)| SchemaWalker::walk(op))
            .chain(self.parameters.iter().flat_map(SchemaWalker::walk))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a> SchemaWalker<'a> for Operation {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.parameters
            .iter()
            .flat_map(SchemaWalker::walk)
            .chain(self.request_body.walk())
            .chain(self.responses.default.walk())
            .chain(self.responses.responses.walk())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a> SchemaWalker<'a> for Components {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.responses
            .walk()
            .chain(
                self.parameters
                    .iter()
                    .flat_map(|(_, parameter)| parameter.walk()),
            )
            .chain(
                self.request_bodies
                    .iter()
                    .flat_map(|(_, request_body)| request_body.walk()),
            )
            .chain(self.schemas.walk())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a> SchemaWalker<'a> for Response {
    type SchemaIterator = <IndexMap<String, MediaType> as SchemaWalker<'a>>::SchemaIterator;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.content.walk()
    }
}

impl<'a> SchemaWalker<'a> for MediaType {
    type SchemaIterator = <Option<Schema> as SchemaWalker<'a>>::SchemaIterator;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.schema.walk()
    }
}

impl<'a> SchemaWalker<'a> for Parameter {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        match self {
            Parameter::Query { parameter_data, .. } => parameter_data,
            Parameter::Header { parameter_data, .. } => parameter_data,
            Parameter::Path { parameter_data, .. } => parameter_data,
            Parameter::Cookie { parameter_data, .. } => parameter_data,
        }
        .format
        .walk()
    }
}

impl<'a> SchemaWalker<'a> for ParameterSchemaOrContent {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        match self {
            ParameterSchemaOrContent::Schema(schema) => {
                schema.walk().collect::<Vec<_>>().into_iter()
            }
            ParameterSchemaOrContent::Content(content) => {
                content.walk().collect::<Vec<_>>().into_iter()
            }
        }
    }
}

impl<'a> SchemaWalker<'a> for RequestBody {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        self.content.walk()
    }
}

impl<'a> SchemaWalker<'a> for Schema {
    type SchemaIterator = std::vec::IntoIter<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        let children: Vec<_> = match &self.schema_kind {
            // Objects have properties and additional (i.e. arbitrarily-
            // named) properties that have schemas.
            openapiv3::SchemaKind::Type(Type::Object(ObjectType {
                properties,
                additional_properties,
                ..
            })) => {
                let additional = match additional_properties {
                    Some(AdditionalProperties::Schema(schema)) => schema.walk().collect(),
                    _ => vec![],
                };
                properties
                    .iter()
                    .flat_map(|(_, prop)| match prop {
                        ReferenceOr::Reference { .. } => vec![],
                        ReferenceOr::Item(schema) => schema.walk().collect(),
                    })
                    .chain(additional)
                    .collect()
            }
            // Arrays have items with schemas.
            openapiv3::SchemaKind::Type(Type::Array(ArrayType {
                items: Some(schema),
                ..
            })) => schema.walk().collect(),
            // Other types don't have subordinate schemas.
            openapiv3::SchemaKind::Type(_) => vec![],

            // Lists of subschemas...
            openapiv3::SchemaKind::OneOf { one_of: subschemas }
            | openapiv3::SchemaKind::AllOf { all_of: subschemas }
            | openapiv3::SchemaKind::AnyOf { any_of: subschemas } => {
                subschemas.iter().flat_map(SchemaWalker::walk).collect()
            }
            // Not is an odd case, but it should still be formatted properly...
            openapiv3::SchemaKind::Not { not } => not.walk().collect(),

            // TODO we may need to look in here...
            openapiv3::SchemaKind::Any(AnySchema { .. }) => vec![],
        };

        children
            .into_iter()
            .chain(std::iter::once((None, self)))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
