mod runtime_test_utils;

mod external_values {
    use {
        crate::runtime_test_utils::{string, test_script_with_vm},
        koto_runtime::{
            runtime_error, type_error, type_error_with_slice, BinaryOp, ExternalData,
            ExternalValue, MetaMap, MetaMapBuilder, UnaryOp, Value, Vm,
        },
        std::{cell::RefCell, rc::Rc},
    };

    #[derive(Debug)]
    struct TestExternalData {
        x: f64,
    }

    impl TestExternalData {
        fn make_value(x: f64) -> Value {
            Value::ExternalValue(ExternalValue::with_shared_meta_map(
                Self { x },
                EXTERNAL_META.with(|meta| meta.clone()),
            ))
        }
    }

    impl ExternalData for TestExternalData {}

    thread_local! {
        static EXTERNAL_META: Rc<RefCell<MetaMap>> = make_external_value_meta_map();
    }

    fn make_external_value_meta_map() -> Rc<RefCell<MetaMap>> {
        use Value::{Bool, Null, Number};
        use {BinaryOp::*, UnaryOp::*};

        MetaMapBuilder::<TestExternalData>::new("TestExternalValue")
            .data_fn(Display, |data| {
                Ok(format!("TestExternalData: {}", data.x).into())
            })
            .data_fn(Negate, |data| Ok(TestExternalData::make_value(-data.x)))
            .data_fn_2(Add, |a, b| Ok(TestExternalData::make_value(a.x + b.x)))
            .data_fn_2(Subtract, |a, b| Ok(TestExternalData::make_value(a.x - b.x)))
            .data_fn_2(Multiply, |a, b| Ok(TestExternalData::make_value(a.x * b.x)))
            .data_fn_2(Divide, |a, b| Ok(TestExternalData::make_value(a.x / b.x)))
            .data_fn_2(Remainder, |a, b| {
                Ok(TestExternalData::make_value(a.x % b.x))
            })
            .data_fn_2(Less, |a, b| Ok(Bool(a.x < b.x)))
            .data_fn_2(LessOrEqual, |a, b| Ok(Bool(a.x <= b.x)))
            .data_fn_2(Greater, |a, b| Ok(Bool(a.x > b.x)))
            .data_fn_2(GreaterOrEqual, |a, b| Ok(Bool(a.x >= b.x)))
            .data_fn_2(Equal, |a, b| {
                #[allow(clippy::float_cmp)]
                Ok(Bool(a.x == b.x))
            })
            .data_fn_2(NotEqual, |a, b| {
                #[allow(clippy::float_cmp)]
                Ok(Bool(a.x != b.x))
            })
            .data_fn_with_args(Index, |data, args| match args {
                [Number(index)] => {
                    let index = usize::from(index);
                    let result = data.x + index as f64;
                    Ok(result.into())
                }
                unexpected => type_error_with_slice("ExternalValue.@Index", "a Number", unexpected),
            })
            .instance_fn("get_data", |instance| {
                // We want to return a Rc clone of the internal data,
                // so instance_fn is used here to get easy access to the Rc.
                Ok(Value::ExternalData(instance.data.clone()))
            })
            .data_fn("to_number", |data| Ok(Number(data.x.into())))
            .data_fn_mut("invert", |data| {
                data.x *= -1.0;
                Ok(Null)
            })
            .data_fn_2_mut("set_all_instances", |a, b| {
                a.x = b.x;
                Ok(Null)
            })
            .data_fn_with_args_mut("absorb_values", |data, args| {
                for arg in args.iter() {
                    match arg {
                        Number(n) => data.x += f64::from(n),
                        other => return type_error("ExternalValue.absorb_values", "Number", other),
                    }
                }
                Ok(Null)
            })
            .build()
    }

    fn test_script_with_external_value(script: &str, expected_output: impl Into<Value>) {
        let vm = Vm::default();
        let prelude = vm.prelude();

        prelude.add_fn("make_external", |vm, args| match vm.get_args(args) {
            [Value::Number(x)] => Ok(TestExternalData::make_value(x.into())),
            [Value::ExternalData(data)] => Ok(ExternalValue {
                data: data.clone(),
                meta: EXTERNAL_META.with(|meta| meta.clone()),
            }
            .into()),
            _ => runtime_error!("make_external: Expected a Number or ExternalData as argument"),
        });

        test_script_with_vm(vm, script, expected_output.into());
    }

    mod named_functions {
        use super::*;

        #[test]
        fn to_number() {
            let script = "
x = make_external 42
x.to_number()
";
            test_script_with_external_value(script, 42);
        }

        #[test]
        fn invert() {
            let script = "
x = make_external 42
x.invert()
x.to_number()
";
            test_script_with_external_value(script, -42.0_f64);
        }

        #[test]
        fn set_all_instances() {
            let script = "
x = make_external 42
y = x
y.set_all_instances make_external 99
x.to_number()
";
            test_script_with_external_value(script, 99);
        }

        #[test]
        fn get_data() {
            let script = "
x = make_external 42
x_data = x.get_data()
y = make_external x_data
y.to_number()
";
            test_script_with_external_value(script, 42);
        }

        #[test]
        fn absorb_values() {
            let script = "
x = make_external 42
x.absorb_values 10, 20, 30
x.to_number()
";
            test_script_with_external_value(script, 102);
        }
    }

    mod unary_op {
        use super::*;

        #[test]
        fn display() {
            let script = "'{}'.format make_external 42";
            test_script_with_external_value(script, string("TestExternalData: 42"));
        }

        #[test]
        fn negate() {
            let script = "
x = make_external -123
x = -x
x.to_number()
";
            test_script_with_external_value(script, 123);
        }
    }

    mod binary_op {
        use {super::*, Value::Bool};

        #[test]
        fn add() {
            let script = "
x = (make_external 11) + (make_external 22)
x.to_number()
";
            test_script_with_external_value(script, 33);
        }

        #[test]
        fn subtract() {
            let script = "
x = (make_external 99) - (make_external 90)
x.to_number()
";
            test_script_with_external_value(script, 9);
        }

        #[test]
        fn multiply() {
            let script = "
x = (make_external 3) * (make_external 11)
x.to_number()
";
            test_script_with_external_value(script, 33);
        }

        #[test]
        fn divide() {
            let script = "
x = (make_external 90) / (make_external 10)
x.to_number()
";
            test_script_with_external_value(script, 9);
        }

        #[test]
        fn remainder() {
            let script = "
x = (make_external 45) % (make_external 10)
x.to_number()
";
            test_script_with_external_value(script, 5);
        }

        #[test]
        fn less() {
            let script = "(make_external 1) < (make_external 2)";
            test_script_with_external_value(script, Bool(true));
        }

        #[test]
        fn less_or_equal() {
            let script = "(make_external 2) <= (make_external 2)";
            test_script_with_external_value(script, Bool(true));
        }

        #[test]
        fn equal() {
            let script = "(make_external 2) == (make_external 3)";
            test_script_with_external_value(script, Bool(false));
        }

        #[test]
        fn not_equal() {
            let script = "(make_external 2) != (make_external 3)";
            test_script_with_external_value(script, Bool(true));
        }

        #[test]
        fn index() {
            let script = "
x = make_external 100
x[23]
";
            test_script_with_external_value(script, 123);
        }
    }

    mod temporaries {
        use super::*;

        #[test]
        fn overloaded_unary_op_as_lookup_root() {
            let script = "
x = make_external -100
(-x).to_number()
    ";
            test_script_with_external_value(script, 100);
        }

        #[test]
        fn overloaded_binary_op_as_lookup_root() {
            let script = "
x = make_external 100
y = make_external 100
(x - y).to_number()
    ";
            test_script_with_external_value(script, 0);
        }
    }
}
