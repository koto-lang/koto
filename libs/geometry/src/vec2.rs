use koto_runtime::{prelude::*, Result};
use std::{
    fmt,
    ops::{self, Deref},
};

type Inner = nannou_core::geom::DVec2;

#[derive(Copy, Clone, PartialEq)]
pub struct Vec2(Inner);

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self(Inner::new(x, y))
    }

    pub fn inner(&self) -> Inner {
        self.0
    }
}

impl KotoType for Vec2 {
    const TYPE: &'static str = "Vec2";
}

impl KotoObject for Vec2 {
    fn object_type(&self) -> ValueString {
        VEC2_TYPE_STRING.with(|s| s.clone())
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        VEC2_ENTRIES.with(|entries| entries.get(key).cloned())
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
                other => runtime_error!("index out of range (got {other}, should be <= 1)"),
            },
            unexpected => type_error("Number", unexpected),
        }
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut Vm) -> Result<ValueIterator> {
        let v = *self;

        let iter = (0..=1).map(move |i| {
            let result = match i {
                0 => v.x,
                1 => v.y,
                _ => unreachable!(),
            };
            ValueIteratorOutput::Value(result.into())
        });

        Ok(ValueIterator::with_std_iter(iter))
    }
}

fn make_vec2_entries() -> DataMap {
    ObjectEntryBuilder::<Vec2>::new()
        .method("angle", |ctx| {
            Ok(Inner::X.angle_between(**ctx.instance()?).into())
        })
        .method("length", |ctx| Ok(ctx.instance()?.length().into()))
        .method("x", |ctx| Ok(ctx.instance()?.x.into()))
        .method("y", |ctx| Ok(ctx.instance()?.y.into()))
        .build()
}

thread_local! {
    static VEC2_TYPE_STRING: ValueString = Vec2::TYPE.into();
    static VEC2_ENTRIES: DataMap = make_vec2_entries();
}

impl Deref for Vec2 {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Inner> for Vec2 {
    fn from(v: Inner) -> Self {
        Self(v)
    }
}

impl From<Vec2> for Value {
    fn from(point: Vec2) -> Self {
        Object::from(point).into()
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl fmt::Display for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vec2{{x: {}, y: {}}}", self.x, self.y)
    }
}

crate::impl_arithmetic_ops!(Vec2);
