//! Contains convenience macros for declaring types for the Koto runtime

#![warn(missing_docs)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, LitStr};

/// TODO
#[proc_macro_derive(KotoType, attributes(koto_type))]
pub fn koto_type_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let mut type_name = quote!(#name).to_string();

    for attr in input.attrs {
        if attr.path().is_ident("koto_type") {
            let name: LitStr = attr.parse_args().expect("Expected string");
            type_name = name.value();
            break;
        }
    }

    let result = quote! {
        impl KotoType for #name {
            const TYPE: &'static str = #type_name;
        }
    };

    result.into()
}
