use crate::{
    geometry_arithmetic_op, geometry_arithmetic_op_rhs, geometry_comparison_op,
    geometry_compound_assign_op,
};
use glam::DVec3;
use koto_runtime::{Result, derive::*, prelude::*};
use std::{fmt, ops};

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(runtime = koto_runtime, use_copy)]
pub struct Vec3(DVec3);

#[koto_impl(runtime = koto_runtime)]
impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }

    #[koto_get]
    fn x(&self) -> f64 {
        self.0.x
    }

    #[koto_get]
    fn y(&self) -> f64 {
        self.0.y
    }

    #[koto_get]
    fn z(&self) -> f64 {
        self.0.z
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

    #[koto_set]
    fn set_z(&mut self, value: &KValue) -> Result<()> {
        match value {
            KValue::Number(z) => {
                self.0.z = z.into();
                Ok(())
            }
            unexpected => unexpected_type("a Number", unexpected),
        }
    }

    #[koto_method]
    fn length(&self) -> f64 {
        self.0.length()
    }
}

impl KotoObject for Vec3 {
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
                2 => Ok(self.z().into()),
                other => runtime_error!("index out of range (got {other}, should be <= 2)"),
            },
            unexpected => unexpected_type("Number", unexpected),
        }
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        let v = *self;

        let iter = (0..=2).map(move |i| {
            let result = match i {
                0 => v.0.x,
                1 => v.0.y,
                2 => v.0.z,
                _ => unreachable!(),
            };
            KIteratorOutput::Value(result.into())
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

impl From<DVec3> for Vec3 {
    fn from(v: DVec3) -> Self {
        Self(v)
    }
}

impl From<f64> for Vec3 {
    fn from(x: f64) -> Self {
        Self::new(x, x, x)
    }
}

impl From<(f64, f64, f64)> for Vec3 {
    fn from((x, y, z): (f64, f64, f64)) -> Self {
        Self::new(x, y, z)
    }
}

impl From<Vec3> for KValue {
    fn from(vec3: Vec3) -> Self {
        KObject::from(vec3).into()
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Vec3{{x: {}, y: {}, z: {}}}",
            self.0.x, self.0.y, self.0.z
        )
    }
}

crate::impl_arithmetic_ops!(Vec3);
