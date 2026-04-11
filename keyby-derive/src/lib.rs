use proc_macro::TokenStream;

#[proc_macro_derive(KeyBy, attributes(key_by))]
pub fn derive_key_by(input: TokenStream) -> TokenStream {
    let _ = input;
    TokenStream::new()
}
