use koto_runtime::{derive::*, prelude::*, Result};
use nannou_core::geom::DVec3;
use std::{fmt, ops};

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(use_copy)]
pub struct Vec3(DVec3);

#[koto_impl(runtime = koto_runtime)]
impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }

    #[koto_method]
    fn x(&self) -> KValue {
        self.0.x.into()
    }

    #[koto_method]
    fn y(&self) -> KValue {
        self.0.y.into()
    }

    #[koto_method]
    fn z(&self) -> KValue {
        self.0.z.into()
    }
}

impl KotoObject for Vec3 {
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
                2 => Ok(self.z()),
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
