//! A Koto language module for working with colors

mod color;

pub use color::Color;

use koto_runtime::{prelude::*, Result};

pub fn make_module() -> KMap {
    use KValue::{Number, Str};
    let mut result = KMap::default();

    macro_rules! color_init_fn {
        ($name:expr, $type3:path, $type4:path) => {
            result.add_fn($name, |ctx| match ctx.args() {
                [Number(c1), Number(c2), Number(c3)] => {
                    use $type3 as ColorType;
                    let result = ColorType::new(f32::from(c1), f32::from(c2), f32::from(c3));
                    Ok(Color::from(result).into())
                }
                [Number(c1), Number(c2), Number(c3), Number(c4)] => {
                    use $type4 as ColorType;
                    let result =
                        ColorType::new(f32::from(c1), f32::from(c2), f32::from(c3), f32::from(c4));
                    Ok(Color::from(result).into())
                }
                unexpected => unexpected_args("|Number, Number, Number|", unexpected),
            });
        };
    }

    color_init_fn!("hsl", palette::Hsl, palette::Hsla);
    color_init_fn!("hsv", palette::Hsv, palette::Hsva);

    result.add_fn("named", |ctx| match ctx.args() {
        [Str(s)] => named(s),
        unexpected => unexpected_args("|String|", unexpected),
    });

    color_init_fn!("okhsl", palette::Okhsl, palette::Okhsla);
    color_init_fn!("oklab", palette::Oklab, palette::Oklaba);
    color_init_fn!("oklch", palette::Oklch, palette::Oklcha);

    result.add_fn("rgb", |ctx| match ctx.args() {
        [Number(r), Number(g), Number(b)] => rgb(r, g, b),
        unexpected => unexpected_args("|Number, Number, Number|", unexpected),
    });

    let mut meta = MetaMap::default();

    meta.insert(MetaKey::Type, "color".into());
    meta.add_fn(MetaKey::Call, |ctx| match ctx.args() {
        [Str(s)] => named(s),
        [Number(r), Number(g), Number(b)] => rgb(r, g, b),
        [Number(r), Number(g), Number(b), Number(a)] => rgba(r, g, b, a),
        unexpected => unexpected_args(
            "|String|, or |Number, Number, Number|, or |Number, Number, Number, Number|",
            unexpected,
        ),
    });

    result.set_meta_map(Some(meta.into()));
    result
}

fn named(name: &str) -> Result<KValue> {
    match Color::named(name) {
        Some(c) => Ok(c.into()),
        None => Ok(KValue::Null),
    }
}

fn rgb(r: &KNumber, g: &KNumber, b: &KNumber) -> Result<KValue> {
    Ok(Color::rgb(r.into(), g.into(), b.into()).into())
}

fn rgba(r: &KNumber, g: &KNumber, b: &KNumber, a: &KNumber) -> Result<KValue> {
    Ok(Color::rgba(r.into(), g.into(), b.into(), a.into()).into())
}
