use {
    koto_runtime::prelude::*,
    nannou_core::geom::DVec3,
    std::{
        fmt,
        ops::{self, Deref},
    },
};

fn make_vec3_meta_map() -> PtrMut<MetaMap> {
    use {BinaryOp::*, Value::*};

    let builder = MetaMapBuilder::<Vec3>::new("Vec3");
    add_ops!(Vec3, builder)
        .function("x", |context| Ok(context.data()?.x.into()))
        .function("y", |context| Ok(context.data()?.y.into()))
        .function("z", |context| Ok(context.data()?.z.into()))
        .function("sum", |context| {
            let v = context.data()?;
            Ok((v.x + v.y + v.z).into())
        })
        .function(Index, |context| {
            let v = context.data()?;
            match context.args {
                [Number(n)] => match usize::from(n) {
                    0 => Ok(v.x.into()),
                    1 => Ok(v.y.into()),
                    2 => Ok(v.z.into()),
                    other => runtime_error!("index out of range (got {other}, should be <= 2)"),
                },
                unexpected => type_error_with_slice("expected a Number", unexpected),
            }
        })
        .function(UnaryOp::Iterator, |context| {
            let v = *context.data()?;
            let iter = (0..=2).map(move |i| {
                let result = match i {
                    0 => v.x,
                    1 => v.y,
                    2 => v.z,
                    _ => unreachable!(),
                };
                result.into()
            });
            Ok(ValueIterator::with_std_iter(iter).into())
        })
        .build()
}

thread_local! {
    static VEC3_META: PtrMut<MetaMap> = make_vec3_meta_map();
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

    fn make_copy(&self) -> PtrMut<dyn ExternalData> {
        make_data_ptr(*self)
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
        External::with_shared_meta_map(vec3, meta).into()
    }
}

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Vec3{{x: {}, y: {}, z: {}}}", self.x, self.y, self.z)
    }
}

crate::impl_arithmetic_ops!(Vec3);
