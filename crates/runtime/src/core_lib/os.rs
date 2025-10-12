//! The `os` core library module

mod command;

use self::command::Command;
use crate::{Result, derive::*, prelude::*};
use chrono::prelude::*;
use instant::Instant;

/// Initializes the `os` core library module
pub fn make_module() -> KMap {
    use KValue::Number;

    let result = KMap::with_type("core.os");

    result.insert("args", KValue::Tuple(KTuple::default()));

    result.add_fn("command", |ctx| match ctx.args() {
        [KValue::Str(command)] => Ok(Command::make_value(command)),
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("name", |ctx| match ctx.args() {
        [] => Ok(std::env::consts::OS.into()),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("process_id", |ctx| match ctx.args() {
        [] => {
            #[cfg(target_arch = "wasm32")]
            {
                // `process::id()` panics on wasm targets
                return runtime_error!(crate::ErrorKind::UnsupportedPlatform);
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                Ok(std::process::id().into())
            }
        }
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("start_timer", |ctx| match ctx.args() {
        [] => Ok(Timer::now()),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("time", |ctx| match ctx.args() {
        [] => Ok(DateTime::now()),
        [Number(seconds)] => DateTime::from_seconds(seconds.into(), None),
        [Number(seconds), Number(offset)] => {
            DateTime::from_seconds(seconds.into(), Some(offset.into()))
        }
        unexpected => unexpected_args("||, or |Number|, or |Number, Number|", unexpected),
    });

    result
}

/// The underlying data type returned by `os.time()`
#[derive(Clone, Debug, KotoCopy, KotoType)]
#[koto(runtime = crate)]
pub struct DateTime(chrono::DateTime<FixedOffset>);

#[koto_impl(runtime = crate)]
impl DateTime {
    fn with_chrono_datetime(time: chrono::DateTime<FixedOffset>) -> KValue {
        KObject::from(Self(time)).into()
    }

    fn now() -> KValue {
        Self::with_chrono_datetime(Local::now().fixed_offset())
    }

    fn from_seconds(seconds: f64, maybe_offset: Option<i64>) -> Result<KValue> {
        let seconds_i64 = seconds as i64;
        let sub_nanos = (seconds.fract() * 1.0e9) as u32;
        match chrono::DateTime::from_timestamp(seconds_i64, sub_nanos) {
            Some(utc) => {
                let offset = match maybe_offset {
                    Some(offset) => match FixedOffset::east_opt(offset as i32) {
                        Some(offset) => offset,
                        None => return runtime_error!("time offset is out of range: {offset}"),
                    },
                    None => *Local::now().offset(),
                };
                let local = utc.with_timezone(&offset);
                Ok(Self::with_chrono_datetime(local))
            }
            None => runtime_error!("timestamp in seconds is out of range: {seconds}"),
        }
    }

    #[koto_method]
    fn day(&self) -> KValue {
        self.0.day().into()
    }

    #[koto_method]
    fn hour(&self) -> KValue {
        self.0.hour().into()
    }

    #[koto_method]
    fn minute(&self) -> KValue {
        self.0.minute().into()
    }

    #[koto_method]
    fn month(&self) -> KValue {
        self.0.month().into()
    }

    #[koto_method]
    fn second(&self) -> KValue {
        self.0.second().into()
    }

    #[koto_method]
    fn nanosecond(&self) -> KValue {
        self.0.nanosecond().into()
    }

    #[koto_method]
    fn timestamp(&self) -> KValue {
        let seconds = self.0.timestamp() as f64;
        let sub_nanos = self.0.timestamp_subsec_nanos();
        (seconds + sub_nanos as f64 / 1.0e9).into()
    }

    #[koto_method]
    fn timezone_offset(&self) -> KValue {
        self.0.offset().local_minus_utc().into()
    }

    #[koto_method]
    fn timezone_string(&self) -> KValue {
        self.0.format("%z").to_string().into()
    }

    #[koto_method]
    fn year(&self) -> KValue {
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
#[koto(runtime = crate)]
pub struct Timer(Instant);

#[koto_impl(runtime = crate)]
impl Timer {
    fn now() -> KValue {
        let timer = Self(Instant::now());
        KObject::from(timer).into()
    }

    fn elapsed_seconds(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }

    #[koto_method]
    fn elapsed(&self) -> KValue {
        self.elapsed_seconds().into()
    }
}

impl KotoObject for Timer {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("Timer({:.3}s)", self.elapsed_seconds()));
        Ok(())
    }

    fn subtract(&self, other: &KValue) -> Result<KValue> {
        match other {
            KValue::Object(o) if o.is_a::<Self>() => {
                let other_timer = o.cast::<Self>().unwrap();

                let result = if self.0 >= other_timer.0 {
                    self.0.duration_since(other_timer.0).as_secs_f64()
                } else {
                    -(other_timer.0.duration_since(self.0).as_secs_f64())
                };

                Ok(result.into())
            }
            unexpected => unexpected_type(Self::type_static(), unexpected),
        }
    }
}
