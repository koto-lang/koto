use crate::Vec2;
use koto_runtime::{derive::*, prelude::*, Result};
use std::fmt;

type Inner = nannou_core::geom::Rect<f64>;

#[derive(Copy, Clone, PartialEq, KotoCopy, KotoType)]
#[koto(use_copy)]
pub struct Rect(Inner);

#[koto_impl(runtime = koto_runtime)]
impl Rect {
    pub fn from_x_y_w_h(x: f64, y: f64, width: f64, height: f64) -> Self {
        Inner::from_x_y_w_h(x, y, width, height).into()
    }

    #[koto_method]
    fn left(&self) -> KValue {
        self.0.left().into()
    }

    #[koto_method]
    fn right(&self) -> KValue {
        self.0.right().into()
    }

    #[koto_method]
    fn top(&self) -> KValue {
        self.0.top().into()
    }

    #[koto_method]
    fn bottom(&self) -> KValue {
        self.0.bottom().into()
    }

    #[koto_method]
    fn width(&self) -> KValue {
        self.0.w().into()
    }

    #[koto_method]
    fn height(&self) -> KValue {
        self.0.h().into()
    }

    #[koto_method]
    fn center(&self) -> KValue {
        Vec2::from(self.0.xy()).into()
    }

    #[koto_method]
    fn x(&self) -> KValue {
        self.0.x().into()
    }

    #[koto_method]
    fn y(&self) -> KValue {
        self.0.y().into()
    }

    #[koto_method]
    fn contains(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Object(p)] if p.is_a::<Vec2>() => {
                let p = p.cast::<Vec2>().unwrap();
                let result = self.0.contains(p.inner());
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
        this.0 = Inner::from_x_y_w_h(x, y, this.0.w(), this.0.h());

        // Return a clone of the Rect instance
        ctx.instance_result()
    }
}

impl KotoObject for Rect {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.to_string());
        Ok(())
    }

    fn equal(&self, rhs: &KValue) -> Result<bool> {
        geometry_comparison_op!(self, rhs, ==)
    }

    fn not_equal(&self, rhs: &KValue) -> Result<bool> {
        geometry_comparison_op!(self, rhs, !=)
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::Iterable
    }

    fn make_iterator(&self, _vm: &mut KotoVm) -> Result<KIterator> {
        let r = *self;

        let iter = (0..=3).map(move |i| {
            let result = match i {
                0 => r.0.x(),
                1 => r.0.y(),
                2 => r.0.w(),
                3 => r.0.h(),
                _ => unreachable!(),
            };
            KIteratorOutput::Value(result.into())
        });

        Ok(KIterator::with_std_iter(iter))
    }
}

impl From<Inner> for Rect {
    fn from(r: Inner) -> Self {
        Self(r)
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
        let r = &self.0;
        write!(
            f,
            "Rect{{x: {}, y: {}, width: {}, height: {}}}",
            r.x(),
            r.y(),
            r.w(),
            r.h()
        )
    }
}
