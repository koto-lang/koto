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
    let builder = MetaMapBuilder::<Vec2>::new("Vec2");
    add_ops!(Vec2, builder)
        .data_fn("angle", |v| Ok(DVec2::X.angle_between(*v.deref()).into()))
        .data_fn("length", |v| Ok(v.length().into()))
        .data_fn("x", |v| Ok(v.x.into()))
        .data_fn("y", |v| Ok(v.y.into()))
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
