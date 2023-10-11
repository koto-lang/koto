use koto_runtime::{prelude::*, Result};
use nannou_core::geom::DVec3;
use std::{
    fmt,
    ops::{self, Deref},
};

#[derive(Copy, Clone, PartialEq)]
pub struct Vec3(DVec3);

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }
}

impl KotoType for Vec3 {
    const TYPE: &'static str = "Vec3";
}

impl KotoObject for Vec3 {
    fn object_type(&self) -> KString {
        VEC3_TYPE_STRING.with(|s| s.clone())
    }

    fn copy(&self) -> Object {
        (*self).into()
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        VEC3_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn negate(&self, _vm: &mut Vm) -> Result<Value> {
        Ok(Self(-self.0).into())
    }

    fn add(&self, rhs: &Value) -> Result<Value> {
        geometry_arithmetic_op!(self, rhs, +)
    }

    fn subtract(&self, rhs: &Value) -> Result<Value> {
        geometry_arithmetic_op!(self, rhs, -)
    }

    fn multiply(&self, rhs: &Value) -> Result<Value> {
        geometry_arithmetic_op!(self, rhs, *)
    }

    fn divide(&self, rhs: &Value) -> Result<Value> {
        geometry_arithmetic_op!(self, rhs, /)
    }

    fn add_assign(&mut self, rhs: &Value) -> Result<()> {
        geometry_arithmetic_assign_op!(self, rhs, +=)
    }

    fn subtract_assign(&mut self, rhs: &Value) -> Result<()> {
        geometry_arithmetic_assign_op!(self, rhs, -=)
    }

    fn multiply_assign(&mut self, rhs: &Value) -> Result<()> {
        geometry_arithmetic_assign_op!(self, rhs, *=)
    }

    fn divide_assign(&mut self, rhs: &Value) -> Result<()> {
        geometry_arithmetic_assign_op!(self, rhs, /=)
    }

    fn equal(&self, rhs: &Value) -> Result<bool> {
        geometry_comparison_op!(self, rhs, ==)
    }

    fn not_equal(&self, rhs: &Value) -> Result<bool> {
        geometry_comparison_op!(self, rhs, !=)
    }

    fn index(&self, index: &Value) -> Result<Value> {
        match index {
            Value::Number(n) => match usize::from(n) {
                0 => Ok(self.x.into()),
                1 => Ok(self.y.into()),
                2 => Ok(self.z.into()),
                other => runtime_error!("index out of range (got {other}, should be <= 2)"),
            },
            unexpected => type_error("Number", unexpected),
        }
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut Vm) -> Result<KIterator> {
        let v = *self;

        let iter = (0..=2).map(move |i| {
            let result = match i {
                0 => v.x,
                1 => v.y,
                2 => v.z,
                _ => unreachable!(),
            };
            KIteratorOutput::Value(result.into())
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

fn make_vec3_entries() -> ValueMap {
    ObjectEntryBuilder::<Vec3>::new()
        .method("sum", |ctx| {
            let v = ctx.instance()?;
            Ok((v.x + v.y + v.z).into())
        })
        .method("x", |ctx| Ok(ctx.instance()?.x.into()))
        .method("y", |ctx| Ok(ctx.instance()?.y.into()))
        .method("z", |ctx| Ok(ctx.instance()?.z.into()))
        .build()
}

thread_local! {
    static VEC3_TYPE_STRING: KString = Vec3::TYPE.into();
    static VEC3_ENTRIES: ValueMap = make_vec3_entries();
}

impl Deref for Vec3 {
    type Target = DVec3;

    fn deref(&self) -> &Self::Target {
        &self.0
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

impl From<Vec3> for Value {
    fn from(vec3: Vec3) -> Self {
        Object::from(vec3).into()
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vec3{{x: {}, y: {}, z: {}}}", self.x, self.y, self.z)
    }
}

crate::impl_arithmetic_ops!(Vec3);
