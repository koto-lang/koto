use derive_more::From;
use koto_runtime::{derive::*, prelude::*, Result};
use palette::FromColor;
use std::fmt;

#[macro_export]
macro_rules! color_comparison_op {
    ($self:ident, $rhs:expr, $op:tt) => {{}};
}

#[derive(Copy, Clone, From, PartialEq, KotoCopy, KotoType)]
#[koto(use_copy)]
pub enum Color {
    Srgb(palette::Srgba),
    Hsl(palette::Hsla),
    Hsv(palette::Hsva),
    Oklab(palette::Oklaba),
    Oklch(palette::Oklcha),
}

#[koto_impl(runtime = koto_runtime)]
impl Color {
    pub fn named(name: &str) -> Option<Self> {
        palette::named::from_str(name).map(Self::from)
    }

    pub fn hex_int(n: u32) -> Self {
        palette::Srgb::from(n).into()
    }

    pub fn hex_str(s: &str) -> Option<Self> {
        s.parse::<palette::Srgb<u8>>().ok().map(Self::from)
    }

    pub fn get_component(&self, n: usize) -> Option<f32> {
        use Color::*;

        let result = match (self, n) {
            (Srgb(c), 0) => c.color.red,
            (Srgb(c), 1) => c.color.green,
            (Srgb(c), 2) => c.color.blue,
            (Srgb(c), 3) => c.alpha,
            (Hsl(c), 0) => c.color.hue.into_inner(),
            (Hsl(c), 1) => c.color.saturation,
            (Hsl(c), 2) => c.color.lightness,
            (Hsl(c), 3) => c.alpha,
            (Hsv(c), 0) => c.color.hue.into_inner(),
            (Hsv(c), 1) => c.color.saturation,
            (Hsv(c), 2) => c.color.value,
            (Hsv(c), 3) => c.alpha,
            (Oklab(c), 0) => c.color.l,
            (Oklab(c), 1) => c.color.a,
            (Oklab(c), 2) => c.color.b,
            (Oklab(c), 3) => c.alpha,
            (Oklch(c), 0) => c.color.l,
            (Oklch(c), 1) => c.color.chroma,
            (Oklch(c), 2) => c.color.hue.into_inner(),
            (Oklch(c), 3) => c.alpha,
            _ => return None,
        };

        Some(result)
    }

    pub fn color_space_str(&self) -> &str {
        use Color::*;

        match self {
            Srgb(_) => "RGB",
            Hsl(_) => "HSL",
            Hsv(_) => "HSV",
            Oklab(_) => "Oklab",
            Oklch(_) => "Oklch",
        }
    }

    #[koto_method]
    pub fn color_space(&self) -> KValue {
        self.color_space_str().into()
    }

    #[koto_method]
    pub fn mix(ctx: MethodContext<Self>) -> Result<KValue> {
        let (a, b, amount) = match ctx.args {
            [KValue::Object(b)] if b.is_a::<Color>() => {
                (*ctx.instance()?, *b.cast::<Color>()?, 0.5)
            }
            [KValue::Object(b), KValue::Number(x)] if b.is_a::<Color>() => {
                (*ctx.instance()?, *b.cast::<Color>()?, f32::from(x))
            }
            unexpected => return unexpected_args("|Color|, or |Color, Number|", unexpected),
        };

        use palette::Mix;
        use Color::*;

        let result: Color = match (a, b) {
            (Srgb(a), Srgb(b)) => a.mix(b, amount).into(),
            (Hsl(a), Hsl(b)) => a.mix(b, amount).into(),
            (Hsv(a), Hsv(b)) => a.mix(b, amount).into(),
            (Oklab(a), Oklab(b)) => a.mix(b, amount).into(),
            (Oklch(a), Oklch(b)) => a.mix(b, amount).into(),
            _ => {
                return runtime_error!(
                    "mix only works with matching color spaces (found {}, {})",
                    a.color_space_str(),
                    b.color_space_str()
                )
            }
        };

        Ok(result.into())
    }

    #[koto_method]
    pub fn to_rgb(&self) -> KValue {
        Self::from(palette::Srgba::from(*self)).into()
    }

    #[koto_method]
    pub fn to_hsl(&self) -> KValue {
        Self::from(palette::Hsla::from(*self)).into()
    }

    #[koto_method]
    pub fn to_hsv(&self) -> KValue {
        Self::from(palette::Hsva::from(*self)).into()
    }

    #[koto_method]
    pub fn to_oklab(&self) -> KValue {
        Self::from(palette::Oklaba::from(*self)).into()
    }

    #[koto_method]
    pub fn to_oklch(&self) -> KValue {
        Self::from(palette::Oklcha::from(*self)).into()
    }
}

impl KotoObject for Color {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn equal(&self, rhs: &KValue) -> Result<bool> {
        match rhs {
            KValue::Object(rhs) if rhs.is_a::<Self>() => {
                let rhs = rhs.cast::<Self>().unwrap();
                Ok(*self == *rhs)
            }
            unexpected => unexpected_type(Self::type_static(), unexpected),
        }
    }

    fn not_equal(&self, rhs: &KValue) -> Result<bool> {
        match rhs {
            KValue::Object(rhs) if rhs.is_a::<Self>() => {
                let rhs = rhs.cast::<Self>().unwrap();
                Ok(*self != *rhs)
            }
            unexpected => unexpected_type(Self::type_static(), unexpected),
        }
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        match index {
            KValue::Number(n) => match self.get_component(n.into()) {
                Some(result) => Ok(result.into()),
                None => runtime_error!("index out of range ({n}, should be <= 3)"),
            },
            unexpected => unexpected_type("Number", unexpected),
        }
    }

    fn size(&self) -> Option<usize> {
        // All current color spaces have 4 components
        Some(4)
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        let c = *self;

        let iter = (0..=3).map(move |i| match c.get_component(i) {
            Some(component) => KIteratorOutput::Value(component.into()),
            None => unreachable!(), // All color variants have 4 components
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Color({}, ", self.color_space_str())?;

        match self {
            Color::Srgb(c) => {
                write!(
                    f,
                    "r: {}, g: {}, b: {}, a: {}",
                    c.color.red, c.color.green, c.color.blue, c.alpha
                )?;
            }
            Color::Hsl(c) => {
                write!(
                    f,
                    "h: {}, s: {}, l: {}, a: {}",
                    c.color.hue.into_inner(),
                    c.color.saturation,
                    c.color.lightness,
                    c.alpha
                )?;
            }
            Color::Hsv(c) => {
                write!(
                    f,
                    "h: {}, s: {}, v: {}, a: {}",
                    c.color.hue.into_inner(),
                    c.color.saturation,
                    c.color.value,
                    c.alpha
                )?;
            }
            Color::Oklab(c) => {
                write!(
                    f,
                    "l: {}, a: {}, b: {}, a: {}",
                    c.color.l, c.color.a, c.color.b, c.alpha
                )?;
            }
            Color::Oklch(c) => {
                write!(
                    f,
                    "l: {}, c: {}, h: {}, a: {}",
                    c.color.l,
                    c.color.chroma,
                    c.color.hue.into_inner(),
                    c.alpha
                )?;
            }
        }

        write!(f, ")")
    }
}

impl From<palette::Srgb<u8>> for Color {
    fn from(c: palette::Srgb<u8>) -> Self {
        palette::Srgba::new(
            c.red as f32 / 255.0,
            c.green as f32 / 255.0,
            c.blue as f32 / 255.0,
            1.0,
        )
        .into()
    }
}

impl From<Color> for KValue {
    fn from(color: Color) -> Self {
        KObject::from(color).into()
    }
}

impl From<Color> for palette::Srgba {
    fn from(color: Color) -> Self {
        use Color::*;

        match color {
            Srgb(c) => c,
            Hsl(c) => Self::from_color(c),
            Hsv(c) => Self::from_color(c),
            Oklab(c) => Self::from_color(c),
            Oklch(c) => Self::from_color(c),
        }
    }
}

impl From<Color> for palette::Hsla {
    fn from(color: Color) -> Self {
        use Color::*;

        match color {
            Srgb(c) => Self::from_color(c),
            Hsl(c) => c,
            Hsv(c) => Self::from_color(c),
            Oklab(c) => Self::from_color(c),
            Oklch(c) => Self::from_color(c),
        }
    }
}

impl From<Color> for palette::Hsva {
    fn from(color: Color) -> Self {
        use Color::*;

        match color {
            Srgb(c) => Self::from_color(c),
            Hsl(c) => Self::from_color(c),
            Hsv(c) => c,
            Oklab(c) => Self::from_color(c),
            Oklch(c) => Self::from_color(c),
        }
    }
}

impl From<Color> for palette::Oklaba {
    fn from(color: Color) -> Self {
        use Color::*;

        match color {
            Srgb(c) => Self::from_color(c),
            Hsl(c) => Self::from_color(c),
            Hsv(c) => Self::from_color(c),
            Oklab(c) => c,
            Oklch(c) => Self::from_color(c),
        }
    }
}

impl From<Color> for palette::Oklcha {
    fn from(color: Color) -> Self {
        use Color::*;

        match color {
            Srgb(c) => Self::from_color(c),
            Hsl(c) => Self::from_color(c),
            Hsv(c) => Self::from_color(c),
            Oklab(c) => Self::from_color(c),
            Oklch(c) => c,
        }
    }
}
