//! A Koto language module for working with geometry

#[macro_use]
mod macros;
mod rect;
mod vec2;
mod vec3;

pub use rect::Rect;
pub use vec2::Vec2;
pub use vec3::Vec3;

use koto_runtime::{derive::koto_fn, prelude::*};

pub fn make_module() -> KMap {
    koto_fn! {
        runtime = koto_runtime;

        fn rect() -> Rect {
            (0.0, 0.0, 0.0, 0.0).into()
        }

        fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
            (x, y, w, h).into()
        }

        fn rect(xy: &Vec2, size: &Vec2) -> Rect {
            (xy.inner().x, xy.inner().y, size.inner().x, size.inner().y).into()
        }

        fn vec2() -> Vec2 {
            (0.0, 0.0).into()
        }

        fn vec2(x: f64) -> Vec2 {
            (x, 0.0).into()
        }

        fn vec2(x: f64, y: f64) -> Vec2 {
            (x, y).into()
        }

        fn vec2(v: Vec2) -> Vec2 {
            v
        }

        fn vec3() -> Vec3 {
            (0.0, 0.0, 0.0).into()
        }

        fn vec3(x: f64) -> Vec3 {
            (x, 0.0, 0.0).into()
        }

        fn vec3(x: f64, y: f64) -> Vec3 {
            (x, y, 0.0).into()
        }

        fn vec3(x: f64, y: f64, z: f64) -> Vec3 {
            (x, y, z).into()
        }

        fn vec3(v: &Vec2) -> Vec3 {
            (v.inner().x, v.inner().y, 0.0).into()
        }

        fn vec3(v: &Vec2, z: f64) -> Vec3 {
            (v.inner().x, v.inner().y, z).into()
        }

        fn vec3(v: Vec3) -> Vec3 {
            v
        }
    }

    let result = KMap::with_type("geometry");

    result.add_fn("rect", rect);
    result.add_fn("vec2", vec2);
    result.add_fn("vec3", vec3);

    result
}
