use {
    crate::Vec2,
    koto_runtime::prelude::*,
    std::{fmt, ops::Deref},
};

type Inner = nannou_core::geom::Rect<f64>;

fn make_rect_meta_map() -> RcCell<MetaMap> {
    use {BinaryOp::*, UnaryOp::*, Value::*};

    MetaMapBuilder::<Rect>::new("Rect")
        .data_fn("left", |r| Ok(r.left().into()))
        .data_fn("right", |r| Ok(r.right().into()))
        .data_fn("top", |r| Ok(r.top().into()))
        .data_fn("bottom", |r| Ok(r.bottom().into()))
        .data_fn("width", |r| Ok(r.w().into()))
        .data_fn("height", |r| Ok(r.h().into()))
        .data_fn("center", |r| Ok(Vec2::from(r.xy()).into()))
        .data_fn("x", |r| Ok(r.x().into()))
        .data_fn("y", |r| Ok(r.y().into()))
        .data_fn_with_args("contains", |r, args| match args {
            [ExternalValue(p)] if p.has_data::<Vec2>() => {
                let p = p.data::<Vec2>().unwrap();
                let result = r.0.contains(p.inner());
                Ok(result.into())
            }
            unexpected => type_error_with_slice("Vec2", unexpected),
        })
        .value_fn("set_center", |value, args| {
            let mut r = value.data_mut::<Rect>().unwrap();
            let (x, y) = match args {
                [Number(x), Number(y)] => (x.into(), y.into()),
                [ExternalValue(p)] if p.has_data::<Vec2>() => {
                    let p = p.data::<Vec2>().unwrap();
                    (p.x, p.y)
                }
                unexpected => return type_error_with_slice("two Numbers or a Vec2", unexpected),
            };
            r.0 = Inner::from_x_y_w_h(x, y, r.w(), r.h());
            Ok(value.clone().into())
        })
        .data_fn(Display, |r| Ok(r.to_string().into()))
        .data_fn_with_args(Equal, koto_comparison_op!(Rect, ==))
        .data_fn_with_args(NotEqual, koto_comparison_op!(Rect, !=))
        .data_fn(UnaryOp::Iterator, |r| {
            let r = *r;
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
        ExternalValue::with_shared_meta_map(point, meta).into()
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
