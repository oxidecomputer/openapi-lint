# openapi-lint

This is a simple crate to validate OpenAPI v3.0.3 content. It flags constructs
that we've determined are not "ergonomic" or "well-designed". In particular we
try to avoid constructs that lead to structures that SDK generators would have
a hard time turning into easy-to-use native constructs.

## Rules

### Type mismatch

A schema that describes a type may include subschemas where one, all, or any of
the subschemas might match ( for the `oneOf`, `allOf`, and `anyOf` fields
respectively). For example, the following Rust code produces such a schema with
mixed types:

```rust
#[derive(JsonSchema)]
pub enum E {
    ThingA(String),
    ThingB,
}
```

A JSON object that used this `enum` for the type of a field could look like this:

```json
{
    "field": { "ThingA": "some value" }
}
```

or this:

```json
{
    "field": "ThingB"
}
```

So `field` may be either a string **or** an object. This complicates the
description of these types and is harder to represent in SDKs (in particular
those without Rust's ability for enums to have associated values). To avoid
this, we can simply use `serde`'s facility for annotating enums. In particular,
we prefer ["adjacently
tagged"](https://serde.rs/container-attrs.html#tag--content) enums:

```rust
#[derive(JsonSchema)]
#[serde(tag = "type", content = "value")]
pub enum E {
    ThingA(String),
    ThingB,
}
```

This produces JSON like this:

```json
{
    "field1": { "type": "ThingA", "value": "some value" },
    "field2": { "type": "ThingB" }
}
```

### Naming

- All struct (and struct enum variant) members should be camelCase.
- All `operation_id`s should be snake_case.
- All type names should be PascalCase.

To rename all fields in a struct do...

```rust
#[derive(JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NeedsRenaming{
    long_member_name: u32,
    even_longer_snake_case_member_name: u8,
}
```

Operation IDs come from the function name. If you obey the normal Rust
convention, your functions are already snake_case. There isn't currently a
facility to change the operation name; file an issue in
(dropshot)[https://github.com/oxidecomputer/dropshot] if this is required.

Type names are already PascalCase by normal Rust conventions. If you need
(really?) to have a type with a non-PascalCase name, you can renamed it like
this:

```rust
#[derive(JsonSchema)]
#[allow(non_camel_case_types)]
#[serde(rename = "IllumosButUpperCase")]
struct illumosIsAlwaysLowerCaseIGuess {
    // ...
}
```