use {
    koto_runtime::prelude::*,
    std::{
        fmt,
        ops::{self, Deref, DerefMut},
    },
};

type Inner = palette::rgb::LinSrgba;

#[derive(Copy, Clone, PartialEq)]
pub struct Color(Inner);

impl Color {
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self(Inner::new(r, g, b, 1.0))
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(Inner::new(r, g, b, a))
    }

    pub fn named(name: &str) -> Option<Self> {
        palette::named::from_str(name).map(|c| {
            Inner::new(
                c.red as f32 / 255.0,
                c.green as f32 / 255.0,
                c.blue as f32 / 255.0,
                1.0,
            )
            .into()
        })
    }

    pub fn inner(&self) -> Inner {
        self.0
    }

    pub fn red(&self) -> f32 {
        self.color.red
    }

    pub fn green(&self) -> f32 {
        self.color.green
    }

    pub fn blue(&self) -> f32 {
        self.color.blue
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

impl ExternalData for Color {
    fn data_type(&self) -> ValueString {
        TYPE_COLOR.with(|x| x.clone())
    }

    fn make_copy(&self) -> RcCell<dyn ExternalData> {
        (*self).into()
    }
}

impl Deref for Color {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Color {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<(f32, f32, f32, f32)> for Color {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self::rgba(r, g, b, a)
    }
}

impl From<Inner> for Color {
    fn from(c: Inner) -> Self {
        Self(c)
    }
}

impl From<Color> for Value {
    fn from(color: Color) -> Self {
        let meta = COLOR_META.with(|meta| meta.clone());
        ExternalValue::with_shared_meta_map(color, meta).into()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "Color {{r: {}, g: {}, b: {}, a: {}}}",
            self.red(),
            self.green(),
            self.blue(),
            self.alpha()
        )
    }
}

macro_rules! impl_arithmetic_op {
    ($trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for Color {
            type Output = Self;

            fn $trait_fn(self, other: Self) -> Self {
                Inner{
                    color: self.color $op other.color,
                    alpha: self.alpha
                }.into()
            }
        }

        impl ops::$trait<f32> for Color {
            type Output = Self;

            fn $trait_fn(self, other: f32) -> Self {
                Inner{
                    color: self.color $op other,
                    alpha: self.alpha
                }.into()
            }
        }
    };
}

macro_rules! impl_arithmetic_assign_op {
    ($trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for Color {
            fn $trait_fn(&mut self, other: Color) -> () {
                self.color $op other.color;
            }
        }

        impl ops::$trait<f32> for Color {
            fn $trait_fn(&mut self, other: f32) -> () {
                self.color $op other;
            }
        }
    };
}

impl_arithmetic_op!(Add, add, +);
impl_arithmetic_op!(Sub, sub, -);
impl_arithmetic_op!(Mul, mul, *);
impl_arithmetic_op!(Div, div, /);
impl_arithmetic_assign_op!(AddAssign, add_assign, +=);
impl_arithmetic_assign_op!(SubAssign, sub_assign, -=);
impl_arithmetic_assign_op!(MulAssign, mul_assign, *=);
impl_arithmetic_assign_op!(DivAssign, div_assign, /=);

#[macro_export]
macro_rules! color_arithmetic_op {
    ($op:tt) => {
        |a, b| match b {
            [Value::ExternalValue(b)] if b.has_data::<Color>() =>{
                let b = b.data::<Color>().unwrap();
                Ok((*a $op *b).into())
            }
            [Value::Number(n)] => Ok((*a $op f32::from(n)).into()),
            unexpected => {
                type_error_with_slice("a Color or Number", unexpected)
            }
        }
    }
}

#[macro_export]
macro_rules! color_arithmetic_assign_op {
    ($op:tt) => {
        |a, b| match b {
            [Value::ExternalValue(b)] if b.has_data::<Color>() =>{
                let b: Color = *b.data::<Color>().unwrap();
                *a.data_mut::<Color>().unwrap() $op b;
                Ok(a.clone().into())
            }
            [Value::Number(n)] => {
                *a.data_mut::<Color>().unwrap() $op f32::from(n);
                Ok(a.clone().into())
            }
            unexpected => {
                type_error_with_slice("a Color or Number", unexpected)
            }
        }
    }
}

#[macro_export]
macro_rules! color_comparison_op {
    ($op:tt) => {
        |a, b| match b {
            [Value::ExternalValue(b)] if b.has_data::<Color>() =>{
                let b = b.data::<Color>().unwrap();
                Ok((*a $op *b).into())
            }
            unexpected => type_error_with_slice("a Color", unexpected),
        }
    }
}

fn make_color_meta_map() -> RcCell<MetaMap> {
    use {BinaryOp::*, UnaryOp::*, Value::*};

    MetaMapBuilder::<Color>::new("Color")
        .data_fn("red", |c| Ok(c.red().into()))
        .alias("red", "r")
        .data_fn("green", |c| Ok(c.green().into()))
        .alias("green", "g")
        .data_fn("blue", |c| Ok(c.blue().into()))
        .alias("blue", "b")
        .data_fn("alpha", |c| Ok(c.alpha().into()))
        .alias("alpha", "a")
        .value_fn("set_red", |c, args| match args {
            [Number(n)] => {
                c.data_mut::<Color>().unwrap().color.red = n.into();
                Ok(c.into())
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .alias("set_red", "set_r")
        .value_fn("set_green", |c, args| match args {
            [Number(n)] => {
                c.data_mut::<Color>().unwrap().color.green = n.into();
                Ok(c.into())
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .alias("set_green", "set_g")
        .value_fn("set_blue", |c, args| match args {
            [Number(n)] => {
                c.data_mut::<Color>().unwrap().color.blue = n.into();
                Ok(c.into())
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .alias("set_blue", "set_b")
        .value_fn("set_alpha", |c, args| match args {
            [Number(n)] => {
                c.data_mut::<Color>().unwrap().alpha = n.into();
                Ok(c.into())
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .alias("set_alpha", "set_a")
        .data_fn(Display, |c| Ok(c.to_string().into()))
        .data_fn_with_args(Add, color_arithmetic_op!(+))
        .data_fn_with_args(Subtract, color_arithmetic_op!(-))
        .data_fn_with_args(Multiply, color_arithmetic_op!(*))
        .data_fn_with_args(Divide, color_arithmetic_op!(/))
        .value_fn(AddAssign, color_arithmetic_assign_op!(+=))
        .value_fn(SubtractAssign, color_arithmetic_assign_op!(-=))
        .value_fn(MultiplyAssign, color_arithmetic_assign_op!(*=))
        .value_fn(DivideAssign, color_arithmetic_assign_op!(/=))
        .data_fn_with_args(Equal, color_comparison_op!(==))
        .data_fn_with_args(NotEqual, color_comparison_op!(!=))
        .data_fn_with_args(Index, |a, b| match b {
            [Number(n)] => match usize::from(n) {
                0 => Ok(a.red().into()),
                1 => Ok(a.green().into()),
                2 => Ok(a.blue().into()),
                3 => Ok(a.alpha().into()),
                other => runtime_error!("index out of range (got {other}, should be <= 3)"),
            },
            unexpected => type_error_with_slice("expected a Number", unexpected),
        })
        .data_fn(UnaryOp::Iterator, |c| {
            let c = *c;
            let iter = (0..=3).map(move |i| {
                let result = match i {
                    0 => c.red(),
                    1 => c.green(),
                    2 => c.blue(),
                    3 => c.alpha(),
                    _ => unreachable!(),
                };
                ValueIteratorOutput::Value(result.into())
            });
            Ok(ValueIterator::with_std_iter(iter).into())
        })
        .build()
}

thread_local! {
    static COLOR_META: RcCell<MetaMap> = make_color_meta_map();
    static TYPE_COLOR: ValueString = "Color".into();
}
