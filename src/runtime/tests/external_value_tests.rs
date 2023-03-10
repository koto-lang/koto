mod runtime_test_utils;

mod external_values {
    use {crate::runtime_test_utils::*, koto_runtime::prelude::*};

    #[derive(Clone, Copy, Debug)]
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

    impl ExternalData for TestExternalData {
        fn make_copy(&self) -> RcCell<dyn ExternalData> {
            (*self).into()
        }
    }

    thread_local! {
        static EXTERNAL_META: RcCell<MetaMap> = make_external_value_meta_map();
    }

    fn make_external_value_meta_map() -> RcCell<MetaMap> {
        use Value::{Bool, ExternalValue, Null, Number};
        use {BinaryOp::*, UnaryOp::*};

        macro_rules! arithmetic_op {
            ($op:tt) => {
                |a, b| match b {
                    [ExternalValue(b)] if b.has_data::<TestExternalData>() => {
                        let b = b.data::<TestExternalData>().unwrap();
                        Ok(TestExternalData::make_value(a.x $op b.x))
                    }
                    [Number(n)] => {
                        Ok(TestExternalData::make_value(a.x $op f64::from(n)))
                    }
                    unexpected => {
                        type_error_with_slice("a TestExternal or Number", unexpected)
                    }
                }
            }
        }

        macro_rules! assignment_op {
            ($op:tt) => {
                |value, args| match args {
                    [ExternalValue(b)] if b.has_data::<TestExternalData>() => {
                        let b = b.data::<TestExternalData>().unwrap().x;
                        value.data_mut::<TestExternalData>().unwrap().x $op b;
                        Ok(value.into())
                    }
                    [Number(n)] => {
                        value.data_mut::<TestExternalData>().unwrap().x $op f64::from(n);
                        Ok(value.into())
                    }
                    unexpected => {
                        type_error_with_slice("a TestExternal or Number", unexpected)
                    }
                }
            }
        }

        macro_rules! comparison_op {
            ($op:tt) => {
                |a, b| match b {
                    [ExternalValue(b)] if b.has_data::<TestExternalData>() => {
                        let b = b.data::<TestExternalData>().unwrap();
                        #[allow(clippy::float_cmp)]
                        Ok(Bool(a.x $op b.x))
                    }
                    [Number(n)] => {
                        #[allow(clippy::float_cmp)]
                        Ok(Bool(a.x $op f64::from(n)))
                    }
                    unexpected => {
                        type_error_with_slice("a TestExternal or Number", unexpected)
                    }
                }
            }
        }

        MetaMapBuilder::<TestExternalData>::new("TestExternalValue")
            .data_fn(Display, |data| {
                Ok(format!("TestExternalValue: {}", data.x).into())
            })
            .data_fn(Negate, |data| Ok(TestExternalData::make_value(-data.x)))
            .data_fn_with_args(Add, arithmetic_op!(+))
            .data_fn_with_args(Subtract, arithmetic_op!(-))
            .data_fn_with_args(Multiply, arithmetic_op!(*))
            .data_fn_with_args(Divide, arithmetic_op!(/))
            .data_fn_with_args(Remainder, arithmetic_op!(%))
            .value_fn(AddAssign, assignment_op!(+=))
            .value_fn(SubtractAssign, assignment_op!(-=))
            .value_fn(MultiplyAssign, assignment_op!(*=))
            .value_fn(DivideAssign, assignment_op!(/=))
            .value_fn(RemainderAssign, assignment_op!(%=))
            .data_fn_with_args(Less, comparison_op!(<))
            .data_fn_with_args(LessOrEqual, comparison_op!(<=))
            .data_fn_with_args(Greater, comparison_op!(>))
            .data_fn_with_args(GreaterOrEqual, comparison_op!(>=))
            .data_fn_with_args(Equal, comparison_op!(==))
            .data_fn_with_args(NotEqual, comparison_op!(!=))
            .data_fn_with_args(Index, |data, args| match args {
                [Number(index)] => {
                    let index = usize::from(index);
                    let result = data.x + index as f64;
                    Ok(result.into())
                }
                unexpected => type_error_with_slice("Number", unexpected),
            })
            .data_fn(Iterator, |data| {
                Ok(ValueIterator::with_std_forward_iter(
                    ((data.x as usize)..).map(|n| ValueIteratorOutput::Value(n.into())),
                )
                .into())
            })
            .data_fn(MetaKey::Call, |data| Ok(Number(data.x.into())))
            .data_fn("to_number", |data| Ok(Number(data.x.into())))
            .data_fn_mut("invert", |data| {
                data.x *= -1.0;
                Ok(Null)
            })
            .value_fn("set_all_instances", |a, b| match b {
                [ExternalValue(b)] if b.has_data::<TestExternalData>() => {
                    let b_x = b.data::<TestExternalData>().unwrap().x;
                    a.data_mut::<TestExternalData>().unwrap().x = b_x;
                    Ok(Null)
                }
                unexpected => type_error_with_slice("TestExternalValue", unexpected),
            })
            .data_fn_with_args_mut("absorb_values", |data, args| {
                for arg in args.iter() {
                    match arg {
                        Number(n) => data.x += f64::from(n),
                        other => return type_error("Number", other),
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
            _ => runtime_error!("make_external: Expected a Number"),
        });

        if let Err(e) = run_script_with_vm(vm, script, expected_output.into()) {
            panic!("{e}");
        }
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
            test_script_with_external_value(script, string("TestExternalValue: 42"));
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
x = (make_external 11) + (make_external 22) + 33
x.to_number()
";
            test_script_with_external_value(script, 66);
        }

        #[test]
        fn subtract() {
            let script = "
x = (make_external 99) - (make_external 90) - 9
x.to_number()
";
            test_script_with_external_value(script, 0);
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
        fn add_assign() {
            let script = "
x = make_external 11
x += make_external 22
x += 33
x.to_number()
";
            test_script_with_external_value(script, 66);
        }

        #[test]
        fn add_assign_to_self() {
            let script = "
x = make_external 11
x += x
x.to_number()
";
            test_script_with_external_value(script, 22);
        }

        #[test]
        fn subtract_assign() {
            let script = "
x = make_external 42
x -= make_external 20
x -= 2
x.to_number()
";
            test_script_with_external_value(script, 20);
        }

        #[test]
        fn multiply_assign() {
            let script = "
x = make_external 3
x *= make_external 11
x *= 3
x.to_number()
";
            test_script_with_external_value(script, 99);
        }

        #[test]
        fn divide_assign() {
            let script = "
x = make_external 99
x /= make_external 3
x /= 3
x.to_number()
";
            test_script_with_external_value(script, 11);
        }

        #[test]
        fn remainder_assign() {
            let script = "
x = make_external 99
x %= make_external 90
x %= 5
x.to_number()
";
            test_script_with_external_value(script, 4);
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

        #[test]
        fn multi_assignment_via_iterator() {
            let script = "
x = make_external 10
a, b, c = x
a, b, c
";
            test_script_with_external_value(script, number_tuple(&[10, 11, 12]));
        }

        #[test]
        fn call() {
            let script = "
x = make_external 256
x()
";
            test_script_with_external_value(script, 256);
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

    mod copy {
        use super::*;

        #[test]
        fn copy_makes_unique_value() {
            let script = "
x = make_external 100
y = x
z = copy x
y -= 100
z += 50
x + z
";
            test_script_with_external_value(script, 150);
        }

        #[test]
        fn deep_copy_makes_unique_value() {
            let script = "
x = make_external 100
y = x
z = deep_copy x
y -= 50
z += 200
x + z
";
            test_script_with_external_value(script, 350);
        }
    }
}
