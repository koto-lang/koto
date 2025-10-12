use crate::PREFIX_FUNCTION;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, FnArg, Generics, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr, Meta, Path,
    ReturnType, Signature, Type, TypePath, meta::ParseNestedMeta, parse::Result, parse_macro_input,
    parse_quote,
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

// Derives an implementation of `KotoEntries` for types tagged with `#[koto_impl]`
pub(crate) fn koto_impl(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut attrs = KotoImplParser::default();
    let parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with parser);
    let runtime = attrs.runtime_path;

    // Parse the tagged impl block, mutable so that generated functions can be appended.
    let mut input_struct = parse_macro_input!(item as ItemImpl);

    let struct_type = input_struct.self_ty.as_ref();
    let struct_ident = match &struct_type {
        Type::Path(TypePath { path, .. }) => {
            let last_segment = path.segments.last().expect("Expected an identifier");
            &last_segment.ident
        }
        _ => panic!("Expected a type path"),
    };

    // Generate wrapper functions an entry map insertion ops for each impl function tagged with
    // `#[koto_method]`
    let mut insert_op_count = 0;
    let (wrapper_functions, insert_ops): (Vec<_>, Vec<_>) = input_struct
        .items
        .iter()
        // Find impl funtions tagged with #[koto_method]
        .filter_map(|item| match item {
            ImplItem::Fn(f) => f
                .attrs
                .iter()
                .find(|a| a.path().is_ident("koto_method"))
                .map(|attr| (f, attr)),
            _ => None,
        })
        // Generate wrappers and lookup inserts for each tagged function
        .map(|(f, koto_method_attr)| {
            let wrapper_name = format_ident!("{PREFIX_FUNCTION}{}", f.sig.ident);
            let wrapper_fn = wrap_function(f, &wrapper_name, &runtime);
            let (insert_ops, op_count) =
                wrapper_function_insert_ops(&f.sig, koto_method_attr, wrapper_name);
            insert_op_count += op_count;
            (ImplItem::Fn(wrapper_fn), insert_ops)
        })
        .unzip();

    // Append the generated wrapper functions
    input_struct.items.extend(wrapper_functions);

    // Append entries map initializer and getter functions.
    // The initializer and getter are separated to avoid "can't use Self from outer item" errors
    // when dealing with generic impls.
    let entries_initializer_name = format_ident!("{PREFIX_FUNCTION}initialize_entries_map");
    let entries_getter_name = format_ident!("{PREFIX_FUNCTION}get_entries_map");

    input_struct
        .items
        .push(ImplItem::Fn(koto_entries_initializer(
            &entries_initializer_name,
            &insert_ops,
            insert_op_count,
            &runtime,
        )));

    input_struct.items.push(ImplItem::Fn(koto_entries_getter(
        struct_ident,
        &entries_getter_name,
        &entries_initializer_name,
        &input_struct.generics,
        &runtime,
    )));

    // Generate an implementation of KotoEntries that calls the generated entries getter
    let (impl_generics, ty_generics, where_clause) = input_struct.generics.split_for_impl();
    let turbofish = ty_generics.as_turbofish();
    let koto_entries = quote! {
        impl #impl_generics #runtime::KotoEntries for #struct_type #where_clause {
            fn entries(&self) -> Option<#runtime::KMap> {
                Some(#struct_ident #turbofish::#entries_getter_name())
            }
        }
    };

    let result = quote! {
        #input_struct
        #koto_entries
    };

    result.into()
}

fn wrap_function(fn_to_wrap: &ImplItemFn, wrapper_name: &Ident, runtime: &Path) -> ImplItemFn {
    let fn_name = &fn_to_wrap.sig.ident;

    let arg_count = fn_to_wrap.sig.inputs.len();
    let mut args = fn_to_wrap.sig.inputs.iter();

    let return_type = detect_return_type(&fn_to_wrap.sig.output);

    let wrapper_body = match args.next() {
        // Functions that have a &self or &mut self arg
        Some(FnArg::Receiver(f)) => {
            // Mutable or immutable instance?
            let (cast, instance) = if f.mutability.is_some() {
                (quote! {cast_mut}, quote! {mut instance})
            } else {
                (quote! {cast}, quote! {instance})
            };

            // Does the function expect additional arguments after the instance?
            let (args_match, call_args, error_arm) = match args.next() {
                None => (
                    quote! {[]}, // No args expected
                    quote! {},   // No args to call with
                    quote! { (_, unexpected) =>  #runtime::unexpected_args("||", unexpected)},
                ),
                Some(FnArg::Typed(pattern))
                    if arg_count == 2 && matches!(*pattern.ty, Type::Reference(_)) =>
                {
                    (
                        // Match against any number of args
                        quote! {args},
                        // Append the args to the call
                        quote! {args},
                        // Any number of args will be captured
                        quote! { _ => #runtime::runtime_error!(#runtime::ErrorKind::UnexpectedError) },
                    )
                }
                _ => panic!("Expected &[KValue] as the extra argument for a Koto method"),
            };

            // Wrap the call differently depending on the declared return type
            let call = quote! { instance.#fn_name(#call_args) };
            let wrapped_call = match return_type {
                MethodReturnType::None => quote! {{
                    #call;
                    Ok(KValue::Null)
                }},
                MethodReturnType::Value => quote! { Ok(#call) },
                MethodReturnType::Result => call,
            };

            quote! {{
                use #runtime::KValue;
                match ctx.instance_and_args(|i| matches!(i, KValue::Object(_)), Self::type_static())? {
                    (KValue::Object(o), #args_match) => {
                        match o.#cast::<Self>() {
                            Ok(#instance) => #wrapped_call,
                            Err(e) => Err(e),
                        }
                    },
                    #error_arm,
                }
            }}
        }
        // Functions that take a MethodContext
        _ => {
            // Wrap the call differently depending on the declared return type
            let call = quote! { Self::#fn_name(MethodContext::new(&o, extra_args, ctx.vm)) };
            let wrapped_call = match return_type {
                MethodReturnType::None => quote! {
                    #call;
                    Ok(KValue::Null)
                },
                MethodReturnType::Value => quote! { Ok(#call) },
                MethodReturnType::Result => call,
            };

            quote! {{
                use #runtime::{ErrorKind, KValue, MethodContext, runtime_error};
                match ctx.instance_and_args(
                    |i| matches!(i, KValue::Object(_)), Self::type_static())?
                {
                    (KValue::Object(o), extra_args) => { #wrapped_call }
                    _ => #runtime::runtime_error!(ErrorKind::UnexpectedError),
                }
            }}
        }
    };

    let wrapped_fn = quote! {
        fn #wrapper_name(ctx: &mut #runtime::CallContext) -> #runtime::Result<#runtime::KValue> {
            #wrapper_body
        }
    };

    syn::parse2(wrapped_fn).expect("Failed to parse wrapper body")
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

// Generates insertion ops for a wrapped function, checking the attribute for function aliases
fn wrapper_function_insert_ops(
    sig: &Signature,
    koto_method_attr: &Attribute,
    wrapper_name: Ident,
) -> (proc_macro2::TokenStream, usize) {
    let fn_name = sig.ident.to_string();

    if matches!(koto_method_attr.meta, Meta::List(_)) {
        // Generate additional entries for each function alias
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

        (
            quote! {
                let f = KValue::from(KNativeFunction::new(Self::#wrapper_name));
                #(result.insert(ValueKey::from(#fn_names), f.clone());)*
            },
            1 + fn_names.len(),
        )
    } else {
        (
            quote! {
                result.insert(
                    ValueKey::from(#fn_name),
                    KNativeFunction::new(Self::#wrapper_name).into());
            },
            1,
        )
    }
}

fn koto_entries_initializer(
    entries_initializer_name: &Ident,
    insert_ops: &[proc_macro2::TokenStream],
    insert_op_count: usize, // Can be more than insert_ops.len() when aliases are used
    runtime: &Path,
) -> ImplItemFn {
    let initializer_fn = quote! {
        #[automatically_derived]
        fn #entries_initializer_name() -> #runtime::KMap {
            use #runtime::{KMap, KNativeFunction, KValue, ValueKey, ValueMap};

            let mut result = ValueMap::with_capacity(#insert_op_count);
            #(#insert_ops)*
            result.into()
        }
    };

    syn::parse2(initializer_fn).expect("Failed to parse entries initializer function")
}

#[allow(clippy::collapsible_else_if)] // is more readable
fn koto_entries_getter(
    struct_ident: &Ident,
    entries_getter_name: &Ident,
    entries_initializer_name: &Ident,
    generics: &Generics,
    runtime: &Path,
) -> ImplItemFn {
    let entries_getter_body = if cfg!(feature = "rc") {
        if generics.params.is_empty() {
            // Non-generic types can cache the entries map in a thread-local static
            quote! {
                thread_local! {
                    static ENTRIES: KMap = #struct_ident::#entries_initializer_name();
                }

                ENTRIES.with(KMap::clone)
            }
        } else {
            // Rust doesn't support generic statics, so entries are cached in a hashmap with the
            // concrete instantiation type used as the key.
            let (_impl_generics, ty_generics, _where_clause) = generics.split_for_impl();
            let turbofish = ty_generics.as_turbofish();

            quote! {
                use std::{any::TypeId, cell::RefCell, collections::HashMap, hash::BuildHasherDefault};
                use #runtime::{KMap, KotoHasher};

                type PerTypeEntriesMap = HashMap<TypeId, KMap, BuildHasherDefault<KotoHasher>>;

                thread_local! {
                    static PER_TYPE_ENTRIES: RefCell<PerTypeEntriesMap> = RefCell::default();
                }

                PER_TYPE_ENTRIES
                    .with_borrow_mut(|per_type_entries| {
                        per_type_entries
                            .entry(TypeId::of::<Self>())
                            .or_insert_with(#struct_ident #turbofish :: #entries_initializer_name)
                            .clone()
                    })
            }
        }
    } else if cfg!(feature = "arc") {
        if generics.params.is_empty() {
            // Non-generic types can cache the entries map in a LazyLock
            quote! {
                use std::sync::LazyLock;
                static ENTRIES: LazyLock<KMap> = LazyLock::new(#struct_ident::#entries_initializer_name);
                ENTRIES.clone()
            }
        } else {
            // Rust doesn't support generic statics, so entries are cached in a hashmap with the
            // concrete instantiation type used as the key.
            let (_impl_generics, type_generics, _where_clause) = generics.split_for_impl();
            let turbofish = type_generics.as_turbofish();

            quote! {
                use std::{any::TypeId, collections::HashMap, hash::BuildHasherDefault, sync::LazyLock};
                use #runtime::{KCell, KMap, KotoHasher};

                type PerTypeEntriesMap = HashMap<TypeId, KMap, BuildHasherDefault<KotoHasher>>;

                static PER_TYPE_ENTRIES: LazyLock<KCell<PerTypeEntriesMap>> =
                    LazyLock::new(KCell::default);

                PER_TYPE_ENTRIES
                    .borrow_mut()
                    .entry(TypeId::of::<Self>())
                    .or_insert_with(#struct_ident #turbofish :: #entries_initializer_name)
                    .clone()
            }
        }
    } else {
        quote! { unimplemented!() }
    };

    let entries_getter = quote! {
        #[automatically_derived]
        fn #entries_getter_name() -> #runtime::KMap {
            #entries_getter_body
        }
    };

    syn::parse2(entries_getter).expect("Failed to parse entries getter function")
}
