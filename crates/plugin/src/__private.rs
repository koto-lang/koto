pub use koto_ffi;

use crate::{KNativeFunction, KValue, Result};

/// Used by the `#[koto_impl]` macro.
#[doc(hidden)]
pub enum MethodOrField<T: ?Sized> {
    Method(KNativeFunction),
    Field(fn(&T) -> Result<KValue>),
}

impl<T: ?Sized> Clone for MethodOrField<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Method(x) => Self::Method(x.clone()),
            Self::Field(x) => Self::Field(*x),
        }
    }
}

pub fn make_method_value(
    method: fn(&mut crate::CallContext) -> crate::Result<crate::KValue>,
) -> crate::KValue {
    crate::types::make_method_value(method)
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_fn]` must return a value that implements `Into<KValue>`, optionally wrapped in `koto_plugin::Result`",
    label = "wrong return type",
    note = "for more info see the `#[koto_fn]` documentation"
)]
pub trait KotoFunctionReturn {
    fn into_result(self) -> crate::Result<crate::KValue>;
}

impl<T: Into<crate::KValue>> KotoFunctionReturn for crate::Result<T> {
    fn into_result(self) -> crate::Result<crate::KValue> {
        self.map(Into::into)
    }
}

impl<T: Into<crate::KValue>> KotoFunctionReturn for T {
    fn into_result(self) -> crate::Result<crate::KValue> {
        Ok(self.into())
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_get]` method must return a value that implements `Into<KValue>`, optionally wrapped in `koto_plugin::Result`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoGetReturn {
    fn into_result(self) -> crate::Result<crate::KValue>;
}

impl<T: Into<crate::KValue>> KotoGetReturn for crate::Result<T> {
    fn into_result(self) -> crate::Result<crate::KValue> {
        self.map(Into::into)
    }
}

impl<T: Into<crate::KValue>> KotoGetReturn for T {
    fn into_result(self) -> crate::Result<crate::KValue> {
        Ok(self.into())
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_set]` method must return `()` or `koto_plugin::Result<()>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoSetReturn {
    fn into_result(self) -> crate::Result<()>;
}

impl KotoSetReturn for crate::Result<()> {
    fn into_result(self) -> crate::Result<()> {
        self
    }
}

impl KotoSetReturn for () {
    fn into_result(self) -> crate::Result<()> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_method]` method must return a value that implements `Into<KValue>`, optionally wrapped in `koto_plugin::Result`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoMethodReturn {
    fn into_result(self) -> crate::Result<crate::KValue>;
}

impl<T: Into<crate::KValue>> KotoMethodReturn for crate::Result<T> {
    fn into_result(self) -> crate::Result<crate::KValue> {
        self.map(Into::into)
    }
}

impl<T: Into<crate::KValue>> KotoMethodReturn for T {
    fn into_result(self) -> crate::Result<crate::KValue> {
        Ok(self.into())
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_get_fallback]` method must return a value that implements `Into<KValue>`, wrapped in an option, optionally wrapped in `koto_plugin::Result`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoGetFallbackReturn {
    fn into_result(self) -> crate::Result<Option<crate::KValue>>;
}

impl<T: Into<crate::KValue>> KotoGetFallbackReturn for crate::Result<Option<T>> {
    fn into_result(self) -> crate::Result<Option<crate::KValue>> {
        self.map(|o| o.map(Into::into))
    }
}

impl<T: Into<crate::KValue>> KotoGetFallbackReturn for Option<T> {
    fn into_result(self) -> crate::Result<Option<crate::KValue>> {
        Ok(self.map(Into::into))
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_set_fallback]` method must return `()` or `koto_plugin::Result<()>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoSetFallbackReturn {
    fn into_result(self) -> crate::Result<()>;
}

impl KotoSetFallbackReturn for crate::Result<()> {
    fn into_result(self) -> crate::Result<()> {
        self
    }
}

impl KotoSetFallbackReturn for () {
    fn into_result(self) -> crate::Result<()> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_get_override]` method must return a value that implements `Into<KValue>`, wrapped in an option, optionally wrapped in `koto_plugin::Result`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoGetOverrideReturn {
    fn into_result(self) -> crate::Result<Option<crate::KValue>>;
}

impl<T: Into<crate::KValue>> KotoGetOverrideReturn for crate::Result<Option<T>> {
    fn into_result(self) -> crate::Result<Option<crate::KValue>> {
        self.map(|o| o.map(Into::into))
    }
}

impl<T: Into<crate::KValue>> KotoGetOverrideReturn for Option<T> {
    fn into_result(self) -> crate::Result<Option<crate::KValue>> {
        Ok(self.map(Into::into))
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_set_override]` method must return `bool` or `koto_plugin::Result<bool>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoSetOverrideReturn {
    fn into_result(self) -> crate::Result<bool>;
}

impl KotoSetOverrideReturn for crate::Result<bool> {
    fn into_result(self) -> crate::Result<bool> {
        self
    }
}

impl KotoSetOverrideReturn for bool {
    fn into_result(self) -> crate::Result<bool> {
        Ok(self)
    }
}
