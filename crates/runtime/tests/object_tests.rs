mod objects {
    use std::ptr;

    use indexmap::IndexMap;
    use koto_memory::Address;
    use koto_runtime::{Result, derive::*, prelude::*};
    use koto_test_utils::*;

    #[derive(Clone, Copy, Debug, KotoCopy, KotoType)]
    #[koto(runtime = koto_runtime, use_copy)]
    struct TestObject {
        x: i64,
    }

    #[koto_impl(runtime = koto_runtime)]
    impl TestObject {
        fn make_value(x: i64) -> KValue {
            KObject::from(Self { x }).into()
        }

        #[koto_method]
        fn as_number(&self) -> KValue {
            self.x.into()
        }

        #[koto_method]
        fn invert(&mut self) {
            self.x *= -1;
        }

        #[koto_method(alias = "absorb1", alias = "absorb2")]
        fn absorb_values(&mut self, args: &[KValue]) -> Result<KValue> {
            for arg in args.iter() {
                match arg {
                    KValue::Number(n) => self.x += i64::from(n),
                    other => return unexpected_type("Number", other),
                }
            }
            Ok(KValue::Null)
        }

        #[koto_method]
        fn set_all_instances(ctx: MethodContext<Self>) -> Result<KValue> {
            match ctx.args {
                [KValue::Object(b)] if b.is_a::<TestObject>() => {
                    let b_x = b.cast::<TestObject>().unwrap().x;
                    ctx.instance_mut()?.x = b_x;
                    Ok(KValue::Null)
                }
                unexpected => unexpected_args("|TestExternal|", unexpected),
            }
        }
    }

    macro_rules! arithmetic_op {
        ($self:ident, $other:expr, $op:tt) => {
            {
                use KValue::*;
                match $other {
                    Object(other) if other.is_a::<Self>() => {
                        let other = other.cast::<Self>().unwrap();
                        Ok(Self::make_value($self.x $op other.x))
                    }
                    Number(n) => {
                        Ok(Self::make_value($self.x $op i64::from(n)))
                    }
                    unexpected => {
                        unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                    }
                }
            }
        }
    }

    macro_rules! arithmetic_op_rhs {
        ($self:ident, $other:expr, $op:tt) => {
            {
                match $other {
                    KValue::Number(n) => {
                        Ok(Self::make_value(i64::from(n) $op $self.x))
                    }
                    unexpected => {
                        unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                    }
                }
            }
        }
    }

    macro_rules! assignment_op {
        ($self:ident, $other:expr, $op:tt) => {
            {
                use KValue::*;
                match $other {
                    Object(other) if other.is_a::<Self>() => {
                        let other = other.cast::<Self>().unwrap();
                        $self.x $op other.x;
                        Ok(())
                    }
                    Number(n) => {
                        $self.x $op i64::from(n);
                        Ok(())
                    }
                    unexpected => {
                        unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                    }
                }
            }
        }
    }

    macro_rules! comparison_op {
        ($self:ident, $other:expr, $op:tt) => {
            {
                use KValue::*;
                match $other {
                    Object(other) if other.is_a::<Self>() => {
                        let other = other.cast::<Self>().unwrap();
                        #[allow(clippy::float_cmp)]
                        Ok($self.x $op other.x)
                    }
                    Number(n) => {
                        #[allow(clippy::float_cmp)]
                        Ok($self.x $op i64::from(n))
                    }
                    unexpected => {
                        unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                    }
                }
            }
        }
    }

    impl KotoObject for TestObject {
        fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
            if ctx.debug_enabled() {
                ctx.append('{');
            }

            ctx.append(format!("{}: {}", self.type_string(), self.x));

            if ctx.debug_enabled() {
                ctx.append('}');
            }

            Ok(())
        }

        fn index(&self, index: &KValue) -> Result<KValue> {
            match index {
                KValue::Number(index) => {
                    let result = self.x + i64::from(index);
                    Ok(result.into())
                }
                KValue::Range(range) => match (range.start(), range.end()) {
                    (Some(start), Some((end, inclusive))) => {
                        let end = if inclusive {
                            self.x + end + 1
                        } else {
                            self.x + end
                        };
                        Ok(KRange::from((self.x + start)..end).into())
                    }
                    _ => unimplemented!(),
                },
                unexpected => unexpected_type("Number as index", unexpected),
            }
        }

        fn index_assign(&mut self, index: &KValue, value: &KValue) -> Result<()> {
            match index {
                KValue::Number(index) => {
                    assert_eq!(usize::from(index), 0);
                    match value {
                        KValue::Number(value) => {
                            self.x = value.into();
                            Ok(())
                        }
                        unexpected => unexpected_type("Number as value", unexpected),
                    }
                }
                unexpected => unexpected_type("Number as index", unexpected),
            }
        }

        fn size(&self) -> Option<usize> {
            Some(self.x.unsigned_abs() as usize)
        }

        fn is_callable(&self) -> bool {
            true
        }

        fn call(&mut self, _ctx: &mut CallContext) -> Result<KValue> {
            Ok(self.x.into())
        }

        fn negate(&self) -> Result<KValue> {
            Ok(Self::make_value(-self.x))
        }

        fn add(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op!(self, other, +)
        }

        fn add_rhs(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op_rhs!(self, other, +)
        }

        fn subtract(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op!(self, other, -)
        }

        fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op_rhs!(self, other, -)
        }

        fn multiply(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op!(self, other, *)
        }

        fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op_rhs!(self, other, *)
        }

        fn divide(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op!(self, other, /)
        }

        fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op_rhs!(self, other, /)
        }

        fn remainder(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op!(self, other, %)
        }

        fn remainder_rhs(&self, other: &KValue) -> Result<KValue> {
            arithmetic_op_rhs!(self, other, %)
        }

        fn power(&self, other: &KValue) -> Result<KValue> {
            match other {
                KValue::Object(other) if other.is_a::<Self>() => {
                    let other = other.cast::<Self>().unwrap();
                    Ok(Self::make_value(self.x.pow(other.x as u32)))
                }
                KValue::Number(n) => Ok(Self::make_value(self.x.pow(u32::from(n)))),
                unexpected => {
                    unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                }
            }
        }

        fn power_rhs(&self, other: &KValue) -> Result<KValue> {
            match other {
                KValue::Number(n) => Ok(Self::make_value(i64::from(n).pow(self.x as u32))),
                unexpected => {
                    unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                }
            }
        }

        fn add_assign(&mut self, other: &KValue) -> Result<()> {
            assignment_op!(self, other, +=)
        }

        fn subtract_assign(&mut self, other: &KValue) -> Result<()> {
            assignment_op!(self, other, -=)
        }

        fn multiply_assign(&mut self, other: &KValue) -> Result<()> {
            assignment_op!(self, other, *=)
        }

        fn divide_assign(&mut self, other: &KValue) -> Result<()> {
            assignment_op!(self, other, /=)
        }

        fn remainder_assign(&mut self, other: &KValue) -> Result<()> {
            assignment_op!(self, other, %=)
        }

        fn power_assign(&mut self, other: &KValue) -> Result<()> {
            use KValue::*;
            match other {
                Object(other) if other.is_a::<Self>() => {
                    let other = other.cast::<Self>().unwrap();
                    self.x = self.x.pow(other.x as u32);
                    Ok(())
                }
                Number(n) => {
                    self.x = self.x.pow(u32::from(n));
                    Ok(())
                }
                unexpected => {
                    unexpected_type(&format!("a {} or Number", Self::type_static()), unexpected)
                }
            }
        }

        fn less(&self, other: &KValue) -> Result<bool> {
            comparison_op!(self, other, <)
        }

        fn less_or_equal(&self, other: &KValue) -> Result<bool> {
            comparison_op!(self, other, <=)
        }

        fn greater(&self, other: &KValue) -> Result<bool> {
            comparison_op!(self, other, >)
        }

        fn greater_or_equal(&self, other: &KValue) -> Result<bool> {
            comparison_op!(self, other, >=)
        }

        fn equal(&self, other: &KValue) -> Result<bool> {
            comparison_op!(self, other, ==)
        }

        fn not_equal(&self, other: &KValue) -> Result<bool> {
            comparison_op!(self, other, !=)
        }

        fn is_iterable(&self) -> IsIterable {
            IsIterable::Iterable
        }

        fn make_iterator(&self, vm: &mut KotoVm) -> Result<KIterator> {
            KIterator::with_object(vm.spawn_shared_vm(), TestIterator::make_object(self.x))
        }
    }

    #[derive(Clone, Debug, KotoCopy, KotoType)]
    #[koto(runtime = koto_runtime)]
    struct TestObjectAccess {
        field: KValue,
        field_for_override: KValue,
        field_for_fallback: KValue,
    }

    impl TestObjectAccess {
        fn make_value() -> KValue {
            KObject::from(Self {
                field: KValue::Null,
                field_for_override: KValue::Null,
                field_for_fallback: KValue::Null,
            })
            .into()
        }
    }

    #[koto_impl(runtime = koto_runtime)]
    impl TestObjectAccess {
        #[koto_access]
        fn field(&self) -> KValue {
            self.field.clone()
        }

        #[koto_access_assign]
        fn set_field(&mut self, data: &KValue) {
            self.field = data.clone();
        }

        // For testing naming and aliases.
        #[koto_access(name = "field_1", alias = "field_2", alias = "field_3")]
        fn field_x(&self) -> KValue {
            self.field.clone()
        }

        #[koto_access_assign(name = "field_1", alias = "field_2", alias = "field_3")]
        fn set_field_x(&mut self, data: &KValue) {
            self.field = data.clone();
        }

        #[koto_method]
        fn method(&self) -> KValue {
            "method".into()
        }

        #[koto_access_override]
        fn access_override(&self, key: &KString) -> Option<KValue> {
            if key.as_str() == "field_for_override" {
                return Some(self.field_for_override.clone());
            }

            None
        }

        #[koto_access_fallback]
        fn access_fallback(&self, key: &KString) -> Result<Option<KValue>> {
            if key.as_str() != "field_for_fallback" {
                return runtime_error!("unexpected key '{key}'");
            }

            Ok(Some(self.field_for_fallback.clone()))
        }

        #[koto_access_assign_override]
        fn access_assign_override(&mut self, key: &KString, value: &KValue) -> bool {
            if key.as_str() == "field_for_override" {
                self.field_for_override = value.clone();
                return true;
            }

            false
        }

        #[koto_access_assign_fallback]
        fn access_assign_fallback(&mut self, key: &KString, value: &KValue) -> Result<()> {
            if key.as_str() != "field_for_fallback" {
                return runtime_error!("unexpected key '{key}'");
            }

            self.field_for_fallback = value.clone();
            Ok(())
        }
    }

    impl KotoObject for TestObjectAccess {}

    /// An object that behaves similar to a `KMap`.
    #[derive(Clone, Debug, KotoCopy, KotoType)]
    #[koto(runtime = koto_runtime)]
    struct MapLikeObject {
        map: IndexMap<KString, KValue>,
    }

    impl MapLikeObject {
        fn make_value() -> KValue {
            KObject::from(Self {
                map: IndexMap::new(),
            })
            .into()
        }
    }

    #[koto_impl(runtime = koto_runtime)]
    impl MapLikeObject {
        #[koto_method]
        fn length(&self) -> KValue {
            self.map.len().into()
        }

        #[koto_access_override]
        fn access_override(&self, key: &KString) -> Result<Option<KValue>> {
            Ok(self.map.get(key).cloned())
        }

        // This object has no other `access_assign` entries, so it doesn't
        // matter whether we use `_override` or `_fallback` here.
        #[koto_access_assign_fallback]
        fn access_assign_fallback(&mut self, key: &KString, value: &KValue) -> Result<()> {
            self.map.insert(key.clone(), value.clone());
            Ok(())
        }
    }

    impl KotoObject for MapLikeObject {
        fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
            if ctx.debug_enabled() {
                ctx.append('{');
            }

            ctx.append(self.type_string());
            ctx.append(": {");
            ctx.push_container(Address::from(ptr::from_ref(self)));

            for (i, (key, value)) in self.map.iter().enumerate() {
                if i > 0 {
                    ctx.append(", ");
                }

                ctx.append(key);
                ctx.append(": ");
                value.display(ctx)?;
            }

            ctx.append('}');
            ctx.pop_container();

            if ctx.debug_enabled() {
                ctx.append('}');
            }

            Ok(())
        }
    }

    #[derive(Clone, Debug, KotoCopy, KotoType)]
    #[koto(runtime = koto_runtime)]
    struct TestIterator {
        x: i64,
    }

    impl TestIterator {
        fn make_object(x: i64) -> KObject {
            KObject::from(Self { x })
        }
    }

    impl KotoAccess for TestIterator {}

    impl KotoObject for TestIterator {
        fn is_iterable(&self) -> IsIterable {
            IsIterable::BidirectionalIterator
        }

        fn iterator_next(&mut self, _vm: &mut KotoVm) -> Option<KIteratorOutput> {
            self.x += 1;
            Some(self.x.into())
        }

        fn iterator_next_back(&mut self, _vm: &mut KotoVm) -> Option<KIteratorOutput> {
            self.x -= 1;
            Some(self.x.into())
        }
    }

    #[derive(Clone, KotoCopy, KotoType)]
    #[koto(runtime = koto_runtime)]
    struct GenericObject<T>
    where
        T: KotoField,
        KValue: From<T>,
    {
        value: T,
    }

    #[koto_impl(runtime = koto_runtime)]
    impl<T> GenericObject<T>
    where
        T: KotoField,
        KValue: From<T>,
    {
        fn make_value(value: T) -> KValue {
            KObject::from(Self { value }).into()
        }

        #[koto_method]
        fn get(&self) -> KValue {
            self.value.clone().into()
        }

        #[koto_access]
        fn value(&self) -> KValue {
            self.value.clone().into()
        }

        #[koto_access_assign]
        fn set_value(&mut self, value: &KValue) {
            _ = value;
            // not implemented, just checking that it compiles
        }
    }

    impl<T> KotoObject for GenericObject<T>
    where
        T: KotoField,
        KValue: From<T>,
    {
    }

    fn test_object_script(script: &str, expected_output: impl Into<KValue>) {
        let vm = KotoVm::default();
        let prelude = vm.prelude();

        prelude.add_fn("make_object", make_object);
        prelude.add_fn("make_object_access", make_object_access);
        prelude.add_fn("make_map_like", make_map_like);
        prelude.add_fn("make_generic", make_generic);

        if let Err(e) = check_script_output_with_vm(vm, script, expected_output.into()) {
            panic!("{e}");
        }
    }

    koto_fn! {
        runtime = koto_runtime;

        fn make_object(x: i64) -> KValue {
            TestObject::make_value(x)
        }

        fn make_object_access() -> KValue {
            TestObjectAccess::make_value()
        }

        fn make_map_like() -> KValue {
            MapLikeObject::make_value()
        }

        fn make_generic(x: bool) -> KValue {
             GenericObject::<bool>::make_value(x)
        }

        fn make_generic(x: KNumber) -> KValue {
             GenericObject::<KNumber>::make_value(x)
        }

        fn make_generic(x: &KString) -> KValue {
             GenericObject::<KString>::make_value(x.clone())
        }
    }

    mod named_functions {
        use super::*;

        #[test]
        fn as_number() {
            let script = "
x = make_object 42
x.as_number()
";
            test_object_script(script, 42);
        }

        #[test]
        fn invert() {
            let script = "
x = make_object 42
x.invert()
x.as_number()
";
            test_object_script(script, -42.0_f64);
        }

        #[test]
        fn set_all_instances() {
            let script = "
x = make_object 42
y = x
y.set_all_instances make_object 99
x.as_number()
";
            test_object_script(script, 99);
        }

        #[test]
        fn absorb_values() {
            let script = "
x = make_object 42
x.absorb_values 10, 20, 30
x.as_number()
";
            test_object_script(script, 102);
        }

        #[test]
        fn absorb_values_aliased_1() {
            let script = "
x = make_object 1
x.absorb1 2, 3, 4, 5
x.as_number()
";
            test_object_script(script, 15);
        }

        #[test]
        fn absorb_values_aliased_2() {
            let script = "
x = make_object 10
x.absorb2 20, 30
x.as_number()
";
            test_object_script(script, 60);
        }
    }

    mod generic_object {
        use super::*;

        #[test]
        fn bool() {
            let script = "
x = make_generic true
x.get()
";
            test_object_script(script, true);
        }

        #[test]
        fn number() {
            let script = "
x = make_generic 99
x.get()
";
            test_object_script(script, 99);
        }

        #[test]
        fn string() {
            let script = "
x = make_generic 'hello'
x.get()
";
            test_object_script(script, "hello");
        }

        #[test]
        fn combined() {
            let script = "
b = make_generic true
s = make_generic '@'
n = make_generic 3
if b.get()
  iterator.repeat(s.get(), n.get()).to_string()
else
  'error'
";
            test_object_script(script, "@@@");
        }

        #[test]
        fn access() {
            let script = "
x = make_generic 99
x.value
";
            test_object_script(script, 99);
        }
    }

    mod unary_op {
        use super::*;

        #[test]
        fn display() {
            let script = "'{make_object 42}'";
            test_object_script(script, "TestObject: 42");
        }

        #[test]
        fn debug() {
            let script = "'{make_object 42:?}'";
            test_object_script(script, "{TestObject: 42}");
        }

        #[test]
        fn negate() {
            let script = "
x = make_object -123
x = -x
x.as_number()
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

        #[test]
        fn add() {
            let script = "
x = (make_object 11) + (make_object 22)
y = 33 + x
y.as_number()
";
            test_object_script(script, 66);
        }

        #[test]
        fn subtract() {
            let script = "
x = (make_object 99) - (make_object 90) - 1
y = 8 - x
y.as_number()
";
            test_object_script(script, 0);
        }

        #[test]
        fn multiply() {
            let script = "
x = (make_object 3) * (make_object 11)
y = 10 * x
y.as_number()
";
            test_object_script(script, 330);
        }

        #[test]
        fn divide() {
            let script = "
x = (make_object 90) / (make_object 10)
y = 9 / x
y.as_number()
";
            test_object_script(script, 1);
        }

        #[test]
        fn remainder() {
            let script = "
x = (make_object 45) % (make_object 10)
y = 12 % x
y.as_number()
";
            test_object_script(script, 2);
        }

        #[test]
        fn power() {
            let script = "
x = (make_object 2) ^ (make_object 3)
y = 2 ^ x
y.as_number()
";
            test_object_script(script, 256);
        }

        #[test]
        fn add_assign() {
            let script = "
x = make_object 11
x += make_object 22
x += 33
x.as_number()
";
            test_object_script(script, 66);
        }

        #[test]
        fn add_assign_to_self() {
            let script = "
x = make_object 11
x += x
x.as_number()
";
            test_object_script(script, 22);
        }

        #[test]
        fn subtract_assign() {
            let script = "
x = make_object 42
x -= make_object 20
x -= 2
x.as_number()
";
            test_object_script(script, 20);
        }

        #[test]
        fn subtract_assign_to_self() {
            let script = "
x = make_object 11
x -= x
x.as_number()
";
            test_object_script(script, 0);
        }

        #[test]
        fn multiply_assign() {
            let script = "
x = make_object 3
x *= make_object 11
x *= 3
x.as_number()
";
            test_object_script(script, 99);
        }

        #[test]
        fn mutliply_assign_to_self() {
            let script = "
x = make_object 11
x *= x
x.as_number()
";
            test_object_script(script, 121);
        }

        #[test]
        fn divide_assign() {
            let script = "
x = make_object 99
x /= make_object 3
x /= 3
x.as_number()
";
            test_object_script(script, 11);
        }

        #[test]
        fn divide_assign_to_self() {
            let script = "
x = make_object 11
x /= x
x.as_number()
";
            test_object_script(script, 1);
        }

        #[test]
        fn remainder_assign() {
            let script = "
x = make_object 99
x %= make_object 90
x %= 5
x.as_number()
";
            test_object_script(script, 4);
        }

        #[test]
        fn remainder_assign_to_self() {
            let script = "
x = make_object 11
x %= x
x.as_number()
";
            test_object_script(script, 0);
        }

        #[test]
        fn power_assign() {
            let script = "
x = make_object 2
x ^= make_object 3
x ^= 2
x.as_number()
";
            test_object_script(script, 64);
        }

        #[test]
        fn power_assign_to_self() {
            let script = "
x = make_object 3
x ^= x
x.as_number()
";
            test_object_script(script, 27);
        }

        #[test]
        fn less() {
            let script = "(make_object 1) < (make_object 2)";
            test_object_script(script, true);
        }

        #[test]
        fn less_or_equal() {
            let script = "(make_object 2) <= (make_object 2)";
            test_object_script(script, true);
        }

        #[test]
        fn equal() {
            let script = "(make_object 2) == (make_object 3)";
            test_object_script(script, false);
        }

        #[test]
        fn not_equal() {
            let script = "(make_object 2) != (make_object 3)";
            test_object_script(script, true);
        }

        #[test]
        fn equal_null_lhs() {
            let script = "(make_object 2) == null";
            test_object_script(script, false);
        }

        #[test]
        fn equal_null_rhs() {
            let script = "null == (make_object 2)";
            test_object_script(script, false);
        }

        #[test]
        fn not_equal_null_lhs() {
            let script = "(make_object 2) != null";
            test_object_script(script, true);
        }

        #[test]
        fn not_equal_null_rhs() {
            let script = "null != (make_object 2)";
            test_object_script(script, true);
        }
    }

    mod type_hints {
        use super::*;

        #[test]
        fn let_expression() {
            let script = "
let x: TestObject = make_object 256
x.as_number()
";
            test_object_script(script, 256);
        }

        #[test]
        fn callable() {
            let script = "
let x: Callable = make_object 256
x()
";
            test_object_script(script, 256);
        }

        #[test]
        fn iterable() {
            let script = "
let x: Iterable = make_object 256
x.as_number()
";
            test_object_script(script, 256);
        }

        #[test]
        fn indexable() {
            let script = "
let x: Indexable = make_object 256
x.as_number()
";
            test_object_script(script, 256);
        }
    }

    mod index_and_size {
        use super::*;

        #[test]
        fn index() {
            let script = "
x = make_object 100
x[23]
";
            test_object_script(script, 123);
        }

        #[test]
        fn size() {
            let script = "
x = make_object 42
# Report x as the size
koto.size x
";
            test_object_script(script, 42);
        }

        #[test]
        fn function_argument_unpacking() {
            let script = "
f = |(a, b, c...)| a + b + size c
x = make_object 10
f x # 10 + 11 + 8
";
            test_object_script(script, 29);
        }

        #[test]
        fn match_arm_unpacking() {
            let script = "
match make_object 10
  (x, y) then -1
  (rest..., y, z) then (size rest) + y + z # 8 + 18 + 19
";
            test_object_script(script, 45);
        }

        #[test]
        fn index_assign() {
            let script = "
x = make_object 100
x[0] = 23
";
            test_object_script(script, 23);
        }

        #[test]
        fn index_compound_assign() {
            let script = "
x = make_object 100
x[0] += 1
";
            test_object_script(script, 101);
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

    #[test]
    fn object_access() {
        let script = r##"
x = make_object_access()

assert_eq x.method(), "method"

x.field = "foo"
x.field_for_override = "bar"
x.field_for_fallback = "baz"

assert_eq x.field, "foo"
assert_eq x.field_for_override, "bar"
assert_eq x.field_for_fallback, "baz"

# testing aliases

should_throw = |expected_error, f|
  result = try 
    f()
    { @type: "Ok" }
  catch error
    { @type: "Err", msg: error }
  match type result
    "Ok" then throw "function should have panicked!"
    "Err" then 
      if result.msg != expected_error
        throw "expected error '{expected_error}' but got '{error}'"

# make sure the function identifier does not become an access key
# if an explicit `name` argument was given
should_throw "unexpected key 'field_x'", || x.field_x
should_throw "unexpected key 'field_x'", || x.field_x = "something"

assert_eq x.field_1, "foo"
assert_eq x.field_2, "foo"
assert_eq x.field_3, "foo"

x.field_1 = "foo_1"
assert_eq x.field, "foo_1"

x.field_2 = "foo_2"
assert_eq x.field, "foo_2"

x.field_3 = "foo_3"
assert_eq x.field, "foo_3"
"##;
        test_object_script(script, ());
    }

    #[test]
    fn map_like_object() {
        let script = r##"
x = make_map_like()
assert_eq x.length(), 0

try
  print x.foo
  throw "expression above should have errored"
catch error
  assert_eq error, "'foo' not found in 'MapLikeObject'"

x.foo = 1
assert_eq x.foo, 1
assert_eq x.length(), 1
assert_eq '{x}', r'MapLikeObject: {foo: 1}'

x.bar = 2
x.baz = 3
assert_eq x.length(), 3
assert_eq '{x}', r'MapLikeObject: {foo: 1, bar: 2, baz: 3}'

x.length = "oops"
assert_eq x.length, "oops"
assert_eq '{x}', r"MapLikeObject: {foo: 1, bar: 2, baz: 3, length: 'oops'}"
"##;
        test_object_script(script, ());
    }

    mod temporaries {
        use super::*;

        #[test]
        fn overridden_unary_op_as_chain_root() {
            let script = "
x = make_object -100
(-x).as_number()
";
            test_object_script(script, 100);
        }

        #[test]
        fn overridden_binary_op_as_chain_root() {
            let script = "
x = make_object 100
y = make_object 100
(x - y).as_number()
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
z = koto.deep_copy x
y -= 50
z += 200
x + z
";
            test_object_script(script, 350);
        }
    }
}
