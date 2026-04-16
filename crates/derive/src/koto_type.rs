use crate::attributes::koto_derive_attributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input, parse_quote};

pub fn derive_koto_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let mut impl_generics_input = input.generics.clone();
    impl_generics_input.params.push(parse_quote! { B });
    let (impl_generics, _, generic_where_clause) = impl_generics_input.split_for_impl();
    let (static_impl_generics, _, static_where_clause) = input.generics.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();

    let attributes = koto_derive_attributes(&input.attrs);

    let name = input.ident;

    let type_name = attributes
        .type_name
        .unwrap_or_else(|| quote!(#name).to_string());

    let runtime = attributes.runtime;
    let backend_where_clause = quote! { B: #runtime::api::KotoBackend };
    let where_clause = if let Some(generic_where_clause) = generic_where_clause {
        if generic_where_clause.predicates.trailing_punct() {
            quote! { #generic_where_clause #backend_where_clause }
        } else {
            quote! { #generic_where_clause, #backend_where_clause }
        }
    } else {
        quote! { where #backend_where_clause }
    };

    let result = quote! {
        #[automatically_derived]
        impl #static_impl_generics #runtime::api::KotoStaticType for #name #ty_generics
            #static_where_clause
        {
            fn type_static() -> &'static str {
                #type_name
            }
        }

        #[automatically_derived]
        impl #impl_generics #runtime::api::KotoType<B> for #name #ty_generics
            #where_clause
        {
            fn type_string(&self) -> B::String {
                #type_name.into()
            }
        }
    };

    result.into()
}
