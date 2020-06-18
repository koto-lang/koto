mod io;
mod json;
mod list;
mod map;
mod math;
mod serializable_value;
mod string;
mod test;
mod thread;
mod toml;

pub use koto_runtime::{external_error, EXTERNAL_DATA_ID};

use {
    koto_runtime::{
        value::type_as_string, ExternalValue, IntRange, RuntimeResult, Value, ValueMap, Vm,
    },
    std::sync::Arc,
};

#[macro_export]
macro_rules! single_arg_fn {
    ($map_name: ident, $fn_name: expr, $type: ident, $match_name: ident, $body: block) => {
        $map_name.add_fn($fn_name, |_, args| {
            if args.len() == 1 {
                match &args[0] {
                    $type($match_name) => $body
                    unexpected => {
                        koto_runtime::external_error!(
                            "{}.{} only accepts a {} as its argument, found {}",
                            stringify!($map_name),
                            $fn_name,
                            stringify!($type),
                            value::type_as_string(&unexpected),
                        )
                    }
                }
            } else {
                koto_runtime::external_error!("{}.{} expects a single argument, found {}",
                    stringify!($map_name),
                    $fn_name,
                    args.len(),
                )
            }
        });
    }
}

// TODO split out _mut version
pub fn visit_external_value<T>(
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
                    value.value_type(),
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
            return $crate::external_error!(
                "{0}.{1}: Expected {0} instance as first argument",
                $external_name,
                $fn_name,
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
                unexpected,
            ),
        }
    }};
}

pub fn register(runtime: &mut Vm) {
    use Value::*;

    let global = runtime.global_mut();

    io::register(global);
    json::register(global);
    list::register(global);
    map::register(global);
    math::register(global);
    string::register(global);
    test::register(global);
    thread::register(global);
    toml::register(global);

    global.add_fn("size", |_, args| match &args {
        [Empty] => Ok(Number(0.0)),
        [List(list)] => Ok(Number(list.data().len() as f64)),
        [Map(map)] => Ok(Number(map.data().len() as f64)),
        [Range(IntRange { start, end })] => {
            let result = if end >= start {
                end - start
            } else {
                start - end
            };
            Ok(Number(result as f64))
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
            unexpected,
        ),
        _ => external_error!("number expects a single argument, found {}", args.len()),
    });

    global.add_fn("type", |_, args| {
        let result = match &args {
            [Bool(_)] => "bool",
            [Empty] => "empty",
            [Function(_)] => "function",
            [ExternalFunction(_)] => "function",
            [ExternalValue(value)] => return Ok(Str(Arc::new(value.read().unwrap().value_type()))),
            [List(_)] => "list",
            [Map(_)] => "map",
            [Number(_)] => "number",
            [Num2(_)] => "num2",
            [Num4(_)] => "num4",
            [Range(_)] => "range",
            [Str(_)] => "string",
            [unexpected] => {
                return external_error!(
                    "type is only supported for user types, found {}",
                    unexpected,
                )
            }
            _ => return external_error!("type expects a single argument, found {}", args.len()),
        };
        Ok(Str(Arc::new(result.to_string())))
    });

    global.add_fn("print", |_, args| {
        for value in args.iter() {
            print!("{}", value);
        }
        println!();
        Ok(Empty)
    });
}
