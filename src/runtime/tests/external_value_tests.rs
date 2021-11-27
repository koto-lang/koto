mod runtime_test_utils;

mod external_values {
    use {
        crate::runtime_test_utils::{string, test_script_with_vm},
        koto_runtime::{
            runtime_error, BinaryOp, ExternalData, ExternalValue, MetaMap, UnaryOp, Value, Vm,
        },
        std::{cell::RefCell, fmt, rc::Rc},
    };

    #[derive(Debug)]
    struct TestExternalData {
        x: f64,
    }

    impl ExternalData for TestExternalData {}

    impl fmt::Display for TestExternalData {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "TestExternalData: {}", self.x)
        }
    }

    thread_local!(
        static EXTERNAL_META: Rc<RefCell<MetaMap>> = {
            use Value::{Bool, Empty, Number};

            let mut meta = MetaMap::with_type_name("TestExternalData");

            meta.add_named_instance_fn("to_number", |data: &TestExternalData, _, _| {
                Ok(Number(data.x.into()))
            });

            meta.add_named_instance_fn_mut(
                "set_all_instances",
                |data: &mut TestExternalData, _, extra_args| {
                    let fn_name = "TestExternalData.set_all_instances";

                    match extra_args {
                        [Value::ExternalValue(other)] => {
                            match other.data().downcast_ref::<TestExternalData>() {
                                Some(other_data) => {
                                    data.x = other_data.x;
                                    Ok(Empty)
                                }
                                None => runtime_error!(
                                    "{} - unexpected other type: {}",
                                    fn_name,
                                    other.data().value_type(),
                                ),
                            }
                        }
                        _ => {
                            runtime_error!("{} - expected two ExternalData arguments", fn_name)
                        }
                    }
                },
                );

            meta.add_named_instance_fn(
                "get_data",
                |_data: &TestExternalData, value: &ExternalValue, _| {
                    Ok(Value::ExternalData(value.data.clone()))
                },
            );

            meta.add_unary_op(UnaryOp::Display, |data: &TestExternalData, _| {
                Ok(format!("TestExternalData: {}", data.x).into())
            });

            meta.add_unary_op(UnaryOp::Negate, |data: &TestExternalData, value| {
                let result = value.with_new_data(TestExternalData { x: -data.x });
                Ok(result.into())
            });

            meta.add_binary_op(
                BinaryOp::Add,
                |data_a: &TestExternalData, data_b, value_a, _| {
                    let result = value_a.with_new_data(TestExternalData {
                        x: data_a.x + data_b.x,
                    });
                    Ok(result.into())
                },
            );

            meta.add_binary_op(
                BinaryOp::Subtract,
                |data_a: &TestExternalData, data_b, value_a, _| {
                    let result = value_a.with_new_data(TestExternalData {
                        x: data_a.x - data_b.x,
                    });
                    Ok(result.into())
                },
            );

            meta.add_binary_op(
                BinaryOp::Multiply,
                |data_a: &TestExternalData, data_b, value_a, _| {
                    let result = value_a.with_new_data(TestExternalData {
                        x: data_a.x * data_b.x,
                    });
                    Ok(result.into())
                },
            );

            meta.add_binary_op(
                BinaryOp::Divide,
                |data_a: &TestExternalData, data_b, value_a, _| {
                    let result = value_a.with_new_data(TestExternalData {
                        x: data_a.x / data_b.x,
                    });
                    Ok(result.into())
                },
            );

            meta.add_binary_op(
                BinaryOp::Modulo,
                |data_a: &TestExternalData, data_b, value_a, _| {
                    let result = value_a.with_new_data(TestExternalData {
                        x: data_a.x % data_b.x,
                    });
                    Ok(result.into())
                },
            );

            meta.add_binary_op(BinaryOp::Less, |data_a: &TestExternalData, data_b, _, _| {
                Ok(Bool(data_a.x < data_b.x))
            });

            meta.add_binary_op(
                BinaryOp::LessOrEqual,
                |data_a: &TestExternalData, data_b, _, _| Ok(Bool(data_a.x <= data_b.x)),
            );

            meta.add_binary_op(
                BinaryOp::Greater,
                |data_a: &TestExternalData, data_b, _, _| Ok(Bool(data_a.x > data_b.x)),
            );

            meta.add_binary_op(
                BinaryOp::GreaterOrEqual,
                |data_a: &TestExternalData, data_b, _, _| Ok(Bool(data_a.x >= data_b.x)),
            );

            meta.add_binary_op(
                BinaryOp::Equal,
                |data_a: &TestExternalData, data_b, _, _| {
                    #[allow(clippy::float_cmp)]
                    Ok(Bool(data_a.x == data_b.x))
                },
            );

            meta.add_binary_op(
                BinaryOp::NotEqual,
                |data_a: &TestExternalData, data_b, _, _| {
                    #[allow(clippy::float_cmp)]
                    Ok(Bool(data_a.x != data_b.x))
                },
            );

            meta.add_binary_op_with_any_rhs(
                BinaryOp::Index,
                |data_a: &TestExternalData, _, value_b| match value_b {
                    Number(index) => {
                        let index = usize::from(index);
                        let result = data_a.x + index as f64;
                        Ok(Number(result.into()))
                    }
                    unexpected => runtime_error!(
                        "ExternalValue.@Index - Expected Number as argument, found {}",
                        unexpected.type_as_string()
                    ),
                },
            );

            Rc::new(RefCell::new(meta))
        }
    );

    fn test_script_with_external_value(script: &str, expected_output: Value) {
        let vm = Vm::default();
        let mut prelude = vm.prelude();

        prelude.add_fn("make_external", |vm, args| match vm.get_args(args) {
            [Value::Number(x)] => Ok(ExternalValue::with_shared_meta_map(
                TestExternalData { x: x.into() },
                EXTERNAL_META.with(|meta| meta.clone()),
            )
            .into()),
            [Value::ExternalData(data)] => Ok(ExternalValue {
                data: data.clone(),
                meta: EXTERNAL_META.with(|meta| meta.clone()),
            }
            .into()),
            _ => runtime_error!("make_external: Expected a Number or ExternalData as argument"),
        });

        test_script_with_vm(vm, script, expected_output);
    }

    mod named_functions {
        use super::*;

        #[test]
        fn to_number() {
            let script = "
x = make_external 42
x.to_number()
";
            test_script_with_external_value(script, 42.into());
        }

        #[test]
        fn set_all_instances() {
            let script = "
x = make_external 42
y = x
y.set_all_instances make_external 99
x.to_number()
";
            test_script_with_external_value(script, 99.into());
        }

        #[test]
        fn get_data() {
            let script = "
x = make_external 42
x_data = x.get_data()
y = make_external x_data
y.to_number()
";
            test_script_with_external_value(script, 42.into());
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
            test_script_with_external_value(script, 123.into());
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
            test_script_with_external_value(script, 33.into());
        }

        #[test]
        fn subtract() {
            let script = "
x = (make_external 99) - (make_external 90)
x.to_number()
";
            test_script_with_external_value(script, 9.into());
        }

        #[test]
        fn multiply() {
            let script = "
x = (make_external 3) * (make_external 11)
x.to_number()
";
            test_script_with_external_value(script, 33.into());
        }

        #[test]
        fn divide() {
            let script = "
x = (make_external 90) / (make_external 10)
x.to_number()
";
            test_script_with_external_value(script, 9.into());
        }

        #[test]
        fn modulo() {
            let script = "
x = (make_external 45) % (make_external 10)
x.to_number()
";
            test_script_with_external_value(script, 5.into());
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
            test_script_with_external_value(script, 123.into());
        }
    }
}
