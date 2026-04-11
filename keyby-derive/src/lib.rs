use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(KeyBy, attributes(key_by))]
pub fn derive_key_by(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_key_by(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct KeyByConfig {
    field: String,
    map_type: MapType,
}

enum MapType {
    HashMap,
    BTreeMap,
}

fn expand_key_by(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = &input.ident;
    let config = parse_key_by_config(input)?;
    let field_ident = syn::Ident::new(&config.field, proc_macro2::Span::call_site());

    validate_key_field(input, &config.field)?;

    let expecting_msg = format!("a map of {} to {}", config.field, struct_name);

    let map_type = match config.map_type {
        MapType::HashMap => quote! { std::collections::HashMap<String, #struct_name> },
        MapType::BTreeMap => quote! { std::collections::BTreeMap<String, #struct_name> },
    };

    let map_constructor = match config.map_type {
        MapType::HashMap => quote! { std::collections::HashMap::new() },
        MapType::BTreeMap => quote! { std::collections::BTreeMap::new() },
    };

    Ok(quote! {
        impl #struct_name {
            pub fn deserialize_key_by_map<'de, D>(
                deserializer: D,
            ) -> std::result::Result<#map_type, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct KeyByVisitor;

                impl<'de> serde::de::Visitor<'de> for KeyByVisitor {
                    type Value = #map_type;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(#expecting_msg)
                    }

                    fn visit_map<M>(self, mut map: M) -> std::result::Result<Self::Value, M::Error>
                    where
                        M: serde::de::MapAccess<'de>,
                    {
                        let mut result = #map_constructor;
                        while let Some((key, mut value)) =
                            map.next_entry::<String, #struct_name>()?
                        {
                            value.#field_ident = key.clone();
                            result.insert(key, value);
                        }
                        Ok(result)
                    }
                }
                deserializer.deserialize_map(KeyByVisitor)
            }
        }
    })
}

fn parse_key_by_config(input: &DeriveInput) -> syn::Result<KeyByConfig> {
    let mut field = None;
    let mut map_type = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("key_by") {
            continue;
        }
        // Handle bare #[key_by] with no arguments
        if attr.meta.require_list().is_err() {
            return Ok(KeyByConfig {
                field: "name".to_string(),
                map_type: MapType::HashMap,
            });
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("field") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                field = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("map") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                match lit.value().as_str() {
                    "HashMap" => map_type = Some(MapType::HashMap),
                    "BTreeMap" => map_type = Some(MapType::BTreeMap),
                    other => {
                        return Err(syn::Error::new_spanned(
                            lit,
                            format!("unsupported map type `{}`, expected `HashMap` or `BTreeMap`", other),
                        ));
                    }
                }
                Ok(())
            } else {
                Err(meta.error("expected `field = \"...\"` or `map = \"...\"`"))
            }
        })?;

        return Ok(KeyByConfig {
            field: field.unwrap_or_else(|| "name".to_string()),
            map_type: map_type.unwrap_or(MapType::HashMap),
        });
    }
    Err(syn::Error::new_spanned(&input.ident, "missing #[key_by] attribute"))
}

fn validate_key_field(input: &DeriveInput, field_name: &str) -> syn::Result<()> {
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "KeyBy can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "KeyBy can only be derived for structs",
            ));
        }
    };

    let target = fields
        .iter()
        .find(|f| f.ident.as_ref().is_some_and(|ident| ident == field_name));

    let target = match target {
        Some(f) => f,
        None => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                format!("key_by field `{}` does not exist on this struct", field_name),
            ));
        }
    };

    // Verify the field type is String
    if !is_string_type(&target.ty) {
        return Err(syn::Error::new_spanned(
            &target.ty,
            format!("key_by field `{}` must be of type `String`", field_name),
        ));
    }

    // Verify the field has #[serde(skip_deserializing)]
    if !has_serde_skip_deserializing(target) {
        return Err(syn::Error::new_spanned(
            target,
            format!(
                "key_by field `{}` must have `#[serde(skip_deserializing)]` - \
                 without it, serde will try to deserialize the field from the value \
                 AND the macro will inject it from the key",
                field_name
            ),
        ));
    }

    Ok(())
}

fn is_string_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "String";
    }
    false
}

fn has_serde_skip_deserializing(field: &syn::Field) -> bool {
    for attr in &field.attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let mut found = false;
        let result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip_deserializing") {
                found = true;
            }
            Ok(())
        });
        if result.is_ok() && found {
            return true;
        }
    }
    false
}
