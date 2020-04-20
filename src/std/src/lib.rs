mod io;
mod list;
mod map;
mod math;
mod thread;

pub use koto_runtime::EXTERNAL_DATA_ID;

use koto_runtime::{
    value, value::type_as_string, Error, ExternalValue, Runtime, RuntimeResult, Value, ValueList,
    ValueMap, ValueVec,
};
use std::sync::Arc;

#[macro_export]
macro_rules! make_external_error {
    ($message:expr) => {{
        let error = Error::ExternalError { message: $message };
        #[cfg(panic_on_runtime_error)]
        {
            panic!();
        }
        error
    }};
}

#[macro_export]
macro_rules! external_error {
    ($error:expr) => {
        Err($crate::make_external_error!(String::from($error)))
    };
    ($error:expr, $($y:expr),+) => {
        Err($crate::make_external_error!(format!($error, $($y),+)))
    };
}

#[macro_export]
macro_rules! single_arg_fn {
    ($map_name: ident, $fn_name: expr, $type: ident, $match_name: ident, $body: block) => {
        $map_name.add_fn($fn_name, |_, args| {
            if args.len() == 1 {
                match &args[0] {
                    $type($match_name) => $body
                    unexpected => {
                        $crate::external_error!(
                            "{}.{} only accepts a {} as its argument, found {}",
                            stringify!($map_name),
                            $fn_name,
                            stringify!($type),
                            value::type_as_string(&unexpected)
                        )
                    }
                }
            } else {
                $crate::external_error!("{}.{} expects a single argument, found {}",
                    stringify!($map_name),
                    $fn_name,
                    args.len()
                )
            }
        });
    }
}

// TODO split out _mut version
pub fn visit_external_value<'a, T>(
    map: &ValueMap,
    mut f: impl FnMut(&mut T) -> RuntimeResult,
) -> RuntimeResult
where
    T: ExternalValue,
{
    match map.data().get(EXTERNAL_DATA_ID) {
        Some(Value::ExternalValue(maybe_external)) => {
            let mut value = maybe_external.as_ref().write().unwrap();
            match value.downcast_mut::<T>() {
                Some(external) => f(external),
                None => external_error!(
                    "Invalid type for external value, found '{}'",
                    value.value_type()
                ),
            }
        }
        _ => external_error!("External value not found"),
    }
}

#[macro_export]
macro_rules! get_external_instance {
    ($args: ident,
     $external_name: expr,
     $fn_name: expr,
     $external_type: ident,
     $match_name: ident,
     $body: block) => {{
        if $args.len() == 0 {
            return external_error!(
                "{0}.{1}: Expected {0} instance as first argument",
                $external_name,
                $fn_name
            );
        }

        match &$args[0] {
            Value::Map(instance) => {
                $crate::visit_external_value(instance, |$match_name: &mut $external_type| $body)
            }
            unexpected => $crate::external_error!(
                "{0}.{1}: Expected {0} instance as first argument, found '{2}'",
                $external_name,
                $fn_name,
                unexpected
            ),
        }
    }};
}

pub fn register(runtime: &mut Runtime) {
    use Value::*;

    let global = runtime.global_mut();

    io::register(global);
    list::register(global);
    map::register(global);
    math::register(global);
    thread::register(global);

    {
        let mut string = ValueMap::new();

        single_arg_fn!(string, "escape", Str, s, {
            Ok(Str(Arc::new(s.escape_default().to_string())))
        });

        single_arg_fn!(string, "lines", Str, s, {
            Ok(List(ValueList::with_data(
                s.lines()
                    .map(|line| Str(Arc::new(line.to_string())))
                    .collect::<ValueVec>(),
            )))
        });

        global.add_value("string", Map(string));
    }

    global.add_fn("assert", |_, args| {
        for value in args.iter() {
            match value {
                Bool(b) => {
                    if !b {
                        return external_error!("Assertion failed");
                    }
                }
                unexpected => {
                    return external_error!(
                        "assert expects booleans as arguments, found '{}'",
                        type_as_string(unexpected)
                    )
                }
            }
        }
        Ok(Empty)
    });

    global.add_fn("assert_eq", |_, args| match &args {
        [a, b] => {
            if a == b {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' is not equal to '{}'",
                    args[0],
                    args[1]
                )
            }
        }
        _ => external_error!("assert_eq expects two arguments, found {}", args.len()),
    });

    global.add_fn("assert_ne", |_, args| match &args {
        [a, b] => {
            if a != b {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' should not be equal to '{}'",
                    args[0],
                    args[1]
                )
            }
        }
        _ => external_error!("assert_ne expects two arguments, found {}", args.len()),
    });

    global.add_fn("assert_near", |_, args| match &args {
        [Number(a), Number(b), Number(allowed_diff)] => {
            if (a - b).abs() <= *allowed_diff {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' and '{}' are not within {} of each other",
                    a,
                    b,
                    allowed_diff
                )
            }
        }
        [a, b, c] => external_error!(
            "assert_near expects Numbers as arguments, found '{}', '{}', and '{}'",
            type_as_string(&a),
            type_as_string(&b),
            type_as_string(&c)
        ),
        _ => external_error!("assert_eq expects three arguments, found {}", args.len()),
    });

    global.add_fn("size", |_, args| match &args {
        [Empty] => Ok(Number(0.0)),
        [List(list)] => Ok(Number(list.data().len() as f64)),
        [Map(map)] => Ok(Number(map.data().len() as f64)),
        [Range { start, end }] => {
            println!("size: start: {} end: {}", start, end);

            Ok(Number(if end >= start {
                end - start
            } else {
                start - end
            } as f64))
        }
        [unexpected] => external_error!("size - '{}' is unsupported", unexpected),
        _ => external_error!("size expects a single argument, found {}", args.len()),
    });

    global.add_fn("number", |_, args| match &args {
        [n @ Number(_)] => Ok(n.clone()),
        [Str(s)] => match s.parse::<f64>() {
            Ok(n) => Ok(Number(n)),
            Err(_) => external_error!("Failed to convert '{}' into a Number", s),
        },
        [unexpected] => external_error!(
            "number is only supported for numbers and strings, found {}",
            unexpected
        ),
        _ => external_error!("number expects a single argument, found {}", args.len()),
    });

    global.add_fn("print", |_, args| {
        for value in args.iter() {
            print!("{}", value);
        }
        println!();
        Ok(Empty)
    });
}
