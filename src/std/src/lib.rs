mod io;
mod list;
mod map;
mod math;
mod random;
mod string;
mod test;
mod thread;

pub use koto_runtime::{external_error, EXTERNAL_DATA_ID};

use koto_runtime::{value::type_as_string, ExternalValue, RuntimeResult, Value, ValueMap};

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

pub fn register(prelude: &mut ValueMap) {
    io::register(prelude);
    list::register(prelude);
    map::register(prelude);
    math::register(prelude);
    random::register(prelude);
    string::register(prelude);
    test::register(prelude);
    thread::register(prelude);
}
