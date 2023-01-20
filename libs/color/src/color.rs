use {
    koto_runtime::prelude::*,
    std::{
        cell::RefCell,
        fmt,
        ops::{self, Deref, DerefMut},
        rc::Rc,
    },
};

type Inner = palette::rgb::LinSrgba;

#[derive(Copy, Clone, PartialEq)]
pub struct Color(Inner);

impl Color {
    pub fn from_r_g_b(r: f32, g: f32, b: f32) -> Self {
        Self(Inner::new(r, b, g, 1.0))
    }

    pub fn from_r_g_b_a(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(Inner::new(r, b, g, a))
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

    pub fn r(&self) -> f32 {
        self.color.red
    }

    pub fn g(&self) -> f32 {
        self.color.green
    }

    pub fn b(&self) -> f32 {
        self.color.blue
    }

    pub fn a(&self) -> f32 {
        self.alpha
    }
}

impl ExternalData for Color {
    fn data_type(&self) -> ValueString {
        TYPE_COLOR.with(|x| x.clone())
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
        Self::from_r_g_b_a(r, g, b, a)
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
            self.r(),
            self.g(),
            self.b(),
            self.a()
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
                let b = b.data::<Color>().unwrap();
                *a $op *b;
                Ok(Value::Null)
            }
            [Value::Number(n)] => {
                *a $op f32::from(n);
                Ok(Value::Null)
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

fn make_color_meta_map() -> Rc<RefCell<MetaMap>> {
    use {BinaryOp::*, UnaryOp::*, Value::*};

    MetaMapBuilder::<Color>::new("Color")
        .data_fn("r", |c| Ok(c.r().into()))
        .data_fn("g", |c| Ok(c.g().into()))
        .data_fn("b", |c| Ok(c.b().into()))
        .data_fn("a", |c| Ok(c.a().into()))
        .data_fn_with_args_mut("set_r", |c, args| match args {
            [Number(n)] => {
                c.color.red = n.into();
                Ok(Null)
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .data_fn_with_args_mut("set_g", |c, args| match args {
            [Number(n)] => {
                c.color.green = n.into();
                Ok(Null)
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .data_fn_with_args_mut("set_b", |c, args| match args {
            [Number(n)] => {
                c.color.blue = n.into();
                Ok(Null)
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .data_fn_with_args_mut("set_a", |c, args| match args {
            [Number(n)] => {
                c.alpha = n.into();
                Ok(Null)
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .data_fn(Display, |x| Ok(x.to_string().into()))
        .data_fn_with_args(Add, color_arithmetic_op!(+))
        .data_fn_with_args(Subtract, color_arithmetic_op!(-))
        .data_fn_with_args(Multiply, color_arithmetic_op!(*))
        .data_fn_with_args(Divide, color_arithmetic_op!(/))
        .data_fn_with_args_mut(AddAssign, color_arithmetic_assign_op!(+=))
        .data_fn_with_args_mut(SubtractAssign, color_arithmetic_assign_op!(-=))
        .data_fn_with_args_mut(MultiplyAssign, color_arithmetic_assign_op!(*=))
        .data_fn_with_args_mut(DivideAssign, color_arithmetic_assign_op!(/=))
        .data_fn_with_args(Equal, color_comparison_op!(==))
        .data_fn_with_args(NotEqual, color_comparison_op!(!=))
        .build()
}

thread_local! {
    static COLOR_META: Rc<RefCell<MetaMap>> = make_color_meta_map();
    static TYPE_COLOR: ValueString = "Color".into();
}
