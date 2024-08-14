use koto_runtime::{derive::*, prelude::*, Result};
use std::{fmt, ops};

use palette::{rgb::LinSrgba as Inner, FromColor, Mix};

macro_rules! impl_arithmetic_op {
    ($trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for Color {
            type Output = Self;

            fn $trait_fn(self, other: Self) -> Self {
                Inner{
                    color: self.0.color $op other.0.color,
                    alpha: self.0.alpha
                }.into()
            }
        }

        impl ops::$trait<f32> for Color {
            type Output = Self;

            fn $trait_fn(self, other: f32) -> Self {
                Inner{
                    color: self.0.color $op other,
                    alpha: self.0.alpha
                }.into()
            }
        }
    };
}

macro_rules! impl_compound_assign_op {
    ($trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for Color {
            fn $trait_fn(&mut self, other: Color) -> () {
                self.0.color $op other.0.color;
            }
        }

        impl ops::$trait<f32> for Color {
            fn $trait_fn(&mut self, other: f32) -> () {
                self.0.color $op other;
            }
        }
    };
}

impl_arithmetic_op!(Add, add, +);
impl_arithmetic_op!(Sub, sub, -);
impl_arithmetic_op!(Mul, mul, *);
impl_arithmetic_op!(Div, div, /);
impl_compound_assign_op!(AddAssign, add_assign, +=);
impl_compound_assign_op!(SubAssign, sub_assign, -=);
impl_compound_assign_op!(MulAssign, mul_assign, *=);
impl_compound_assign_op!(DivAssign, div_assign, /=);

#[macro_export]
macro_rules! color_arithmetic_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                KValue::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    Ok((*$self $op *rhs).into())
                }
                KValue::Number(n) => {
                    Ok((*$self $op f32::from(n)).into())
                }
                unexpected => {
                    unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! color_compound_assign_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                KValue::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    *$self $op *rhs;
                    Ok(())
                }
                KValue::Number(n) => {
                    *$self $op f32::from(n);
                    Ok(())
                }
                unexpected => {
                    unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
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
                KValue::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    Ok(*$self $op *rhs)
                }
                unexpected => {
                    unexpected_type(&format!("a {}", Self::type_static()), unexpected)
                }
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(use_copy)]
pub struct Color(Inner);

#[koto_impl(runtime = koto_runtime)]
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

    pub fn inner(&self) -> &Inner {
        &self.0
    }

    #[koto_method(alias = "r")]
    pub fn red(&self) -> KValue {
        self.0.color.red.into()
    }

    #[koto_method(alias = "g")]
    pub fn green(&self) -> KValue {
        self.0.color.green.into()
    }

    #[koto_method(alias = "b")]
    pub fn blue(&self) -> KValue {
        self.0.color.blue.into()
    }

    #[koto_method(alias = "a")]
    pub fn alpha(&self) -> KValue {
        self.0.alpha.into()
    }

    #[koto_method(alias = "set_r")]
    pub fn set_red(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [KValue::Number(n)] => {
                ctx.instance_mut()?.0.color.red = n.into();
                ctx.instance_result()
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method(alias = "set_g")]
    pub fn set_green(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [KValue::Number(n)] => {
                ctx.instance_mut()?.0.color.green = n.into();
                ctx.instance_result()
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method(alias = "set_b")]
    pub fn set_blue(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [KValue::Number(n)] => {
                ctx.instance_mut()?.0.color.blue = n.into();
                ctx.instance_result()
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method(alias = "set_a")]
    pub fn set_alpha(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [KValue::Number(n)] => {
                ctx.instance_mut()?.0.alpha = n.into();
                ctx.instance_result()
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method]
    pub fn mix(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [KValue::Object(b)] if b.is_a::<Color>() => {
                let a = ctx.instance()?;
                let b = b.cast::<Color>()?;

                Ok(Color::from(a.0.mix(b.0, 0.5)).into())
            }
            [KValue::Object(b), KValue::Number(x)] if b.is_a::<Color>() => {
                let a = ctx.instance()?;
                let b = b.cast::<Color>()?;
                let n = f32::from(x);

                Ok(Color::from(a.0.mix(b.0, n)).into())
            }
            unexpected => unexpected_args("|Color|, or |Color, Number|", unexpected),
        }
    }
}

impl KotoObject for Color {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn add(&self, rhs: &KValue) -> Result<KValue> {
        color_arithmetic_op!(self, rhs, +)
    }

    fn subtract(&self, rhs: &KValue) -> Result<KValue> {
        color_arithmetic_op!(self, rhs, -)
    }

    fn multiply(&self, rhs: &KValue) -> Result<KValue> {
        color_arithmetic_op!(self, rhs, *)
    }

    fn divide(&self, rhs: &KValue) -> Result<KValue> {
        color_arithmetic_op!(self, rhs, /)
    }

    fn add_assign(&mut self, rhs: &KValue) -> Result<()> {
        color_compound_assign_op!(self, rhs, +=)
    }

    fn subtract_assign(&mut self, rhs: &KValue) -> Result<()> {
        color_compound_assign_op!(self, rhs, -=)
    }

    fn multiply_assign(&mut self, rhs: &KValue) -> Result<()> {
        color_compound_assign_op!(self, rhs, *=)
    }

    fn divide_assign(&mut self, rhs: &KValue) -> Result<()> {
        color_compound_assign_op!(self, rhs, /=)
    }

    fn equal(&self, rhs: &KValue) -> Result<bool> {
        color_comparison_op!(self, rhs, ==)
    }

    fn not_equal(&self, rhs: &KValue) -> Result<bool> {
        color_comparison_op!(self, rhs, !=)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        match index {
            KValue::Number(n) => match usize::from(n) {
                0 => Ok(self.red()),
                1 => Ok(self.green()),
                2 => Ok(self.blue()),
                3 => Ok(self.alpha()),
                other => runtime_error!("index out of range (got {other}, should be <= 3)"),
            },
            unexpected => unexpected_type("Number", unexpected),
        }
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        let c = *self;

        let iter = (0..=3).map(move |i| {
            let result = match i {
                0 => c.0.color.red,
                1 => c.0.color.green,
                2 => c.0.color.blue,
                3 => c.0.alpha,
                _ => unreachable!(),
            };
            KIteratorOutput::Value(result.into())
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

impl From<Color> for KValue {
    fn from(color: Color) -> Self {
        KObject::from(color).into()
    }
}

impl<T> From<T> for Color
where
    Inner: FromColor<T>,
{
    fn from(c: T) -> Self {
        Self(Inner::from_color(c))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Color {{r: {}, g: {}, b: {}, a: {}}}",
            self.0.color.red, self.0.color.green, self.0.color.blue, self.0.alpha
        )
    }
}
