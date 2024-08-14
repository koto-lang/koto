use koto_runtime::{derive::*, prelude::*, Result};
use std::{fmt, ops};

type Inner = nannou_core::geom::DVec2;

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(use_copy)]
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
    fn angle(&self) -> KValue {
        Inner::X.angle_between(self.0).into()
    }

    #[koto_method]
    fn length(&self) -> KValue {
        self.0.length().into()
    }

    #[koto_method]
    fn x(&self) -> KValue {
        self.0.x.into()
    }

    #[koto_method]
    fn y(&self) -> KValue {
        self.0.y.into()
    }
}

impl KotoObject for Vec2 {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn negate(&self, _vm: &mut KotoVm) -> Result<KValue> {
        Ok(Self(-self.0).into())
    }

    fn add(&self, rhs: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, rhs, +)
    }

    fn subtract(&self, rhs: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, rhs, -)
    }

    fn multiply(&self, rhs: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, rhs, *)
    }

    fn divide(&self, rhs: &KValue) -> Result<KValue> {
        geometry_arithmetic_op!(self, rhs, /)
    }

    fn add_assign(&mut self, rhs: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, rhs, +=)
    }

    fn subtract_assign(&mut self, rhs: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, rhs, -=)
    }

    fn multiply_assign(&mut self, rhs: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, rhs, *=)
    }

    fn divide_assign(&mut self, rhs: &KValue) -> Result<()> {
        geometry_compound_assign_op!(self, rhs, /=)
    }

    fn equal(&self, rhs: &KValue) -> Result<bool> {
        geometry_comparison_op!(self, rhs, ==)
    }

    fn not_equal(&self, rhs: &KValue) -> Result<bool> {
        geometry_comparison_op!(self, rhs, !=)
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        match index {
            KValue::Number(n) => match usize::from(n) {
                0 => Ok(self.x()),
                1 => Ok(self.y()),
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
