use {
    crate::Vec2,
    koto_runtime::prelude::*,
    std::{fmt, ops::Deref},
};

type Inner = nannou_core::geom::Rect<f64>;

fn make_rect_meta_map() -> RcCell<MetaMap> {
    use {BinaryOp::*, UnaryOp::*, Value::*};

    MetaMapBuilder::<Rect>::new("Rect")
        .function("left", |context| Ok(context.data()?.left().into()))
        .function("right", |context| Ok(context.data()?.right().into()))
        .function("top", |context| Ok(context.data()?.top().into()))
        .function("bottom", |context| Ok(context.data()?.bottom().into()))
        .function("width", |context| Ok(context.data()?.w().into()))
        .function("height", |context| Ok(context.data()?.h().into()))
        .function("center", |context| {
            Ok(Vec2::from(context.data()?.xy()).into())
        })
        .function("x", |context| Ok(context.data()?.x().into()))
        .function("y", |context| Ok(context.data()?.y().into()))
        .function("contains", |context| match context.args {
            [External(p)] if p.has_data::<Vec2>() => {
                let p = p.data::<Vec2>().unwrap();
                let result = context.data()?.contains(p.inner());
                Ok(result.into())
            }
            unexpected => type_error_with_slice("Vec2", unexpected),
        })
        .function("set_center", |context| {
            let (x, y) = match context.args {
                [Number(x), Number(y)] => (x.into(), y.into()),
                [External(p)] if p.has_data::<Vec2>() => {
                    let p = p.data::<Vec2>().unwrap();
                    (p.x, p.y)
                }
                unexpected => return type_error_with_slice("two Numbers or a Vec2", unexpected),
            };
            let mut r = context.data_mut()?;
            r.0 = Inner::from_x_y_w_h(x, y, r.w(), r.h());
            context.ok_value()
        })
        .function(Display, |context| Ok(context.data()?.to_string().into()))
        .function(Equal, koto_comparison_op!(Rect, ==))
        .function(NotEqual, koto_comparison_op!(Rect, !=))
        .function(UnaryOp::Iterator, |context| {
            let r = *context.data()?;
            let iter = (0..=3).map(move |i| {
                let result = match i {
                    0 => r.x(),
                    1 => r.y(),
                    2 => r.w(),
                    3 => r.h(),
                    _ => unreachable!(),
                };
                result.into()
            });
            Ok(ValueIterator::with_std_iter(iter).into())
        })
        .build()
}

thread_local! {
    static RECT_META: RcCell<MetaMap> = make_rect_meta_map();
    static TYPE_RECT: ValueString = "Rect".into();
}

#[derive(Copy, Clone, PartialEq)]
pub struct Rect(Inner);

impl Rect {
    pub fn from_x_y_w_h(x: f64, y: f64, width: f64, height: f64) -> Self {
        Inner::from_x_y_w_h(x, y, width, height).into()
    }
}

impl ExternalData for Rect {
    fn data_type(&self) -> ValueString {
        TYPE_RECT.with(|x| x.clone())
    }

    fn make_copy(&self) -> RcCell<dyn ExternalData> {
        (*self).into()
    }
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
        let meta = RECT_META.with(|meta| meta.clone());
        External::with_shared_meta_map(point, meta).into()
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
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
