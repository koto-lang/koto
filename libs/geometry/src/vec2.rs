use {
    koto_runtime::prelude::*,
    nannou_core::geom::DVec2,
    std::{
        cell::RefCell,
        fmt,
        ops::{self, Deref},
        rc::Rc,
    },
};

fn make_vec2_meta_map() -> Rc<RefCell<MetaMap>> {
    use {BinaryOp::*, Value::*};

    let builder = MetaMapBuilder::<Vec2>::new("Vec2");
    add_ops!(Vec2, builder)
        .data_fn("angle", |v| Ok(DVec2::X.angle_between(*v.deref()).into()))
        .data_fn("length", |v| Ok(v.length().into()))
        .data_fn("x", |v| Ok(v.x.into()))
        .data_fn("y", |v| Ok(v.y.into()))
        .data_fn_with_args(Index, |a, b| match b {
            [Number(n)] => match usize::from(n) {
                0 => Ok(a.x.into()),
                1 => Ok(a.y.into()),
                other => runtime_error!("index out of range (got {other}, should be <= 1)"),
            },
            unexpected => type_error_with_slice("expected a Number", unexpected),
        })
        .data_fn(UnaryOp::Iterator, |v| {
            let v = *v;
            let iter = (0..=1).map(move |i| {
                let result = match i {
                    0 => v.x,
                    1 => v.y,
                    _ => unreachable!(),
                };
                result.into()
            });
            Ok(ValueIterator::with_std_iter(iter).into())
        })
        .build()
}

thread_local! {
    static VEC2_META: Rc<RefCell<MetaMap>> = make_vec2_meta_map();
    static TYPE_VEC2: ValueString = "Vec2".into();
}

#[derive(Copy, Clone, PartialEq)]
pub struct Vec2(DVec2);

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self(DVec2::new(x, y))
    }

    pub fn inner(&self) -> DVec2 {
        self.0
    }
}

impl ExternalData for Vec2 {
    fn data_type(&self) -> ValueString {
        TYPE_VEC2.with(|x| x.clone())
    }
}

impl Deref for Vec2 {
    type Target = DVec2;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<DVec2> for Vec2 {
    fn from(v: DVec2) -> Self {
        Self(v)
    }
}

impl From<Vec2> for Value {
    fn from(point: Vec2) -> Self {
        let meta = VEC2_META.with(|meta| meta.clone());
        ExternalValue::with_shared_meta_map(point, meta).into()
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl fmt::Display for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Vec2{{x: {}, y: {}}}", self.x, self.y)
    }
}

crate::impl_arithmetic_ops!(Vec2);
