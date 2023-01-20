use {
    koto_runtime::prelude::*,
    nannou_core::geom::DVec3,
    std::{
        cell::RefCell,
        fmt,
        ops::{self, Deref},
        rc::Rc,
    },
};

fn make_vec3_meta_map() -> Rc<RefCell<MetaMap>> {
    let builder = MetaMapBuilder::<Vec3>::new("Vec3");
    add_ops!(Vec3, builder)
        .data_fn("x", |v| Ok(v.x.into()))
        .data_fn("y", |v| Ok(v.y.into()))
        .data_fn("z", |v| Ok(v.z.into()))
        .data_fn("sum", |v| Ok((v.x + v.y + v.z).into()))
        .build()
}

thread_local! {
    static VEC3_META: Rc<RefCell<MetaMap>> = make_vec3_meta_map();
    static TYPE_VEC3: ValueString = "Vec3".into();
}

#[derive(Copy, Clone, PartialEq)]
pub struct Vec3(DVec3);

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }
}

impl ExternalData for Vec3 {
    fn data_type(&self) -> ValueString {
        TYPE_VEC3.with(|x| x.clone())
    }
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
        let meta = VEC3_META.with(|meta| meta.clone());
        ExternalValue::with_shared_meta_map(vec3, meta).into()
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Vec3{{x: {}, y: {}, z: {}}}", self.x, self.y, self.z)
    }
}

crate::impl_arithmetic_ops!(Vec3);
