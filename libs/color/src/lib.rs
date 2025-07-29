//! A Koto language module for working with colors

mod color;
pub use color::{Color, Encoding};

use koto_runtime::{derive::koto_fn, prelude::*};
use palette::{Hsl, Hsla, Hsv, Hsva, Oklab, Oklaba, Oklch, Oklcha, Srgb, Srgba};

pub fn make_module() -> KMap {
    let mut result = KMap::default();

    macro_rules! color_init_fn {
        ($name:ident, $type3:path, $type4:path) => {{
            use $type3 as ColorType3;
            use $type4 as ColorType4;

            koto_fn! {
                runtime = koto_runtime;

                fn $name(c1: f32, c2: f32, c3: f32) -> Color {
                    ColorType4::from(ColorType3::new(c1, c2, c3)).into()
                }

                fn $name(c1: f32, c2: f32, c3: f32, c4: f32) -> Color {
                    ColorType4::new(c1, c2, c3, c4).into()
                }
            }

            result.add_fn(stringify!($name), $name);
        }};
    }

    koto_fn! {
        runtime = koto_runtime;

        fn hex(s: &str) -> KValue {
            from_hex_str(s)
        }

        fn hex(n: &KNumber) -> KValue {
            from_hex_number(n)
        }


        fn named(s: &str) -> KValue {
            match Color::named(s) {
                Some(c) => c.into(),
                None => KValue::Null,
            }
        }

        fn meta_call(s: &str) -> KValue {
            match Color::named(s) {
                Some(result) => result.into(),
                None => from_hex_str(s),
            }
        }

        fn meta_call(n: &KNumber) -> KValue {
            from_hex_number(n)
        }

        fn meta_call(r: f32, g: f32, b: f32) -> Color {
            Srgba::new(r, g, b, 1.0).into()
        }

        fn meta_call(r: f32, g: f32, b: f32, a: f32) -> Color {
            Srgba::new(r, g, b, a).into()
        }
    }

    result.add_fn("hex", hex);

    color_init_fn!(hsl, Hsl, Hsla);
    color_init_fn!(hsv, Hsv, Hsva);

    result.add_fn("named", named);

    color_init_fn!(oklab, Oklab, Oklaba);
    color_init_fn!(oklch, Oklch, Oklcha);
    color_init_fn!(rgb, Srgb, Srgba);

    // Allow users to simply call `color` for basic color initializers
    let mut meta = MetaMap::default();
    meta.insert(MetaKey::Type, "color".into());
    meta.add_fn(MetaKey::Call, meta_call);

    result.set_meta_map(Some(meta.into()));
    result
}

fn from_hex_str(s: &str) -> KValue {
    match Color::hex_str(s) {
        Some(c) => c.into(),
        None => KValue::Null,
    }
}

fn from_hex_number(n: &KNumber) -> KValue {
    if *n < 0.0 || *n > 2_u32.pow(24) {
        KValue::Null
    } else {
        Color::hex_int(n.into()).into()
    }
}
