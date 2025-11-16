use std::iter;

use indexmap::IndexMap;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Attribute, Error, FnArg, GenericArgument, Ident, ImplItemFn, ItemFn, LitStr, Meta, PatType,
    PathArguments, PathSegment, Result, ReturnType, Signature, Type, TypePath, TypeReference,
    TypeSlice, spanned::Spanned,
};

#[derive(Clone, Copy)]
pub(crate) enum OverloadOptions {
    Function,
    Method,
}

impl OverloadOptions {
    fn allow_self(&self) -> bool {
        matches!(self, OverloadOptions::Method)
    }

    fn allow_method_context(&self) -> bool {
        matches!(self, OverloadOptions::Method)
    }

    fn allow_call_context(&self) -> bool {
        matches!(self, OverloadOptions::Function)
    }

    fn max_arguments(&self) -> usize {
        // May be used in the future when overloading `#[koto_set]`.
        usize::MAX
    }
}

#[derive(Default)]
pub(crate) struct OverloadedFunctions {
    pub(crate) inner: IndexMap<String, OverloadedFunction>,
}

impl OverloadedFunctions {
    pub(crate) fn insert(&mut self, definition: OverloadedFunctionCandidate) {
        self.inner
            .entry(definition.name.value())
            .or_default()
            .candidates
            .push(definition);
    }
}

#[derive(Default)]
pub(crate) struct OverloadedFunction {
    // The vec of candidates must not be empty or some methods will panic.
    // In practice, this is ensured by the `OverloadedFunctions::insert` implementation,
    // which is the only place `OverloadedFunction`s are created.
    pub(crate) candidates: Vec<OverloadedFunctionCandidate>,
}

impl OverloadedFunction {
    pub(crate) fn first_ident(&self) -> &Ident {
        &self.candidates.first().unwrap().ident
    }

    pub(crate) fn name(&self) -> &LitStr {
        // All candidates have the same name.
        &self.candidates.first().unwrap().name
    }

    pub(crate) fn options(&self) -> OverloadOptions {
        self.candidates.first().unwrap().options
    }

    pub(crate) fn name_and_aliases(&self) -> Vec<LitStr> {
        iter::once(self.name())
            .chain(
                self.candidates
                    .iter()
                    .flat_map(|candidate| &candidate.aliases),
            )
            .map(|alias| (alias.value(), alias.clone()))
            .collect::<IndexMap<_, _>>()
            .into_values()
            .collect()
    }

    pub(crate) fn match_arms(&self) -> Result<TokenStream> {
        let mut match_arms = Vec::with_capacity(self.candidates.len());
        let mut unexpected_args_error = String::new();

        for (i, definition) in self.candidates.iter().enumerate() {
            match_arms.push(definition.match_arm()?);

            if i > 0 {
                unexpected_args_error.push_str(", ");
            }

            if self.candidates.len() > 1 && i == self.candidates.len() - 1 {
                unexpected_args_error.push_str("or ");
            }

            unexpected_args_error.push_str(&definition.args.signature());
        }

        let error_expr = quote! {
            unexpected_args(#unexpected_args_error, unexpected)
        };

        let error_arm = if matches!(self.options(), OverloadOptions::Method) {
            quote!((_, unexpected) => #error_expr,)
        } else {
            quote!(unexpected => #error_expr,)
        };

        let arms = quote! {
            #(#match_arms)*
            #error_arm
        };

        Ok(arms)
    }
}

pub(crate) struct OverloadedFunctionCandidate {
    pub(crate) name: LitStr,
    pub(crate) aliases: Vec<LitStr>,
    pub(crate) ident: Ident,
    pub(crate) args: KotoArgs,
    // This may originally have been an `ItemFn` or `ImplItemFn`.
    // We use `ImplItemFn` because any `ItemFn` can also be represented by an `ImplItemFn`.
    pub(crate) item: ImplItemFn,
    pub(crate) options: OverloadOptions,
}

impl OverloadedFunctionCandidate {
    pub(crate) fn new(
        item: impl ItemFnOrImplItemFn,
        args: AccessAttributeArgs,
        options: OverloadOptions,
    ) -> Result<Self> {
        let item = item.into_impl_item_fn();
        let ident = item.sig.ident.clone();
        Self::with_name_fallback(item, args, options, || {
            Ok(LitStr::new(&ident.to_string(), ident.span()))
        })
    }

    pub(crate) fn with_name_fallback(
        item: ImplItemFn,
        args: AccessAttributeArgs,
        options: OverloadOptions,
        name_fallback: impl FnOnce() -> Result<LitStr>,
    ) -> Result<Self> {
        Ok(OverloadedFunctionCandidate {
            name: match args.name {
                Some(name) => name,
                None => name_fallback()?,
            },
            aliases: args.aliases,
            ident: item.sig.ident.clone(),
            args: KotoArgs::from_sig(&item.sig, options)?,
            item,
            options,
        })
    }

    pub(crate) fn match_arm(&self) -> Result<TokenStream> {
        let call_exprs = self.args.call_exprs();
        let fn_name = &self.item.sig.ident;

        let mut call = quote! {
            #fn_name(#(#call_exprs, )*)
        };

        if matches!(self.options, OverloadOptions::Method) {
            call = quote!(Self::#call);
        }

        let match_pats = self
            .value_args()
            .map(|(arg, value)| value.match_pats(&arg.name))
            .collect::<Vec<_>>();

        let setup_exprs = self
            .value_args()
            .map(|(arg, _)| &arg.setup_expr)
            .collect::<Vec<_>>();

        let match_conditions = self
            .value_args()
            .flat_map(|(_, value)| value.match_condition.as_ref())
            .collect::<Vec<_>>();

        let condition = match match_conditions.as_slice() {
            [] => quote!(),
            [first, rest @ ..] => quote! {
                if #first #(&& #rest)*
            },
        };

        // Special handling of methods with a `MethodContext`.
        if matches!(self.options, OverloadOptions::Method) {
            let has_method_context_param = self.args.inner.iter().any(|arg| matches!(&arg.kind, KotoArgKind::Context(context) if matches!(context.kind, KotoContextArgKind::MethodContext)));

            if has_method_context_param {
                if self.args.inner.len() > 1 {
                    return Err(Error::new_spanned(
                        &self.item.sig.inputs,
                        "Unexpected additional parameter for a `#[koto_method]` taking a `MethodContext`",
                    ));
                }

                let wrapped_call = self.wrap_call(call);

                let arm = quote! {
                    (KValue::Object(o), extra_args) => { #wrapped_call }
                };

                return Ok(arm);
            }
        }

        let mut pattern = quote! {
            [#(#match_pats,)*]
        };

        if matches!(self.options, OverloadOptions::Method) {
            pattern = quote!((KValue::Object(o), #pattern));
        }

        // For functions we insert the implementation in the match arm,
        // but methods remain in the `impl` block and are called using `Self::fun(...)`.
        let expr = match self.options {
            OverloadOptions::Function => {
                let fn_impl = &self.item;
                let wrapped_call = self.wrap_call(call);

                quote! {{
                    #fn_impl
                    #(#setup_exprs)*
                    return #wrapped_call;
                }}
            }
            OverloadOptions::Method => {
                // Special handling of methods with `self`.
                if let Some(KotoArg {
                    kind: KotoArgKind::Receiver(receiver),
                    ..
                }) = self.args.inner.first()
                {
                    let cast = if receiver.is_mut {
                        quote!(cast_mut)
                    } else {
                        quote!(cast)
                    };

                    let instance = if receiver.is_mut {
                        quote!(mut instance)
                    } else {
                        quote!(instance)
                    };

                    enum ReturnKind {
                        // `-> &Self` or `-> &mut Self`
                        RefSelf,
                        // `-> Result<&Self>` or `-> Result<&mut Self>`
                        ResultRefSelf,
                        // Any other return type
                        Other,
                    }

                    let return_kind = match &self.item.sig.output {
                        ReturnType::Default => ReturnKind::Other,
                        ReturnType::Type(_, ty) => match &**ty {
                            Type::Reference(ty) => match &*ty.elem {
                                Type::Path(type_path) if type_path.path.is_ident("Self") => {
                                    ReturnKind::RefSelf
                                }
                                _ => ReturnKind::Other,
                            },
                            Type::Path(type_path) => match type_path.path.segments.last() {
                                Some(PathSegment {
                                    arguments: PathArguments::AngleBracketed(args),
                                    ..
                                }) => match args.args.first() {
                                    Some(GenericArgument::Type(Type::Reference(ty))) => {
                                        match &*ty.elem {
                                            Type::Path(type_path)
                                                if type_path.path.is_ident("Self") =>
                                            {
                                                ReturnKind::ResultRefSelf
                                            }
                                            _ => ReturnKind::Other,
                                        }
                                    }
                                    _ => ReturnKind::Other,
                                },
                                _ => ReturnKind::Other,
                            },
                            _ => ReturnKind::Other,
                        },
                    };

                    let expr = match return_kind {
                        ReturnKind::RefSelf => quote! {{
                            #(#setup_exprs)*
                            #call;
                            return Ok(o.clone().into());
                        }},
                        ReturnKind::ResultRefSelf => quote! {{
                            #(#setup_exprs)*
                            #call?;
                            return Ok(o.clone().into());
                        }},
                        ReturnKind::Other => {
                            let wrapped_call = self.wrap_call(call);

                            quote! {{
                                #(#setup_exprs)*
                                return #wrapped_call;
                            }}
                        }
                    };

                    quote! {
                        match o.#cast::<Self>() {
                            Ok(#instance) => #expr
                            Err(e) => Err(e),
                        }
                    }
                } else {
                    let wrapped_call = self.wrap_call(call);

                    quote! {{
                        #(#setup_exprs)*
                        return #wrapped_call;
                    }}
                }
            }
        };

        let arm = quote! {
            #pattern #condition => #expr
        };

        Ok(arm)
    }

    fn value_args(&self) -> impl Iterator<Item = (&KotoArg, &KotoValueArg)> {
        self.args.inner.iter().filter_map(|arg| match &arg.kind {
            KotoArgKind::Value(value) => Some((arg, value)),
            _ => None,
        })
    }

    fn wrap_call(&self, call: TokenStream) -> TokenStream {
        let span = match &self.item.sig.output {
            ReturnType::Type(_, ty) => ty.span(),
            ReturnType::Default => Span::call_site(),
        };

        let return_trait = match self.options {
            OverloadOptions::Function => quote_spanned!(span=> KotoFunctionReturn),
            OverloadOptions::Method => quote_spanned!(span=> KotoMethodReturn),
        };

        // Attach a span to so a type error will point at the right place.
        quote_spanned!(span=> #return_trait::into_result(#call))
    }
}

/// Either an `ItemFn` or `ImplItemFn`.
pub(crate) trait ItemFnOrImplItemFn {
    fn into_impl_item_fn(self) -> ImplItemFn;
}

impl ItemFnOrImplItemFn for ImplItemFn {
    fn into_impl_item_fn(self) -> ImplItemFn {
        self
    }
}

impl ItemFnOrImplItemFn for ItemFn {
    fn into_impl_item_fn(self) -> ImplItemFn {
        let Self {
            attrs,
            vis,
            sig,
            block,
        } = self;
        ImplItemFn {
            attrs,
            vis,
            defaultness: None,
            sig,
            block: *block,
        }
    }
}

pub(crate) struct KotoArgs {
    inner: Vec<KotoArg>,
}

impl KotoArgs {
    pub(crate) fn from_sig(sig: &Signature, options: OverloadOptions) -> Result<Self> {
        if sig.inputs.len() > options.max_arguments() {
            return Err(Error::new_spanned(sig, "too many arguments"));
        }

        let args = sig
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| match input {
                FnArg::Receiver(receiver) => {
                    if !options.allow_self() {
                        return Err(Error::new_spanned(
                            input,
                            "`self` arguments are not supported",
                        ));
                    }

                    Ok(KotoArg {
                        name: format_ident!("instance"),
                        kind: KotoArgKind::Receiver(KotoReceiverArg {
                            is_mut: receiver.mutability.is_some(),
                        }),
                        setup_expr: None,
                        call_expr: None,
                    })
                }
                FnArg::Typed(PatType { ty: arg_type, .. }) => {
                    let is_last_arg = i == sig.inputs.len() - 1;
                    KotoArg::new(arg_type, i, is_last_arg, options)
                }
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { inner: args })
    }

    pub(crate) fn signature(&self) -> String {
        let mut result = "|".to_string();

        for (i, arg) in self
            .inner
            .iter()
            .filter_map(|arg| match &arg.kind {
                KotoArgKind::Value(value) => Some(value),
                _ => None,
            })
            .enumerate()
        {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&arg.display_name());
        }

        result.push('|');
        result
    }

    fn call_exprs(&self) -> Vec<TokenStream> {
        self.inner.iter().flat_map(|arg| arg.call_expr()).collect()
    }
}

struct KotoArg {
    name: Ident,
    kind: KotoArgKind,
    // Pre-call setup (e.g. calling make_iterator)
    setup_expr: Option<TokenStream>,
    // How the arg should be passed to the user's function
    call_expr: Option<TokenStream>,
}

impl KotoArg {
    fn new(arg_type: &Type, id: usize, is_last: bool, options: OverloadOptions) -> Result<Self> {
        let arg_name = format_ident!("arg_{id}");
        Self::from_type_and_name(arg_type, &arg_name, is_last, options)
    }

    fn from_type_and_name(
        mut arg_type: &Type,
        name: &Ident,
        is_last: bool,
        options: OverloadOptions,
    ) -> Result<Self> {
        use KotoContextArgKind::*;
        use KotoValueArgKind::*;

        let mut arg = KotoArg::builder(name.clone());

        if let Type::Reference(TypeReference { elem, .. }) = arg_type {
            arg_type = elem;
            arg = arg.as_ref();
        }

        match arg_type {
            Type::Path(TypePath { path, .. }) => {
                let ident = &path.segments.last().unwrap().ident;
                let ident_string = ident.to_string();

                Ok(match ident_string.as_str() {
                    "bool" => arg.value(Bool),
                    "str" => arg.value(String).call_expr(quote!(#name.as_str())),
                    "String" => arg.value(String).call_expr(quote!(#name.into())),
                    "KString" => arg.value(String),
                    "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "f32" | "f64" => {
                        arg.value(Number).call_expr(quote!(#name.into()))
                    }
                    "KNumber" => arg.value(Number),
                    "KRange" => arg.value(Range),
                    "KList" => arg.value(List),
                    "KTuple" => arg.value(Tuple),
                    "KMap" => arg.value(Map),
                    "KIterator" => arg
                        .value(Iterable)
                        .match_condition(quote!(#name.is_iterable()))
                        .setup_expr(quote! {
                            let #name = #name.clone();
                            let #name = ctx.vm.make_iterator(#name)?;
                        }),
                    "KValue" => arg.value(Any),
                    "KotoVm" => {
                        if options.allow_call_context() {
                            arg.context(KotoVm)
                        } else {
                            return Err(Error::new_spanned(
                                ident,
                                "`KotoVm` is not supported here",
                            ));
                        }
                    }
                    "CallContext" => {
                        if options.allow_call_context() {
                            arg.context(CallContext)
                        } else {
                            return Err(Error::new_spanned(
                                ident,
                                "`CallContext` is not supported here",
                            ));
                        }
                    }
                    "MethodContext" => {
                        if options.allow_method_context() {
                            arg.context(MethodContext)
                        } else {
                            return Err(Error::new_spanned(
                                ident,
                                "`MethodContext` is not supported here",
                            ));
                        }
                    }
                    // Unknown types can be assumed to implement `KotoObject`
                    _ => arg
                        .value(Object(ident_string))
                        .match_condition(quote!(#name.is_a::<#ident>()))
                        .setup_expr(quote!(let #name = #name.cast::<#ident>().unwrap();)),
                }
                .build())
            }
            // Pass remaining args to `&[KValue]` if it's the last arg
            Type::Slice(TypeSlice { elem, .. }) => match elem.as_ref() {
                Type::Path(TypePath { path, .. }) => {
                    let ident_string = path.segments.last().unwrap().ident.to_string();
                    if ident_string == "KValue" {
                        if is_last {
                            Ok(arg.value(Any).variadic().call_expr(quote!(#name)).build())
                        } else {
                            Err(Error::new(
                                arg_type.span(),
                                "Variadic args are only supported as the last argument",
                            ))
                        }
                    } else {
                        unsupported_arg_type(arg_type)
                    }
                }
                _ => unsupported_arg_type(arg_type),
            },
            _ => unsupported_arg_type(arg_type),
        }
    }

    fn builder(name: Ident) -> KotoArgBuilderStage1 {
        KotoArgBuilderStage1 {
            name,
            as_ref: false,
        }
    }

    fn call_expr(&self) -> Option<TokenStream> {
        let Self {
            name,
            call_expr,
            kind,
            ..
        } = self;

        match call_expr {
            Some(expr) => Some(expr.clone()),
            None => match kind {
                KotoArgKind::Value(value) => {
                    if value.as_ref {
                        Some(quote!(&#name))
                    } else {
                        Some(quote!(#name.clone()))
                    }
                }
                KotoArgKind::Context(context) => match context.kind {
                    KotoContextArgKind::KotoVm => Some(quote!(ctx.vm)),
                    KotoContextArgKind::CallContext => Some(quote!(ctx)),
                    KotoContextArgKind::MethodContext => Some(quote! {
                        MethodContext::new(&o, extra_args, ctx.vm)
                    }),
                },
                KotoArgKind::Receiver(receiver) => {
                    if receiver.is_mut {
                        Some(quote!(&mut *instance))
                    } else {
                        Some(quote!(&*instance))
                    }
                }
            },
        }
    }
}

enum KotoArgKind {
    Value(KotoValueArg),
    Context(KotoContextArg),
    Receiver(KotoReceiverArg),
}

struct KotoValueArg {
    kind: KotoValueArgKind,
    is_variadic: bool,
    as_ref: bool,

    // An optional condition to check on the matched value
    match_condition: Option<TokenStream>,
}

impl KotoValueArg {
    /// The type name to show in error messages
    fn display_name(&self) -> String {
        let name = match &self.kind {
            KotoValueArgKind::Bool => "Bool",
            KotoValueArgKind::String => "String",
            KotoValueArgKind::Number => "Number",
            KotoValueArgKind::Range => "Range",
            KotoValueArgKind::List => "List",
            KotoValueArgKind::Tuple => "Tuple",
            KotoValueArgKind::Map => "Map",
            KotoValueArgKind::Iterable => "Iterable",
            KotoValueArgKind::Any => "Any",
            KotoValueArgKind::Object(name) => name.as_str(),
        };

        let dots = if self.is_variadic { "..." } else { "" };

        format!("{name}{dots}")
    }

    /// The KValue variant to match for the arg
    fn match_pats(&self, name: &Ident) -> TokenStream {
        if self.is_variadic {
            quote!(#name @ ..)
        } else {
            match &self.kind {
                KotoValueArgKind::Bool => quote!(KValue::Bool(#name)),
                KotoValueArgKind::String => quote!(KValue::Str(#name)),
                KotoValueArgKind::Number => quote!(KValue::Number(#name)),
                KotoValueArgKind::Range => quote!(KValue::Range(#name)),
                KotoValueArgKind::List => quote!(KValue::List(#name)),
                KotoValueArgKind::Tuple => quote!(KValue::Tuple(#name)),
                KotoValueArgKind::Map => quote!(KValue::Map(#name)),
                KotoValueArgKind::Iterable => quote!(#name),
                KotoValueArgKind::Any => quote!(#name),
                KotoValueArgKind::Object(_) => quote!(KValue::Object(#name)),
            }
        }
    }
}

enum KotoValueArgKind {
    Bool,
    String,
    Number,
    Range,
    List,
    Tuple,
    Map,
    Iterable,
    Any,
    Object(String),
}

struct KotoContextArg {
    kind: KotoContextArgKind,
}

enum KotoContextArgKind {
    MethodContext,
    CallContext,
    KotoVm,
}

struct KotoReceiverArg {
    is_mut: bool,
}

struct KotoArgBuilderStage1 {
    name: Ident,
    as_ref: bool,
}

impl KotoArgBuilderStage1 {
    #[expect(clippy::wrong_self_convention)]
    fn as_ref(mut self) -> Self {
        self.as_ref = true;
        self
    }

    fn value(self, kind: KotoValueArgKind) -> KotoArgBuilderStage2 {
        self.inner(KotoArgBuilderKind::Value(kind))
    }

    fn context(self, kind: KotoContextArgKind) -> KotoArgBuilderStage2 {
        self.inner(KotoArgBuilderKind::Context(kind))
    }

    fn inner(self, kind: KotoArgBuilderKind) -> KotoArgBuilderStage2 {
        let KotoArgBuilderStage1 { name, as_ref } = self;

        KotoArgBuilderStage2 {
            name,
            as_ref,
            kind,
            is_variadic: false,
            match_condition: None,
            setup_expr: None,
            call_expr: None,
        }
    }
}

struct KotoArgBuilderStage2 {
    // Set by the first stage
    name: Ident,
    as_ref: bool,

    // Set when transitioning from first stage
    kind: KotoArgBuilderKind,

    // Does the argument represent multiple values?
    is_variadic: bool,
    // An optional condition to check on the matched value
    match_condition: Option<TokenStream>,
    // Pre-call setup (e.g. calling make_iterator)
    setup_expr: Option<TokenStream>,
    // How the arg should be passed to the user's function
    call_expr: Option<TokenStream>,
}

enum KotoArgBuilderKind {
    Value(KotoValueArgKind),
    Context(KotoContextArgKind),
}

impl KotoArgBuilderStage2 {
    fn variadic(mut self) -> Self {
        self.is_variadic = true;
        self
    }

    fn match_condition(mut self, match_condition: TokenStream) -> Self {
        self.match_condition = Some(match_condition);
        self
    }

    fn setup_expr(mut self, setup_expr: TokenStream) -> Self {
        self.setup_expr = Some(setup_expr);
        self
    }

    fn call_expr(mut self, call_expr: TokenStream) -> Self {
        self.call_expr = Some(call_expr);
        self
    }

    fn build(self) -> KotoArg {
        let Self {
            name,
            kind,
            as_ref,
            match_condition,
            setup_expr,
            is_variadic,
            call_expr,
        } = self;

        KotoArg {
            name,
            kind: match kind {
                KotoArgBuilderKind::Value(kind) => KotoArgKind::Value(KotoValueArg {
                    kind,
                    is_variadic,
                    match_condition,
                    as_ref,
                }),
                KotoArgBuilderKind::Context(kind) => KotoArgKind::Context(KotoContextArg { kind }),
            },
            setup_expr,
            call_expr,
        }
    }
}

fn unsupported_arg_type<T>(arg_type: &Type) -> Result<T> {
    Err(Error::new(arg_type.span(), "Unsupported argument type"))
}

#[derive(Default)]
pub(crate) struct AccessAttributeArgs {
    pub(crate) name: Option<LitStr>,
    pub(crate) aliases: Vec<LitStr>,
}

impl AccessAttributeArgs {
    pub(crate) fn new(attr: &Attribute) -> Result<Self> {
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
    pub(crate) fn names(
        self,
        name_fallback: impl FnOnce() -> Result<LitStr>,
    ) -> Result<Vec<LitStr>> {
        let name = match self.name {
            Some(name) => name,
            None => name_fallback()?,
        };

        let mut names = vec![name];
        names.extend(self.aliases);
        Ok(names)
    }
}
