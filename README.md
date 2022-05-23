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

In general, we use the typical Rust naming conventions.

- All type names should be `PascalCase`.
- All `operation_id`s should be `snake_case`.
- All operation properties should be `snake_case`.
- All struct (and struct enum variant) members should be `snake_case`.
- All enum variants should be `snake_case`. (Note that depending on the serde
tagging scheme used, variant names may appear in OpenAPI as either struct
property names (external tagging) or as constant values (internal or adjacent
tagging). The choice of `snake_case` makes naming uniform regardless of the
tagging scheme.)

Type names are already `PascalCase` by normal Rust conventions. If you need
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

Operation IDs come from the function name. If you obey the normal Rust
convention, your functions are already snake_case. There isn't currently a
facility to change the operation name; file an issue in
(dropshot)[https://github.com/oxidecomputer/dropshot] if this is required.

Rust `enum`s typically name variants with `PascalCase`. Typically you'll rename
them all to `snake_case`:

```rust
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Things {
    ThingA,
    ThingB,
}
```

Sometimes you might prefer `SCREAMING_SNAKE_CASE` e.g. for things that are more
typically abbreviated:

```rust
#[derive(JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Things {
    ThingA,
    ThingB,
}
```

### UUIDs

It's tempting to name fields that are UUIDs with an `_uuid` suffix, but this
is redundant. For simplicity and consistency we use the `_id` suffix instead.