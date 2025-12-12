use crate::{Vec2, geometry_comparison_op};
use koto_runtime::{Result, derive::*, prelude::*};
use std::{
    fmt,
    ops::{Add, Div, Sub},
};

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(runtime = koto_runtime, use_copy)]
pub struct Rect {
    x: Bounds<f64>,
    y: Bounds<f64>,
}

#[koto_impl(runtime = koto_runtime)]
impl Rect {
    pub fn from_x_y_w_h(x: f64, y: f64, width: f64, height: f64) -> Self {
        let x_start = x - width / 2.0;
        let x_end = x_start + width;
        let y_start = y - height / 2.0;
        let y_end = y_start + height;
        Self {
            x: Bounds {
                start: x_start,
                end: x_end,
            },
            y: Bounds {
                start: y_start,
                end: y_end,
            },
        }
    }

    #[koto_method]
    fn left(&self) -> f64 {
        self.x.start
    }

    #[koto_method]
    fn right(&self) -> f64 {
        self.x.end
    }

    #[koto_method]
    fn bottom(&self) -> f64 {
        self.y.start
    }

    #[koto_method]
    fn top(&self) -> f64 {
        self.y.end
    }

    #[koto_method]
    fn width(&self) -> f64 {
        self.x.len()
    }

    #[koto_method]
    fn height(&self) -> f64 {
        self.y.len()
    }

    #[koto_method]
    fn center(&self) -> Vec2 {
        Vec2::new(self.x.center(), self.y.center())
    }

    #[koto_method]
    fn x(&self) -> f64 {
        self.x.center()
    }

    #[koto_method]
    fn y(&self) -> f64 {
        self.y.center()
    }

    #[koto_method]
    fn contains(&self, p: &Vec2) -> bool {
        self.x.contains(p.inner().x) && self.y.contains(p.inner().y)
    }

    #[koto_method(name = "set_center")]
    fn set_center_point(&mut self, p: &Vec2) -> &mut Self {
        self.x.set_center(p.inner().x);
        self.y.set_center(p.inner().y);
        self
    }

    #[koto_method(name = "set_center")]
    fn set_center_xy(&mut self, x: f64, y: f64) -> &mut Self {
        self.x.set_center(x);
        self.y.set_center(y);
        self
    }
}

impl KotoObject for Rect {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn equal(&self, value: &KValue) -> Result<bool> {
        geometry_comparison_op!(self, value, ==)
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        let r = *self;

        let iter = (0..=3).map(move |i| {
            let result = match i {
                0 => r.x(),
                1 => r.y(),
                2 => r.width(),
                3 => r.height(),
                _ => unreachable!(),
            };
            KIteratorOutput::Value(result.into())
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

impl From<(f64, f64, f64, f64)> for Rect {
    fn from((x, y, w, h): (f64, f64, f64, f64)) -> Self {
        Self::from_x_y_w_h(x, y, w, h)
    }
}

impl From<Rect> for KValue {
    fn from(point: Rect) -> Self {
        KObject::from(point).into()
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect{{x: {}, y: {}, width: {}, height: {}}}",
            self.x.center(),
            self.y.center(),
            self.x.len(),
            self.y.len()
        )
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Bounds<T>
where
    T: Clone + Copy + Default + PartialEq,
{
    start: T,
    end: T,
}

impl<T> Bounds<T>
where
    T: Clone
        + Copy
        + Default
        + PartialEq
        + PartialOrd
        + Sub<Output = T>
        + Add<Output = T>
        + Div<Output = T>
        + From<u8>,
{
    fn center(&self) -> T {
        self.start + self.len() / T::from(2)
    }

    fn set_center(&mut self, center: T) {
        let len = self.len();
        self.start = center - len / T::from(2);
        self.end = self.start + len;
    }

    fn len(&self) -> T {
        self.end - self.start
    }

    fn contains(&self, value: T) -> bool {
        value >= self.start && value <= self.end
    }
}
