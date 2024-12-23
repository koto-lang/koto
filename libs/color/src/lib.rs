//! A Koto language module for working with colors

mod color;
pub use color::{Color, Encoding};

use koto_runtime::{prelude::*, Result};
use palette::{Hsl, Hsla, Hsv, Hsva, Oklab, Oklaba, Oklch, Oklcha, Srgb, Srgba};

pub fn make_module() -> KMap {
    use KValue::{Number, Str};

    let mut result = KMap::default();

    macro_rules! color_init_fn {
        ($name:expr, $type3:path, $type4:path) => {
            result.add_fn($name, |ctx| {
                use $type3 as ColorType3;
                use $type4 as ColorType4;
                match ctx.args() {
                    [Number(c1), Number(c2), Number(c3)] => {
                        let result = ColorType3::new(f32::from(c1), f32::from(c2), f32::from(c3));
                        Ok(Color::from(ColorType4::from(result)).into())
                    }
                    [Number(c1), Number(c2), Number(c3), Number(c4)] => {
                        let result = ColorType4::new(
                            f32::from(c1),
                            f32::from(c2),
                            f32::from(c3),
                            f32::from(c4),
                        );
                        Ok(Color::from(result).into())
                    }
                    unexpected => unexpected_args("|Number, Number, Number|", unexpected),
                }
            });
        };
    }

    result.add_fn("hex", |ctx| match ctx.args() {
        [Str(s)] => from_hex_str(s),
        [Number(n)] => from_hex_number(n),
        unexpected => unexpected_args("|String|, or |Number|", unexpected),
    });

    color_init_fn!("hsl", Hsl, Hsla);
    color_init_fn!("hsv", Hsv, Hsva);

    result.add_fn("named", |ctx| match ctx.args() {
        [Str(s)] => match Color::named(s) {
            Some(c) => Ok(c.into()),
            None => Ok(KValue::Null),
        },
        unexpected => unexpected_args("|String|", unexpected),
    });

    color_init_fn!("oklab", Oklab, Oklaba);
    color_init_fn!("oklch", Oklch, Oklcha);
    color_init_fn!("rgb", Srgb, Srgba);

    let mut meta = MetaMap::default();

    meta.insert(MetaKey::Type, "color".into());
    meta.add_fn(MetaKey::Call, |ctx| match ctx.args() {
        [Str(s)] => match Color::named(s) {
            Some(result) => Ok(result.into()),
            None => from_hex_str(s),
        },
        [Number(n)] => from_hex_number(n),
        [Number(r), Number(g), Number(b)] => {
            Ok(Color::from(Srgba::new(r.into(), g.into(), b.into(), 1.0)).into())
        }
        [Number(r), Number(g), Number(b), Number(a)] => {
            Ok(Color::from(Srgba::new(r.into(), g.into(), b.into(), a.into())).into())
        }
        unexpected => unexpected_args(
            "|String|, |Number|, |Number, Number, Number|, or |Number, Number, Number, Number|",
            unexpected,
        ),
    });

    result.set_meta_map(Some(meta.into()));
    result
}

fn from_hex_str(s: &str) -> Result<KValue> {
    match Color::hex_str(s) {
        Some(c) => Ok(c.into()),
        None => Ok(KValue::Null),
    }
}

fn from_hex_number(n: &KNumber) -> Result<KValue> {
    if *n < 0.0 || *n > 2_u32.pow(24) {
        Ok(KValue::Null)
    } else {
        Ok(Color::hex_int(n.into()).into())
    }
}
