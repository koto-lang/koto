mod random;

pub use koto_runtime::external_error;

use koto_runtime::ValueMap;

#[macro_export]
macro_rules! single_arg_fn {
    ($map_name: ident, $fn_name: expr, $type: ident, $match_name: ident, $body: block) => {
        $map_name.add_fn($fn_name, |vm, args| match vm.get_args(args) {
            [$type($match_name)] => $body
            [unexpected] => {
                koto_runtime::external_error!(
                    "{}.{} only accepts a {} as its argument, found {}",
                    stringify!($map_name),
                    $fn_name,
                    stringify!($type),
                    value::type_as_string(&unexpected),
                )
            }
            _ => {
                koto_runtime::external_error!(
                    "{}.{} expects a single argument, found {}",
                    stringify!($map_name),
                    $fn_name,
                    args.count,
                )
            }
        });
    }
}

pub fn register(prelude: &mut ValueMap) {
    random::register(prelude);
}
