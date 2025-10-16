use std::{
    cell::{Cell, RefCell},
    mem,
};

use crate::PREFIX_FUNCTION;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Attribute, Error, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr, Meta, Path, ReturnType,
    Type, TypePath,
    meta::ParseNestedMeta,
    parse::{Parse, Result},
    parse_macro_input, parse_quote,
    spanned::Spanned as _,
};

pub(crate) fn koto_impl(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut attrs = KotoImplParser::default();
    let parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with parser);

    let impl_item = parse_macro_input!(item as ItemImpl);

    match Context::new(impl_item, attrs) {
        Ok(ctx) => koto_impl_inner(ctx).into(),
        Err(err) => err.into_compile_error().into(),
    }
}

struct KotoImplParser {
    runtime: Path,
}

impl Default for KotoImplParser {
    fn default() -> Self {
        Self {
            runtime: parse_quote! { ::koto::runtime },
        }
    }
}

impl KotoImplParser {
    fn parse(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("runtime") {
            self.runtime = meta.value()?.parse()?;
            Ok(())
        } else {
            Err(meta.error("unsupported attribute argument"))
        }
    }
}

// Derives an implementation of `KotoAccess` for types tagged with `#[koto_impl]`
fn koto_impl_inner(ctx: Context) -> proc_macro2::TokenStream {
    // We do most of the work in `process`. We're not propagating the error just yet.
    // We want to output an implementation for `KotoAccess` even if we
    // encountered errors, so we don't cause more irrelevant compile errors to occur
    // due to a missing `KotoAccess` implementation.
    let process_result = process(&ctx);

    let Context {
        mut impl_item,
        impl_item_ident,
        runtime,
        get_access,
        get_access_assign,
        access_fallback_fn,
        access_override_fn,
        access_assign_fallback_fn,
        access_assign_override_fn,
        ..
    } = ctx;

    let (impl_generics, ty_generics, where_clause) = impl_item.generics.split_for_impl();
    let ty = impl_item.self_ty.as_ref();
    let turbofish = ty_generics.as_turbofish();

    let koto_access_impl_content = if process_result.is_ok() {
        // Add the generated functions to the impl block.
        let additional_items = mem::take(&mut *ctx.additional_items.borrow_mut());
        impl_item.items.extend(additional_items);

        let access_override = if let Some(override_fn) = access_override_fn.into_inner() {
            quote! {
                if let Some(value) = self.#override_fn(key)? {
                    return Ok(Some(value));
                }
            }
        } else {
            quote! {}
        };

        let access_fallback = if let Some(fallback_fn) = access_fallback_fn.into_inner() {
            quote! {
                self.#fallback_fn(key)
            }
        } else {
            quote! {
                Ok(None)
            }
        };

        let access_assign_override =
            if let Some(override_fn) = access_assign_override_fn.into_inner() {
                quote! {
                    if self.#override_fn(key, value)? {
                        return Ok(());
                    }
                }
            } else {
                quote! {}
            };

        let access_assign_fallback =
            if let Some(fallback_fn) = access_assign_fallback_fn.into_inner() {
                quote! {
                    self.#fallback_fn(key, value)
                }
            } else {
                quote! {
                    #runtime::runtime_error!("unexpected key: {key}")
                }
            };

        quote! {
            fn access(&self, key: &#runtime::KString)
                -> #runtime::Result<::std::option::Option<#runtime::KValue>>
            {
                use #runtime::{KValue, __private::MethodOrField};

                #access_override

                if let Some(method_or_field) = #impl_item_ident #turbofish::#get_access(key) {
                    return match method_or_field {
                        MethodOrField::Method(f) => Ok(Some(
                            KValue::NativeFunction(f)
                        )),
                        MethodOrField::Field(getter) => Ok(Some(
                            getter(self)?
                        )),
                    };
                };

                #access_fallback
            }

            fn access_assign(&mut self, key: &#runtime::KString, value: &#runtime::KValue)
                -> #runtime::Result<()>
            {
                #access_assign_override

                if let Some(setter) = #impl_item_ident #turbofish::#get_access_assign(key) {
                    return setter(self, value);
                };

                #access_assign_fallback
            }
        }
    } else {
        quote! {}
    };

    let koto_access_impl = quote! {
        #[automatically_derived]
        impl #impl_generics #runtime::KotoAccess for #ty #where_clause {
            #koto_access_impl_content
        }
    };

    let errors = process_result.err().map(Error::into_compile_error);

    quote! {
        #impl_item
        #koto_access_impl
        #errors
    }
}

struct Context {
    impl_item: ItemImpl,
    impl_item_ident: Ident,
    runtime: Path,

    create_access_map: Ident,
    create_access_assign_map: Ident,
    get_access: Ident,
    get_access_assign: Ident,

    insert_ops_for_access: InsertOps,
    insert_ops_for_access_assign: InsertOps,
    additional_items: RefCell<Vec<ImplItem>>,

    access_fallback_fn: Cell<Option<Ident>>,
    access_assign_fallback_fn: Cell<Option<Ident>>,
    access_override_fn: Cell<Option<Ident>>,
    access_assign_override_fn: Cell<Option<Ident>>,
}

impl Context {
    fn new(impl_item: ItemImpl, attr: KotoImplParser) -> Result<Self> {
        let impl_item_ident = match &impl_item.self_ty.as_ref() {
            Type::Path(TypePath { path, .. }) => {
                let Some(last_segment) = path.segments.last() else {
                    return Err(Error::new_spanned(path, "Expected an identifier"));
                };
                &last_segment.ident
            }
            ty => return Err(Error::new_spanned(ty, "Expected a type path")),
        }
        .clone();

        Ok(Context {
            // input data
            impl_item,
            runtime: attr.runtime,

            // cached values
            impl_item_ident,

            // The names have an intentional extra underscore at the start as not to conflict with
            // generated wrapper methods.
            create_access_map: format_ident!("_{PREFIX_FUNCTION}create_access_map"),
            create_access_assign_map: format_ident!("_{PREFIX_FUNCTION}create_access_assign_map"),
            get_access: format_ident!("_{PREFIX_FUNCTION}get_access"),
            get_access_assign: format_ident!("_{PREFIX_FUNCTION}get_access_assign"),

            // output data
            insert_ops_for_access: Default::default(),
            insert_ops_for_access_assign: Default::default(),
            additional_items: Default::default(),
            access_fallback_fn: Default::default(),
            access_assign_fallback_fn: Default::default(),
            access_override_fn: Default::default(),
            access_assign_override_fn: Default::default(),
        })
    }

    fn has_generics(&self) -> bool {
        !self.impl_item.generics.params.is_empty()
    }

    fn add_fn_to_impl(&self, item: ImplItemFn) {
        self.additional_items.borrow_mut().push(item.into());
    }

    fn fns_with_attr(&self, attr_name: &str) -> impl Iterator<Item = (&ImplItemFn, &Attribute)> {
        self.impl_item.items.iter().filter_map(|item| match item {
            ImplItem::Fn(f) => f
                .attrs
                .iter()
                .find(|a| a.path().is_ident(attr_name))
                .map(|attr| (f, attr)),
            _ => None,
        })
    }

    fn one_fn_with_attr(&self, attr_name: &str) -> Result<Option<(&ImplItemFn, &Attribute)>> {
        let fns = self.fns_with_attr(attr_name).collect::<Vec<_>>();

        if fns.len() > 1 {
            return Err(Error::new_spanned(
                fns[1].1,
                format!("`#[{attr_name}]` must not be set for multiple functions"),
            ));
        }

        Ok(fns.into_iter().next())
    }

    fn ty(&self) -> proc_macro2::TokenStream {
        let Self {
            impl_item,
            impl_item_ident,
            ..
        } = self;

        let (_, ty_generics, _) = impl_item.generics.split_for_impl();

        quote! {
            #impl_item_ident #ty_generics
        }
    }

    fn ty_turbofish(&self) -> proc_macro2::TokenStream {
        let Self {
            impl_item,
            impl_item_ident,
            ..
        } = self;

        let (_, ty_generics, _) = impl_item.generics.split_for_impl();
        let turbofish = ty_generics.as_turbofish();

        quote! {
            #impl_item_ident #turbofish
        }
    }
}

#[derive(Default)]
struct InsertOps(RefCell<Vec<proc_macro2::TokenStream>>);

impl InsertOps {
    fn add(&self, tokens: proc_macro2::TokenStream) {
        self.0.borrow_mut().push(tokens);
    }

    // Insert multiple insert ops at once.
    fn add_many(&self, count: usize, tokens: proc_macro2::TokenStream) {
        if count == 0 {
            return;
        }

        self.0.borrow_mut().push(tokens);

        // This is kind of hacky, we make sure `insert_ops.len()` reports the correct
        // amount of insert operations by pushing empty tokens for the remaining items.
        for _ in 0..count - 1 {
            self.0.borrow_mut().push(Default::default());
        }
    }

    fn take(&self) -> Vec<proc_macro2::TokenStream> {
        mem::take(&mut *self.0.borrow_mut())
    }
}

fn process(ctx: &Context) -> Result<()> {
    for (fun, attr) in ctx.fns_with_attr("koto_method") {
        handle_koto_method(ctx, fun, attr)?;
    }

    for (fun, attr) in ctx.fns_with_attr("koto_get") {
        handle_koto_get(ctx, fun, attr)?;
    }

    for (fun, attr) in ctx.fns_with_attr("koto_set") {
        handle_koto_set(ctx, fun, attr)?;
    }

    if let Some((fun, attr)) = ctx.one_fn_with_attr("koto_get_fallback")? {
        handle_koto_get_fallback(ctx, fun, attr)?;
    }

    if let Some((fun, attr)) = ctx.one_fn_with_attr("koto_set_fallback")? {
        handle_koto_set_fallback(ctx, fun, attr)?;
    }

    if let Some((fun, attr)) = ctx.one_fn_with_attr("koto_get_override")? {
        handle_koto_get_override(ctx, fun, attr)?;
    }

    if let Some((fun, attr)) = ctx.one_fn_with_attr("koto_set_override")? {
        handle_koto_set_override(ctx, fun, attr)?;
    }

    // Add access and access assign map creation and getter functions.
    //
    // The map creator and getter are separated to avoid "can't use Self from outer item" errors
    // when dealing with generic impls.

    add_access_map_creator(ctx)?;
    add_access_assign_map_creator(ctx)?;

    add_access_getter(ctx)?;
    add_access_assign_getter(ctx)?;

    Ok(())
}

fn handle_koto_method(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let args = AccessAttributeArgs::new(attr)?;
    let names = args.names(|| {
        // Use the function name as key if no explicit name is given.
        Ok(LitStr::new(
            &fun.sig.ident.to_string(),
            fun.sig.ident.span(),
        ))
    })?;

    let wrapper = wrap_koto_method(ctx, fun)?;
    ctx.add_fn_to_impl(wrapper);

    let wrapper_name = koto_method_wrapper_name(fun);

    let value = quote! {
        MethodOrField::Method(KNativeFunction::new(Self::#wrapper_name))
    };

    if names.len() == 1 {
        let name = names.into_iter().next().unwrap();

        ctx.insert_ops_for_access.add(quote! {
            result.insert(
                #name,
                #value,
            );
        });
    } else {
        // Generate additional entries for each function alias
        ctx.insert_ops_for_access.add_many(
            names.len(),
            quote! {
                {
                    let value = #value;
                    #(
                        result.insert(
                            #names,
                            value.clone(),
                        );
                    )*
                }
            },
        );
    }

    Ok(())
}

fn handle_koto_get(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let args = AccessAttributeArgs::new(attr)?;
    let names = args.names(|| {
        // Use the function name as key if no explicit name is given.
        Ok(LitStr::new(
            &fun.sig.ident.to_string(),
            fun.sig.ident.span(),
        ))
    })?;

    let mut args = fun.sig.inputs.iter();
    let self_arg = args.next();

    // The first argument must be `&self`.
    match self_arg {
        Some(FnArg::Receiver(r)) if r.reference.is_some() && r.mutability.is_none() => {}
        _ => {
            return Err(Error::new(
                self_arg.map(|s| s.span()).unwrap_or(fun.sig.span()),
                "Expected `&self` as the first argument of a `#[koto_get]` method",
            ));
        }
    }

    // There must be no further arguments.
    if let Some(arg) = args.next() {
        return Err(Error::new_spanned(
            arg,
            "Expected no extra argument for a `#[koto_get]` method",
        ));
    }

    let return_ty_span = match &fun.sig.output {
        ReturnType::Type(_, ty) => ty.span(),
        ReturnType::Default => {
            return Err(Error::new_spanned(
                &fun.sig,
                "a `#[koto_get]` method must return `KValue` or `koto_runtime::Result<KValue>`",
            ));
        }
    };

    // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
    // the error will point to the function return type.
    let call_result = quote_spanned!(return_ty_span=> call_result);

    let fn_ident = &fun.sig.ident;
    let runtime = &ctx.runtime;
    let ty = ctx.ty();

    let wrapped_call = quote! {
        let #call_result = instance.#fn_ident();
        #runtime::__private::KotoAccessReturn::into_result(#call_result)
    };

    let value = if ctx.has_generics() {
        quote! {
            MethodOrField::Field(
                |instance: &dyn ::std::any::Any| {
                    let instance = instance.downcast_ref::<#ty>().unwrap();
                    #wrapped_call
                }
            )
        }
    } else {
        quote_spanned! { return_ty_span =>
            MethodOrField::Field(
                |instance: &#ty| {
                    #wrapped_call
                }
            )
        }
    };

    if names.len() == 1 {
        let name = names.into_iter().next().unwrap();

        ctx.insert_ops_for_access.add(quote! {
            result.insert(
                #name,
                #value
            );
        });
    } else {
        // Generate additional entries for each function alias
        ctx.insert_ops_for_access.add_many(
            names.len(),
            quote! {
                {
                    let value = #value;
                    #(
                        result.insert(
                            #names,
                            value.clone(),
                        );
                    )*
                }
            },
        );
    }

    Ok(())
}

fn handle_koto_set(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let args = AccessAttributeArgs::new(attr)?;
    let names = args.names(|| {
        // Use the function name without `set_` as key if no explicit name is given.
        let fun_name = fun.sig.ident.to_string();

        let Some(name) = fun_name.strip_prefix("set_") else {
            return Err(Error::new_spanned(
                attr,
                "A `#[koto_set]` method must either start with `set_`,\
                 or have an explicit name given like `#[koto_set(name = \"foo\")]`",
            ));
        };

        Ok(LitStr::new(name, fun.sig.ident.span()))
    })?;

    let mut args = fun.sig.inputs.iter();
    let self_arg = args.next();
    let second_arg = args.next();

    // The first argument must be `&mut self`.
    match self_arg {
        Some(FnArg::Receiver(r)) if r.reference.is_some() && r.mutability.is_some() => {}
        _ => {
            return Err(Error::new(
                self_arg.map(|s| s.span()).unwrap_or(fun.sig.span()),
                "Expected `&mut self` as the first argument of a `#[koto_set]` method",
            ));
        }
    }

    // The second argument must be `&KValue`.
    match second_arg {
        Some(FnArg::Typed(pat)) if matches!(*pat.ty, Type::Reference(_)) => {}
        _ => {
            return Err(Error::new(
                second_arg.map(|s| s.span()).unwrap_or(fun.sig.span()),
                "Expected `&KValue` as the extra argument for a Koto method",
            ));
        }
    }

    // There must be no further arguments.
    if let Some(arg) = args.next() {
        return Err(Error::new_spanned(
            arg,
            "Expected no additional argument for a `#[koto_set]` method",
        ));
    }

    let return_ty_span = match &fun.sig.output {
        ReturnType::Type(_, ty) => ty.span(),
        ReturnType::Default => Span::call_site(),
    };

    // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
    // the error will point to the function return type.
    let call_result = quote_spanned!(return_ty_span=> call_result);

    let fn_ident = &fun.sig.ident;
    let runtime = &ctx.runtime;
    let ty = ctx.ty();

    let wrapped_call = quote! {
        let #call_result = instance.#fn_ident(value);
        #runtime::__private::KotoAccessAssignReturn::into_result(#call_result)
    };

    let value = if ctx.has_generics() {
        quote! {
            |instance: &mut dyn Any, value: &KValue| -> Result<()> {
                let instance = instance.downcast_mut::<#ty>().unwrap();
                #wrapped_call
            }
        }
    } else {
        quote! {
            |instance: &mut #ty, value: &KValue| -> Result<()> {
                #wrapped_call
            }
        }
    };

    if names.len() == 1 {
        let name = names.into_iter().next().unwrap();

        ctx.insert_ops_for_access_assign.add(quote! {
            result.insert(
                #name,
                #value
            );
        });
    } else {
        // Generate additional entries for each function alias
        ctx.insert_ops_for_access_assign.add_many(
            names.len(),
            quote! {
                {
                    let value = #value;
                    #(
                        result.insert(
                            #names,
                            value.clone(),
                        );
                    )*
                }
            },
        );
    }

    Ok(())
}

fn handle_koto_get_fallback(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let _args = FallbackAttributeArgs::new(attr)?;

    let return_ty_span = match &fun.sig.output {
        ReturnType::Type(_, ty) => ty.span(),
        ReturnType::Default => {
            return Err(Error::new_spanned(
                &fun.sig,
                "a `#[koto_get_fallback]` method must return `Option<KValue>` or `koto_runtime::Result<Option<KValue>>`",
            ));
        }
    };

    // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
    // the error will point to the function return type.
    let call_result = quote_spanned!(return_ty_span=> call_result);

    let fn_ident = &fun.sig.ident;
    let runtime = &ctx.runtime;

    let wrapped_call = quote! {
        let #call_result = self.#fn_ident(key);
        #runtime::__private::KotoAccessFallbackReturn::into_result(#call_result)
    };

    let wrapper_name = koto_method_wrapper_name(fun);

    let wrapped_fn = quote! {
        fn #wrapper_name(&self, key: &#runtime::KString)
            -> #runtime::Result<Option<#runtime::KValue>>
        {
            #wrapped_call
        }
    };

    let item = parse2(
        wrapped_fn,
        "the generated `#[koto_get_fallback]` method wrapper",
    )?;

    ctx.add_fn_to_impl(item);
    ctx.access_fallback_fn.set(Some(wrapper_name));
    Ok(())
}

fn handle_koto_set_fallback(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let _args = FallbackAttributeArgs::new(attr)?;

    let return_ty_span = match &fun.sig.output {
        ReturnType::Type(_, ty) => ty.span(),
        ReturnType::Default => Span::call_site(),
    };

    // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
    // the error will point to the function return type.
    let call_result = quote_spanned!(return_ty_span=> call_result);

    let fn_ident = &fun.sig.ident;
    let runtime = &ctx.runtime;

    let wrapped_call = quote! {
        let #call_result = self.#fn_ident(key, value);
        #runtime::__private::KotoAccessAssignFallbackReturn::into_result(#call_result)
    };

    let wrapper_name = koto_method_wrapper_name(fun);

    let wrapped_fn = quote! {
        fn #wrapper_name(&mut self, key: &KString, value: &KValue)
            -> #runtime::Result<()>
        {
            #wrapped_call
        }
    };

    let item = parse2(
        wrapped_fn,
        "the generated `#[koto_set_fallback]` method wrapper",
    )?;

    ctx.add_fn_to_impl(item);
    ctx.access_assign_fallback_fn.set(Some(wrapper_name));
    Ok(())
}

fn handle_koto_get_override(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let _args = FallbackAttributeArgs::new(attr)?;

    let return_ty_span = match &fun.sig.output {
        ReturnType::Type(_, ty) => ty.span(),
        ReturnType::Default => {
            return Err(Error::new_spanned(
                &fun.sig,
                "a `#[koto_get_override]` method must return `Option<KValue>` or `koto_runtime::Result<Option<KValue>>`",
            ));
        }
    };

    // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
    // the error will point to the function return type.
    let call_result = quote_spanned!(return_ty_span=> call_result);

    let fn_ident = &fun.sig.ident;
    let runtime = &ctx.runtime;

    let wrapped_call = quote! {
        let #call_result = self.#fn_ident(key);
        #runtime::__private::KotoAccessOverrideReturn::into_result(#call_result)
    };

    let wrapper_name = koto_method_wrapper_name(fun);

    let wrapped_fn = quote! {
        fn #wrapper_name(&self, key: &KString)
            -> #runtime::Result<Option<KValue>>
        {
            #wrapped_call
        }
    };

    let item = parse2(
        wrapped_fn,
        "the generated `#[koto_get_override]` method wrapper",
    )?;

    ctx.add_fn_to_impl(item);
    ctx.access_override_fn.set(Some(wrapper_name));
    Ok(())
}

fn handle_koto_set_override(ctx: &Context, fun: &ImplItemFn, attr: &Attribute) -> Result<()> {
    let _args = FallbackAttributeArgs::new(attr)?;

    let return_ty_span = match &fun.sig.output {
        ReturnType::Type(_, ty) => ty.span(),
        ReturnType::Default => {
            return Err(Error::new_spanned(
                &fun.sig,
                "a `#[koto_set_override]` method must return `bool` or `koto_runtime::Result<bool>`",
            ));
        }
    };

    // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
    // the error will point to the function return type.
    let call_result = quote_spanned!(return_ty_span=> call_result);

    let fn_ident = &fun.sig.ident;
    let runtime = &ctx.runtime;

    let wrapped_call = quote! {
        let #call_result = self.#fn_ident(key, value);
        #runtime::__private::KotoAccessAssignOverrideReturn::into_result(#call_result)
    };

    let wrapper_name = koto_method_wrapper_name(fun);

    let wrapped_fn = quote! {
        fn #wrapper_name(&mut self, key: &KString, value: &KValue)
            -> #runtime::Result<bool>
        {
            #wrapped_call
        }
    };

    let item = parse2(
        wrapped_fn,
        "the generated `#[koto_set_override]` method wrapper",
    )?;

    ctx.add_fn_to_impl(item);
    ctx.access_assign_override_fn.set(Some(wrapper_name));
    Ok(())
}

fn wrap_koto_method(ctx: &Context, fun: &ImplItemFn) -> Result<ImplItemFn> {
    let arg_count = fun.sig.inputs.len();
    let mut args = fun.sig.inputs.iter();

    let runtime = &ctx.runtime;

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
                        quote! {
                            _ => #runtime::runtime_error!(#runtime::ErrorKind::UnexpectedError)
                        },
                    )
                }
                _ => panic!("Expected &[KValue] as the extra argument for a Koto method"),
            };

            let return_ty_span = match &fun.sig.output {
                ReturnType::Type(_, ty) => ty.span(),
                ReturnType::Default => Span::call_site(),
            };

            // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
            // the error will point to the function return type.
            let call_result = quote_spanned!(return_ty_span=> call_result);

            let fn_ident = &fun.sig.ident;
            let runtime = &ctx.runtime;

            let wrapped_call = quote! {
                let #call_result = instance.#fn_ident(#call_args);
                #runtime::__private::KotoMethodReturn::into_result(#call_result)
            };

            quote! {{
                use #runtime::KValue;
                match ctx.instance_and_args(
                    |i| matches!(i, KValue::Object(_)),
                    <Self as #runtime::KotoType>::type_static()
                )? {
                    (KValue::Object(o), #args_match) => {
                        match o.#cast::<Self>() {
                            Ok(#instance) => { #wrapped_call }
                            Err(e) => Err(e),
                        }
                    },
                    #error_arm,
                }
            }}
        }
        // Functions that take a MethodContext
        _ => {
            let return_ty_span = match &fun.sig.output {
                ReturnType::Type(_, ty) => ty.span(),
                ReturnType::Default => Span::call_site(),
            };

            // Attach a span to `call_result` so that when it does not implement the required return `*Return` trait
            // the error will point to the function return type.
            let call_result = quote_spanned!(return_ty_span=> call_result);

            let fn_ident = &fun.sig.ident;
            let runtime = &ctx.runtime;

            let wrapped_call = quote! {
                let #call_result = Self::#fn_ident(MethodContext::new(&o, extra_args, ctx.vm));
                #runtime::__private::KotoMethodReturn::into_result(#call_result)
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

    let wrapper_name = koto_method_wrapper_name(fun);

    let wrapped_fn = quote! {
        #[automatically_derived]
        fn #wrapper_name(ctx: &mut #runtime::CallContext) -> #runtime::Result<#runtime::KValue> {
            #wrapper_body
        }
    };

    parse2(
        wrapped_fn,
        "Failed to parse the generated `#[koto_method]` method wrapper",
    )
}

fn koto_method_wrapper_name(f: &ImplItemFn) -> Ident {
    format_ident!("{PREFIX_FUNCTION}{}", f.sig.ident)
}

fn add_access_map_creator(ctx: &Context) -> Result<()> {
    let name = &ctx.create_access_map;
    let insert_ops = ctx.insert_ops_for_access.take();
    let insert_ops_len = insert_ops.len();
    let runtime = &ctx.runtime;

    let create_fn = if ctx.has_generics() {
        quote! {
            #[automatically_derived]
            fn #name() -> ::std::collections::HashMap<
                &'static str,
                #runtime::__private::MethodOrField<dyn ::std::any::Any>,
                ::std::hash::BuildHasherDefault<#runtime::KotoHasher>,
            > {
                use ::std::{any::Any, collections::HashMap, hash::BuildHasherDefault};
                use #runtime::{
                    KMap, KNativeFunction, KotoHasher, KValue, ValueKey, ValueMap,
                    __private::MethodOrField,
                };

                let mut result = HashMap::<
                    &'static str,
                    MethodOrField<dyn Any>,
                    BuildHasherDefault<KotoHasher>,
                >::with_capacity_and_hasher(#insert_ops_len, BuildHasherDefault::new());

                #(#insert_ops)*

                result
            }
        }
    } else {
        let ty = ctx.ty();

        quote! {
            #[automatically_derived]
            fn #name() -> ::std::collections::HashMap<
                &'static str,
                #runtime::__private::MethodOrField<#ty>,
                ::std::hash::BuildHasherDefault<#runtime::KotoHasher>,
            > {
                use ::std::{collections::HashMap, hash::BuildHasherDefault};
                use #runtime::{
                    KMap, KNativeFunction, KotoHasher, KValue, ValueKey, ValueMap,
                    __private::MethodOrField,
                };

                let mut result = HashMap::<
                    &'static str,
                    MethodOrField<#ty>,
                    BuildHasherDefault<KotoHasher>,
                >::with_capacity_and_hasher(#insert_ops_len, BuildHasherDefault::new());

                #(#insert_ops)*

                result
            }
        }
    };

    let item = parse2(
        create_fn,
        "the generated `#[koto_impl]` access map creation function",
    )?;

    ctx.add_fn_to_impl(item);
    Ok(())
}

fn add_access_assign_map_creator(ctx: &Context) -> Result<()> {
    let name = &ctx.create_access_assign_map;
    let insert_ops = ctx.insert_ops_for_access_assign.take();
    let insert_ops_len = insert_ops.len();
    let runtime = &ctx.runtime;

    let create_fn = if ctx.impl_item.generics.params.is_empty() {
        let ty = ctx.ty();

        quote! {
            #[automatically_derived]
            fn #name() -> ::std::collections::HashMap<
                &'static str,
                fn(&mut #ty, &#runtime::KValue) -> #runtime::Result<()>,
                ::std::hash::BuildHasherDefault<#runtime::KotoHasher>,
            > {
                use ::std::{any::Any, collections::HashMap, hash::BuildHasherDefault};
                use #runtime::{
                    KMap, KNativeFunction, KotoHasher, KValue, Result, ValueKey, ValueMap,
                };

                let mut result = HashMap::<
                    &'static str,
                    fn(&mut #ty, &KValue) -> Result<()>,
                    BuildHasherDefault<KotoHasher>,
                >::with_capacity_and_hasher(#insert_ops_len, BuildHasherDefault::new());

                #(#insert_ops)*

                result
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            fn #name() -> ::std::collections::HashMap<
                &'static str,
                fn(&mut dyn ::std::any::Any, &#runtime::KValue) -> #runtime::Result<()>,
                ::std::hash::BuildHasherDefault<#runtime::KotoHasher>,
            > {
                use ::std::{any::Any, collections::HashMap, hash::BuildHasherDefault};
                use #runtime::{KMap, KNativeFunction, KValue, ValueKey, ValueMap, Result};

                let mut result = ::std::collections::HashMap::<
                    &'static str,
                    fn(&mut dyn Any, &KValue) -> Result<()>,
                    BuildHasherDefault<#runtime::KotoHasher>,
                >::with_capacity_and_hasher(#insert_ops_len, BuildHasherDefault::new());

                #(#insert_ops)*

                result
            }
        }
    };

    let item = parse2(
        create_fn,
        "the generated `#[koto_impl]` access assign map creation function",
    )?;

    ctx.add_fn_to_impl(item);
    Ok(())
}

fn add_access_getter(ctx: &Context) -> Result<()> {
    let name = &ctx.get_access;
    let create_access_map = &ctx.create_access_map;
    let runtime = &ctx.runtime;
    let ty_turbofish = ctx.ty_turbofish();

    let getter_fn = if ctx.impl_item.generics.params.is_empty() {
        // Non-generic types can cache the entries map in a `thread_local`/`LazyLock`
        let ty = ctx.ty();

        if cfg!(feature = "rc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str) -> Option<#runtime::__private::MethodOrField<#ty>> {
                    use ::std::{collections::HashMap, hash::BuildHasherDefault};
                    use #runtime::{KotoHasher, __private::MethodOrField};

                    thread_local! {
                        static ENTRIES: HashMap<
                            &'static str,
                            MethodOrField<#ty>,
                            BuildHasherDefault<KotoHasher>,
                        > = #ty_turbofish::#create_access_map();
                    }

                    ENTRIES.with(|entries| entries.get(key).cloned())
                }
            }
        } else if cfg!(feature = "arc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str) -> Option<#runtime::__private::MethodOrField<#ty>> {
                    use ::std::{collections::HashMap, sync::LazyLock, hash::BuildHasherDefault};
                    use #runtime::{lazy, __private::MethodOrField, KotoHasher};

                    static ENTRIES: LazyLock<HashMap<
                        &'static str,
                        MethodOrField<#ty>,
                        BuildHasherDefault<KotoHasher>,
                    >> = LazyLock::new(#ty_turbofish::#create_access_map);

                    LazyLock::force(&ENTRIES).get(key).cloned()
                }
            }
        } else {
            no_feature_set!()
        }
    } else {
        // Rust doesn't support generic statics, so entries are cached in a hashmap with the
        // concrete instantiation type used as the key.

        if cfg!(feature = "rc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str)
                    -> Option<#runtime::__private::MethodOrField<dyn ::std::any::Any>>
                {
                    use ::std::{
                        any::TypeId, cell::RefCell, collections::HashMap, hash::BuildHasherDefault,
                    };
                    use #runtime::{KMap, KotoHasher, __private::MethodOrField};

                    type PerTypeEntriesMap = HashMap<
                        TypeId,
                        HashMap<
                            &'static str,
                            MethodOrField<dyn ::std::any::Any>,
                            BuildHasherDefault<KotoHasher>,
                        >,
                        BuildHasherDefault<KotoHasher>,
                    >;

                    thread_local! {
                        static PER_TYPE_ENTRIES: RefCell<PerTypeEntriesMap> = RefCell::default();
                    }

                    PER_TYPE_ENTRIES
                        .with_borrow_mut(|per_type_entries| {
                            per_type_entries
                                .entry(TypeId::of::<Self>())
                                .or_insert_with(#ty_turbofish::#create_access_map)
                                .get(key).cloned()
                        })
                }
            }
        } else if cfg!(feature = "arc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str)
                    -> Option<#runtime::__private::MethodOrField<dyn ::std::any::Any>>
                {
                    use ::std::{
                        any::TypeId, collections::HashMap, hash::BuildHasherDefault, sync::LazyLock,
                    };
                    use #runtime::{
                        KCell, KMap, KotoHasher, KNativeFunction, __private::MethodOrField,
                    };

                    type PerTypeEntriesMap = HashMap<
                        TypeId,
                        HashMap<
                            &'static str,
                            MethodOrField<dyn ::std::any::Any>,
                            BuildHasherDefault<KotoHasher>
                        >,
                        BuildHasherDefault<KotoHasher>
                    >;

                    static PER_TYPE_ENTRIES: LazyLock<KCell<PerTypeEntriesMap>> =
                        LazyLock::new(KCell::default);

                    PER_TYPE_ENTRIES
                        .borrow_mut()
                        .entry(TypeId::of::<Self>())
                        .or_insert_with(#ty_turbofish::#create_access_map)
                        .get(key).cloned()
                }
            }
        } else {
            no_feature_set!()
        }
    };

    let item = parse2(
        getter_fn,
        "the generated `#[koto_impl]` access getter function",
    )?;

    ctx.add_fn_to_impl(item);
    Ok(())
}

fn add_access_assign_getter(ctx: &Context) -> Result<()> {
    let name = &ctx.get_access_assign;
    let create_access_map = &ctx.create_access_assign_map;
    let runtime = &ctx.runtime;
    let ty = ctx.ty();
    let ty_turbofish = ctx.ty_turbofish();

    let getter_fn = if ctx.impl_item.generics.params.is_empty() {
        // Non-generic types can cache the entries map in a `thread_local`/`LazyLock`

        if cfg!(feature = "rc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str) -> Option<fn(&mut #ty, &KValue) -> #runtime::Result<()>> {
                    use ::std::{collections::HashMap, hash::BuildHasherDefault};
                    use #runtime::{Result, KotoHasher};

                    thread_local! {
                        static ENTRIES: HashMap<
                            &'static str,
                            fn(&mut #ty, &KValue) -> Result<()>,
                            BuildHasherDefault<KotoHasher>,
                        > = #ty_turbofish::#create_access_map();
                    }

                    ENTRIES.with(|entries| entries.get(key).cloned())
                }
            }
        } else if cfg!(feature = "arc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str) -> Option<fn(&mut #ty, &KValue) -> #runtime::Result<()>> {
                    use ::std::{collections::HashMap, sync::LazyLock, hash::BuildHasherDefault};
                    use #runtime::{lazy, KotoHasher};

                    static ENTRIES: LazyLock<HashMap<
                        &'static str,
                        fn(&mut #ty, &KValue) -> Result<()>,
                        BuildHasherDefault<KotoHasher>,
                    >> = LazyLock::new(#ty_turbofish::#create_access_map);

                    LazyLock::force(&ENTRIES).get(key).cloned()
                }
            }
        } else {
            no_feature_set!()
        }
    } else {
        // Rust doesn't support generic statics, so entries are cached in a hashmap with the
        // concrete instantiation type used as the key.

        if cfg!(feature = "rc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str) -> Option<fn(&mut dyn ::std::any::Any, &#runtime::KValue)
                    -> #runtime::Result<()>>
                {
                    use ::std::{
                        any::TypeId, cell::RefCell, collections::HashMap, hash::BuildHasherDefault,
                    };
                    use #runtime::{KMap, KotoHasher, KValue, Result};

                    type PerTypeEntriesMap = HashMap<
                        TypeId,
                        HashMap<
                            &'static str,
                            fn(&mut dyn ::std::any::Any, &KValue) -> Result<()>,
                            BuildHasherDefault<KotoHasher>,
                        >,
                        BuildHasherDefault<KotoHasher>,
                    >;

                    thread_local! {
                        static PER_TYPE_ENTRIES: RefCell<PerTypeEntriesMap> = RefCell::default();
                    }

                    PER_TYPE_ENTRIES
                        .with_borrow_mut(|per_type_entries| {
                            per_type_entries
                                .entry(TypeId::of::<Self>())
                                .or_insert_with(#ty_turbofish::#create_access_map)
                                .get(key).cloned()
                        })
                }
            }
        } else if cfg!(feature = "arc") {
            quote! {
                #[automatically_derived]
                fn #name(key: &str)
                    -> Option<fn(&mut dyn ::std::any::Any, &KValue) -> #runtime::Result<()>>
                {
                    use ::std::{
                        any::TypeId, collections::HashMap, hash::BuildHasherDefault, sync::LazyLock,
                    };
                    use #runtime::{KCell, KMap, KotoHasher, KNativeFunction, Result};

                    type PerTypeEntriesMap = HashMap<
                        TypeId,
                        HashMap<
                            &'static str,
                            fn(&mut dyn ::std::any::Any, &KValue) -> Result<()>,
                            BuildHasherDefault<KotoHasher>,
                        >,
                        BuildHasherDefault<KotoHasher>,
                    >;

                    static PER_TYPE_ENTRIES: LazyLock<KCell<PerTypeEntriesMap>> =
                        LazyLock::new(KCell::default);

                    PER_TYPE_ENTRIES
                        .borrow_mut()
                        .entry(TypeId::of::<Self>())
                        .or_insert_with(#ty_turbofish::#create_access_map)
                        .get(key).cloned()
                }
            }
        } else {
            no_feature_set!()
        }
    };

    let item = parse2(
        getter_fn,
        "the generated `#[koto_impl]` access assign getter function",
    )?;

    ctx.add_fn_to_impl(item);
    Ok(())
}

struct AccessAttributeArgs {
    name: Option<LitStr>,
    aliases: Vec<LitStr>,
}

impl AccessAttributeArgs {
    fn new(attr: &Attribute) -> Result<Self> {
        let mut name = None::<LitStr>;
        let mut aliases = Vec::new();

        if matches!(attr.meta, Meta::List(_)) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    name = meta.value()?.parse()?;
                    Ok(())
                } else if meta.path.is_ident("alias") {
                    aliases.push(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("unsupported attribute argument"))
                }
            })?;
        }

        Ok(Self { name, aliases })
    }

    /// Returns entries for all names that should be associated with this access.
    ///
    /// If there is no `name` attribute, then `name_fallback` will be invoked to
    /// produce a name in its stead.
    fn names(self, name_fallback: impl FnOnce() -> Result<LitStr>) -> Result<Vec<LitStr>> {
        let name = match self.name {
            Some(name) => name,
            None => name_fallback()?,
        };

        let mut names = vec![name];
        names.extend(self.aliases);
        Ok(names)
    }
}

struct FallbackAttributeArgs {}

impl FallbackAttributeArgs {
    fn new(attr: &Attribute) -> Result<Self> {
        if !matches!(attr.meta, Meta::Path(_)) {
            // We already filtered out paths that were no idents.
            let name = attr.path().get_ident().unwrap();

            return Err(Error::new_spanned(
                attr,
                format!("The `#[{name}]` attribute has no arguments"),
            ));
        }

        Ok(Self {})
    }
}

// Like `syn::parse2` but with a more helpful error message.
fn parse2<T: Parse>(tokens: proc_macro2::TokenStream, what: &str) -> Result<T> {
    let tokens_string = tokens.to_string();

    syn::parse2(tokens).map_err(|err| {
        Error::new(
            Span::call_site(),
            format!("Failed to parse {what}\nerror: {err}\ntokens: {tokens_string}"),
        )
    })
}

macro_rules! no_feature_set {
    () => {
        return Err(Error::new(
            Span::call_site(),
            r#"Either the \"rc\" or \"arc\" feature must be enabled!"#,
        ))
    };
}

use no_feature_set;
