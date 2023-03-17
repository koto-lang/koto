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

    fn make_copy(&self) -> PtrMut<dyn ExternalData> {
        make_data_ptr(*self)
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
        External::with_shared_meta_map(color, meta).into()
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
        |context| match context.args {
            [Value::External(b)] if b.has_data::<Color>() =>{
                let b = *b.data::<Color>().unwrap();
                Ok((*context.data()? $op b).into())
            }
            [Value::Number(n)] => Ok((*context.data()? $op f32::from(n)).into()),
            unexpected => {
                type_error_with_slice("a Color or Number", unexpected)
            }
        }
    }
}

#[macro_export]
macro_rules! color_arithmetic_assign_op {
    ($op:tt) => {
        |context| match context.args {
            [Value::External(b)] if b.has_data::<Color>() =>{
                let b: Color = *b.data::<Color>().unwrap();
                *context.data_mut()? $op b;
                context.ok_value()
            }
            [Value::Number(n)] => {
                *context.data_mut()? $op f32::from(n);
                context.ok_value()
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
        |context| match context.args {
            [Value::External(b)] if b.has_data::<Color>() =>{
                let b = *b.data::<Color>().unwrap();
                Ok((*context.data()? $op b).into())
            }
            unexpected => type_error_with_slice("a Color", unexpected),
        }
    }
}

fn make_color_meta_map() -> PtrMut<MetaMap> {
    use {BinaryOp::*, UnaryOp::*, Value::*};

    MetaMapBuilder::<Color>::new("Color")
        .function_aliased(&["red", "r"], |context| Ok(context.data()?.red().into()))
        .function_aliased(&["green", "g"], |context| {
            Ok(context.data()?.green().into())
        })
        .function_aliased(&["blue", "b"], |context| Ok(context.data()?.blue().into()))
        .function_aliased(&["alpha", "a"], |context| {
            Ok(context.data()?.alpha().into())
        })
        .function_aliased(&["set_red", "set_r"], |context| match context.args {
            [Number(n)] => {
                context.data_mut()?.color.red = n.into();
                context.ok_value()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .function_aliased(&["set_green", "set_g"], |context| match context.args {
            [Number(n)] => {
                context.data_mut()?.color.green = n.into();
                context.ok_value()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .function_aliased(&["set_blue", "set_b"], |context| match context.args {
            [Number(n)] => {
                context.data_mut()?.color.blue = n.into();
                context.ok_value()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .function_aliased(&["set_alpha", "set_a"], |context| match context.args {
            [Number(n)] => {
                context.data_mut()?.alpha = n.into();
                context.ok_value()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        })
        .function(Display, |context| Ok(context.data()?.to_string().into()))
        .function(Add, color_arithmetic_op!(+))
        .function(Subtract, color_arithmetic_op!(-))
        .function(Multiply, color_arithmetic_op!(*))
        .function(Divide, color_arithmetic_op!(/))
        .function(AddAssign, color_arithmetic_assign_op!(+=))
        .function(SubtractAssign, color_arithmetic_assign_op!(-=))
        .function(MultiplyAssign, color_arithmetic_assign_op!(*=))
        .function(DivideAssign, color_arithmetic_assign_op!(/=))
        .function(Equal, color_comparison_op!(==))
        .function(NotEqual, color_comparison_op!(!=))
        .function(Index, |context| match context.args {
            [Number(n)] => {
                let c = context.data()?;
                match usize::from(n) {
                    0 => Ok(c.red().into()),
                    1 => Ok(c.green().into()),
                    2 => Ok(c.blue().into()),
                    3 => Ok(c.alpha().into()),
                    other => runtime_error!("index out of range (got {other}, should be <= 3)"),
                }
            }
            unexpected => type_error_with_slice("expected a Number", unexpected),
        })
        .function(UnaryOp::Iterator, |context| {
            let c = *context.data()?;
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
    static COLOR_META: PtrMut<MetaMap> = make_color_meta_map();
    static TYPE_COLOR: ValueString = "Color".into();
}
