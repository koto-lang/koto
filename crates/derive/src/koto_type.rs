use crate::attributes::koto_derive_attributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_koto_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let attributes = koto_derive_attributes(&input.attrs);

    let name = input.ident;
    let type_name = attributes
        .type_name
        .unwrap_or_else(|| quote!(#name).to_string());

    // Short type names don't need to be cached, 22 is the `MAX_INLINE_STRING_LEN` constant
    let result = if type_name.len() <= 22 {
        quote! {
            #[automatically_derived]
            impl #impl_generics KotoType for #name #ty_generics #where_clause {
                fn type_static() -> &'static str {
                    #type_name
                }

                fn type_string(&self) -> KString {
                    #type_name.into()
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            impl #impl_generics KotoType for #name #ty_generics #where_clause {
                fn type_static() -> &'static str {
                    #type_name
                }

                fn type_string(&self) -> KString {
                    thread_local! {
                        static TYPE_NAME: KString = #type_name.into();
                    }

                    TYPE_NAME.with(KString::clone)
                }
            }

        }
    };

    result.into()
}
