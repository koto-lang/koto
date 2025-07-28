use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, Ident, ItemFn, PatType, Path, Result, ReturnType, Token, Type, TypePath,
    TypeReference, TypeSlice,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
};

pub fn koto_fn(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let FunctionDefinitions { runtime, functions } =
        parse_macro_input!(input as FunctionDefinitions);

    let fn_wrappers: Vec<_> = functions
        .iter()
        .map(|(name, function_impls)| {
            let mut match_arms = Vec::with_capacity(function_impls.len());
            let mut unexpected_args_error = String::new();

            for (i, fn_impl) in function_impls.iter().enumerate() {
                match_arms.push(fn_impl.match_arm());

                if i > 0 {
                    unexpected_args_error.push_str(", ");
                }
                if function_impls.len() > 1 && i == function_impls.len() - 1 {
                    unexpected_args_error.push_str("or ");
                }
                unexpected_args_error.push_str(&fn_impl.signature());
            }

            quote! {
                fn #name(ctx: &mut #runtime::CallContext) -> #runtime::Result<#runtime::KValue> {
                    use #runtime::KValue;

                    match ctx.args() {
                        #(#match_arms)*
                        unexpected => unexpected_args(#unexpected_args_error, unexpected),
                    }
                }
            }
        })
        .collect();

    let output = quote! {
        #(#fn_wrappers)*
    };

    output.into()
}

struct FunctionDefinitions {
    runtime: Path,
    functions: IndexMap<Ident, Vec<FunctionDefinition>>,
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

        let mut functions = IndexMap::<Ident, Vec<FunctionDefinition>>::new();

        while !input.is_empty() {
            let f: FunctionDefinition = input.parse()?;
            functions.entry(f.name.clone()).or_default().push(f);
        }

        Ok(Self {
            runtime: runtime.unwrap_or_else(|| parse_quote! {::koto::runtime }),
            functions,
        })
    }
}

struct FunctionDefinition {
    name: Ident,
    args: Vec<FunctionArg>,
    output: Option<FunctionOutputInfo>,
    item: ItemFn,
}

impl Parse for FunctionDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let item: ItemFn = input.parse()?;

        let args = item
            .sig
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| match input {
                FnArg::Receiver(_) => {
                    Err(Error::new(input.span(), "self arguments are unsupported"))
                }
                FnArg::Typed(PatType { ty: arg_type, .. }) => {
                    let is_last_arg = i == item.sig.inputs.len() - 1;
                    FunctionArg::new(arg_type, i, is_last_arg)
                }
            })
            .collect::<Result<Vec<_>>>()?;

        let output = match &item.sig.output {
            ReturnType::Default => None,
            ReturnType::Type(_, return_type) => {
                let is_result = match return_type.as_ref() {
                    Type::Path(type_path) => type_path
                        .path
                        .segments
                        .last()
                        .unwrap()
                        .ident
                        .to_string()
                        .starts_with("Result"),
                    _ => {
                        return Err(Error::new(return_type.span(), "Unsupported return type"));
                    }
                };
                Some(FunctionOutputInfo { is_result })
            }
        };

        Ok(FunctionDefinition {
            name: item.sig.ident.clone(),
            args,
            output,
            item,
        })
    }
}

impl FunctionDefinition {
    fn signature(&self) -> String {
        let mut result = "|".to_string();

        for (i, arg) in self
            .args
            .iter()
            .filter_map(|arg| match arg {
                FunctionArg::Koto(koto_arg) => Some(koto_arg),
                _ => None,
            })
            .enumerate()
        {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&arg.display_name);
        }

        result.push('|');
        result
    }

    fn match_arm(&self) -> TokenStream {
        let call_exprs = self
            .args
            .iter()
            .map(|arg| match arg {
                FunctionArg::CallContext { call_expr } => call_expr,
                FunctionArg::Koto(KotoArg { call_expr, .. }) => call_expr,
            })
            .collect::<Vec<_>>();

        let fn_name = &self.name;
        let call = quote! {
            #fn_name(#(#call_exprs, )*)
        };

        let (match_exprs, setup_exprs) = self
            .args
            .iter()
            .filter_map(|arg| match arg {
                FunctionArg::Koto(KotoArg {
                    match_expr,
                    setup_expr,
                    ..
                }) => Some((match_expr, setup_expr)),
                _ => None,
            })
            .collect::<(Vec<_>, Vec<_>)>();

        let match_conditions = self
            .args
            .iter()
            .filter_map(|arg| match arg {
                FunctionArg::Koto(KotoArg {
                    match_condition, ..
                }) => match_condition.as_ref(),
                _ => None,
            })
            .collect::<Vec<_>>();

        let condition = match match_conditions.as_slice() {
            [] => quote! {},
            [first, rest @ ..] => quote! { if #first #(&& #rest)*},
        };

        let wrapped_call = match &self.output {
            Some(output_info) => {
                if output_info.is_result {
                    quote! { #call.map(KValue::from) }
                } else {
                    quote! { Ok(#call.into()) }
                }
            }
            None => quote! {{
                #call;
                Ok(KValue::Null)
            }},
        };

        let fn_impl = &self.item;
        quote! {
            [#(#match_exprs, )*] #condition => {
                #fn_impl
                #(#setup_exprs)*
                return #wrapped_call;
            }
        }
    }
}

#[derive(Default)]
struct KotoArg {
    // The type name to show in error messages
    display_name: String,
    // The KValue variant to match for the arg
    match_expr: TokenStream,
    // An optional condition to check on the matched value
    match_condition: Option<TokenStream>,
    // Pre-call setup (e.g. calling make_iterator)
    setup_expr: Option<TokenStream>,
    // How the arg should be passed to the user's function
    call_expr: TokenStream,
}

enum FunctionArg {
    CallContext { call_expr: TokenStream },
    Koto(KotoArg),
}

impl FunctionArg {
    fn new(arg_type: &Type, id: usize, is_last: bool) -> Result<Self> {
        let arg_name = format_ident!("arg_{}", id);
        Self::from_type_and_name(arg_type, arg_name, false, is_last)
    }

    fn from_type_and_name(
        arg_type: &Type,
        name: Ident,
        as_ref: bool,
        is_last: bool,
    ) -> Result<Self> {
        match arg_type {
            Type::Path(TypePath { path, .. }) => {
                let ident = &path.segments.last().unwrap().ident;
                let ident_string = ident.to_string();

                let mut result = match ident_string.as_str() {
                    "bool" => Self::Koto(KotoArg {
                        display_name: "Bool".into(),
                        match_expr: quote! { KValue::Bool(#name) },
                        ..Default::default()
                    }),
                    "str" => Self::Koto(KotoArg {
                        display_name: "String".into(),
                        match_expr: quote! { KValue::Str(#name) },
                        call_expr: quote! { #name.as_str() },
                        ..Default::default()
                    }),
                    "String" => Self::Koto(KotoArg {
                        display_name: "String".into(),
                        match_expr: quote! { KValue::Str(#name) },
                        call_expr: quote! { #name.into() },
                        ..Default::default()
                    }),
                    "KString" => Self::Koto(KotoArg {
                        display_name: "String".into(),
                        match_expr: quote! { KValue::Str(#name) },
                        ..Default::default()
                    }),
                    "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "f32" | "f64" => {
                        Self::Koto(KotoArg {
                            display_name: "Number".into(),
                            match_expr: quote! { KValue::Number(#name) },
                            call_expr: quote! { #name.into() },
                            ..Default::default()
                        })
                    }
                    "KNumber" => Self::Koto(KotoArg {
                        display_name: "Number".into(),
                        match_expr: quote! { KValue::Number(#name) },
                        ..Default::default()
                    }),
                    "KRange" => Self::Koto(KotoArg {
                        display_name: "Range".into(),
                        match_expr: quote! { KValue::Range(#name) },
                        ..Default::default()
                    }),
                    "KList" => Self::Koto(KotoArg {
                        display_name: "List".into(),
                        match_expr: quote! { KValue::List(#name) },
                        ..Default::default()
                    }),
                    "KTuple" => Self::Koto(KotoArg {
                        display_name: "Tuple".into(),
                        match_expr: quote! { KValue::Tuple(#name) },
                        ..Default::default()
                    }),
                    "KMap" => Self::Koto(KotoArg {
                        display_name: "Map".into(),
                        match_expr: quote! { KValue::Map(#name) },
                        ..Default::default()
                    }),
                    "KIterator" => Self::Koto(KotoArg {
                        display_name: "Iterable".into(),
                        match_expr: quote! { #name },
                        match_condition: Some(quote! { #name.is_iterable() }),
                        setup_expr: Some(quote! {
                            let #name = #name.clone();
                            let #name = ctx.vm.make_iterator(#name)?;
                        }),
                        ..Default::default()
                    }),
                    "KValue" => Self::Koto(KotoArg {
                        display_name: "Any".into(),
                        match_expr: quote! { #name },
                        ..Default::default()
                    }),
                    "CallContext" => Self::CallContext {
                        call_expr: quote! { ctx },
                    },
                    "KotoVm" => Self::CallContext {
                        call_expr: quote! { ctx.vm },
                    },
                    // Unknown types can be assumed to implement `KotoObject`
                    _ => Self::Koto(KotoArg {
                        display_name: ident_string,
                        match_expr: quote! { KValue::Object(#name) },
                        match_condition: Some(quote! { #name.is_a::<#ident>() }),
                        setup_expr: Some(quote! { let #name = #name.cast::<#ident>().unwrap(); }),
                        ..Default::default()
                    }),
                };

                match &mut result {
                    Self::Koto(KotoArg { call_expr, .. }) if call_expr.is_empty() => {
                        *call_expr = if as_ref {
                            quote! { &#name }
                        } else {
                            quote! { #name.clone() }
                        }
                    }
                    _ => {}
                }

                Ok(result)
            }
            // Support args that take a reference
            Type::Reference(TypeReference { elem, .. }) => {
                Self::from_type_and_name(elem, name, true, is_last)
            }
            // Pass remaining args to `&[KValue]` if it's the last arg
            Type::Slice(TypeSlice { elem, .. }) => match elem.as_ref() {
                Type::Path(TypePath { path, .. }) => {
                    let ident_string = path.segments.last().unwrap().ident.to_string();
                    if ident_string == "KValue" {
                        if is_last {
                            Ok(Self::Koto(KotoArg {
                                display_name: "Any...".into(),
                                match_expr: quote! { #name @ .. },
                                call_expr: quote! { #name },
                                ..Default::default()
                            }))
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
}

fn unsupported_arg_type<T>(arg_type: &Type) -> Result<T> {
    Err(Error::new(arg_type.span(), "Unsupported argument type"))
}

struct FunctionOutputInfo {
    is_result: bool,
}
