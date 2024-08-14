//! A Koto language module for working with geometry

#[macro_use]
mod macros;
mod rect;
mod vec2;
mod vec3;

pub use rect::Rect;
pub use vec2::Vec2;
pub use vec3::Vec3;

use koto_runtime::prelude::*;

pub fn make_module() -> KMap {
    use KValue::{Number, Object};

    let result = KMap::with_type("geometry");

    result.add_fn("rect", |ctx| {
        let (x, y, width, height) = match ctx.args() {
            [] => (0.0, 0.0, 0.0, 0.0),
            [Number(x), Number(y), Number(width), Number(height)] => {
                (x.into(), y.into(), width.into(), height.into())
            }
            [Object(xy), Object(size)] if xy.is_a::<Vec2>() && size.is_a::<Vec2>() => {
                let xy = xy.cast::<Vec2>().unwrap().inner();
                let size = size.cast::<Vec2>().unwrap().inner();
                (xy.x, xy.y, size.x, size.y)
            }
            unexpected => {
                return unexpected_args(
                    "||, or |Vec2, Vec2|, or |Number, Number, Number, Number|",
                    unexpected,
                )
            }
        };

        Ok(Rect::from_x_y_w_h(x, y, width, height).into())
    });

    result.add_fn("vec2", |ctx| {
        let (x, y) = match ctx.args() {
            [] => (0.0, 0.0),
            [Number(x)] => (x.into(), 0.0),
            [Number(x), Number(y)] => (x.into(), y.into()),
            [Object(vec2)] if vec2.is_a::<Vec2>() => {
                return Ok((*vec2.cast::<Vec2>().unwrap()).into())
            }
            unexpected => {
                return unexpected_args(
                    "||, or |Number|, or |Number, Number|, or |Vec2|",
                    unexpected,
                )
            }
        };

        Ok(Vec2::new(x, y).into())
    });

    result.add_fn("vec3", |ctx| {
        let (x, y, z) = match ctx.args() {
            [] => (0.0, 0.0, 0.0),
            [Number(x)] => (x.into(), 0.0, 0.0),
            [Number(x), Number(y)] => (x.into(), y.into(), 0.0),
            [Number(x), Number(y), Number(z)] => (x.into(), y.into(), z.into()),
            [Object(v)] if v.is_a::<Vec2>() => {
                let xy = v.cast::<Vec2>().unwrap();
                (xy.inner().x, xy.inner().y, 0.0)
            }
            [Object(v), Number(z)] if v.is_a::<Vec2>() => {
                let xy = v.cast::<Vec2>().unwrap();
                (xy.inner().x, xy.inner().y, z.into())
            }
            [Object(v)] if v.is_a::<Vec3>() => return Ok((*v.cast::<Vec3>().unwrap()).into()),
            unexpected => {
                return unexpected_args(
                    "||, or |Number|, or |Number, Number|, or |Number, Number, Number|,\
                     or |Vec2|, or |Vec2, Number|, or |Vec3|",
                    unexpected,
                )
            }
        };

        Ok(Vec3::new(x, y, z).into())
    });

    result
}
