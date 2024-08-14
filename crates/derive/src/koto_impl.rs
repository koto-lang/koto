use crate::{PREFIX_FUNCTION, PREFIX_STATIC};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    meta::ParseNestedMeta, parse::Result, parse_macro_input, parse_quote, Attribute, FnArg, Ident,
    ImplItem, ItemImpl, LitStr, Meta, Path, ReturnType, Signature, Type,
};

struct KotoImplParser {
    runtime_path: Path,
}

impl Default for KotoImplParser {
    fn default() -> Self {
        Self {
            runtime_path: parse_quote! {::koto::runtime },
        }
    }
}

impl KotoImplParser {
    fn parse(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("runtime") {
            self.runtime_path = meta.value()?.parse()?;
            Ok(())
        } else {
            Err(meta.error("Unsupported attribute"))
        }
    }
}

pub(crate) fn generate_koto_access_entries(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut attrs = KotoImplParser::default();
    let parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with parser);
    let runtime = attrs.runtime_path;

    let item_clone = item.clone();
    let input = parse_macro_input!(item_clone as ItemImpl);

    let struct_ident = match input.self_ty.as_ref() {
        Type::Path(type_path) => type_path.path.get_ident().expect("Simple ident"),
        _ => panic!("Expected a struct"),
    };
    let entries_map_name = format_ident!(
        "{PREFIX_STATIC}_ENTRIES_{}",
        struct_ident.to_string().to_uppercase()
    );

    let (wrapper_functions, insert_ops): (Vec<_>, Vec<_>) = input
        .items
        .iter()
        // find impl funtions tagged with #[koto_method]
        .filter_map(|item| match item {
            ImplItem::Fn(f) => f
                .attrs
                .iter()
                .find(|a| a.path().is_ident("koto_method"))
                .map(|attr| (f, attr)),
            _ => None,
        })
        // Generate wrappers and lookup inserts for each koto method
        .map(|(f, attr)| {
            let (wrapper, wrapper_name) = wrap_function(&f.sig, struct_ident, &runtime);
            let insert = insert_wrapped_function(&f.sig, attr, wrapper_name, &runtime);
            (wrapper, insert)
        })
        .unzip();

    let item = proc_macro2::TokenStream::from(item);
    let result = quote! {
        #item

        #(#wrapper_functions)*

        #[automatically_derived]
        thread_local! {
            static #entries_map_name: #runtime::KMap = {
                let mut result = #runtime::KMap::default();
                #(#insert_ops)*
                result
            };
        }

        #[automatically_derived]
        impl #runtime::KotoEntries for #struct_ident {
            fn entries(&self) -> Option<#runtime::KMap> {
                #entries_map_name.with(|map| Some(map.clone()))
            }
        }
    };

    result.into()
}

fn wrap_function(
    sig: &Signature,
    struct_ident: &Ident,
    runtime: &Path,
) -> (proc_macro2::TokenStream, Ident) {
    let type_name = quote! { #struct_ident::type_static() };
    let fn_name = &sig.ident;
    let fn_ident = quote! {#struct_ident::#fn_name};
    let wrapper_name = format_ident!("{PREFIX_FUNCTION}{struct_ident}_{fn_name}");

    let arg_count = sig.inputs.len();
    let mut args = sig.inputs.iter();

    let return_type = detect_return_type(&sig.output);

    let wrapper_body = match args.next() {
        // Functions that have a &self or &mut self arg
        Some(FnArg::Receiver(f)) => {
            let (cast, instance) = if f.mutability.is_some() {
                (quote! {cast_mut}, quote! {mut instance})
            } else {
                (quote! {cast}, quote! {instance})
            };

            let (args_match, call_args, error_arm) = match args.next() {
                None => (
                    quote! {[]}, // no args expected
                    quote! {},   // no args to call with
                    quote! { (_, unexpected) =>  #runtime::unexpected_args("||", unexpected)},
                ),
                Some(FnArg::Typed(pattern))
                    if arg_count == 2 && matches!(*pattern.ty, Type::Reference(_)) =>
                {
                    (
                        // match against any number of args
                        quote! {args},
                        // append the args to the call
                        quote! {, args},
                        // any number of args will be captured
                        quote! { _ => #runtime::runtime_error!(#runtime::ErrorKind::UnexpectedError) },
                    )
                }
                _ => panic!("Expected &[KValue] as the extra argument for a Koto method"),
            };

            let call = quote! { #fn_ident(&#instance #call_args) };

            let wrapped_call = match return_type {
                MethodReturnType::None => quote! {
                    #call;
                    Ok(#runtime::KValue::Null)
                },
                MethodReturnType::Value => quote! { Ok(#call) },
                MethodReturnType::Result => call,
            };

            quote! {
                match ctx.instance_and_args(
                    |i| matches!(i, #runtime::KValue::Object(_)), #type_name)?
                {
                    (#runtime::KValue::Object(o), #args_match) => {
                        match o.#cast::<#struct_ident>() {
                            Ok(#instance) => {
                                #wrapped_call
                            },
                            Err(e) => Err(e),
                        }
                    },
                    #error_arm,
                }
            }
        }
        _ => {
            let call = quote! { #fn_ident(#runtime::MethodContext::new(o, extra_args, ctx.vm)) };

            let wrapped_call = match return_type {
                MethodReturnType::None => quote! {
                    #call;
                    Ok(#runtime::KValue::Null)
                },
                MethodReturnType::Value => quote! { Ok(#call) },
                MethodReturnType::Result => call,
            };

            quote! {
                match ctx.instance_and_args(
                    |i| matches!(i, #runtime::KValue::Object(_)), #type_name)?
                {
                    (#runtime::KValue::Object(o), extra_args) => { #wrapped_call }
                    _ => #runtime::runtime_error!(#runtime::ErrorKind::UnexpectedError),
                }
            }
        }
    };

    let wrapper = quote! {
        #[automatically_derived]
        fn #wrapper_name(ctx: &mut #runtime::CallContext) -> #runtime::Result<#runtime::KValue> {
            #wrapper_body
        }
    };

    (wrapper, wrapper_name)
}

fn insert_wrapped_function(
    sig: &Signature,
    koto_method_attr: &Attribute,
    wrapper_name: Ident,
    runtime: &Path,
) -> proc_macro2::TokenStream {
    let fn_name = sig.ident.to_string();

    if matches!(koto_method_attr.meta, Meta::List(_)) {
        let mut fn_names = vec![fn_name];

        koto_method_attr
            .parse_nested_meta(|meta| {
                if meta.path.is_ident("alias") {
                    let value = meta.value()?;
                    let s: LitStr = value.parse()?;
                    fn_names.push(s.value());
                    Ok(())
                } else {
                    Err(meta.error("unsupported attribute"))
                }
            })
            .expect("failed to parse koto_method attribute");

        quote! {
            let f = #runtime::KValue::NativeFunction(#runtime::KNativeFunction::new(#wrapper_name));
            #(result.insert(KString::from(#fn_names), f.clone());)*
        }
    } else {
        quote! {
            result.insert(
                KString::from(#fn_name),
                #runtime::KValue::NativeFunction(#runtime::KNativeFunction::new(#wrapper_name)));
        }
    }
}

enum MethodReturnType {
    None,
    Value,
    Result,
}

fn detect_return_type(return_type: &ReturnType) -> MethodReturnType {
    match return_type {
        ReturnType::Default => MethodReturnType::None,
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Tuple(t) if t.elems.is_empty() => MethodReturnType::None,
            Type::Path(p) if p.path.is_ident("KValue") => MethodReturnType::Value,
            // Default to expecting a Result to be the return value
            // Ideally we would detect that this is precisely koto_runtime::Result,
            // but in practice type aliases may be used so we should just let the compiler complain
            // if the wrong type is used.
            _ => MethodReturnType::Result,
        },
    }
}
