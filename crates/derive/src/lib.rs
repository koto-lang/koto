//! Contains convenience macros for declaring types for the Koto runtime

#![warn(missing_docs)]

mod attributes;
mod koto_copy;
mod koto_impl;
mod koto_type;

use proc_macro::TokenStream;

/// `#[derive(KotoType)]`
///
/// The `KotoType` trait will be implemented using the name of the struct.
/// If another name should be displayed in the Koto runtime then use
/// `#[koto(type_name = "other_name)]`.
///
/// ## Example
///
/// ```ignore
/// // Derive a KotoType implementation using 'KotoFoo' as the type name.
/// #[derive(KotoType)]
/// struct KotoFoo {}
///
/// // Derive a KotoType implementation using 'Bar' as the type name.
/// #[derive(KotoType)]
/// #[koto(type_name = "Bar")]
/// struct KotoBar {}
/// ```
#[proc_macro_derive(KotoType, attributes(koto))]
pub fn derive_koto_type(input: TokenStream) -> TokenStream {
    koto_type::derive_koto_type(input)
}

/// `#[derive(KotoCopy)]`
///
/// The `KotoCopy` trait will be implemented using the struct's `Clone` implementation.
///
/// If the struct implements `Copy` then that should most likely be used instead.
/// I haven't found an automatic way to detect that the struct implements `Copy`,
/// so use the `#[koto(use_copy)]` attribute to tell the macro that `Copy` is available.
///
/// ## Example
///
/// ```ignore
/// // Derive a KotoCopy implementation using KotoFoo's Clone implementation
/// #[derive(Clone, KotoCopy)]
/// struct KotoFoo {}
///
/// // Derive a KotoCopy implementation using KotoBar's Copy implementation
/// #[derive(Copy, Clone, KotoCopy)]
/// #[koto(use_copy)]
/// struct KotoBar {}
/// ```
#[proc_macro_derive(KotoCopy, attributes(koto))]
pub fn derive_koto_copy(input: TokenStream) -> TokenStream {
    koto_copy::derive_koto_copy(input)
}

/// A helper for deriving `KotoEntries` with functions tagged with `#[koto_method]`
///
/// Any function tagged with `#[koto_method]` will be made available via '.' lookup.
///
/// Wrapper functions are generated that take care of checking that the function has been called
/// with an instance of the correct object type.
///
/// The function can take `&self` or `&mut self` along with an optional `&[KValue]` slice of
/// additional arguments, or for more advanced functions a `MethodContext<Self>` can be provided.
///
/// The return type can be ommitted (in which case the result will be `KValue::Null`),
/// or a `KValue`, or a `Result<KValue>`.
///
/// For cases where it would be preferable to return a clone of the object instance
/// (e.g. if you want to implement chainable setters), then you can accept a `MethodContext<Self`>
/// as the function argument and then return `MethodContext::instance_result()`.
///
/// ## `runtime` attribute
///
/// The macro generates code assuming that the top-level `koto` crate is being used,
/// with the koto_runtime crate re-exported at `::koto::runtime`.
/// If the runtime crate is located at a different path (e.g., if your crate depends on
/// `koto_runtime` directly), then use the `runtime` attribute to define the alternative path,
/// e.g. `#[koto_impl(runtime = koto_runtime)]`.
///
/// ## Example
///
/// ```ignore
/// #[derive(Clone, KotoType, KotoCopy)]
/// struct Foo {
///   x: f64
/// }
///
/// #[koto_impl]
/// impl Foo {
///     fn new(x: f64) -> Self {
///         Self { x }
///     }
///
///     // Add an `x()` method to the Foo object, and also make it available via `get_x()`
///     #[koto_method(alias = "get_x")]
///     fn x(&self) -> KValue {
///         self.x.into()
///     }
///
///     // A wrapper function
///     #[koto_method]
///     fn reset(&mut self, args: &[KValue]) -> Result<KValue> {
///         let reset_value = match args {
///             [] => 0.0,
///             [KValue::Number(reset_value)] => reset_value.into(),
///             unexpected => return unexpected_args("||, or |Number|", unexpected),
///         };
///         self.x = reset_value;
///         Ok(())
///     }
///
///     #[koto_method]
///     fn set_x(ctx: MethodContext) -> Result<KValue> {
///         match args {
///             [KValue::Number(new_x)] => {
///                 ctx.instance_mut()?.x = new_x.into();
///                 // Return a clone of the instance that's being modified
///                 ctx.instance_result()
///             }
///             unexpected => unexpected_args("|Number|", unexpected),
///         }
///     }
/// }
///
///
/// ```
#[proc_macro_attribute]
pub fn koto_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    koto_impl::generate_koto_access_entries(attr, item)
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

const PREFIX_STATIC: &str = "__KOTO_";
const PREFIX_FUNCTION: &str = "__koto_";
