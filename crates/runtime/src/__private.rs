use crate::{KNativeFunction, KValue, Result};

/// Used by the `#[koto_impl]` macro.
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

#[diagnostic::on_unimplemented(
    message = "a `#[koto_method]` method must return `()`, `KValue`, or `koto_runtime::Result<KValue>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoMethodReturn {
    fn into_result(self) -> Result<KValue>;
}

impl KotoMethodReturn for Result<KValue> {
    fn into_result(self) -> Result<KValue> {
        self
    }
}

impl KotoMethodReturn for KValue {
    fn into_result(self) -> Result<KValue> {
        Ok(self)
    }
}

impl KotoMethodReturn for () {
    fn into_result(self) -> Result<KValue> {
        Ok(KValue::Null)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_get]` method must return `KValue` or `koto_runtime::Result<KValue>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoGetReturn {
    fn into_result(self) -> Result<KValue>;
}

impl KotoGetReturn for Result<KValue> {
    fn into_result(self) -> Result<KValue> {
        self
    }
}

impl KotoGetReturn for KValue {
    fn into_result(self) -> Result<KValue> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_set]` method must return `()` or `koto_runtime::Result<()>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoSetReturn {
    fn into_result(self) -> Result<()>;
}

impl KotoSetReturn for Result<()> {
    fn into_result(self) -> Result<()> {
        self
    }
}

impl KotoSetReturn for () {
    fn into_result(self) -> Result<()> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_get_fallback]` method must return `Option<KValue>` or `koto_runtime::Result<Option<KValue>>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoGetFallbackReturn {
    fn into_result(self) -> Result<Option<KValue>>;
}

impl KotoGetFallbackReturn for Result<Option<KValue>> {
    fn into_result(self) -> Result<Option<KValue>> {
        self
    }
}

impl KotoGetFallbackReturn for Option<KValue> {
    fn into_result(self) -> Result<Option<KValue>> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_set_fallback]` method must return `()` or `koto_runtime::Result<()>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoSetFallbackReturn {
    fn into_result(self) -> Result<()>;
}

impl KotoSetFallbackReturn for Result<()> {
    fn into_result(self) -> Result<()> {
        self
    }
}

impl KotoSetFallbackReturn for () {
    fn into_result(self) -> Result<()> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_get_override]` method must return `Option<KValue>` or `koto_runtime::Result<Option<KValue>>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoGetOverrideReturn {
    fn into_result(self) -> Result<Option<KValue>>;
}

impl KotoGetOverrideReturn for Result<Option<KValue>> {
    fn into_result(self) -> Result<Option<KValue>> {
        self
    }
}

impl KotoGetOverrideReturn for Option<KValue> {
    fn into_result(self) -> Result<Option<KValue>> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_set_override]` method must return `bool` or `koto_runtime::Result<bool>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoSetOverrideReturn {
    fn into_result(self) -> Result<bool>;
}

impl KotoSetOverrideReturn for Result<bool> {
    fn into_result(self) -> Result<bool> {
        self
    }
}

impl KotoSetOverrideReturn for bool {
    fn into_result(self) -> Result<bool> {
        Ok(self)
    }
}
