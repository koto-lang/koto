use koto_runtime::{Result, derive::*, prelude::*};
use std::{fmt, ops};

type Inner = glam::DVec2;

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(runtime = koto_runtime, use_copy)]
pub struct Vec2(Inner);

#[koto_impl(runtime = koto_runtime)]
impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self(Inner::new(x, y))
    }

    pub fn inner(&self) -> Inner {
        self.0
    }

    #[koto_method]
    fn angle(&self) -> f64 {
        Inner::X.angle_to(self.0)
    }

    #[koto_method]
    fn length(&self) -> f64 {
        self.0.length()
    }

    #[koto_get]
    fn x(&self) -> f64 {
        self.0.x
    }

    #[koto_get]
    fn y(&self) -> f64 {
        self.0.y
    }

    #[koto_set]
    fn set_x(&mut self, value: &KValue) -> Result<()> {
        match value {
            KValue::Number(x) => {
                self.0.x = x.into();
                Ok(())
            }
            unexpected => unexpected_type("a Number", unexpected),
        }
    }

    #[koto_set]
    fn set_y(&mut self, value: &KValue) -> Result<()> {
        match value {
            KValue::Number(y) => {
                self.0.y = y.into();
                Ok(())
            }
            unexpected => unexpected_type("a Number", unexpected),
        }
    }
}

impl KotoObject for Vec2 {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn negate(&self) -> Result<KValue> {
        Ok(Self(-self.0).into())
    }

    fn add(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, other, +)
    }

    fn add_rhs(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op_rhs!(self, other, +)
    }

    fn subtract(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, other, -)
    }

    fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op_rhs!(self, other, -)
    }

    fn multiply(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, other, *)
    }

    fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op_rhs!(self, other, *)
    }

    fn divide(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, other, /)
    }

    fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
        geometry_arithmetic_op_rhs!(self, other, /)
    }

    fn add_assign(&mut self, other: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, other, +=)
    }

    fn subtract_assign(&mut self, other: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, other, -=)
    }

    fn multiply_assign(&mut self, other: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, other, *=)
    }

    fn divide_assign(&mut self, other: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, other, /=)
    }

    fn equal(&self, other: &KValue) -> Result<bool> {
        geometry_comparison_op!(self, other, ==)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        match index {
            KValue::Number(n) => match usize::from(n) {
                0 => Ok(self.x().into()),
                1 => Ok(self.y().into()),
                other => runtime_error!("index out of range (got {other}, should be <= 1)"),
            },
            unexpected => unexpected_type("Number", unexpected),
        }
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        let v = *self;

        let iter = (0..=1).map(move |i| {
            let result = match i {
                0 => v.0.x,
                1 => v.0.y,
                _ => unreachable!(),
            };
            KIteratorOutput::Value(result.into())
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

impl From<Inner> for Vec2 {
    fn from(v: Inner) -> Self {
        Self(v)
    }
}

impl From<Vec2> for KValue {
    fn from(point: Vec2) -> Self {
        KObject::from(point).into()
    }
}

impl From<f64> for Vec2 {
    fn from(x: f64) -> Self {
        Self::new(x, x)
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl fmt::Display for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vec2{{x: {}, y: {}}}", self.0.x, self.0.y)
    }
}

crate::impl_arithmetic_ops!(Vec2);
