use crate::attributes::koto_derive_attributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

pub fn derive_koto_copy(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let (impl_generics, ty_generics, generic_where_clause) = input.generics.split_for_impl();

    let attributes = match koto_derive_attributes(&input.attrs) {
        Ok(attributes) => attributes,
        Err(error) => return error.into_compile_error().into(),
    };
    let runtime = attributes.runtime;
    let (required_trait, copy_impl) = if attributes.use_copy {
        (
            quote! {::std::marker::Copy},
            quote! {::std::convert::Into::into(*self)},
        )
    } else {
        (
            quote! {::std::clone::Clone},
            quote! {::std::convert::Into::into(::std::clone::Clone::clone(self))},
        )
    };

    let object_where_clause = quote! {
        #name #ty_generics: #runtime::KotoObject + #required_trait
    };
    let where_clause = if let Some(generic_where_clause) = generic_where_clause {
        if generic_where_clause.predicates.trailing_punct() {
            quote! { #generic_where_clause #object_where_clause }
        } else {
            quote! { #generic_where_clause, #object_where_clause }
        }
    } else {
        quote! { where #object_where_clause }
    };

    let result = quote! {
        #[automatically_derived]
        impl #impl_generics #runtime::KotoCopy for #name #ty_generics
            #where_clause
        {
            fn copy(&self) -> #runtime::KObject {
                #copy_impl
            }
        }
    };

    result.into()
}
