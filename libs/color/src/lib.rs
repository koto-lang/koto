//! A Koto language module for working with colors

mod color;

pub use color::Color;

use koto_runtime::{prelude::*, Result};
use palette::{Hsl, Hsv};

pub fn make_module() -> KMap {
    use KValue::{Number, Str};
    let mut result = KMap::default();

    result.add_fn("hsl", |ctx| match ctx.args() {
        [Number(h), Number(s), Number(l)] => {
            let hsv = Hsl::new(f32::from(h), f32::from(s), f32::from(l));
            Ok(Color::from(hsv).into())
        }
        unexpected => unexpected_args("|Number, Number, Number|", unexpected),
    });

    result.add_fn("hsv", |ctx| match ctx.args() {
        [Number(h), Number(s), Number(v)] => {
            let hsv = Hsv::new(f32::from(h), f32::from(s), f32::from(v));
            Ok(Color::from(hsv).into())
        }
        unexpected => unexpected_args("|Number, Number, Number|", unexpected),
    });

    result.add_fn("named", |ctx| match ctx.args() {
        [Str(s)] => named(s),
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("rgb", |ctx| match ctx.args() {
        [Number(r), Number(g), Number(b)] => rgb(r, g, b),
        unexpected => unexpected_args("|Number, Number, Number|", unexpected),
    });

    result.add_fn("rgba", |ctx| match ctx.args() {
        [Number(r), Number(g), Number(b), Number(a)] => rgba(r, g, b, a),
        unexpected => unexpected_args("|Number, Number, Number, Number|", unexpected),
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
