use crate::attributes::koto_derive_attributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub(crate) fn derive_koto_copy(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let attributes = koto_derive_attributes(&input.attrs);

    let (required_trait, copy_impl) = if attributes.use_copy {
        (quote! {Copy}, quote! {(*self).into()})
    } else {
        (quote! {Clone}, quote! {self.clone().into()})
    };

    let name = input.ident;
    let result = quote! {
        #[automatically_derived]
        impl KotoCopy for #name where #name: #required_trait {
            fn copy(&self) -> KObject {
                #copy_impl
            }
        }
    };

    result.into()
}
