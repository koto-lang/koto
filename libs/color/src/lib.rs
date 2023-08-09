//! A Koto language module for working with colors

mod color;

pub use color::Color;

use koto_runtime::{prelude::*, Result};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("named", |vm, args| match vm.get_args(args) {
        [Str(s)] => named(s),
        unexpected => type_error_with_slice("a String", unexpected),
    });

    result.add_fn("rgb", |vm, args| match vm.get_args(args) {
        [Number(r), Number(g), Number(b)] => rgb(r, g, b),
        unexpected => type_error_with_slice("3 Numbers", unexpected),
    });

    result.add_fn("rgba", |vm, args| match vm.get_args(args) {
        [Number(r), Number(g), Number(b), Number(a)] => rgba(r, g, b, a),
        unexpected => type_error_with_slice("4 Numbers", unexpected),
    });

    let mut meta = MetaMap::default();

    meta.add_fn(MetaKey::Call, |vm, args| match vm.get_args(args) {
        [Str(s)] => named(s),
        [Number(r), Number(g), Number(b)] => rgb(r, g, b),
        [Number(r), Number(g), Number(b), Number(a)] => rgba(r, g, b, a),
        unexpected => type_error_with_slice("a String", unexpected),
    });

    result.set_meta_map(Some(meta));
    result
}

fn named(name: &str) -> Result<Value> {
    match Color::named(name) {
        Some(c) => Ok(c.into()),
        None => Ok(Value::Null),
    }
}

fn rgb(r: &ValueNumber, g: &ValueNumber, b: &ValueNumber) -> Result<Value> {
    Ok(Color::rgb(r.into(), g.into(), b.into()).into())
}

fn rgba(r: &ValueNumber, g: &ValueNumber, b: &ValueNumber, a: &ValueNumber) -> Result<Value> {
    Ok(Color::rgba(r.into(), g.into(), b.into(), a.into()).into())
}
