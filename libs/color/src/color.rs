use derive_more::From;
use koto_runtime::{Result, derive::*, prelude::*};
use palette::FromColor;
use std::fmt;

macro_rules! get_component {
    ($self:ident, $component:ident, $(($variant:ident, $c:ident => $expr:expr)),+ $(,)?) => {{
        match &$self.color {
            $(
                Encoding::$variant($c) => Ok(($expr).into()),
            )+
            _ => $self.component_error(stringify!($component)),
        }
    }};
}

macro_rules! set_component {
    (
        $self:ident,
        $arg:ident, $component:ident,
        $(($variant:ident, $c:ident => $expr:expr)),+
        $(,)?
    ) => {{
        match $arg {
            KValue::Number($component) => {
                let component: f32 = $component.into();
                match &mut $self.color {
                    $(
                        Encoding::$variant($c) => {
                            $expr = component.into();
                            Ok(())
                        },
                    )+
                    _ => $self.component_error(stringify!($component)),
                }
            },
            unexpected => unexpected_type("a Number", unexpected),
        }
    }};
}

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(runtime = koto_runtime, use_copy)]
pub struct Color {
    pub color: Encoding,
    pub alpha: f32,
}

#[derive(Copy, Clone, From, PartialEq)]
pub enum Encoding {
    Srgb(palette::Srgb),
    Hsl(palette::Hsl),
    Hsv(palette::Hsv),
    Oklab(palette::Oklab),
    Oklch(palette::Oklch),
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

    pub fn get_component(&self, index: usize) -> Option<f32> {
        let result = match (&self.color, index) {
            (Encoding::Srgb(c), 0) => c.red,
            (Encoding::Srgb(c), 1) => c.green,
            (Encoding::Srgb(c), 2) => c.blue,
            (Encoding::Hsl(c), 0) => c.hue.into_inner(),
            (Encoding::Hsl(c), 1) => c.saturation,
            (Encoding::Hsl(c), 2) => c.lightness,
            (Encoding::Hsv(c), 0) => c.hue.into_inner(),
            (Encoding::Hsv(c), 1) => c.saturation,
            (Encoding::Hsv(c), 2) => c.value,
            (Encoding::Oklab(c), 0) => c.l,
            (Encoding::Oklab(c), 1) => c.a,
            (Encoding::Oklab(c), 2) => c.b,
            (Encoding::Oklch(c), 0) => c.l,
            (Encoding::Oklch(c), 1) => c.chroma,
            (Encoding::Oklch(c), 2) => c.hue.into_inner(),
            (_, 3) => self.alpha,
            _ => return None,
        };

        Some(result)
    }

    pub fn set_component(&mut self, index: usize, value: f32) -> Result<()> {
        match (&mut self.color, index) {
            (Encoding::Srgb(c), 0) => c.red = value,
            (Encoding::Srgb(c), 1) => c.green = value,
            (Encoding::Srgb(c), 2) => c.blue = value,
            (Encoding::Hsl(c), 0) => c.hue = value.into(),
            (Encoding::Hsl(c), 1) => c.saturation = value,
            (Encoding::Hsl(c), 2) => c.lightness = value,
            (Encoding::Hsv(c), 0) => c.hue = value.into(),
            (Encoding::Hsv(c), 1) => c.saturation = value,
            (Encoding::Hsv(c), 2) => c.value = value,
            (Encoding::Oklab(c), 0) => c.l = value,
            (Encoding::Oklab(c), 1) => c.a = value,
            (Encoding::Oklab(c), 2) => c.b = value,
            (Encoding::Oklch(c), 0) => c.l = value,
            (Encoding::Oklch(c), 1) => c.chroma = value,
            (Encoding::Oklch(c), 2) => c.hue = value.into(),
            (_, 3) => self.alpha = value,
            _ => return runtime_error!("invalid component index ({index})"),
        }

        Ok(())
    }

    pub fn color_space_str(&self) -> &str {
        match &self.color {
            Encoding::Srgb(_) => "RGB",
            Encoding::Hsl(_) => "HSL",
            Encoding::Hsv(_) => "HSV",
            Encoding::Oklab(_) => "Oklab",
            Encoding::Oklch(_) => "Oklch",
        }
    }

    #[koto_get(alias = "r")]
    pub fn red(&self) -> Result<KValue> {
        get_component!(self, red, (Srgb, c => c.red))
    }

    #[koto_set(alias = "r")]
    pub fn set_red(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, red, (Srgb, c => c.red))
    }

    #[koto_get(alias = "g")]
    pub fn green(&self) -> Result<KValue> {
        get_component!(self, green, (Srgb, c => c.green))
    }

    #[koto_set(alias = "g")]
    pub fn set_green(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, green, (Srgb, c => c.green))
    }

    #[koto_get]
    pub fn blue(&self) -> Result<KValue> {
        get_component!(self, blue, (Srgb, c => c.blue))
    }

    #[koto_set(alias = "b")]
    pub fn set_blue(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, blue, (Srgb, c => c.blue))
    }

    #[koto_get(alias = "h")]
    pub fn hue(&self) -> Result<KValue> {
        get_component!(self, hue,
            (Hsl, c => c.hue.into_inner()),
            (Hsv, c => c.hue.into_inner()),
            (Oklch, c => c.hue.into_inner()),
        )
    }

    #[koto_set(alias = "h")]
    pub fn set_hue(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, hue,
            (Hsl, c => c.hue),
            (Hsv, c => c.hue),
            (Oklch, c => c.hue),
        )
    }

    #[koto_get(alias = "s")]
    pub fn saturation(&self) -> Result<KValue> {
        get_component!(self, saturation,
            (Hsl, c => c.saturation),
            (Hsv, c => c.saturation),
        )
    }

    #[koto_set(alias = "s")]
    pub fn set_saturation(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, saturation,
            (Hsl, c => c.saturation),
            (Hsv, c => c.saturation),
        )
    }

    #[koto_get(alias = "l")]
    pub fn lightness(&self) -> Result<KValue> {
        get_component!(self, lightness,
            (Hsl, c => c.lightness),
            (Oklab, c => c.l),
            (Oklch, c => c.l),
        )
    }

    #[koto_set(alias = "l")]
    pub fn set_lightness(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, lightness,
            (Hsl, c => c.lightness),
            (Oklab, c => c.l),
            (Oklch, c => c.l),
        )
    }

    #[koto_get(alias = "v")]
    pub fn value(&self) -> Result<KValue> {
        get_component!(self, value, (Hsv, c => c.value))
    }

    #[koto_set(alias = "v")]
    pub fn set_value(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, value, (Hsv, c => c.value))
    }

    #[koto_get]
    pub fn a(&self) -> Result<KValue> {
        get_component!(self, a, (Oklab, c => c.a))
    }

    #[koto_set]
    pub fn set_a(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, a, (Oklab, c => c.a))
    }

    #[koto_get]
    pub fn b(&self) -> Result<KValue> {
        get_component!(self, b,
            (Srgb, c => c.blue),
            (Oklab, c => c.b),
        )
    }

    #[koto_set]
    pub fn set_b(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, b, (Oklab, c => c.b))
    }

    #[koto_get(alias = "c")]
    pub fn chroma(&self) -> Result<KValue> {
        get_component!(self, chroma, (Oklch, c => c.chroma))
    }

    #[koto_set(alias = "c")]
    pub fn set_chroma(&mut self, arg: &KValue) -> Result<()> {
        set_component!(self, arg, chroma, (Oklch, c => c.chroma))
    }

    #[koto_get]
    pub fn alpha(&self) -> KValue {
        self.alpha.into()
    }

    #[koto_set]
    pub fn set_alpha(&mut self, value: &KValue) -> Result<()> {
        match value {
            KValue::Number(alpha) => {
                self.alpha = alpha.into();
                Ok(())
            }
            unexpected => unexpected_type("a Number", unexpected),
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

        let mixed: Encoding = match (&a.color, &b.color) {
            (Encoding::Srgb(a), Encoding::Srgb(b)) => a.mix(*b, amount).into(),
            (Encoding::Hsl(a), Encoding::Hsl(b)) => a.mix(*b, amount).into(),
            (Encoding::Hsv(a), Encoding::Hsv(b)) => a.mix(*b, amount).into(),
            (Encoding::Oklab(a), Encoding::Oklab(b)) => a.mix(*b, amount).into(),
            (Encoding::Oklch(a), Encoding::Oklch(b)) => a.mix(*b, amount).into(),
            _ => {
                return runtime_error!(
                    "mix only works with matching color spaces (found {}, {})",
                    a.color_space_str(),
                    b.color_space_str()
                );
            }
        };

        let result = Self {
            color: mixed,
            alpha: (a.alpha + b.alpha) * 0.5,
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

    fn component_error<T>(&self, component: &str) -> Result<T> {
        runtime_error!(
            "The {} color space doesn’t define a {component} component",
            self.color_space_str()
        )
    }
}

impl KotoObject for Color {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn equal(&self, other: &KValue) -> Result<bool> {
        match other {
            KValue::Object(o) if o.is_a::<Self>() => {
                let other = o.cast::<Self>().unwrap();
                Ok(*self == *other)
            }
            unexpected => unexpected_type(Self::type_static(), unexpected),
        }
    }

    fn not_equal(&self, other: &KValue) -> Result<bool> {
        match other {
            KValue::Object(o) if o.is_a::<Self>() => {
                let other = o.cast::<Self>().unwrap();
                Ok(*self != *other)
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

    fn index_assign(&mut self, index: &KValue, value: &KValue) -> Result<()> {
        use KValue::Number;

        match (index, value) {
            (Number(index), Number(value)) => self.set_component(index.into(), value.into()),
            _ => unexpected_args("two Numbers", &[index.clone(), value.clone()]),
        }
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

        match &self.color {
            Encoding::Srgb(c) => {
                write!(
                    f,
                    "r: {}, g: {}, b: {}, a: {}",
                    c.red, c.green, c.blue, self.alpha
                )?;
            }
            Encoding::Hsl(c) => {
                write!(
                    f,
                    "h: {}, s: {}, l: {}, a: {}",
                    c.hue.into_inner(),
                    c.saturation,
                    c.lightness,
                    self.alpha
                )?;
            }
            Encoding::Hsv(c) => {
                write!(
                    f,
                    "h: {}, s: {}, v: {}, a: {}",
                    c.hue.into_inner(),
                    c.saturation,
                    c.value,
                    self.alpha
                )?;
            }
            Encoding::Oklab(c) => {
                write!(
                    f,
                    "l: {}, a: {}, b: {}, alpha: {}",
                    c.l, c.a, c.b, self.alpha
                )?;
            }
            Encoding::Oklch(c) => {
                write!(
                    f,
                    "l: {}, c: {}, h: {}, a: {}",
                    c.l,
                    c.chroma,
                    c.hue.into_inner(),
                    self.alpha
                )?;
            }
        }

        write!(f, ")")
    }
}

impl From<Encoding> for Color {
    fn from(color: Encoding) -> Self {
        Self { color, alpha: 1.0 }
    }
}

impl From<palette::Srgb<u8>> for Color {
    fn from(c: palette::Srgb<u8>) -> Self {
        Encoding::from(palette::Srgb::new(
            c.red as f32 / 255.0,
            c.green as f32 / 255.0,
            c.blue as f32 / 255.0,
        ))
        .into()
    }
}

impl From<palette::Srgba> for Color {
    fn from(c: palette::Srgba) -> Self {
        Self {
            color: Encoding::from(c.color),
            alpha: c.alpha,
        }
    }
}

impl From<palette::Hsla> for Color {
    fn from(c: palette::Hsla) -> Self {
        Self {
            color: Encoding::from(c.color),
            alpha: c.alpha,
        }
    }
}

impl From<palette::Hsva> for Color {
    fn from(c: palette::Hsva) -> Self {
        Self {
            color: Encoding::from(c.color),
            alpha: c.alpha,
        }
    }
}

impl From<palette::Oklaba> for Color {
    fn from(c: palette::Oklaba) -> Self {
        Self {
            color: Encoding::from(c.color),
            alpha: c.alpha,
        }
    }
}

impl From<palette::Oklcha> for Color {
    fn from(c: palette::Oklcha) -> Self {
        Self {
            color: Encoding::from(c.color),
            alpha: c.alpha,
        }
    }
}

impl From<Color> for palette::Srgba {
    fn from(color: Color) -> Self {
        let inner = match color.color {
            Encoding::Srgb(c) => c,
            Encoding::Hsl(c) => palette::Srgb::from_color(c),
            Encoding::Hsv(c) => palette::Srgb::from_color(c),
            Encoding::Oklab(c) => palette::Srgb::from_color(c),
            Encoding::Oklch(c) => palette::Srgb::from_color(c),
        };
        Self {
            color: inner,
            alpha: color.alpha,
        }
    }
}

impl From<Color> for palette::Hsla {
    fn from(color: Color) -> Self {
        let inner = match color.color {
            Encoding::Srgb(c) => palette::Hsl::from_color(c),
            Encoding::Hsl(c) => c,
            Encoding::Hsv(c) => palette::Hsl::from_color(c),
            Encoding::Oklab(c) => palette::Hsl::from_color(c),
            Encoding::Oklch(c) => palette::Hsl::from_color(c),
        };

        Self {
            color: inner,
            alpha: color.alpha,
        }
    }
}

impl From<Color> for palette::Hsva {
    fn from(color: Color) -> Self {
        let inner = match color.color {
            Encoding::Srgb(c) => palette::Hsv::from_color(c),
            Encoding::Hsl(c) => palette::Hsv::from_color(c),
            Encoding::Hsv(c) => c,
            Encoding::Oklab(c) => palette::Hsv::from_color(c),
            Encoding::Oklch(c) => palette::Hsv::from_color(c),
        };

        Self {
            color: inner,
            alpha: color.alpha,
        }
    }
}

impl From<Color> for palette::Oklaba {
    fn from(color: Color) -> Self {
        let inner = match color.color {
            Encoding::Srgb(c) => palette::Oklab::from_color(c),
            Encoding::Hsl(c) => palette::Oklab::from_color(c),
            Encoding::Hsv(c) => palette::Oklab::from_color(c),
            Encoding::Oklab(c) => c,
            Encoding::Oklch(c) => palette::Oklab::from_color(c),
        };

        Self {
            color: inner,
            alpha: color.alpha,
        }
    }
}

impl From<Color> for palette::Oklcha {
    fn from(color: Color) -> Self {
        let inner = match color.color {
            Encoding::Srgb(c) => palette::Oklch::from_color(c),
            Encoding::Hsl(c) => palette::Oklch::from_color(c),
            Encoding::Hsv(c) => palette::Oklch::from_color(c),
            Encoding::Oklab(c) => palette::Oklch::from_color(c),
            Encoding::Oklch(c) => c,
        };

        Self {
            color: inner,
            alpha: color.alpha,
        }
    }
}

impl From<Color> for KValue {
    fn from(color: Color) -> Self {
        KObject::from(color).into()
    }
}
