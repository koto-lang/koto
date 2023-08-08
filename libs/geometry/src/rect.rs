use crate::Vec2;
use koto_runtime::{prelude::*, Result};
use std::{fmt, ops::Deref};

type Inner = nannou_core::geom::Rect<f64>;

#[derive(Copy, Clone, PartialEq)]
pub struct Rect(Inner);

impl Rect {
    pub fn from_x_y_w_h(x: f64, y: f64, width: f64, height: f64) -> Self {
        Inner::from_x_y_w_h(x, y, width, height).into()
    }
}

impl KotoType for Rect {
    const TYPE: &'static str = "Rect";
}

impl KotoObject for Rect {
    fn object_type(&self) -> ValueString {
        RECT_TYPE_STRING.with(|s| s.clone())
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        RECT_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn equal(&self, rhs: &Value) -> Result<bool> {
        geometry_comparison_op!(self, rhs, ==)
    }

    fn not_equal(&self, rhs: &Value) -> Result<bool> {
        geometry_comparison_op!(self, rhs, !=)
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut Vm) -> Result<ValueIterator> {
        let r = *self;

        let iter = (0..=3).map(move |i| {
            let result = match i {
                0 => r.x(),
                1 => r.y(),
                2 => r.w(),
                3 => r.h(),
                _ => unreachable!(),
            };
            ValueIteratorOutput::Value(result.into())
        });

        Ok(ValueIterator::with_std_iter(iter))
    }
}

fn make_rect_entries() -> DataMap {
    use Value::*;

    ObjectEntryBuilder::<Rect>::new()
        .method("left", |ctx| Ok(ctx.instance()?.left().into()))
        .method("right", |ctx| Ok(ctx.instance()?.right().into()))
        .method("top", |ctx| Ok(ctx.instance()?.top().into()))
        .method("bottom", |ctx| Ok(ctx.instance()?.bottom().into()))
        .method("width", |ctx| Ok(ctx.instance()?.w().into()))
        .method("height", |ctx| Ok(ctx.instance()?.h().into()))
        .method("center", |ctx| Ok(Vec2::from(ctx.instance()?.xy()).into()))
        .method("x", |ctx| Ok(ctx.instance()?.x().into()))
        .method("y", |ctx| Ok(ctx.instance()?.y().into()))
        .method("contains", |ctx| match ctx.args {
            [Object(p)] if p.is_a::<Vec2>() => {
                let p = p.cast::<Vec2>().unwrap();
                let result = ctx.instance()?.contains(p.inner());
                Ok(result.into())
            }
            unexpected => type_error_with_slice("Vec2", unexpected),
        })
        .method("set_center", |ctx| {
            let (x, y) = match ctx.args {
                [Number(x), Number(y)] => (x.into(), y.into()),
                [Object(p)] if p.is_a::<Vec2>() => {
                    let p = p.cast::<Vec2>().unwrap();
                    (p.x, p.y)
                }
                unexpected => return type_error_with_slice("two Numbers or a Vec2", unexpected),
            };
            let mut r = ctx.instance_mut()?;
            r.0 = Inner::from_x_y_w_h(x, y, r.w(), r.h());
            ctx.instance_result()
        })
        .build()
}

thread_local! {
    static RECT_TYPE_STRING: ValueString = Rect::TYPE.into();
    static RECT_ENTRIES: DataMap = make_rect_entries();
}

impl Deref for Rect {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Inner> for Rect {
    fn from(r: Inner) -> Self {
        Self(r)
    }
}

impl From<(f64, f64, f64, f64)> for Rect {
    fn from((x, y, w, h): (f64, f64, f64, f64)) -> Self {
        Self::from_x_y_w_h(x, y, w, h)
    }
}

impl From<Rect> for Value {
    fn from(point: Rect) -> Self {
        Object::from(point).into()
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = &self.0;
        write!(
            f,
            "Rect{{x: {}, y: {}, width: {}, height: {}}}",
            r.x(),
            r.y(),
            r.w(),
            r.h()
        )
    }
}
