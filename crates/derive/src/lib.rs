//! Contains convenience macros for declaring types for the Koto runtime

#![warn(missing_docs)]

#[cfg(all(feature = "arc", feature = "rc"))]
compile_error!("A single memory management feature can be enabled at a time");

mod attributes;
mod function;
mod koto_copy;
mod koto_impl;
mod koto_type;
mod overloading;

use proc_macro::TokenStream;

/// A helper macro for declaring Rust functions for the Koto runtime
///
/// The macro generates a `KotoFunction`-compatible wrapper function with the specified name.
///
/// Multiple functions can be declared in a single macro invocation, with a separate wrapper
/// generated for each function.
///
/// Functions can be overloaded, with the macro logic generating appropriate `match` arms for each
/// overload.
///
/// ## Argument Type Conversions
///
/// The macro will attempt to convert Koto arguments into the expected argument types.
///
/// E.g.: For a function signature like `fn(x: i64, s: &str)`, the macro will expect a
/// `KValue::Number`, followed by a `KValue::String`.
///
/// Inner variant types for `KValue` (like `KMap`, `KString`, etc.) will be unpacked and forwarded.
///
/// The macro will call `.clone()` for arguments that take their inputs by value.
///
/// An argument that takes `&KValue` or `KValue` will match against any value type.
///
/// Variadic functions should take `&[KValue]` as the last argument, and then the wrapper will
/// forward any remaining arguments.
///
/// Any unknown value type is assumed by the macro to implement `KotoObject`. The wrapper will match
/// against `KValue::Object` and then attempt to cast the object to the expected type.
///
/// ## Return Type
///
/// If no return type is specified then the generated wrapper will return `Ok(KValue::Null)`.
///
/// If `koto::runtime::Result<T>` is returned, then `T` is assumed to implement `Into<KValue>`.
///
/// Similarly, if a non-`Result` value is returned, then the generated wrapper will return
/// `Ok(KValue::from(value))`.
///  
/// ## Non-Koto Arguments
///
/// - If an argument takes `&mut CallContext` then it will receive the `CallContext` with which the
///   generated wrapper function was called.
/// - If an argument takes `&mut KotoVm` then it will receive the `CallContext`'s `KotoVm`.
///
/// In both cases, any other arguments will need to be taken by value rather than reference to avoid
/// lifetime errors.
///
/// ## Overriding the Koto Runtime Crate
///
/// The macro generates code assuming that the top-level `koto` crate is being used,
/// with the `koto_runtime` crate re-exported at `::koto::runtime`.
///
/// If the runtime crate is located at a different path (e.g., if your crate depends on
/// `koto_runtime` directly), then specify the runtime path before declaring any functions.
///
/// E.g.:
/// ```ignore
/// koto_fn! {
///     runtime = koto_runtime;   
///
///     fn foo()...
/// }
/// ```
///
/// ## Examples
///
/// ### Getting Started
///
/// ```ignore
/// koto_fn! {
///     fn foo() -> bool {
///         true
///     }
///
///     fn say_hello(name: &str) -> String {
///         format!("Hello, {name}!")
///     }
/// }
/// ```
///
/// ### Returning an Error
///
/// ```ignore
/// koto_fn! {
///     fn first_in_list(list: &KList) -> Result<KValue> {
///         match list.data().first() {
///             Some(result) => Ok(result.clone()),
///             None => runtime_error!("Empty list"),
///         }
///     }
/// }
/// ```
///
/// ### Overloading Functions
///
/// ```ignore
/// koto_fn! {
///     fn rect() -> Rect {
///         (0.0, 0.0, 0.0, 0.0).into()
///     }
///
///     fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
///         (x, y, w, h).into()
///     }
///
///     fn rect(xy: &Vec2, size: &Vec2) -> Rect {
///         (xy.inner().x, xy.inner().y, size.inner().x, size.inner().y).into()
///     }
/// }
/// ```
///
/// ### VM Argument
///
/// ```ignore
/// koto_fn! {
///     fn to_string(arg: KValue, vm: &mut KotoVm) -> Result<KValue> {
///         vm.value_to_string(&value)
///     }
/// }
/// ```
///
#[proc_macro]
pub fn koto_fn(input: TokenStream) -> TokenStream {
    function::koto_fn(input)
}

/// `#[derive(KotoType)]`
///
/// The `KotoType` trait will be implemented using the name of the struct.
/// If another name should be displayed in the Koto runtime then use
/// `#[koto(type_name = "other_name)]`.
///
/// ## `runtime` attribute
///
/// The macro generates code assuming that the top-level `koto` crate is being used,
/// with the `koto_runtime` crate re-exported at `::koto::runtime`.
///
/// If the runtime crate is located at a different path (e.g., if your crate depends on
/// `koto_runtime` directly), then use the `runtime` attribute to define the alternative path,
/// e.g. `#[koto(runtime = koto_runtime)]`.
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

// NOTE: The documentation examples are tested in `crates/koto/tests/derive_koto_impl_doc.rs`
/// A helper for deriving `KotoAccess`
///
/// This macro recognizes functions tagged with the following attributes:
/// - [**`#[koto_method]`**](#koto_method)
/// - [**`#[koto_get]`**](#koto_get)
/// - [**`#[koto_set]`**](#koto_set)
/// - [**`#[koto_get_fallback]`**](#koto_get_fallback)
/// - [**`#[koto_set_fallback]`**](#koto_set_fallback)
/// - [**`#[koto_get_override]`**](#koto_get_override)
/// - [**`#[koto_set_override]`**](#koto_set_override)
///
/// The attributes `#[koto_method]`, `#[koto_get]` and `#[koto_set]` can take optional arguments:
/// - **`name`** — sets the access key, if not set it will be inferred by the function name
/// - **`alias`** *(multiple allowed)* — adds additional keys to access with
///
/// ## `#[koto_method]`
///
/// Any function tagged with `#[koto_method]` will be made available via '.' access.
///
/// Wrapper functions are generated that take care of checking that the function has been called
/// with an instance of the correct object type.
///
/// The function can take `&self` or `&mut self` along with an optional `&[KValue]` slice of
/// additional arguments, or for more advanced functions a `MethodContext<Self>` can be provided.
///
/// The return type can be omitted or be any `T: Into<KValue>`, optionally wrapped in a
/// `koto_runtime::Result`.
///
/// For cases where it would be preferable to return a clone of the object instance
/// (e.g. if you want to implement chainable setters), then you can accept a `MethodContext<Self>`
/// as the function argument and then return `MethodContext::instance_result()`.
///
/// ## `#[koto_get]`
///
/// This function is called when accessing a field via `.` access.
///
/// The field's name is derived from the function name, or from a name given explicitly,
/// e.g. `#[koto_get(name = "my_field_name")]`.
///
/// Aliases for the field name can also be given,
/// e.g. `#[koto_get(name = "my_field_name", alias = "my_alias", alias = "my_other_alias")]`.
///
/// The function must have a signature like either:
/// ```ignore
/// fn foo(&self) -> T { ... }
/// fn foo(&self) -> Result<T> { ... }
/// ```
/// where `T: Into<KValue>`.
///
/// ## `#[koto_set]`
///
/// This function is called when assigning a value to a field via `.` access.
///
/// The field name's is derived from the function name without the `set_` prefix,
/// or from a name given explicitly, e.g. `#[koto_set(name = "my_field_name")]`.
///
/// Aliases for the field name can also be given,
/// e.g. `#[koto_set(name = "my_field_name", alias = "my_alias", alias = "my_other_alias")]`.
///
///
/// The function must have a signature like either:
/// ```ignore
/// fn set_foo(&mut self, value: &KValue) { ... }
/// fn set_foo(&mut self, value: &KValue) -> Result<()> { ... }
/// ```
///
/// ## `#[koto_get_fallback]`
///
/// This function is called when neither `#[koto_get]`s nor `#[koto_method]`s
/// with the requested name were found.
///
/// The function must have a signature like either:
/// ```ignore
/// fn f(&self, key: &KString) -> Option<T> { ... }
/// fn f(&self, key: &KString) -> Result<Option<T>> { ... }
/// ```
/// where `T: Into<KValue>`.
///
/// ## `#[koto_set_fallback]`
///
/// This function is called when no `#[koto_set]`s
/// with the requested name were found.
///
/// The function must have a signature like either:
/// ```ignore
/// fn f(&mut self, key: &KString, value: &KValue) { ... }
/// fn f(&mut self, key: &KString, value: &KValue) -> Result<()> { ... }
/// ```
///
/// ## `#[koto_get_override]`
///
/// This function is called **before** looking for any `#[koto_get]`es or `#[koto_method]`s.
/// If this method returns `Some`, then that value will be returned to koto.
/// If it returns `None` instead, then `#[koto_get]`s and `#[koto_method]`s with the given key
/// will be looked for before finally falling back to the `#[koto_get_fallback]` function.
///
/// The function must have a signature like either:
/// ```ignore
/// fn f(&self, key: &KString) -> Option<T> { ... }
/// fn f(&self, key: &KString) -> Result<Option<T>> { ... }
/// ```
/// where `T: Into<KValue>`.
///
/// ## `#[koto_set_override]`
///
/// This function is called **before** any `#[koto_set]` is looked for.
/// If this method returns `true`, then the assignment operation is done.
/// If it returns `false` instead, then `#[koto_set]`s
/// will be looked for before finally falling back to the `#[koto_set_fallback]` function.
///
/// The function must have a signature like either:
/// ```ignore
/// fn f(&mut self, key: &KString, value: &KValue) -> bool { ... }
/// fn f(&mut self, key: &KString, value: &KValue) -> Result<bool> { ... }
/// ```
///
/// ## `runtime` attribute
///
/// The macro generates code assuming that the top-level `koto` crate is being used,
/// with the `koto_runtime` crate re-exported at `::koto::runtime`.
///
/// If the runtime crate is located at a different path (e.g., if your crate depends on
/// `koto_runtime` directly), then use the `runtime` attribute to define the alternative path,
/// e.g. `#[koto_impl(runtime = koto_runtime)]`.
///
/// ## Example
///
/// ```ignore
/// use koto::{derive::*, prelude::*, runtime::Result};
///
/// #[derive(Clone, KotoType, KotoCopy)]
/// struct Foo {
///     x: f64,
/// }
///
/// impl KotoObject for Foo {}
///
/// #[koto_impl]
/// impl Foo {
///     fn new(x: f64) -> Self {
///         Self { x }
///     }
///
///     #[koto_get]
///     fn x(&self) -> KValue {
///         self.x.into()
///     }
///
///     #[koto_set]
///     fn set_x(&mut self, value: &KValue) -> Result<()> {
///         match value {
///             KValue::Number(value) => {
///                 self.x = value.into();
///                 Ok(())
///             }
///             unexpected => unexpected_type("Number", unexpected),
///         }
///     }
///
///     #[koto_method(alias = "set")]
///     fn reset(&mut self, args: &[KValue]) -> Result<KValue> {
///         let reset_value = match args {
///             [] => 0.0,
///             [KValue::Number(reset_value)] => reset_value.into(),
///             unexpected => return unexpected_args("||, or |Number|", unexpected),
///         };
///         self.x = reset_value;
///         Ok(KValue::Null)
///     }
///
///     #[koto_method]
///     fn add(&mut self, addend: f64) -> &mut Self {
///         self.x += addend;
///         self
///     }
/// }
///
/// #[test]
/// fn test() {
///     let script = r#"
/// v = make_foo()
/// assert_eq v.x, 0
///
/// v.x = 1
/// assert_eq v.x, 1
///
/// v.reset()
/// assert_eq v.x, 0
///
/// v.reset(2)
/// assert_eq v.x, 2
///
/// v.set()
/// assert_eq v.x, 0
///
/// v.set(2)
/// assert_eq v.x, 2
///
/// v.add(1).add(3)
/// assert_eq v.x, 6
/// "#;
///
///     let mut koto = Koto::default();
///
///     koto.prelude()
///         .add_fn("make_foo", |_| Ok(KObject::from(Foo::new(0.0)).into()));
///
///     koto.compile_and_run(script).unwrap();
/// }
/// ```
#[proc_macro_attribute]
pub fn koto_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    koto_impl::koto_impl(attr, item)
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_get(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_get_override(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_get_fallback(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_set(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_set_override(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_set_fallback(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// See [`koto_impl`](macro@koto_impl)
#[proc_macro_attribute]
pub fn koto_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

const PREFIX_FUNCTION: &str = "__koto_";
