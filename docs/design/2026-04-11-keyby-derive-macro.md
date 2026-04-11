# Design Document: keyby - Derive Macro for Keyed-Map YAML Deserialization

**Author:** Scott A. Idler
**Date:** 2026-04-11
**Status:** Draft
**Review Passes Completed:** 3/3

## Summary

keyby is a derive-macro crate that eliminates the boilerplate of deserializing YAML keyed maps where the map key becomes a field on the struct. One `#[derive(KeyBy)]` attribute generates the custom serde `Visitor` and deserializer function that otto, aka, and loopr-v4 currently hand-write for every keyed-map type.

## Problem Statement

### Background

A common YAML pattern uses map keys as identifiers instead of embedding a `name` field in each item:

```yaml
# Keyed-map form (preferred)
tasks:
  lint:
    help: "Run linting"
    bash: "cargo clippy"
  test:
    help: "Run tests"
    bash: "cargo test"
```

This is cleaner than the list-of-dicts alternative (no redundant `name` field, O(1) lookup, duplicate keys are parse errors). But serde's default `HashMap<String, T>` deserialization doesn't inject the key into the struct - the struct never knows its own name.

### Problem

Every project that uses keyed-map YAML must hand-write ~30 lines of serde boilerplate per type:
1. A custom `Visitor` struct
2. A `visit_map` implementation that iterates entries, extracts the key, injects it into the struct
3. A `deserialize_*_map` wrapper function
4. `#[serde(deserialize_with = "...")]` on the parent field
5. `#[serde(skip_deserializing)]` on the name field of the child struct

This boilerplate is identical in structure across otto (`deserialize_task_map`, `deserialize_param_map`) and aka (`deserialize_alias_map`). Loopr v4 needs this pattern for FSM definitions, role definitions, strategy definitions, and policy definitions - at least 6 more types. Hand-writing it each time is error-prone and noisy.

### Goals

- Derive macro that generates the custom deserializer from a single attribute
- Works with `HashMap<String, T>` and `BTreeMap<String, T>`
- The map key is injected into a named field on `T` (configurable field name, default `name`)
- Supports serde's existing ecosystem (works alongside `#[serde(...)]` attributes)
- Published as a standalone crate usable by otto, aka, loopr, and any other project
- Zero runtime overhead vs hand-written code (the macro generates the same code)

### Non-Goals

- Serialization (the reverse direction: struct field -> map key). This can be added later but isn't needed now.
- Support for non-string keys (e.g., integer keys). String keys cover all known use cases.
- Key transformation functions (otto's `namify`, aka's pipe-splitting). These are project-specific and belong in the consuming crate, not in keyby.
- Nested keyby (keyed maps of keyed maps). Handle the outer level; inner levels use standard serde.

## Proposed Solution

### Overview

keyby provides two things:

1. **`#[derive(KeyBy)]`** on the value struct - marks which field receives the map key
2. **`#[key_by("field_name")]`** helper function attribute on the parent field - routes deserialization to the generated code

### API Design

**Value struct (the thing inside the map):**
```rust
use keyby::KeyBy;

#[derive(Debug, Clone, Deserialize, KeyBy)]
#[key_by(field = "name")]
pub struct Task {
    #[serde(skip_deserializing)]
    pub name: String,
    pub help: Option<String>,
    pub bash: String,
}
```

**Parent struct (the thing that holds the map):**
```rust
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default, deserialize_with = "keyby::deserialize::<_, Task, HashMap<String, Task>>")]
    pub tasks: HashMap<String, Task>,
}
```

Or, with the generated module-level function (more ergonomic):

```rust
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default, deserialize_with = "Task::deserialize_key_by_map")]
    pub tasks: HashMap<String, Task>,
}
```

The derive macro generates:

```rust
impl Task {
    pub fn deserialize_key_by_map<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<String, Task>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // ... generated Visitor + visit_map that injects key into `name` field
    }
}
```

### Architecture

The crate is a workspace with two members:

```
keyby/
  Cargo.toml           # workspace root
  keyby/
    Cargo.toml         # re-export crate (what users depend on)
    src/lib.rs         # pub use keyby_derive::KeyBy;
  keyby-derive/
    Cargo.toml         # proc-macro crate
    src/lib.rs         # the derive macro implementation
```

**Why a workspace?** Proc-macro crates can only export procedural macros. The main `keyby` crate re-exports the derive macro and can also provide helper traits or functions if needed.

### Data Model

The derive macro parses these inputs from the struct:

| Input | Source | Example |
|-------|--------|---------|
| Struct name | The type being derived | `Task` |
| Key field name | `#[key_by(field = "name")]` attribute | `name` |
| Key field type | The field's type annotation | `String` |

And generates:

| Output | What |
|--------|------|
| `Task::deserialize_key_by_map` | Associated function that returns `Result<HashMap<String, Task>, D::Error>` |

### Generated Code

For the `Task` example above, the macro generates approximately:

```rust
impl Task {
    pub fn deserialize_key_by_map<'de, D>(
        deserializer: D,
    ) -> Result<std::collections::HashMap<String, Task>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KeyByVisitor;

        impl<'de> serde::de::Visitor<'de> for KeyByVisitor {
            type Value = std::collections::HashMap<String, Task>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map of name to Task")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut result = std::collections::HashMap::new();
                while let Some((key, mut value)) =
                    map.next_entry::<String, Task>()?
                {
                    value.name = key.clone();
                    result.insert(key, value);
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(KeyByVisitor)
    }
}
```

### Implementation Plan

#### Phase 0: Restructure repo

The repo was scaffolded as a CLI binary. Restructure into a library workspace:
1. Convert to workspace with two members: `keyby` (re-export lib) and `keyby-derive` (proc-macro)
2. Remove CLI scaffolding (main.rs, cli.rs, config.rs, clap, colored, dirs, env_logger dependencies)
3. `keyby/Cargo.toml` depends on `keyby-derive` and `serde`
4. `keyby-derive/Cargo.toml` depends on `syn`, `quote`, `proc-macro2`

#### Phase 1: Derive macro (core)

1. Implement `#[derive(KeyBy)]` in `keyby-derive`
2. Implement `#[derive(KeyBy)]` that parses `#[key_by(field = "...")]`
3. Generate `deserialize_key_by_map` associated function
4. Unit tests: basic keyed-map deserialization, missing key field error, wrong field type error

#### Phase 2: Ergonomics

1. Default field name to `name` if `#[key_by(field = "...")]` is omitted
2. Support `BTreeMap` in addition to `HashMap` (via a `#[key_by(map = "BTreeMap")]` option)
3. Compile-time error if the key field doesn't exist or isn't `String`
4. Compile-time error if the key field is missing `#[serde(skip_deserializing)]`

#### Phase 3: Publish and adopt

1. Publish to crates.io
2. Replace hand-written deserializers in otto (`deserialize_task_map`, `deserialize_param_map`)
3. Replace hand-written deserializer in aka (`deserialize_alias_map`)
4. Use in loopr-v4 for FSM definitions, role definitions, strategies, policies

## Alternatives Considered

### Alternative 1: Generic helper function (no macro)

- **Description:** A generic `keyby::deserialize<T>()` function that works with any `T: Deserialize + HasName` where `HasName` is a trait with `fn set_name(&mut self, name: String)`.
- **Pros:** No proc-macro complexity. Simpler crate structure.
- **Cons:** Requires users to implement the `HasName` trait on every struct (more boilerplate, not less). The trait approach can't verify at compile time that the field exists or has `#[serde(skip_deserializing)]`.
- **Why not chosen:** The derive macro eliminates more boilerplate and provides better compile-time errors.

### Alternative 2: serde flatten with a wrapper

- **Description:** Use `#[serde(flatten)]` with a wrapper struct that captures the key.
- **Pros:** No custom code at all - pure serde attributes.
- **Cons:** `#[serde(flatten)]` with serde_yaml has known issues with complex types. It also requires restructuring every consuming struct to use the wrapper. Doesn't actually work for this pattern - flatten merges fields from a nested map, it doesn't capture the parent map's key.
- **Why not chosen:** Doesn't solve the problem. The key lives in the parent map, not in the value's fields.

### Alternative 3: Post-processing (deserialize as HashMap, then inject keys)

- **Description:** Deserialize as plain `HashMap<String, T>`, then iterate and inject keys in a post-processing step.
- **Pros:** No custom serde code. Simple to understand.
- **Cons:** The struct has an uninitialized/default `name` field between deserialization and post-processing. If any code accesses the struct before post-processing, the name is wrong. Also requires remembering to call the post-processor everywhere the config is loaded.
- **Why not chosen:** Error-prone. The derive macro makes it impossible to forget.

## Technical Considerations

### Dependencies

- **keyby-derive:** `syn`, `quote`, `proc-macro2` (standard proc-macro dependencies)
- **keyby:** `serde` (peer dependency - users must already have serde)
- No runtime dependencies beyond serde itself

### Performance

- Zero runtime overhead. The macro generates the exact same code you'd write by hand.
- Compile-time cost is negligible (one small proc-macro expansion per derived struct).

### Security

- No security implications. This is a deserialization helper, not a network-facing component.

### Testing Strategy

- **Macro output tests:** Verify the generated code compiles and produces correct results for:
  - Basic keyed-map deserialization (happy path)
  - Empty map
  - Single-entry map
  - Field with `#[serde(default)]` on other fields
  - Missing optional fields in the value struct
- **Compile-fail tests:** Verify the macro produces clear errors for:
  - `#[key_by(field = "nonexistent")]` - field doesn't exist
  - Key field isn't `String`
  - Missing `#[serde(skip_deserializing)]` on key field (warn or error)
- **Integration tests:** Replace hand-written deserializers in a test copy of otto/aka config and verify identical behavior.

### Rollout Plan

- Implement and publish as `keyby` on crates.io
- Adopt in otto, aka, and loopr-v4 via `cargo add keyby`
- Remove hand-written `deserialize_*_map` functions from each project

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| serde internal API changes break the generated code | Low | High | The generated code uses only stable serde public API (Visitor, MapAccess, Deserializer). These haven't changed in years. |
| Edge cases with serde attribute interactions | Medium | Medium | Test with common serde attributes: `#[serde(default)]`, `#[serde(rename)]`, `#[serde(alias)]`. The macro doesn't interfere with these - they apply to the value struct's normal deserialization. |
| proc-macro debugging is hard | Medium | Low | Keep the generated code minimal and predictable. Provide `cargo expand` examples in docs. |
| Name collision with generated function | Low | Low | The function is an associated function on the struct (`Task::deserialize_key_by_map`), not a free function. Collisions are unlikely. |

## Open Questions

- [x] **Associated function vs module-level function?** Generate as an associated function (`Task::deserialize_key_by_map`). serde's `deserialize_with` accepts any path that resolves to a function with the right signature, and associated functions work via `"Task::deserialize_key_by_map"`. This keeps the generated code namespaced to the type.
- [ ] Should keyby warn or error if the key field is missing `#[serde(skip_deserializing)]`? Without it, serde will try to deserialize the field from the YAML value AND the macro will inject it from the key - the macro injection wins but it's confusing. Leaning toward compile-time error.

## References

- `otto-rs/otto/src/cfg/task.rs:520-546` - hand-written `deserialize_task_map`
- `otto-rs/otto/src/cfg/param.rs:200-241` - hand-written `deserialize_param_map`
- `scottidler/aka/src/cfg/spec.rs:40-108` - hand-written `deserialize_alias_map`
- `scottidler/loopr-v4/docs/design/2026-04-11-fsm-in-yaml.md` - loopr v4 FSM definitions that need this pattern
- serde custom deserialization: https://serde.rs/deserialize-map.html
