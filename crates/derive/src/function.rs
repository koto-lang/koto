use quote::quote;
use syn::{
    Ident, ItemFn, Path, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
};

use crate::overloading::{
    AccessAttributeArgs, OverloadOptions, OverloadedFunctionCandidate, OverloadedFunctions,
};

pub fn koto_fn(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let FunctionDefinitions { runtime, functions } =
        parse_macro_input!(input as FunctionDefinitions);

    let mut function_wrappers = vec![];

    for function in functions.inner.values() {
        let name = function.first_ident();
        let body = match function.match_arms() {
            Ok(arms) => quote! {
                use #runtime::{ KValue, __private::KotoFunctionReturn };

                match ctx.args() {
                    #arms
                }
            },
            Err(error) => {
                let compile_error = error.into_compile_error();
                quote!(#compile_error)
            }
        };

        function_wrappers.push(quote! {
            fn #name(ctx: &mut #runtime::CallContext) -> #runtime::Result<#runtime::KValue> {
                #body
            }
        });
    }

    let output = quote! {
        #(#function_wrappers)*
    };

    output.into()
}

struct FunctionDefinitions {
    runtime: Path,
    functions: OverloadedFunctions,
}

impl Parse for FunctionDefinitions {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut runtime = None;

        while !input.peek(Token![fn]) {
            let option: Ident = input.parse()?;
            let _eq: Token![=] = input.parse()?;
            match option.to_string().as_str() {
                "runtime" => {
                    runtime = Some(input.parse()?);
                }
                _ => return Err(input.error(format!("Unknown option '{option}'"))),
            }

            let _semicolon: Token![;] = input.parse()?;
        }

        let mut functions = OverloadedFunctions::default();

        while !input.is_empty() {
            let item: ItemFn = input.parse()?;

            functions.insert(OverloadedFunctionCandidate::new(
                item,
                AccessAttributeArgs::default(),
                OverloadOptions::Function,
            )?);
        }

        Ok(Self {
            runtime: runtime.unwrap_or_else(|| parse_quote! {::koto::runtime }),
            functions,
        })
    }
}
