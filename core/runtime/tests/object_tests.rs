mod runtime_test_utils;

mod objects {
    use crate::runtime_test_utils::*;
    use koto_runtime::{prelude::*, Result};

    #[derive(Clone, Copy, Debug)]
    struct TestObject {
        x: i64,
    }

    impl TestObject {
        fn make_value(x: i64) -> Value {
            Object::from(Self { x }).into()
        }
    }

    impl KotoType for TestObject {
        const TYPE: &'static str = "TestObject";
    }

    macro_rules! arithmetic_op {
        ($self:ident, $rhs:expr, $op:tt) => {
            {
                use Value::*;
                match $rhs {
                    Object(rhs) if rhs.is_a::<Self>() => {
                        let rhs = rhs.cast::<Self>().unwrap();
                        Ok(Self::make_value($self.x $op rhs.x))
                    }
                    Number(n) => {
                        Ok(Self::make_value($self.x $op i64::from(n)))
                    }
                    unexpected => {
                        type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                    }
                }
            }
        }
    }

    macro_rules! assignment_op {
        ($self:ident, $rhs:expr, $op:tt) => {
            {
                use Value::*;
                match $rhs {
                    Object(rhs) if rhs.is_a::<Self>() => {
                        let rhs = rhs.cast::<Self>().unwrap();
                        $self.x $op rhs.x;
                        Ok(())
                    }
                    Number(n) => {
                        $self.x $op i64::from(n);
                        Ok(())
                    }
                    unexpected => {
                        type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                    }
                }
            }
        }
    }

    macro_rules! comparison_op {
        ($self:ident, $rhs:expr, $op:tt) => {
            {
                use Value::*;
                match $rhs {
                    Object(rhs) if rhs.is_a::<Self>() => {
                        let rhs = rhs.cast::<Self>().unwrap();
                        #[allow(clippy::float_cmp)]
                        Ok($self.x $op rhs.x)
                    }
                    Number(n) => {
                        #[allow(clippy::float_cmp)]
                        Ok($self.x $op i64::from(n))
                    }
                    unexpected => {
                        type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                    }
                }
            }
        }
    }

    impl KotoObject for TestObject {
        fn object_type(&self) -> ValueString {
            TEST_OBJECT_TYPE_STRING.with(|s| s.clone())
        }

        fn copy(&self) -> Object {
            (*self).into()
        }

        fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
            ctx.append(format!("{}: {}", Self::TYPE, self.x));
            Ok(())
        }

        fn lookup(&self, key: &ValueKey) -> Option<Value> {
            TEST_OBJECT_ENTRIES.with(|entries| entries.get(key).cloned())
        }

        fn index(&self, index: &Value) -> Result<Value> {
            match index {
                Value::Number(index) => {
                    let result = self.x + i64::from(index);
                    Ok(result.into())
                }
                unexpected => type_error("Number as index", unexpected),
            }
        }

        fn call(&mut self, _ctx: &mut CallContext) -> Result<Value> {
            Ok(self.x.into())
        }

        fn negate(&self, _vm: &mut Vm) -> Result<Value> {
            Ok(Self::make_value(-self.x))
        }

        fn add(&self, rhs: &Value) -> Result<Value> {
            arithmetic_op!(self, rhs, +)
        }

        fn subtract(&self, rhs: &Value) -> Result<Value> {
            arithmetic_op!(self, rhs, -)
        }

        fn multiply(&self, rhs: &Value) -> Result<Value> {
            arithmetic_op!(self, rhs, *)
        }

        fn divide(&self, rhs: &Value) -> Result<Value> {
            arithmetic_op!(self, rhs, /)
        }

        fn remainder(&self, rhs: &Value) -> Result<Value> {
            arithmetic_op!(self, rhs, %)
        }

        fn add_assign(&mut self, rhs: &Value) -> Result<()> {
            assignment_op!(self, rhs, +=)
        }

        fn subtract_assign(&mut self, rhs: &Value) -> Result<()> {
            assignment_op!(self, rhs, -=)
        }

        fn multiply_assign(&mut self, rhs: &Value) -> Result<()> {
            assignment_op!(self, rhs, *=)
        }

        fn divide_assign(&mut self, rhs: &Value) -> Result<()> {
            assignment_op!(self, rhs, /=)
        }

        fn remainder_assign(&mut self, rhs: &Value) -> Result<()> {
            assignment_op!(self, rhs, %=)
        }

        fn less(&self, rhs: &Value) -> Result<bool> {
            comparison_op!(self, rhs, <)
        }

        fn less_or_equal(&self, rhs: &Value) -> Result<bool> {
            comparison_op!(self, rhs, <=)
        }

        fn greater(&self, rhs: &Value) -> Result<bool> {
            comparison_op!(self, rhs, >)
        }

        fn greater_or_equal(&self, rhs: &Value) -> Result<bool> {
            comparison_op!(self, rhs, >=)
        }

        fn equal(&self, rhs: &Value) -> Result<bool> {
            comparison_op!(self, rhs, ==)
        }

        fn not_equal(&self, rhs: &Value) -> Result<bool> {
            comparison_op!(self, rhs, !=)
        }

        fn is_iterable(&self) -> IsIterable {
            IsIterable::Iterable
        }

        fn make_iterator(&self, vm: &mut Vm) -> Result<ValueIterator> {
            ValueIterator::with_object(vm.spawn_shared_vm(), TestIterator::make_object(self.x))
        }
    }

    fn test_object_entries() -> ValueMap {
        use Value::*;

        ObjectEntryBuilder::<TestObject>::new()
            .method("to_number", |ctx| Ok(Number(ctx.instance()?.x.into())))
            .method("invert", |ctx| {
                ctx.instance_mut()?.x *= -1;
                Ok(Null)
            })
            .method("set_all_instances", |ctx| match ctx.args {
                [Object(b)] if b.is_a::<TestObject>() => {
                    let b_x = b.cast::<TestObject>().unwrap().x;
                    ctx.instance_mut()?.x = b_x;
                    Ok(Null)
                }
                unexpected => type_error_with_slice("TestExternal", unexpected),
            })
            .method("absorb_values", |ctx| {
                let mut data = ctx.instance_mut()?;
                for arg in ctx.args.iter() {
                    match arg {
                        Number(n) => data.x += i64::from(n),
                        other => return type_error("Number", other),
                    }
                }
                Ok(Null)
            })
            .build()
    }

    thread_local! {
        static TEST_OBJECT_TYPE_STRING: ValueString = TestObject::TYPE.into();
        static TEST_OBJECT_ENTRIES: ValueMap = test_object_entries();
    }

    #[derive(Clone, Copy, Debug)]
    struct TestIterator {
        x: i64,
    }

    impl TestIterator {
        fn make_object(x: i64) -> Object {
            Object::from(Self { x })
        }
    }

    impl KotoType for TestIterator {
        const TYPE: &'static str = "TestIterator";
    }

    impl KotoObject for TestIterator {
        fn object_type(&self) -> ValueString {
            TEST_ITERATOR_TYPE_STRING.with(|s| s.clone())
        }

        fn copy(&self) -> Object {
            (*self).into()
        }

        fn is_iterable(&self) -> IsIterable {
            IsIterable::BidirectionalIterator
        }

        fn iterator_next(&mut self, _vm: &mut Vm) -> Option<ValueIteratorOutput> {
            self.x += 1;
            Some(self.x.into())
        }

        fn iterator_next_back(&mut self, _vm: &mut Vm) -> Option<ValueIteratorOutput> {
            self.x -= 1;
            Some(self.x.into())
        }
    }

    thread_local! {
        static TEST_ITERATOR_TYPE_STRING: ValueString = TestIterator::TYPE.into();
    }

    fn test_object_script(script: &str, expected_output: impl Into<Value>) {
        let vm = Vm::default();
        let prelude = vm.prelude();

        prelude.add_fn("make_object", |ctx| match ctx.args() {
            [Value::Number(x)] => Ok(TestObject::make_value(x.into())),
            _ => runtime_error!("make_object: Expected a Number"),
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
x = make_object 42
x.to_number()
";
            test_object_script(script, 42);
        }

        #[test]
        fn invert() {
            let script = "
x = make_object 42
x.invert()
x.to_number()
";
            test_object_script(script, -42.0_f64);
        }

        #[test]
        fn set_all_instances() {
            let script = "
x = make_object 42
y = x
y.set_all_instances make_object 99
x.to_number()
";
            test_object_script(script, 99);
        }

        #[test]
        fn absorb_values() {
            let script = "
x = make_object 42
x.absorb_values 10, 20, 30
x.to_number()
";
            test_object_script(script, 102);
        }
    }

    mod unary_op {
        use super::*;

        #[test]
        fn display() {
            let script = "'{}'.format make_object 42";
            test_object_script(script, string("TestObject: 42"));
        }

        #[test]
        fn negate() {
            let script = "
x = make_object -123
x = -x
x.to_number()
";
            test_object_script(script, 123);
        }
    }

    mod iterator {
        use super::*;

        #[test]
        fn multi_assignment() {
            let script = "
x = make_object 10
a, b, c = x
a, b, c
";
            test_object_script(script, number_tuple(&[11, 12, 13]));
        }

        #[test]
        fn bidirectional() {
            let script = "
make_object(10)
  .skip 3
  .reversed()
  .take 3
  .to_tuple()
";
            test_object_script(script, number_tuple(&[12, 11, 10]));
        }
    }

    mod binary_op {
        use super::*;
        use Value::Bool;

        #[test]
        fn add() {
            let script = "
x = (make_object 11) + (make_object 22) + 33
x.to_number()
";
            test_object_script(script, 66);
        }

        #[test]
        fn subtract() {
            let script = "
x = (make_object 99) - (make_object 90) - 9
x.to_number()
";
            test_object_script(script, 0);
        }

        #[test]
        fn multiply() {
            let script = "
x = (make_object 3) * (make_object 11)
x.to_number()
";
            test_object_script(script, 33);
        }

        #[test]
        fn divide() {
            let script = "
x = (make_object 90) / (make_object 10)
x.to_number()
";
            test_object_script(script, 9);
        }

        #[test]
        fn remainder() {
            let script = "
x = (make_object 45) % (make_object 10)
x.to_number()
";
            test_object_script(script, 5);
        }

        #[test]
        fn add_assign() {
            let script = "
x = make_object 11
x += make_object 22
x += 33
x.to_number()
";
            test_object_script(script, 66);
        }

        #[test]
        fn add_assign_to_self() {
            let script = "
x = make_object 11
x += x
x.to_number()
";
            test_object_script(script, 22);
        }

        #[test]
        fn subtract_assign() {
            let script = "
x = make_object 42
x -= make_object 20
x -= 2
x.to_number()
";
            test_object_script(script, 20);
        }

        #[test]
        fn subtract_assign_to_self() {
            let script = "
x = make_object 11
x -= x
x.to_number()
";
            test_object_script(script, 0);
        }

        #[test]
        fn multiply_assign() {
            let script = "
x = make_object 3
x *= make_object 11
x *= 3
x.to_number()
";
            test_object_script(script, 99);
        }

        #[test]
        fn mutliply_assign_to_self() {
            let script = "
x = make_object 11
x *= x
x.to_number()
";
            test_object_script(script, 121);
        }

        #[test]
        fn divide_assign() {
            let script = "
x = make_object 99
x /= make_object 3
x /= 3
x.to_number()
";
            test_object_script(script, 11);
        }

        #[test]
        fn divide_assign_to_self() {
            let script = "
x = make_object 11
x /= x
x.to_number()
";
            test_object_script(script, 1);
        }

        #[test]
        fn remainder_assign() {
            let script = "
x = make_object 99
x %= make_object 90
x %= 5
x.to_number()
";
            test_object_script(script, 4);
        }

        #[test]
        fn remainder_assign_to_self() {
            let script = "
x = make_object 11
x /= x
x.to_number()
";
            test_object_script(script, 1);
        }

        #[test]
        fn less() {
            let script = "(make_object 1) < (make_object 2)";
            test_object_script(script, Bool(true));
        }

        #[test]
        fn less_or_equal() {
            let script = "(make_object 2) <= (make_object 2)";
            test_object_script(script, Bool(true));
        }

        #[test]
        fn equal() {
            let script = "(make_object 2) == (make_object 3)";
            test_object_script(script, Bool(false));
        }

        #[test]
        fn not_equal() {
            let script = "(make_object 2) != (make_object 3)";
            test_object_script(script, Bool(true));
        }

        #[test]
        fn index() {
            let script = "
x = make_object 100
x[23]
";
            test_object_script(script, 123);
        }
    }

    #[test]
    fn call() {
        let script = "
x = make_object 256
x()
";
        test_object_script(script, 256);
    }

    mod temporaries {
        use super::*;

        #[test]
        fn overloaded_unary_op_as_lookup_root() {
            let script = "
x = make_object -100
(-x).to_number()
";
            test_object_script(script, 100);
        }

        #[test]
        fn overloaded_binary_op_as_lookup_root() {
            let script = "
x = make_object 100
y = make_object 100
(x - y).to_number()
";
            test_object_script(script, 0);
        }
    }

    mod copy {
        use super::*;

        #[test]
        fn copy_makes_unique_value() {
            let script = "
x = make_object 100
y = x
z = copy x
y -= 100
z += 50
x + z
";
            test_object_script(script, 150);
        }

        #[test]
        fn deep_copy_makes_unique_value() {
            let script = "
x = make_object 100
y = x
z = deep_copy x
y -= 50
z += 200
x + z
";
            test_object_script(script, 350);
        }
    }
}
