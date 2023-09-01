//! The `os` core library module

use crate::{prelude::*, Result};
use chrono::prelude::*;
use instant::Instant;
use std::ops::Deref;

/// Initializes the `os` core library module
pub fn make_module() -> ValueMap {
    use Value::Number;

    let result = ValueMap::with_type("core.os");

    result.add_fn("name", |_, _| Ok(std::env::consts::OS.into()));

    result.add_fn("start_timer", |_, _| Ok(Timer::now()));

    result.add_fn("time", |vm, args| match vm.get_args(args) {
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
#[derive(Clone, Debug)]
pub struct DateTime(chrono::DateTime<Local>);

impl Deref for DateTime {
    type Target = chrono::DateTime<Local>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DateTime {
    fn with_chrono_datetime(time: chrono::DateTime<Local>) -> Value {
        Object::from(Self(time)).into()
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
                chrono::DateTime::<Local>::from_utc(utc, offset),
            )),
            None => runtime_error!("timestamp in seconds is out of range: {seconds}"),
        }
    }
}

impl KotoType for DateTime {
    const TYPE: &'static str = "DateTime";
}

impl KotoObject for DateTime {
    fn object_type(&self) -> ValueString {
        DATETIME_TYPE_STRING.with(|t| t.clone())
    }

    fn copy(&self) -> Object {
        self.clone().into()
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        DATETIME_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.format("%F %T").to_string());
        Ok(())
    }
}

fn datetime_entries() -> DataMap {
    ObjectEntryBuilder::<DateTime>::new()
        .method("day", |ctx| Ok(ctx.instance()?.day().into()))
        .method("hour", |ctx| Ok(ctx.instance()?.hour().into()))
        .method("minute", |ctx| Ok(ctx.instance()?.minute().into()))
        .method("month", |ctx| Ok(ctx.instance()?.month().into()))
        .method("second", |ctx| Ok(ctx.instance()?.second().into()))
        .method("nanosecond", |ctx| Ok(ctx.instance()?.nanosecond().into()))
        .method("timestamp", |ctx| {
            let seconds = ctx.instance()?.timestamp() as f64;
            let sub_nanos = ctx.instance()?.timestamp_subsec_nanos();
            Ok((seconds + sub_nanos as f64 / 1.0e9).into())
        })
        .method("timezone_offset", |ctx| {
            Ok(ctx.instance()?.offset().local_minus_utc().into())
        })
        .method("timezone_string", |ctx| {
            Ok(ctx.instance()?.format("%z").to_string().into())
        })
        .method("year", |ctx| Ok(ctx.instance()?.year().into()))
        .build()
}

thread_local! {
    static DATETIME_TYPE_STRING: ValueString = DateTime::TYPE.into();
    static DATETIME_ENTRIES: DataMap = datetime_entries();
}

/// The underlying data type returned by `os.start_timer()`
#[derive(Clone, Debug)]
pub struct Timer(Instant);

impl Deref for Timer {
    type Target = Instant;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Timer {
    fn now() -> Value {
        let timer = Self(Instant::now());
        Object::from(timer).into()
    }

    fn elapsed_seconds(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }
}

impl KotoType for Timer {
    const TYPE: &'static str = "Timer";
}

impl KotoObject for Timer {
    fn object_type(&self) -> ValueString {
        TIMER_TYPE_STRING.with(|t| t.clone())
    }

    fn copy(&self) -> Object {
        self.clone().into()
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        TIMER_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("{}({:.3}s)", Self::TYPE, self.elapsed_seconds()));
        Ok(())
    }

    fn subtract(&self, rhs: &Value) -> Result<Value> {
        match rhs {
            Value::Object(o) if o.is_a::<Self>() => {
                let rhs = o.cast::<Self>().unwrap();

                let result = if self.0 >= rhs.0 {
                    self.duration_since(rhs.0).as_secs_f64()
                } else {
                    -(rhs.duration_since(self.0).as_secs_f64())
                };

                Ok(result.into())
            }
            unexpected => type_error(Self::TYPE, unexpected),
        }
    }
}

fn named_timer_entries() -> DataMap {
    ObjectEntryBuilder::<Timer>::new()
        .method("elapsed", |ctx| {
            Ok(ctx.instance()?.elapsed_seconds().into())
        })
        .build()
}

thread_local! {
    static TIMER_TYPE_STRING: ValueString = Timer::TYPE.into();
    static TIMER_ENTRIES: DataMap = named_timer_entries();
}
