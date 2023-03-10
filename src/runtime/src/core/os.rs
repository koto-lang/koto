//! The `os` core library module

use {
    crate::prelude::*,
    chrono::prelude::*,
    instant::Instant,
};

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
pub struct DateTime(chrono::DateTime<Local>);

impl DateTime {
    fn with_chrono_datetime(time: chrono::DateTime<Local>) -> Value {
        let result = ExternalValue::with_shared_meta_map(Self(time), Self::meta_map());
        Value::ExternalValue(result)
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

    fn meta_map() -> RcCell<MetaMap> {
        SYSTEM_TIME_META.with(|meta| meta.clone())
    }
}

impl ExternalData for DateTime {}

thread_local! {
    /// The meta map used by [DateTime]
    pub static SYSTEM_TIME_META: RcCell<MetaMap> = make_system_time_meta_map();
}

fn make_system_time_meta_map() -> RcCell<MetaMap> {
    MetaMapBuilder::<DateTime>::new("DateTime")
        .data_fn(UnaryOp::Display, |data| {
            Ok(data.0.format("%F %T").to_string().into())
        })
        .data_fn("day", |data| Ok(data.0.day().into()))
        .data_fn("hour", |data| Ok(data.0.hour().into()))
        .data_fn("minute", |data| Ok(data.0.minute().into()))
        .data_fn("month", |data| Ok(data.0.month().into()))
        .data_fn("second", |data| Ok(data.0.second().into()))
        .data_fn("nanosecond", |data| Ok(data.0.nanosecond().into()))
        .data_fn("timestamp", |data| {
            let seconds = data.0.timestamp() as f64;
            let sub_nanos = data.0.timestamp_subsec_nanos();
            Ok((seconds + sub_nanos as f64 / 1.0e9).into())
        })
        .data_fn("timezone_offset", |data| {
            Ok(data.0.offset().local_minus_utc().into())
        })
        .data_fn("timezone_string", |data| {
            Ok(data.0.format("%z").to_string().into())
        })
        .data_fn("year", |data| Ok(data.0.year().into()))
        .build()
}

/// The underlying data type returned by `os.start_timer()`
pub struct Timer(Instant);

impl Timer {
    fn now() -> Value {
        let meta = TIMER_META.with(|meta| meta.clone());
        let result = ExternalValue::with_shared_meta_map(Self(Instant::now()), meta);
        Value::ExternalValue(result)
    }
}

impl ExternalData for Timer {}

thread_local! {
    /// The meta map used by [Timer]
    pub static TIMER_META: RcCell<MetaMap> = make_timer_meta_map();
}

fn make_timer_meta_map() -> RcCell<MetaMap> {
    use Value::ExternalValue;

    MetaMapBuilder::<Timer>::new("Timer")
        .data_fn(UnaryOp::Display, |data| {
            Ok(format!("Timer({:.3}s)", data.0.elapsed().as_secs_f64()).into())
        })
        .data_fn_with_args(BinaryOp::Subtract, |a, b| match b {
            [ExternalValue(b)] if b.has_data::<Timer>() => {
                let b = b.data::<Timer>().unwrap();
                let result = if a.0 >= b.0 {
                    a.0.duration_since(b.0).as_secs_f64()
                } else {
                    -(b.0.duration_since(a.0).as_secs_f64())
                };
                Ok(result.into())
            }
            unexpected => type_error_with_slice("Timer", unexpected),
        })
        .data_fn("elapsed", |instant| {
            Ok(instant.0.elapsed().as_secs_f64().into())
        })
        .build()
}
