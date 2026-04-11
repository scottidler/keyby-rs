use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(KeyBy, attributes(key_by))]
pub fn derive_key_by(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_key_by(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_key_by(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = &input.ident;
    let field_name = parse_key_by_field(input)?;
    let field_ident = syn::Ident::new(&field_name, proc_macro2::Span::call_site());

    let expecting_msg = format!("a map of {} to {}", field_name, struct_name);

    Ok(quote! {
        impl #struct_name {
            pub fn deserialize_key_by_map<'de, D>(
                deserializer: D,
            ) -> std::result::Result<std::collections::HashMap<String, #struct_name>, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct KeyByVisitor;

                impl<'de> serde::de::Visitor<'de> for KeyByVisitor {
                    type Value = std::collections::HashMap<String, #struct_name>;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(#expecting_msg)
                    }

                    fn visit_map<M>(self, mut map: M) -> std::result::Result<Self::Value, M::Error>
                    where
                        M: serde::de::MapAccess<'de>,
                    {
                        let mut result = std::collections::HashMap::new();
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

fn parse_key_by_field(input: &DeriveInput) -> syn::Result<String> {
    for attr in &input.attrs {
        if !attr.path().is_ident("key_by") {
            continue;
        }
        let mut field_name = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("field") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                field_name = Some(lit.value());
                Ok(())
            } else {
                Err(meta.error("expected `field = \"...\"`"))
            }
        })?;
        match field_name {
            Some(name) => return Ok(name),
            None => {
                return Err(syn::Error::new_spanned(attr, "expected #[key_by(field = \"...\")]"));
            }
        }
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "missing #[key_by(field = \"...\")] attribute",
    ))
}
