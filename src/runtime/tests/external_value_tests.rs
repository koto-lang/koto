mod runtime_test_utils;

mod external_values {
    use {crate::runtime_test_utils::*, koto_runtime::prelude::*};

    #[derive(Clone, Copy, Debug)]
    struct TestData {
        x: f64,
    }

    impl TestData {
        fn make_value(x: f64) -> Value {
            Value::External(External::with_shared_meta_map(
                Self { x },
                EXTERNAL_META.with(|meta| meta.clone()),
            ))
        }
    }

    impl ExternalData for TestData {
        fn make_copy(&self) -> RcCell<dyn ExternalData> {
            (*self).into()
        }
    }

    thread_local! {
        static EXTERNAL_META: RcCell<MetaMap> = make_external_meta_map();
    }

    fn make_external_meta_map() -> RcCell<MetaMap> {
        use Value::{Bool, External, Null, Number};
        use {BinaryOp::*, UnaryOp::*};

        macro_rules! arithmetic_op {
            ($op:tt) => {
                |context| match context.args {
                    [External(b)] if b.has_data::<TestData>() => {
                        let b = b.data::<TestData>().unwrap();
                        Ok(TestData::make_value(context.data()?.x $op b.x))
                    }
                    [Number(n)] => {
                        Ok(TestData::make_value(context.data()?.x $op f64::from(n)))
                    }
                    unexpected => {
                        type_error_with_slice("a TestExternal or Number", unexpected)
                    }
                }
            }
        }

        macro_rules! assignment_op {
            ($op:tt) => {
                |context| match context.args {
                    [External(b)] if b.has_data::<TestData>() => {
                        let b = b.data::<TestData>().unwrap().x;
                        context.data_mut()?.x $op b;
                        context.ok_value()
                    }
                    [Number(n)] => {
                        context.data_mut()?.x $op f64::from(n);
                        context.ok_value()
                    }
                    unexpected => {
                        type_error_with_slice("a TestExternal or Number", unexpected)
                    }
                }
            }
        }

        macro_rules! comparison_op {
            ($op:tt) => {
                |context| match context.args {
                    [External(b)] if b.has_data::<TestData>() => {
                        let b = b.data::<TestData>().unwrap();
                        #[allow(clippy::float_cmp)]
                        Ok(Bool(context.data()?.x $op b.x))
                    }
                    [Number(n)] => {
                        #[allow(clippy::float_cmp)]
                        Ok(Bool(context.data()?.x $op f64::from(n)))
                    }
                    unexpected => {
                        type_error_with_slice("a TestExternal or Number", unexpected)
                    }
                }
            }
        }

        MetaMapBuilder::<TestData>::new("TestExternal")
            .function(Display, |context| {
                Ok(format!("TestExternal: {}", context.data()?.x).into())
            })
            .function(Negate, |context| {
                Ok(TestData::make_value(-context.data()?.x))
            })
            .function(Add, arithmetic_op!(+))
            .function(Subtract, arithmetic_op!(-))
            .function(Multiply, arithmetic_op!(*))
            .function(Divide, arithmetic_op!(/))
            .function(Remainder, arithmetic_op!(%))
            .function(AddAssign, assignment_op!(+=))
            .function(SubtractAssign, assignment_op!(-=))
            .function(MultiplyAssign, assignment_op!(*=))
            .function(DivideAssign, assignment_op!(/=))
            .function(RemainderAssign, assignment_op!(%=))
            .function(Less, comparison_op!(<))
            .function(LessOrEqual, comparison_op!(<=))
            .function(Greater, comparison_op!(>))
            .function(GreaterOrEqual, comparison_op!(>=))
            .function(Equal, comparison_op!(==))
            .function(NotEqual, comparison_op!(!=))
            .function(Index, |context| match context.args {
                [Number(index)] => {
                    let index = usize::from(index);
                    let result = context.data()?.x + index as f64;
                    Ok(result.into())
                }
                unexpected => type_error_with_slice("Number", unexpected),
            })
            .function(Iterator, |context| {
                Ok(ValueIterator::with_std_forward_iter(
                    ((context.data()?.x as usize)..).map(|n| ValueIteratorOutput::Value(n.into())),
                )
                .into())
            })
            .function(MetaKey::Call, |context| {
                Ok(Number(context.data()?.x.into()))
            })
            .function("to_number", |context| Ok(Number(context.data()?.x.into())))
            .function("invert", |context| {
                context.data_mut()?.x *= -1.0;
                Ok(Null)
            })
            .function("set_all_instances", |context| match context.args {
                [External(b)] if b.has_data::<TestData>() => {
                    let b_x = b.data::<TestData>().unwrap().x;
                    context.data_mut()?.x = b_x;
                    Ok(Null)
                }
                unexpected => type_error_with_slice("TestExternal", unexpected),
            })
            .function("absorb_values", |context| {
                let mut data = context.data_mut()?;
                for arg in context.args.iter() {
                    match arg {
                        Number(n) => data.x += f64::from(n),
                        other => return type_error("Number", other),
                    }
                }
                Ok(Null)
            })
            .build()
    }

    fn test_script_with_external(script: &str, expected_output: impl Into<Value>) {
        let vm = Vm::default();
        let prelude = vm.prelude();

        prelude.add_fn("make_external", |vm, args| match vm.get_args(args) {
            [Value::Number(x)] => Ok(TestData::make_value(x.into())),
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
            test_script_with_external(script, 42);
        }

        #[test]
        fn invert() {
            let script = "
x = make_external 42
x.invert()
x.to_number()
";
            test_script_with_external(script, -42.0_f64);
        }

        #[test]
        fn set_all_instances() {
            let script = "
x = make_external 42
y = x
y.set_all_instances make_external 99
x.to_number()
";
            test_script_with_external(script, 99);
        }

        #[test]
        fn absorb_values() {
            let script = "
x = make_external 42
x.absorb_values 10, 20, 30
x.to_number()
";
            test_script_with_external(script, 102);
        }
    }

    mod unary_op {
        use super::*;

        #[test]
        fn display() {
            let script = "'{}'.format make_external 42";
            test_script_with_external(script, string("TestExternal: 42"));
        }

        #[test]
        fn negate() {
            let script = "
x = make_external -123
x = -x
x.to_number()
";
            test_script_with_external(script, 123);
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
            test_script_with_external(script, 66);
        }

        #[test]
        fn subtract() {
            let script = "
x = (make_external 99) - (make_external 90) - 9
x.to_number()
";
            test_script_with_external(script, 0);
        }

        #[test]
        fn multiply() {
            let script = "
x = (make_external 3) * (make_external 11)
x.to_number()
";
            test_script_with_external(script, 33);
        }

        #[test]
        fn divide() {
            let script = "
x = (make_external 90) / (make_external 10)
x.to_number()
";
            test_script_with_external(script, 9);
        }

        #[test]
        fn remainder() {
            let script = "
x = (make_external 45) % (make_external 10)
x.to_number()
";
            test_script_with_external(script, 5);
        }

        #[test]
        fn add_assign() {
            let script = "
x = make_external 11
x += make_external 22
x += 33
x.to_number()
";
            test_script_with_external(script, 66);
        }

        #[test]
        fn add_assign_to_self() {
            let script = "
x = make_external 11
x += x
x.to_number()
";
            test_script_with_external(script, 22);
        }

        #[test]
        fn subtract_assign() {
            let script = "
x = make_external 42
x -= make_external 20
x -= 2
x.to_number()
";
            test_script_with_external(script, 20);
        }

        #[test]
        fn multiply_assign() {
            let script = "
x = make_external 3
x *= make_external 11
x *= 3
x.to_number()
";
            test_script_with_external(script, 99);
        }

        #[test]
        fn divide_assign() {
            let script = "
x = make_external 99
x /= make_external 3
x /= 3
x.to_number()
";
            test_script_with_external(script, 11);
        }

        #[test]
        fn remainder_assign() {
            let script = "
x = make_external 99
x %= make_external 90
x %= 5
x.to_number()
";
            test_script_with_external(script, 4);
        }

        #[test]
        fn less() {
            let script = "(make_external 1) < (make_external 2)";
            test_script_with_external(script, Bool(true));
        }

        #[test]
        fn less_or_equal() {
            let script = "(make_external 2) <= (make_external 2)";
            test_script_with_external(script, Bool(true));
        }

        #[test]
        fn equal() {
            let script = "(make_external 2) == (make_external 3)";
            test_script_with_external(script, Bool(false));
        }

        #[test]
        fn not_equal() {
            let script = "(make_external 2) != (make_external 3)";
            test_script_with_external(script, Bool(true));
        }

        #[test]
        fn index() {
            let script = "
x = make_external 100
x[23]
";
            test_script_with_external(script, 123);
        }

        #[test]
        fn multi_assignment_via_iterator() {
            let script = "
x = make_external 10
a, b, c = x
a, b, c
";
            test_script_with_external(script, number_tuple(&[10, 11, 12]));
        }

        #[test]
        fn call() {
            let script = "
x = make_external 256
x()
";
            test_script_with_external(script, 256);
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
            test_script_with_external(script, 100);
        }

        #[test]
        fn overloaded_binary_op_as_lookup_root() {
            let script = "
x = make_external 100
y = make_external 100
(x - y).to_number()
";
            test_script_with_external(script, 0);
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
            test_script_with_external(script, 150);
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
            test_script_with_external(script, 350);
        }
    }
}
