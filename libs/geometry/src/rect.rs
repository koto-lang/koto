use crate::Vec2;
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
    fn left(&self) -> KValue {
        self.x.start.into()
    }

    #[koto_method]
    fn right(&self) -> KValue {
        self.x.end.into()
    }

    #[koto_method]
    fn bottom(&self) -> KValue {
        self.y.start.into()
    }

    #[koto_method]
    fn top(&self) -> KValue {
        self.y.end.into()
    }

    #[koto_method]
    fn width(&self) -> KValue {
        self.x.len().into()
    }

    #[koto_method]
    fn height(&self) -> KValue {
        self.y.len().into()
    }

    #[koto_method]
    fn center(&self) -> KValue {
        Vec2::new(self.x.center(), self.y.center()).into()
    }

    #[koto_method]
    fn x(&self) -> KValue {
        self.x.center().into()
    }

    #[koto_method]
    fn y(&self) -> KValue {
        self.y.center().into()
    }

    #[koto_method]
    fn contains(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Object(p)] if p.is_a::<Vec2>() => {
                let p = p.cast::<Vec2>().unwrap();
                let result = self.x.contains(p.inner().x) && self.y.contains(p.inner().y);
                Ok(result.into())
            }
            unexpected => unexpected_args("|Vec2|", unexpected),
        }
    }

    #[koto_method]
    fn set_center(ctx: MethodContext<Self>) -> Result<KValue> {
        use KValue::{Number, Object};

        let (x, y) = match ctx.args {
            [Number(x), Number(y)] => (x.into(), y.into()),
            [Object(p)] if p.is_a::<Vec2>() => {
                let p = p.cast::<Vec2>().unwrap();
                (p.inner().x, p.inner().y)
            }
            unexpected => return unexpected_args("|Vec2|, or |Number, Number|", unexpected),
        };

        let mut this = ctx.instance_mut()?;
        this.x.set_center(x);
        this.y.set_center(y);

        // Return a clone of the Rect instance
        ctx.instance_result()
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
            KIteratorOutput::Value(result)
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
