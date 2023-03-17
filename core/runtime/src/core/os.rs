//! The `os` core library module

use {crate::prelude::*, chrono::prelude::*, instant::Instant, std::ops::Deref};

/// Initializes the `os` core library module
pub fn make_module() -> ValueMap {
    use Value::Number;

    let result = ValueMap::new();

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
#[derive(Clone)]
pub struct DateTime(chrono::DateTime<Local>);

impl DateTime {
    fn with_chrono_datetime(time: chrono::DateTime<Local>) -> Value {
        let result = External::with_shared_meta_map(Self(time), Self::meta_map());
        Value::External(result)
    }

    fn now() -> Value {
        Self::with_chrono_datetime(Local::now())
    }

    fn from_seconds(seconds: f64, maybe_offset: Option<i64>) -> RuntimeResult {
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

    fn meta_map() -> PtrMut<MetaMap> {
        SYSTEM_TIME_META.with(|meta| meta.clone())
    }
}

impl ExternalData for DateTime {
    fn make_copy(&self) -> PtrMut<dyn ExternalData> {
        make_data_ptr(self.clone())
    }
}

impl Deref for DateTime {
    type Target = chrono::DateTime<Local>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

thread_local! {
    /// The meta map used by [DateTime]
    pub static SYSTEM_TIME_META: PtrMut<MetaMap> = make_system_time_meta_map();
}

fn make_system_time_meta_map() -> PtrMut<MetaMap> {
    MetaMapBuilder::<DateTime>::new("DateTime")
        .function(UnaryOp::Display, |context| {
            Ok(context.data()?.format("%F %T").to_string().into())
        })
        .function("day", |context| Ok(context.data()?.day().into()))
        .function("hour", |context| Ok(context.data()?.hour().into()))
        .function("minute", |context| Ok(context.data()?.minute().into()))
        .function("month", |context| Ok(context.data()?.month().into()))
        .function("second", |context| Ok(context.data()?.second().into()))
        .function("nanosecond", |context| {
            Ok(context.data()?.nanosecond().into())
        })
        .function("timestamp", |context| {
            let seconds = context.data()?.timestamp() as f64;
            let sub_nanos = context.data()?.timestamp_subsec_nanos();
            Ok((seconds + sub_nanos as f64 / 1.0e9).into())
        })
        .function("timezone_offset", |context| {
            Ok(context.data()?.offset().local_minus_utc().into())
        })
        .function("timezone_string", |context| {
            Ok(context.data()?.format("%z").to_string().into())
        })
        .function("year", |context| Ok(context.data()?.year().into()))
        .build()
}

/// The underlying data type returned by `os.start_timer()`
#[derive(Clone)]
pub struct Timer(Instant);

impl Timer {
    fn now() -> Value {
        let meta = TIMER_META.with(|meta| meta.clone());
        let result = External::with_shared_meta_map(Self(Instant::now()), meta);
        Value::External(result)
    }
}

impl ExternalData for Timer {
    fn make_copy(&self) -> PtrMut<dyn ExternalData> {
        make_data_ptr(self.clone())
    }
}

impl Deref for Timer {
    type Target = Instant;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

thread_local! {
    /// The meta map used by [Timer]
    pub static TIMER_META: PtrMut<MetaMap> = make_timer_meta_map();
}

fn make_timer_meta_map() -> PtrMut<MetaMap> {
    use Value::External;

    MetaMapBuilder::<Timer>::new("Timer")
        .function(UnaryOp::Display, |context| {
            Ok(format!("Timer({:.3}s)", context.data()?.elapsed().as_secs_f64()).into())
        })
        .function(BinaryOp::Subtract, |context| match context.args {
            [External(b)] if b.has_data::<Timer>() => {
                let b = b.data::<Timer>().unwrap();
                let a = context.data()?;
                let result = if a.0 >= b.0 {
                    a.0.duration_since(b.0).as_secs_f64()
                } else {
                    -(b.0.duration_since(a.0).as_secs_f64())
                };
                Ok(result.into())
            }
            unexpected => type_error_with_slice("Timer", unexpected),
        })
        .function("elapsed", |context| {
            Ok(context.data()?.0.elapsed().as_secs_f64().into())
        })
        .build()
}
