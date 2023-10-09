use koto_runtime::{prelude::*, Result};
use std::{
    fmt,
    ops::{self, Deref, DerefMut},
};

use palette::{rgb::LinSrgba as Inner, Mix};

#[derive(Copy, Clone, PartialEq)]
pub struct Color(Inner);

macro_rules! impl_arithmetic_op {
    ($trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for Color {
            type Output = Self;

            fn $trait_fn(self, other: Self) -> Self {
                Inner{
                    color: self.color $op other.color,
                    alpha: self.alpha
                }.into()
            }
        }

        impl ops::$trait<f32> for Color {
            type Output = Self;

            fn $trait_fn(self, other: f32) -> Self {
                Inner{
                    color: self.color $op other,
                    alpha: self.alpha
                }.into()
            }
        }
    };
}

macro_rules! impl_arithmetic_assign_op {
    ($trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for Color {
            fn $trait_fn(&mut self, other: Color) -> () {
                self.color $op other.color;
            }
        }

        impl ops::$trait<f32> for Color {
            fn $trait_fn(&mut self, other: f32) -> () {
                self.color $op other;
            }
        }
    };
}

impl_arithmetic_op!(Add, add, +);
impl_arithmetic_op!(Sub, sub, -);
impl_arithmetic_op!(Mul, mul, *);
impl_arithmetic_op!(Div, div, /);
impl_arithmetic_assign_op!(AddAssign, add_assign, +=);
impl_arithmetic_assign_op!(SubAssign, sub_assign, -=);
impl_arithmetic_assign_op!(MulAssign, mul_assign, *=);
impl_arithmetic_assign_op!(DivAssign, div_assign, /=);

#[macro_export]
macro_rules! color_arithmetic_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                Value::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    Ok((*$self $op *rhs).into())
                }
                Value::Number(n) => {
                    Ok((*$self $op f32::from(n)).into())
                }
                unexpected => {
                    type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! color_arithmetic_assign_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                Value::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    *$self $op *rhs;
                    Ok(())
                }
                Value::Number(n) => {
                    *$self $op f32::from(n);
                    Ok(())
                }
                unexpected => {
                    type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! color_comparison_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                Value::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    Ok(*$self $op *rhs)
                }
                unexpected => {
                    type_error(&format!("a {}", Self::TYPE), unexpected)
                }
            }
        }
    }
}

impl Color {
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self(Inner::new(r, g, b, 1.0))
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(Inner::new(r, g, b, a))
    }

    pub fn named(name: &str) -> Option<Self> {
        palette::named::from_str(name).map(|c| {
            Inner::new(
                c.red as f32 / 255.0,
                c.green as f32 / 255.0,
                c.blue as f32 / 255.0,
                1.0,
            )
            .into()
        })
    }

    pub fn inner(&self) -> Inner {
        self.0
    }

    pub fn red(&self) -> f32 {
        self.color.red
    }

    pub fn green(&self) -> f32 {
        self.color.green
    }

    pub fn blue(&self) -> f32 {
        self.color.blue
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

impl KotoType for Color {
    const TYPE: &'static str = "Color";
}

impl KotoObject for Color {
    fn object_type(&self) -> ValueString {
        COLOR_TYPE_STRING.with(|s| s.clone())
    }

    fn copy(&self) -> Object {
        (*self).into()
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        COLOR_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn add(&self, rhs: &Value) -> Result<Value> {
        color_arithmetic_op!(self, rhs, +)
    }

    fn subtract(&self, rhs: &Value) -> Result<Value> {
        color_arithmetic_op!(self, rhs, -)
    }

    fn multiply(&self, rhs: &Value) -> Result<Value> {
        color_arithmetic_op!(self, rhs, *)
    }

    fn divide(&self, rhs: &Value) -> Result<Value> {
        color_arithmetic_op!(self, rhs, /)
    }

    fn add_assign(&mut self, rhs: &Value) -> Result<()> {
        color_arithmetic_assign_op!(self, rhs, +=)
    }

    fn subtract_assign(&mut self, rhs: &Value) -> Result<()> {
        color_arithmetic_assign_op!(self, rhs, -=)
    }

    fn multiply_assign(&mut self, rhs: &Value) -> Result<()> {
        color_arithmetic_assign_op!(self, rhs, *=)
    }

    fn divide_assign(&mut self, rhs: &Value) -> Result<()> {
        color_arithmetic_assign_op!(self, rhs, /=)
    }

    fn equal(&self, rhs: &Value) -> Result<bool> {
        color_comparison_op!(self, rhs, ==)
    }

    fn not_equal(&self, rhs: &Value) -> Result<bool> {
        color_comparison_op!(self, rhs, !=)
    }

    fn index(&self, index: &Value) -> Result<Value> {
        match index {
            Value::Number(n) => match usize::from(n) {
                0 => Ok(self.red().into()),
                1 => Ok(self.green().into()),
                2 => Ok(self.blue().into()),
                3 => Ok(self.alpha().into()),
                other => runtime_error!("index out of range (got {other}, should be <= 3)"),
            },
            unexpected => type_error("Number", unexpected),
        }
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut Vm) -> Result<ValueIterator> {
        let c = *self;

        let iter = (0..=3).map(move |i| {
            let result = match i {
                0 => c.red(),
                1 => c.green(),
                2 => c.blue(),
                3 => c.alpha(),
                _ => unreachable!(),
            };
            ValueIteratorOutput::Value(result.into())
        });

        Ok(ValueIterator::with_std_iter(iter))
    }
}

fn make_color_entries() -> DataMap {
    use Value::{Number, Object};

    ObjectEntryBuilder::<Color>::new()
        .method_aliased(&["red", "r"], |ctx| Ok(ctx.instance()?.red().into()))
        .method_aliased(&["green", "g"], |ctx| Ok(ctx.instance()?.green().into()))
        .method_aliased(&["blue", "b"], |ctx| Ok(ctx.instance()?.blue().into()))
        .method_aliased(&["alpha", "a"], |ctx| Ok(ctx.instance()?.alpha().into()))
        .method_aliased(&["set_red", "set_r"], |ctx| match ctx.args {
            [Number(n)] => {
                ctx.instance_mut()?.color.red = n.into();
                ctx.instance_result()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .method_aliased(&["set_green", "set_g"], |ctx| match ctx.args {
            [Number(n)] => {
                ctx.instance_mut()?.color.green = n.into();
                ctx.instance_result()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .method_aliased(&["set_blue", "set_b"], |ctx| match ctx.args {
            [Number(n)] => {
                ctx.instance_mut()?.color.blue = n.into();
                ctx.instance_result()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .method_aliased(&["set_alpha", "set_a"], |ctx| match ctx.args {
            [Number(n)] => {
                ctx.instance_mut()?.alpha = n.into();
                ctx.instance_result()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .method("mix", |ctx| match ctx.args {
            [Object(b)] if b.is_a::<Color>() => {
                let a = ctx.instance()?;
                let b = b.cast::<Color>()?;

                Ok(Color::from(a.0.mix(b.0, 0.5)).into())
            }
            [Object(b), Number(x)] if b.is_a::<Color>() => {
                let a = ctx.instance()?;
                let b = b.cast::<Color>()?;
                let n = f32::from(x);

                Ok(Color::from(a.0.mix(b.0, n)).into())
            }
            unexpected => type_error_with_slice("2 Colors and an optional mix amount", unexpected),
        })
        .build()
}

thread_local! {
    static COLOR_TYPE_STRING: ValueString = Color::TYPE.into();
    static COLOR_ENTRIES: DataMap = make_color_entries();
}

impl Deref for Color {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Color {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<(f32, f32, f32, f32)> for Color {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self::rgba(r, g, b, a)
    }
}

impl From<Inner> for Color {
    fn from(c: Inner) -> Self {
        Self(c)
    }
}

impl From<Color> for Value {
    fn from(color: Color) -> Self {
        Object::from(color).into()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Color {{r: {}, g: {}, b: {}, a: {}}}",
            self.red(),
            self.green(),
            self.blue(),
            self.alpha()
        )
    }
}
