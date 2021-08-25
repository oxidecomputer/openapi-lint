// Copyright 2021 Oxide Computer Company

use indexmap::IndexMap;
use openapiv3::{
    Components, MediaType, OpenAPI, Operation, Parameter, ParameterSchemaOrContent, PathItem,
    ReferenceOr, RequestBody, Response, Schema,
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
            ReferenceOr::Reference { .. } => Vec::<(Option<String>, &Schema)>::new().into_iter(),
            ReferenceOr::Item(walker) => walker.walk().collect::<Vec<_>>().into_iter(),
        }
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
            .flat_map(SchemaWalker::walk)
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
    type SchemaIterator = std::iter::Once<(Option<String>, &'a Schema)>;

    fn walk(&'a self) -> Self::SchemaIterator {
        // TODO deal with subschemas (any_of, one_of, all_of)
        // TODO deal with object properties and additional properties
        // TODO deal with array items

        std::iter::once((None, self))
    }
}
