//! The `os` core library module

use crate::{derive::*, prelude::*, Result};
use chrono::prelude::*;
use instant::Instant;

/// Initializes the `os` core library module
pub fn make_module() -> KMap {
    use Value::Number;

    let result = KMap::with_type("core.os");

    result.add_fn("name", |_| Ok(std::env::consts::OS.into()));

    result.add_fn("start_timer", |_| Ok(Timer::now()));

    result.add_fn("time", |ctx| match ctx.args() {
        [] => Ok(DateTime::now()),
        [Number(seconds)] => DateTime::from_seconds(seconds.into(), None),
        [Number(seconds), Number(offset)] => {
            DateTime::from_seconds(seconds.into(), Some(offset.into()))
        }
        unexpected => type_error_with_slice(
            "no args, or a timestamp in seconds, with optional timezone offset in seconds",
            unexpected,
        ),
    });

    result
}

/// The underlying data type returned by `os.time()`
#[derive(Clone, Debug, KotoCopy, KotoType)]
pub struct DateTime(chrono::DateTime<Local>);

#[koto_impl(runtime = crate)]
impl DateTime {
    fn with_chrono_datetime(time: chrono::DateTime<Local>) -> Value {
        KObject::from(Self(time)).into()
    }

    fn now() -> Value {
        Self::with_chrono_datetime(Local::now())
    }

    fn from_seconds(seconds: f64, maybe_offset: Option<i64>) -> Result<Value> {
        let seconds_i64 = seconds as i64;
        let sub_nanos = (seconds.fract() * 1.0e9) as u32;
        let offset = match maybe_offset {
            Some(offset) => match FixedOffset::east_opt(offset as i32) {
                Some(offset) => offset,
                None => return runtime_error!("time offset is out of range: {offset}"),
            },
            None => *Local::now().offset(),
        };
        match NaiveDateTime::from_timestamp_opt(seconds_i64, sub_nanos) {
            Some(utc) => Ok(Self::with_chrono_datetime(
                chrono::DateTime::<Local>::from_naive_utc_and_offset(utc, offset),
            )),
            None => runtime_error!("timestamp in seconds is out of range: {seconds}"),
        }
    }

    #[koto_method]
    fn day(&self) -> Value {
        self.0.day().into()
    }

    #[koto_method]
    fn hour(&self) -> Value {
        self.0.hour().into()
    }

    #[koto_method]
    fn minute(&self) -> Value {
        self.0.minute().into()
    }

    #[koto_method]
    fn month(&self) -> Value {
        self.0.month().into()
    }

    #[koto_method]
    fn second(&self) -> Value {
        self.0.second().into()
    }

    #[koto_method]
    fn nanosecond(&self) -> Value {
        self.0.nanosecond().into()
    }

    #[koto_method]
    fn timestamp(&self) -> Value {
        let seconds = self.0.timestamp() as f64;
        let sub_nanos = self.0.timestamp_subsec_nanos();
        (seconds + sub_nanos as f64 / 1.0e9).into()
    }

    #[koto_method]
    fn timezone_offset(&self) -> Value {
        self.0.offset().local_minus_utc().into()
    }

    #[koto_method]
    fn timezone_string(&self) -> Value {
        self.0.format("%z").to_string().into()
    }

    #[koto_method]
    fn year(&self) -> Value {
        self.0.year().into()
    }
}

impl KotoObject for DateTime {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.0.format("%F %T").to_string());
        Ok(())
    }
}

/// The underlying data type returned by `os.start_timer()`
#[derive(Clone, Debug, KotoCopy, KotoType)]
pub struct Timer(Instant);

#[koto_impl(runtime = crate)]
impl Timer {
    fn now() -> Value {
        let timer = Self(Instant::now());
        KObject::from(timer).into()
    }

    fn elapsed_seconds(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }

    #[koto_method]
    fn elapsed(&self) -> Value {
        self.elapsed_seconds().into()
    }
}

impl KotoObject for Timer {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("Timer({:.3}s)", self.elapsed_seconds()));
        Ok(())
    }

    fn subtract(&self, rhs: &Value) -> Result<Value> {
        match rhs {
            Value::Object(o) if o.is_a::<Self>() => {
                let rhs = o.cast::<Self>().unwrap();

                let result = if self.0 >= rhs.0 {
                    self.0.duration_since(rhs.0).as_secs_f64()
                } else {
                    -(rhs.0.duration_since(self.0).as_secs_f64())
                };

                Ok(result.into())
            }
            unexpected => type_error(Self::type_static(), unexpected),
        }
    }
}
