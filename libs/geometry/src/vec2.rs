use {
    koto_runtime::prelude::*,
    nannou_core::geom::DVec2,
    std::{
        fmt,
        ops::{self, Deref},
    },
};

fn make_vec2_meta_map() -> RcCell<MetaMap> {
    use {BinaryOp::*, Value::*};

    let builder = MetaMapBuilder::<Vec2>::new("Vec2");
    add_ops!(Vec2, builder)
        .function("angle", |context| {
            Ok(DVec2::X.angle_between(**context.data()?).into())
        })
        .function("length", |context| Ok(context.data()?.length().into()))
        .function("x", |context| Ok(context.data()?.x.into()))
        .function("y", |context| Ok(context.data()?.y.into()))
        .function(Index, |context| match context.args {
            [Number(n)] => {
                let v = context.data()?;
                match usize::from(n) {
                    0 => Ok(v.x.into()),
                    1 => Ok(v.y.into()),
                    other => runtime_error!("index out of range (got {other}, should be <= 1)"),
                }
            }
            unexpected => type_error_with_slice("expected a Number", unexpected),
        })
        .function(UnaryOp::Iterator, |context| {
            let v = *context.data()?;
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
    static VEC2_META: RcCell<MetaMap> = make_vec2_meta_map();
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

    fn make_copy(&self) -> RcCell<dyn ExternalData> {
        (*self).into()
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
        External::with_shared_meta_map(point, meta).into()
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
