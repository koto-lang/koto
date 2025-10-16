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
    message = "a `#[koto_access]` method must return `KValue` or `koto_runtime::Result<KValue>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoAccessReturn {
    fn into_result(self) -> Result<KValue>;
}

impl KotoAccessReturn for Result<KValue> {
    fn into_result(self) -> Result<KValue> {
        self
    }
}

impl KotoAccessReturn for KValue {
    fn into_result(self) -> Result<KValue> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_access_assign]` method must return `()` or `koto_runtime::Result<()>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoAccessAssignReturn {
    fn into_result(self) -> Result<()>;
}

impl KotoAccessAssignReturn for Result<()> {
    fn into_result(self) -> Result<()> {
        self
    }
}

impl KotoAccessAssignReturn for () {
    fn into_result(self) -> Result<()> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_access_fallback]` method must return `Option<KValue>` or `koto_runtime::Result<Option<KValue>>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoAccessFallbackReturn {
    fn into_result(self) -> Result<Option<KValue>>;
}

impl KotoAccessFallbackReturn for Result<Option<KValue>> {
    fn into_result(self) -> Result<Option<KValue>> {
        self
    }
}

impl KotoAccessFallbackReturn for Option<KValue> {
    fn into_result(self) -> Result<Option<KValue>> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_access_assign_fallback]` method must return `()` or `koto_runtime::Result<()>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoAccessAssignFallbackReturn {
    fn into_result(self) -> Result<()>;
}

impl KotoAccessAssignFallbackReturn for Result<()> {
    fn into_result(self) -> Result<()> {
        self
    }
}

impl KotoAccessAssignFallbackReturn for () {
    fn into_result(self) -> Result<()> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_access_override]` method must return `Option<KValue>` or `koto_runtime::Result<Option<KValue>>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoAccessOverrideReturn {
    fn into_result(self) -> Result<Option<KValue>>;
}

impl KotoAccessOverrideReturn for Result<Option<KValue>> {
    fn into_result(self) -> Result<Option<KValue>> {
        self
    }
}

impl KotoAccessOverrideReturn for Option<KValue> {
    fn into_result(self) -> Result<Option<KValue>> {
        Ok(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `#[koto_access_assign_override]` method must return `bool` or `koto_runtime::Result<bool>`",
    label = "wrong return type",
    note = "for more info see the `#[koto_impl]` documentation"
)]
pub trait KotoAccessAssignOverrideReturn {
    fn into_result(self) -> Result<bool>;
}

impl KotoAccessAssignOverrideReturn for Result<bool> {
    fn into_result(self) -> Result<bool> {
        self
    }
}

impl KotoAccessAssignOverrideReturn for bool {
    fn into_result(self) -> Result<bool> {
        Ok(self)
    }
}
