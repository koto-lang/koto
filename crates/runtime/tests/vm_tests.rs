mod vm {
    use koto_runtime::prelude::*;
    use koto_test_utils::*;

    mod literals {
        use super::*;

        #[test]
        fn null() {
            check_script_output("null", KValue::Null);
            check_script_output("()", KValue::Null);
        }

        #[test]
        fn bool_true() {
            check_script_output("true", true);
        }

        #[test]
        fn bool_false() {
            check_script_output("false", false);
        }

        #[test]
        fn number() {
            check_script_output("24.0", 24);
        }

        #[test]
        fn string() {
            check_script_output("\"Hello\"", "Hello");
        }
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn add_multiply() {
            check_script_output("1 + 2 * 3 + 4", 11);
        }

        #[test]
        fn add_multiply_compressed_whitespace() {
            check_script_output("1+ 2 *3+4", 11);
        }

        #[test]
        fn subtract_divide_remainder() {
            check_script_output("(20 - 2) / 3 % 4", 2);
        }

        #[test]
        fn negation() {
            let script = "
a = 99
-a";
            check_script_output(script, -99);
        }

        #[test]
        fn remainder_negative() {
            check_script_output("assert_near 10 % -1.2, 0.4", KValue::Null);
        }

        #[test]
        fn remainder_with_a_divisor_of_zero() {
            check_script_output("(1 % 0).is_nan()", true);
        }
    }

    mod logic {
        use super::*;

        #[test]
        fn comparison() {
            check_script_output(
                "false or 1 < 2 <= 2 <= 3 and 3 >= 2 >= 2 > 1 or false",
                true,
            );
        }

        #[test]
        fn equality() {
            check_script_output("1 + 1 == 2 and 2 + 2 != 5", true);
        }

        #[test]
        fn not_bool() {
            check_script_output("not false", true);
        }

        #[test]
        fn not_expression() {
            check_script_output("not 1 + 1 == 2", false);
        }

        #[test]
        fn not_coerced_null() {
            check_script_output("not null", true);
        }

        #[test]
        fn not_coerced_value() {
            check_script_output("not 42", false);
        }

        #[test]
        fn or_with_coerced_null() {
            let script = "
x = null
x or 42";
            check_script_output(script, 42);
        }

        #[test]
        fn or_with_coerced_value() {
            let script = "
x = 99
x or 42";
            check_script_output(script, 99);
        }
    }

    mod assignment {
        use super::*;

        #[test]
        fn assignment() {
            let script = "
a = 1 * 3
a + 1";
            check_script_output(script, 4);
        }

        #[test]
        fn repeated_assignment() {
            let script = "
x = x = 1
y = y = 2
";
            check_script_output(script, 2);
        }

        #[test]
        fn compound_assignment_ops() {
            let script = "
a = 10
a += 1 # 11
a *= 6 # 66
a /= 2 # 33
a %= 5
";
            check_script_output(script, 3);
        }

        #[test]
        fn compound_assignment_chain_add_first() {
            let script = "
a = 10
b = 20
c = 30
a += b *= c
a, b, c
";
            check_script_output(script, number_tuple(&[610, 600, 30]));
        }

        #[test]
        fn compound_assignment_chain_multiply_first() {
            let script = "
a = 10
b = 20
c = 30
a *= b += c
a, b, c
";
            check_script_output(script, number_tuple(&[500, 50, 30]));
        }
    }

    #[allow(clippy::reversed_empty_ranges)]
    mod ranges {
        use super::*;

        #[test]
        fn range() {
            check_script_output("0..10", KRange::from(0..10));
            check_script_output("0..-10", KRange::from(0..-10));
            check_script_output("1 + 1..2 + 2", KRange::from(2..4));
        }

        #[test]
        fn range_inclusive() {
            check_script_output("10..=20", KRange::from(10..=20));
            check_script_output("4..=0", KRange::from(4..=0));
            check_script_output("2 * 2..=3 * 3", KRange::from(4..=9));
        }
    }

    mod tuples {
        use super::*;

        #[test]
        fn empty() {
            check_script_output("(,)", KTuple::default());
        }

        #[test]
        fn one_entry() {
            check_script_output("1,", number_tuple(&[1]));
        }

        #[test]
        fn one_entry_in_parens() {
            check_script_output("(2,)", number_tuple(&[2]));
        }

        #[test]
        fn two_entries() {
            check_script_output("1, 2", number_tuple(&[1, 2]));
        }

        #[test]
        fn two_entries_in_parens() {
            check_script_output("(1, 2)", number_tuple(&[1, 2]));
        }

        #[test]
        fn tuple_of_tuples() {
            check_script_output(
                "(1, 2), (3, 4, 5), (6, 7, 8, 9), (0,)",
                tuple(&[
                    number_tuple(&[1, 2]),
                    number_tuple(&[3, 4, 5]),
                    number_tuple(&[6, 7, 8, 9]),
                    number_tuple(&[0]),
                ]),
            );
        }

        #[test]
        fn tuple_slicing() {
            check_script_output("(0, 1, 2, 3, 4, 5)[2..=4]", number_tuple(&[2, 3, 4]));
        }

        #[test]
        fn addition() {
            check_script_output("(1, 2, 3) + (4, 5, 6)", number_tuple(&[1, 2, 3, 4, 5, 6]));
        }
    }

    mod lists {
        use super::*;

        #[test]
        fn empty() {
            check_script_output("[]", KList::default());
        }

        #[test]
        fn literals() {
            check_script_output("[1, 2, 3, 4]", number_list(&[1, 2, 3, 4]));
        }

        #[test]
        fn from_ids() {
            let script = "
a = 1
[a, a, a]";
            check_script_output(script, number_list(&[1, 1, 1]));
        }

        #[test]
        fn access_element() {
            let script = "
a = [1, 2, 3]
a[1]";
            check_script_output(script, 2);
        }

        #[test]
        fn access_range() {
            let script = "
a = [10, 20, 30, 40, 50]
a[1..3]";
            check_script_output(script, number_list(&[20, 30]));
        }

        #[test]
        fn access_range_inclusive() {
            let script = "
a = [10, 20, 30, 40, 50]
a[1..=3]";
            check_script_output(script, number_list(&[20, 30, 40]));
        }

        #[test]
        fn access_range_to() {
            let script = "
a = [10, 20, 30, 40, 50]
a[..2]";
            check_script_output(script, number_list(&[10, 20]));
        }

        #[test]
        fn access_range_to_inclusive() {
            let script = "
a = [10, 20, 30, 40, 50]
a[..=2]";
            check_script_output(script, number_list(&[10, 20, 30]));
        }

        #[test]
        fn access_range_from() {
            let script = "
a = [10, 20, 30, 40, 50]
a[2..]";
            check_script_output(script, number_list(&[30, 40, 50]));
        }

        #[test]
        fn access_range_full() {
            let script = "
a = [10, 20, 30, 40, 50]
a[..]";
            check_script_output(script, number_list(&[10, 20, 30, 40, 50]));
        }

        #[test]
        fn assign_element() {
            let script = "
a = [1, 2, 3]
x = 2
a[x] = -1
a";
            check_script_output(script, number_list(&[1, 2, -1]));
        }

        #[test]
        fn assign_range() {
            let script = "
a = [1, 2, 3, 4, 5]
a[1..=3] = 0
a";
            check_script_output(script, number_list(&[1, 0, 0, 0, 5]));
        }

        #[test]
        fn assign_range_to() {
            let script = "
a = [1, 2, 3, 4, 5]
a[..3] = 0
a";
            check_script_output(script, number_list(&[0, 0, 0, 4, 5]));
        }

        #[test]
        fn assign_range_to_inclusive() {
            let script = "
a = [1, 2, 3, 4, 5]
a[..=3] = 8
a";
            check_script_output(script, number_list(&[8, 8, 8, 8, 5]));
        }

        #[test]
        fn assign_range_from() {
            let script = "
a = [1, 2, 3, 4, 5]
a[2..] = 9
a";
            check_script_output(script, number_list(&[1, 2, 9, 9, 9]));
        }

        #[test]
        fn assign_range_full() {
            let script = "
a = [1, 2, 3, 4, 5]
a[..] = 9
a";
            check_script_output(script, number_list(&[9, 9, 9, 9, 9]));
        }

        #[test]
        fn shared_data_by_default() {
            let script = "
l = [1, 2, 3]
l2 = l
l[1] = -1
l2[1]";
            check_script_output(script, -1);
        }

        #[test]
        fn copy() {
            let script = "
l = [1, 2, 3]
l2 = copy l
l[1] = -1
l2[1]";
            check_script_output(script, 2);
        }

        #[test]
        fn addition() {
            check_script_output("[1, 2, 3] + [4, 5, 6]", number_list(&[1, 2, 3, 4, 5, 6]));
        }
    }

    mod multi_assignment {
        use super::*;

        #[test]
        fn assign_single_value() {
            let script = "a, b = 42";
            check_script_output(script, tuple(&[42.into(), KValue::Null]));
        }

        #[test]
        fn assign_two_values() {
            let script = "a, b = 10, 20";
            check_script_output(script, number_tuple(&[10, 20]));
        }

        #[test]
        fn assign_tuple() {
            let script = "
a = 1, 2
a";
            check_script_output(script, number_tuple(&[1, 2]));
        }

        #[test]
        fn list_elements_2_to_2() {
            let script = "
x = [0, 0]
x[0], x[1] = -1, 42";
            check_script_output(script, number_tuple(&[-1, 42]));
        }

        #[test]
        fn unpack_list() {
            let script = "a, b, c = [7, 8]";
            check_script_output(script, tuple(&[7.into(), 8.into(), KValue::Null]));
        }

        #[test]
        fn multiple_lists() {
            let script = "a, b, c = [1, 2], [3, 4]";
            check_script_output(
                script,
                tuple(&[number_list(&[1, 2]), number_list(&[3, 4]), KValue::Null]),
            );
        }

        #[test]
        fn iterator() {
            let script = "a, b, c = (1, 2).each |x| x * 10";
            check_script_output(script, tuple(&[10.into(), 20.into(), KValue::Null]));
        }

        #[test]
        fn iterator_into_chains() {
            let script = "
x = [1, 2]
x[0], x[1] = (1, 2).each |x| x * 10";
            check_script_output(script, tuple(&[10.into(), 20.into()]));
        }

        #[test]
        fn swap_values() {
            let script = "
a, b = 0, 1
a, b = b, a
b";
            check_script_output(script, 0);
        }

        #[test]
        fn swap_values_with_expressions() {
            let script = "
a, b = 10, 7
a, b = a+b, a%b
b";
            check_script_output(script, 3);
        }

        #[test]
        fn value_is_unmodified_after_unpacking() {
            let script = "
xy = 10, 7
x, y = xy
type xy
";
            check_script_output(script, "Tuple");
        }

        #[test]
        fn exhausted_iterator_in_unpacking_produces_null() {
            let script = "
a, b, c = 1..=3
a, b, c = 1..=2
c
";
            check_script_output(script, KValue::Null);
        }
    }

    mod type_checks {
        use super::*;

        #[test]
        fn assigning_a_string() {
            let script = "
let x: String = 'foo'
x
";
            check_script_output(script, "foo");
        }

        #[test]
        fn assigning_an_optional_string() {
            let script = "
let x: String? = null
x
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn multi_assignment() {
            let script = "
let x: String, y: Bool, z: Number = 'foo', true, 123
x, y, z
";
            check_script_output(script, tuple(&["foo".into(), true.into(), 123.into()]));
        }

        #[test]
        fn multi_assignment_with_wildcard() {
            let script = "
let x: String, _: String = 'foo', 'bar'
x
";
            check_script_output(script, "foo");
        }

        #[test]
        fn multi_assignment_with_tagged_wildcard() {
            let script = "
let x: String, _y: String = 'foo', 'bar'
x
";
            check_script_output(script, "foo");
        }

        #[test]
        fn any_matches_all_values() {
            let script = "
let x: Any, y: Any, z: Any = true, 42, 'foo'
true
";
            check_script_output(script, true);
        }

        #[test]
        fn callable_matches_functions() {
            let script = "
let x: Callable = || true
x()
";
            check_script_output(script, true);
        }

        #[test]
        fn indexable_matches_indexable_values() {
            let script = "
let w: Indexable, x: Indexable, y: Indexable, z: Indexable = {}, (1, 2, 3), [], 'foo'
true
";
            check_script_output(script, true);
        }

        #[test]
        fn iterable_matches_iterable_values() {
            let script = "
let x: Iterable, y: Iterable = 1..10, 'foo'
true
";
            check_script_output(script, true);
        }
    }

    mod if_expressions {
        use super::*;

        #[test]
        fn if_else_if_result_from_if() {
            let script = "
x = if 5 > 4
  42
else if 1 < 2
  -1
else
  99
x";
            check_script_output(script, 42);
        }

        #[test]
        fn if_else_if_result_from_else_if() {
            let script = "
x = if 5 < 4
  42
else if 1 < 2
  -1
else
  99
x";
            check_script_output(script, -1);
        }

        #[test]
        fn if_else_if_result_from_else() {
            let script = "
x = if 5 < 4
  42
else if 2 < 1
  -1
else
  99
x";
            check_script_output(script, 99);
        }

        #[test]
        fn if_no_else_no_match() {
            let script = "
if 5 < 4
  42
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn if_else_if_no_else_no_match() {
            let script = "
if 5 < 4
  42
else if 2 == 3
  -1
else if false
  99
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn if_else_if_no_else_result_from_else_if() {
            let script = "
if false
  42
else if true
  99
";
            check_script_output(script, 99);
        }

        #[test]
        fn multiple_else_ifs() {
            let script = "
if false
  42
else if false
  -1
else if false
  99
else if true
  100
else
  0
";
            check_script_output(script, 100);
        }

        #[test]
        fn inline_if_with_multiple_expressions_in_body() {
            let script = "
foo = true
x = if foo then 1, 2, 3 else 4, 5, 6
x
";
            check_script_output(script, number_tuple(&[1, 2, 3]));
        }
    }

    mod match_expressions {
        use super::*;

        #[test]
        fn match_assignment() {
            let script = "
x = match 0 == 1
  true then 42
  false then 99
x
";
            check_script_output(script, 99);
        }

        #[test]
        fn match_multiple() {
            let script = r#"
x = 11
match x % 3, x % 5
  0, 0 then "Fizz Buzz"
  0, _ then "Fizz"
  _, 0 then "Buzz"
  _ then x # alternative to else
"#;
            check_script_output(script, 11);
        }

        #[test]
        fn match_with_condition() {
            let script = r#"
x = "hello"
match x
  "goodbye" then 1
  r'byeeee' then 2
  () then 99
  y if y == "O_o" then -1
  y if y == "hello" then
    42
"#;
            check_script_output(script, 42);
        }

        #[test]
        fn match_with_condition_after_chain() {
            let script = r#"
foo = {bar: 0, baz: 1}
x = 42
match 0
  foo.bar if x == -1 then 0
  foo.bar if x == 42 then 42
  else -1
"#;
            check_script_output(script, 42);
        }

        #[test]
        fn match_on_alternative() {
            let script = "
match 42
  1 or 2 then 11
  3 or 4 or 5 then 22
  21 or 42 then 33
  else 44
";
            check_script_output(script, 33);
        }

        #[test]
        fn match_tuple() {
            let script = "
match (1, (2, 3), 4)
  (1, (x, y), (p, (q, r))) then -1
  (_, (a, b), _) then a + b
  else 123
";
            check_script_output(script, 5);
        }

        #[test]
        fn match_list() {
            let script = "
match [1, [2, 3], [4, 5, 6]]
  (1, (x, -1), (_, y, _)) then x + y
  (1, (x, 3), (_, 5, y)) then x + y
  else 123
";
            check_script_output(script, 8);
        }

        #[test]
        fn match_list_single_entry() {
            let script = "
x = [0]
match x
  (0) or (1) then 123
  (x, y) or (x, y, z) then 99
  else -1
";
            check_script_output(script, 123);
        }

        #[test]
        fn match_list_subslice() {
            let script = "
x = (1..=5).to_list()
match x
  (0, ...) then 0
  (..., 1) then -1
  (1, ...) then 1
  else 123
";
            check_script_output(script, 1);
        }

        #[test]
        fn match_list_subslice_with_id() {
            let script = "
x = (1..=5).to_list()
match x
  (0, rest...) then rest
  (rest..., 3, 2, 1) then rest
  (1, 2, rest...) then rest
  else 123
";
            check_script_output(script, number_list(&[3, 4, 5]));
        }

        #[test]
        fn match_list_subslice_at_start_with_id() {
            let script = "
x = (1..=5).to_list()
match x
  (0, rest...) then rest
  (rest..., 3, 4, 5) then rest
  (1, 2, rest...) then rest
  else 123
";
            check_script_output(script, number_list(&[1, 2]));
        }

        #[test]
        fn match_tuple_subslice_at_start_with_id() {
            let script = "
x = 1, 2, 3, 4, 5
match x
  (0, rest...) then rest
  (rest..., 3, 4, 5) then rest
  (1, 2, rest...) then rest
  else 123
";
            check_script_output(script, number_tuple(&[1, 2]));
        }

        #[test]
        fn match_string_subslice_at_start_with_id() {
            let script = "
match 'hello!'
  ('h', 'x', ...) then 'nope'
  ('h', 'e', rest...) then 'llo!'
  else 'abc'
";
            check_script_output(script, "llo!");
        }

        #[test]
        fn match_on_multiple_expressions_with_alternatives_wildcard() {
            let script = "
match 0, 1
  0, 0 or 1, 1 then -1
  _, 0 or _, 99 then -2
  x, 0 or x, 2 then -3
  0, _ or 1, _ then -4 # The first alternative (0, _) should match
  else -5
";
            check_script_output(script, -4);
        }

        #[test]
        fn match_on_multiple_expressions_with_alternatives_id() {
            let script = "
match 0, 1
  0, 0 or 1, 1 then -1
  _, 0 or _, 99 then -2
  x, 1 or x, 2 then -3 # The first alternative (x, 1) should match
  0, _ or 1, _ then -4
  else -5
";
            check_script_output(script, -3);
        }

        #[test]
        fn match_with_chain_as_pattern() {
            let script = "
x = {foo: 42, bar: 99}
match 99
  x.foo then 1
  x.bar then 2
  else -1
";
            check_script_output(script, 2);
        }

        #[test]
        fn match_with_chain_as_pattern_in_function() {
            let script = "
x = {foo: 42, bar: 99}
f = ||
  match 42
    x.foo then 1
    x.bar then 2
    else -1
f()
";
            check_script_output(script, 1);
        }

        #[test]
        fn match_with_first_type_hint_in_arm() {
            let script = r#"
match 42
  x: String or x: List then -1
  x: Number or x: Bool then x
  else -2
"#;
            check_script_output(script, 42);
        }

        #[test]
        fn match_with_second_type_hint_in_arm() {
            let script = r#"
match true
  x: String or x: List then -1
  x: Number or x: Bool then x
  else -2
"#;
            check_script_output(script, true);
        }

        #[test]
        fn match_with_no_matching_type_hints() {
            let script = r#"
match 42
  x: String or x: List then -1
  x: Tuple or x: Bool then -2
  else 99
"#;
            check_script_output(script, 99);
        }

        #[test]
        fn match_map_result() {
            let script = r#"
m = match "hello"
  "foo" then
    value_1: -1
    value_2: 99
  "hello" then
    value_1: 4
    value_2: 20
  _ then # alternative to else
    value_1: 10
    value_2: 7
m.value_1 + m.value_2
"#;
            check_script_output(script, 24);
        }

        #[test]
        fn multiple_expressions_in_inline_arm() {
            let script = r#"
m = match 42
  23 then 1, 2
  42 then 3, 4
  else 5, 6
m
"#;
            check_script_output(script, number_tuple(&[3, 4]));
        }

        #[test]
        fn missing_else_branch() {
            // A bug meant that a missing else branch would leak previously assigned values
            let script = r#"
a, b = 42, 43
x = match a, b
  1, 2 then 1, 2
  3, 4 then 3, 4
x
"#;
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn assignment_target_used_in_match_arm() {
            let script = r#"
x = 10
x = match 99
  123 then -1
  _ then x * x
x
"#;
            check_script_output(script, 100);
        }
    }

    mod switch_expressions {
        use super::*;

        #[test]
        fn match_without_expression() {
            let script = r#"
n = 42
switch
  n < 0 then -1
  n == 0 then 0
  n == 42 then 99
  else 1
"#;
            check_script_output(script, 99);
        }

        #[test]
        fn multiple_expressions_in_inline_arm() {
            let script = r#"
x = switch
  false then 1, 2
  true then 3, 4
x
"#;
            check_script_output(script, number_tuple(&[3, 4]));
        }

        #[test]
        fn missing_else_branch() {
            // A bug meant that a missing else branch would leak previously assigned values
            let script = r#"
a, b = 42, 43
x = switch
  1 == 2 then 1, 2
  3 == 4 then 3, 4
x
"#;
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn assignment_target_used_in_switch_arm() {
            let script = r#"
x = 10
x = switch
  1 == 2 then 99
  3 <= 4 then x * x
x
"#;
            check_script_output(script, 100);
        }
    }

    mod prelude {
        use super::*;

        fn test_script_with_prelude(script: &str, expected_output: KValue) {
            let vm = KotoVm::default();
            let prelude = vm.prelude();

            prelude.insert("test_value", 42);
            prelude.add_fn("assert", |ctx| {
                for value in ctx.args().iter() {
                    match value {
                        KValue::Bool(b) => {
                            if !b {
                                return runtime_error!("assertion failed");
                            }
                        }
                        unexpected => {
                            return runtime_error!(
                                "assert expects booleans as arguments, found '{}'",
                                unexpected.type_as_string(),
                            )
                        }
                    }
                }
                Ok(KValue::Null)
            });

            if let Err(e) = check_script_output_with_vm(vm, script, expected_output) {
                panic!("{e}");
            }
        }

        #[test]
        fn load_value() {
            let script = "test_value";
            test_script_with_prelude(script, 42.into());
        }

        #[test]
        fn function() {
            let script = "assert 1 + 1 == 2";
            test_script_with_prelude(script, KValue::Null);
        }

        #[test]
        fn function_two_args() {
            let script = "assert 1 + 1 == 2, 2 < 3";
            test_script_with_prelude(script, KValue::Null);
        }
    }

    mod functions {
        use super::*;

        #[test]
        fn no_args() {
            let script = "
f = || 42
f()";
            check_script_output(script, 42);
        }

        #[test]
        fn single_arg() {
            let script = "
square = |x| x * x
square 8";
            check_script_output(script, 64);
        }

        #[test]
        fn two_args() {
            let script = "
add = |a, b|
  a + b
add 5, 6";
            check_script_output(script, 11);
        }

        #[test]
        fn two_args_in_parens() {
            let script = "
add = |a, b|
  a + b
add(5, 6)";
            check_script_output(script, 11);
        }

        #[test]
        fn call_with_default_value() {
            let script = "
foo = |a, b = 99| b
foo 42
";
            check_script_output(script, 99);
        }

        #[test]
        fn nested_call_without_parens() {
            let script = "
add = |a, b|
  a + b
add 2, add 3, 4";
            check_script_output(script, 9);
        }

        #[test]
        fn nested_call_in_parens() {
            let script = "
add = |a, b|
  a + b
add(5, add 6, 7)";
            check_script_output(script, 18);
        }

        #[test]
        fn wildcard_arg_at_start() {
            let script = "
f = |_, b, c| b + c
f 1, 2, 3
";
            check_script_output(script, 5);
        }

        #[test]
        fn wildcard_arg_in_middle() {
            let script = "
f = |a, _, c| a + c
f 1, 2, 3
";
            check_script_output(script, 4);
        }

        #[test]
        fn wildcard_arg_at_end() {
            let script = "
f = |a, b, _| a + b
f 1, 2, 3
";
            check_script_output(script, 3);
        }

        mod arg_unpacking {
            use super::*;

            #[test]
            fn unpack_second_arg() {
                let script = "
f = |a, (_, c), d| a + c + d
f 1, (2, 3), 4
";
                check_script_output(script, 8);
            }

            #[test]
            fn nested() {
                let script = "
f = |a, (_, (c, d), _), f| a + c + d + f
f 1, (2, (3, 4), 5), 6
";
                check_script_output(script, 14);
            }

            #[test]
            fn mixed_containers() {
                let script = "
f = |a, (b, (_, d)), e| a + b + d + e
f 1, (2, [3, 4]), 5
";
                check_script_output(script, 12);
            }

            #[test]
            fn unpacking_with_capture() {
                let script = "
x = 10
f = |a, (b, c)| a + b + c + x
f 1, (2, 3)
";
                check_script_output(script, 16);
            }

            #[test]
            fn ellipsis_at_end() {
                let script = "
f = |(a, b, ...)| a + b
f (1, 2, 3, 4, 5)
";
                check_script_output(script, 3);
            }

            #[test]
            fn ellipsis_with_id_at_end() {
                let script = "
f = |(a, b, others...)| a + b + size others
f (1, 2, 3, 4, 5)
";
                check_script_output(script, 6);
            }

            #[test]
            fn ellipsis_at_end_with_no_extra_values() {
                let script = "
f = |(a, b, others...)| a + b + size others
f (1, 2)
";
                check_script_output(script, 3);
            }

            #[test]
            fn ellipsis_at_start() {
                let script = "
f = |(..., y, z)| y + z
f (1, 2, 3, 4, 5)
";
                check_script_output(script, 9);
            }

            #[test]
            fn ellipsis_at_start_with_no_extra_values() {
                let script = "
f = |(..., y, z)| y + z
f (1, 2)
";
                check_script_output(script, 3);
            }

            #[test]
            fn ellipsis_with_id_at_start() {
                let script = "
f = |(others..., y, z)| y + z + size others
f (1, 2, 3, 4, 5)
";
                check_script_output(script, 12);
            }

            #[test]
            fn ellipsis_at_start_and_end_with_no_extra_values() {
                let script = "
f = |(..., y, z)| y + z
f (1, 2)
";
                check_script_output(script, 3);
            }

            #[test]
            fn unpacking_a_map() {
                let script = "
f = |((_, a), (_, b))| a + b
f {foo: 42, bar: 99}
";
                check_script_output(script, 141);
            }

            #[test]
            fn ellipsis_mixed() {
                let script = "
f = |(a, (tuple_others..., z), list_others...)|
  a + list_others.sum() + (size tuple_others) + z
f [10, (1, 2, 3), 20, 30]
";
                check_script_output(script, 65);
            }

            #[test]
            fn unpacking_a_temporary_tuple() {
                let script = "
{foo: 1, bar: 2, baz: 3}
  .keep |(key, _)| key.starts_with 'b'
  .count()
";
                check_script_output(script, 2);
            }
        }

        #[test]
        fn variadic_function() {
            let script = "
f = |a, b, c...|
  a + b + c.fold 0, |x, y| x + y
f 5, 10, 20, 30";
            check_script_output(script, 65);
        }

        #[test]
        fn variadic_function_stacked_call() {
            let script = "
f = |a, b, c...|
  a + b + c.fold 0, |x, y| x + y
f (f 5, 10, 20, 30), 40, 50";
            check_script_output(script, 155);
        }

        #[test]
        fn variadic_function_without_contents() {
            let script = "
f = |a, b...| b
f 42";
            check_script_output(script, tuple(&[]));
        }

        #[test]
        fn variadic_function_with_contents() {
            let script = "
f = |a, b...| b
f 42, 1, 2, 3";
            check_script_output(script, number_tuple(&[1, 2, 3]));
        }

        #[test]
        fn variadic_function_after_default_arg() {
            let script = "
f = |a = 3, b...| b
f()";
            check_script_output(script, tuple(&[]));
        }

        #[test]
        fn nested_function() {
            let script = "
add = |a, b|
  add2 = |x, y| x + y
  add2 a, b
add 10, 20";
            check_script_output(script, 30);
        }

        #[test]
        fn nested_calls() {
            let script = "
add = |a, b| a + b
add 10, (add 20, 30)";
            check_script_output(script, 60);
        }

        #[test]
        fn recursive_call() {
            let script = "
f = |n|
  if n == 0
    0
  else
    f n - 1
f 4
";
            check_script_output(script, 0);
        }

        #[test]
        fn recursive_call_fib() {
            let script = "
fib = |n|
  if n <= 0
    0
  else if n == 1
    1
  else
    (fib n - 1) + (fib n - 2)
fib 4
";
            check_script_output(script, 3);
        }

        #[test]
        fn recursive_call_via_multi_assign() {
            let script = "
f, g =
  (|n| if n == 0 then 1 else f n - 1),
  (|n| if n == 0 then 2 else g n - 1)
(f 4), (g 4)
";
            check_script_output(script, number_tuple(&[1, 2]));
        }

        #[test]
        fn multiple_return_values() {
            let script = "
f = |x| x - 1, x + 1
a, b = f 0
a, b";
            check_script_output(script, number_tuple(&[-1, 1]));
        }

        #[test]
        fn return_no_value() {
            let script = "
f = |x|
  if x < 0
    return
  x
f -42";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn return_expression() {
            let script = "
f = |x|
  if x < 0
    return x * -1
  x
f -42";
            check_script_output(script, 42);
        }

        #[test]
        fn return_map() {
            let script = "
f = ||
  return
    foo: 42
    bar: 99
f().bar";
            check_script_output(script, 99);
        }

        #[test]
        fn multi_assignment_of_function_results() {
            let script = "
f = |n| n
a, b = (f 1), (f 2)
a";
            check_script_output(script, 1);
        }

        #[test]
        fn function_blocks_as_args_dont_break_assignment() {
            // The nested block (as first arg to a call to f) in f2 broke parsing,
            // so that f3 wasn't assigned correctly,
            // and then couldn't be found after assignment.
            let script = "
f = |x| x
f2 = ||
  f |x|
    x
f3 = |x| f2() x
f3 1";
            check_script_output(script, 1);
        }

        #[test]
        fn function_blocks_as_args_dont_break_assignment_during_chain() {
            // See comment in test above, the same applies to args in the call to f.g
            let script = "
f = { g: |x| x }
f2 = ||
  f.g |x|
    x
f3 = |x| f2() x
f3 1";
            check_script_output(script, 1);
        }

        #[test]
        fn iterator_fold_after_function_call_shouldnt_error() {
            // Reported in https://github.com/koto-lang/koto/issues/6
            // iterator.fold() was incorrectly reusing its vm rather than spawning a new one
            let script = "
f = || 1, 2, 3
f().fold 0, |x, n| x += n
";
            check_script_output(script, 6);
        }

        mod value_capturing {
            use super::*;

            #[test]
            fn captured_value() {
                let script = "
x = 3
f = || x * x
f()";
                check_script_output(script, 9);
            }

            #[test]
            fn capture_via_mutation() {
                let script = "
data = [1, 2, 3]
f = ||
  data[1] = 99
  data = () # shadowed assignment doesn't affect the original copy of data
f()
data[1]";
                check_script_output(script, 99);
            }

            #[test]
            fn nested_captured_values() {
                let script = "
capture_test = |a, b, c|
  inner = ||
    inner2 = |x|
      x + b + c
    inner2 a
  b, c = (), () # inner and inner2 have captured their own copies of b and c
  inner()
capture_test 1, 2, 3";
                check_script_output(script, 6);
            }

            #[test]
            fn local_copy_of_captured_value() {
                let script = "
x = 99
f = ||
  x = x + 1
  x
if f() == 100
  x
else
  -1
";
                check_script_output(script, 99);
            }

            #[test]
            fn missing_argument_in_function_with_capture() {
                let script = "
x = -1
foo = |a = 101| a + x
# Add some temporary values to the stack,
# a runtime bug prevented registers from being assigned correctly in foo.
z = 1 + 2 + 3
foo()
";
                check_script_output(script, 100);
            }

            #[test]
            fn returning_captured_value_after_if() {
                let script = "
x = 100
f = ||
  if false then return -1
  x
f()
";
                check_script_output(script, 100);
            }

            #[test]
            fn mutation_of_captured_map() {
                let script = "
f = |x|
  inner = ||
    x.foo = 123
  inner()
  x.foo
f {foo: 42, bar: 99}";
                check_script_output(script, 123);
            }

            #[test]
            fn multi_assignment_to_captured_list() {
                let script = "
f = |x|
  inner = ||
    x[0], x[1] = x[0] + 1, x[1] + 1
    x
  inner()
f [1, 2]";
                check_script_output(script, number_list(&[2, 3]));
            }

            #[test]
            fn implicit_map_value_should_be_captured() {
                let script = "
x = 99
f = || {x}
f().x
";
                check_script_output(script, 99);
            }

            mod comparisions {
                use super::*;

                #[test]
                fn matching_functions_no_captures() {
                    let script = "
f = |x| x + x
f2 = f
f == f, f == f2, f != f, f != f2
";
                    check_script_output(
                        script,
                        tuple(&[true.into(), true.into(), false.into(), false.into()]),
                    );
                }

                #[test]
                fn different_functions() {
                    let script = "
f = |x| x + x
g = |y| y * y
f == g, f != g
";
                    check_script_output(script, tuple(&[false.into(), true.into()]));
                }

                #[test]
                fn matching_captures() {
                    let script = "
f, g =
  (1, 1)
    .each |x| return || x * x
    .to_tuple()
f == g, f != g
";
                    check_script_output(script, tuple(&[true.into(), false.into()]));
                }

                #[test]
                fn different_captures() {
                    let script = "
f, g =
  (1, 2)
    .each |x| return || x * x
    .to_tuple()
f == g, f != g
";
                    check_script_output(script, tuple(&[false.into(), true.into()]));
                }
            }
        }

        mod piped_calls {
            use super::*;

            #[test]
            fn chained_piping() {
                let script = "
add = |a, b| a + b
multiply = |a, b| a * b
square = |x| x * x
add 1, 2
  -> square
  -> multiply 10
";
                check_script_output(script, 90);
            }

            #[test]
            fn from_int_into_map_functions() {
                let script = "
ops =
  add: |a, b| a + b
  multiply: |a, b| a * b
  square: |x| x * x

2
  -> ops.add 1
  -> ops.square
  -> ops.multiply 2
";
                check_script_output(script, 18);
            }

            #[test]
            fn piping_into_array_entries_and_function_calls() {
                let script = "
inc = |x| x + 1
dec = |x| x - 1

ops = [inc, dec]
get_op = |i| ops[i]

0
  -> ops[0]     # 1
  -> get_op(0)  # 2
  -> (get_op 0) # 3
  -> get_op(1)  # 2
";
                check_script_output(script, 2);
            }

            #[test]
            fn chained_pipe_call_order() {
                let script = "
calls = []

f = |x|
  calls.push x + 10
  x + 10
g = |x|
  calls.push x
  f

g(1)(100) -> g(2) -> g(3) -> g(4)

calls
";
                check_script_output(script, number_list(&[1, 110, 2, 120, 3, 130, 4, 140]));
            }
        }
    }

    mod for_loops {
        use super::*;

        #[test]
        fn for_loop_with_ignored_args() {
            let script = "
count = 32
for _ignored in 0..10
  count += 1
";
            check_script_output(script, 42);
        }

        #[test]
        fn for_list() {
            let script = "
sum = 0
for a in [10, 20, 30, 40]
  sum += a
";
            check_script_output(script, 100);
        }

        #[test]
        fn for_break() {
            let script = "
sum = 0
for i in 1..10
  sum += i
  if i == 5
    break
sum
";
            check_script_output(script, 15);
        }

        #[test]
        fn for_break_with_expression() {
            let script = "
sum = 0
for i in 1..10
  sum += i
  if i == 4
    break sum
";
            check_script_output(script, 10);
        }

        #[test]
        fn for_break_default_value_is_null() {
            let script = "
sum = 0
for i in 1..10
  sum += i
  if i == 5
    break
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn for_break_nested() {
            let script = "
sum = 0
for i in [1, 2, 3]
  for j in 0..5
    if j == 2
      break
    sum += i
sum
";
            check_script_output(script, 12);
        }

        #[test]
        fn for_continue() {
            let script = "
sum = 0
for i in 1..10
  if i > 5
    continue
  sum += i
sum
";
            check_script_output(script, 15);
        }

        #[test]
        fn for_continue_nested() {
            let script = "
sum = 0
for i in [2, 4, 6]
  for j in 0..10
    if j > 1
      continue
    sum += i
sum
";
            check_script_output(script, 24);
        }

        #[test]
        fn for_continue_result_is_null() {
            let script = "
sum = 0
for i in (1, 2)
  if i == 2
    continue
  else
    i
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn return_from_nested_for_loop() {
            let script = "
f = ||
  for i in 0..100
    for j in 0..100
      if i == j == 5
        return i
  -1
f()";
            check_script_output(script, 5);
        }

        #[test]
        fn for_arg_unpacking() {
            let script = "
sum = 0
for a, _foo, b in ((1, 99, 2), (3, 99, 4))
  sum += a + b
";
            check_script_output(script, 10);
        }

        #[test]
        fn for_loop_assignment() {
            let script = "
f = |x| x * x
result = for x in 0..=10
  f x
result
";
            check_script_output(script, 100);
        }
    }

    mod while_loops {
        use super::*;

        #[test]
        fn while_iteration() {
            let script = "
count = 0
while count < 10
  count += 1
";
            check_script_output(script, 10);
        }

        #[test]
        fn while_break() {
            let script = "
i, sum = 0, 0
while (i += 1) < 1000000
  if i > 5
    break
  sum += i
sum
";
            check_script_output(script, 15);
        }

        #[test]
        fn while_break_with_expression() {
            let script = "
i, sum = 0, 0
while (i += 1) < 1000000
  if i > 5
    break sum * 10
  sum += i
";
            check_script_output(script, 150);
        }

        #[test]
        fn while_continue() {
            let script = "
i, sum = 0, 0
while (i += 1) < 10
  if i < 6
    continue
  # The result will be the sum of 6..=9
  sum += i
";
            check_script_output(script, 30);
        }

        #[test]
        fn while_continue_result_is_null() {
            let script = "
i = 0
while (i += 1) < 5
  if i == 4
    continue
  else
    i
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn while_assignment_of_body_result() {
            let script = "
f = |x| x * x
count = 0
result = while count < 10
  count += 1
  f count
result
";
            check_script_output(script, 100);
        }

        #[test]
        fn while_assignment_in_condition() {
            let script = "
sum = 0
iter = (1..=5).peekable()
while i = iter.peek()
  iter.next()
  sum += i.get()
sum
";
            check_script_output(script, 15);
        }
    }

    mod until_loops {
        use super::*;

        #[test]
        fn until_loop() {
            let script = "
count = 10
until count == 20
  count += 1
";
            check_script_output(script, 20);
        }

        #[test]
        fn until_break() {
            let script = "
count = 0
until count == 100000000
  count += 1
  if count == 5
    break
count";
            check_script_output(script, 5);
        }

        #[test]
        fn until_break_with_expression() {
            let script = "
count = 0
until count == 100000000
  count += 1
  if count == 5
    break count * 2
";
            check_script_output(script, 10);
        }

        #[test]
        fn until_continue() {
            let script = "
sum, count = 0, 0
until count == 6
  count += 1
  if count % 2 == 0
    continue
  sum += count
sum
";
            check_script_output(script, 9);
        }

        #[test]
        fn until_assignment_of_body_result() {
            let script = "
f = |x| x * x
count = 0
result = until count == 5
  count += 1
  f count
result
";
            check_script_output(script, 25);
        }

        #[test]
        fn until_assignment_in_condition() {
            let script = "
sum = 0
iter = (1..=5).peekable()
until not (i = iter.peek())
  iter.next()
  sum += i.get()
sum
";
            check_script_output(script, 15);
        }
    }

    mod loop_expressions {
        use super::*;

        #[test]
        fn loop_break_continue() {
            let script = "
i = 0
loop
  i += 1
  if i < 5
    continue
  else
    break
i";
            check_script_output(script, 5);
        }

        #[test]
        fn loop_break_with_value() {
            let script = "
i = 0
loop
  i += 1
  if i == 5
    break i * 10
";
            check_script_output(script, 50);
        }

        #[test]
        fn loop_assignment() {
            let script = "
i = 0
result = loop
  i += 1
  if i == 5
    break i + i
result";
            check_script_output(script, 10);
        }
    }

    mod maps {
        use super::*;

        #[test]
        fn empty() {
            check_script_output("{}", KValue::Map(KMap::new()));
        }

        #[test]
        fn from_literals() {
            let expected = KMap::default();
            expected.insert("foo", 42);
            expected.insert("bar", "baz");

            check_script_output("{foo: 42, bar: 'baz'}", KValue::Map(expected));
        }

        #[test]
        fn access() {
            let script = "
m = {foo: -1}
m.foo";
            check_script_output(script, -1);
        }

        #[test]
        fn insert() {
            let script = "
m = {}
m.foo = 42
m.foo";
            check_script_output(script, 42);
        }

        #[test]
        fn update() {
            let script = "
m = {bar: -1}
m.bar = 99
m.bar";
            check_script_output(script, 99);
        }

        #[test]
        fn implicit_values() {
            let script = "
foo, baz = 42, -1
m = {foo, bar: 99, baz}
m.baz";
            check_script_output(script, -1);
        }

        #[test]
        fn string_keys() {
            let script = r#"
foo, bar = 42, -1
m = {foo, 'bar': bar, r'baz': 99}
m.baz"#;
            check_script_output(script, 99);
        }

        #[test]
        fn tuple_keys() {
            let script = r#"
m = {}
m.insert (1, 2), 'hello'
m.get (1, 2)"#;
            check_script_output(script, "hello");
        }

        #[test]
        fn instance_function_no_args() {
            let script = "
make_o = ||
  {foo: 42, get_foo: || self.foo}
o = make_o()
o.get_foo()";
            check_script_output(script, 42);
        }

        #[test]
        fn instance_function_with_args() {
            let script = "
make_o = ||
  foo: 0
  set_foo: |a, b| self.foo = a + b
o = make_o()
o.set_foo 10, 20
o.foo";
            check_script_output(script, 30);
        }

        #[test]
        fn equality() {
            let script = "
m = {foo: 42, bar: 'abc'}
m2 = copy m
m == m2";
            check_script_output(script, true);
        }

        #[test]
        fn equality_different_key_order() {
            let script = "
m = {foo: 42, bar: 'abc'}
m2 = {bar: 'abc', foo: 42}
m == m2";
            check_script_output(script, true);
        }

        #[test]
        fn inequality() {
            let script = "
m = {foo: 42, bar: 'xyz'}
m2 = {foo: 42, bar: 'abc'}
m != m2";
            check_script_output(script, true);
        }

        #[test]
        fn shared_data_by_default() {
            let script = "
m = {foo: 42}
m2 = m
m.foo = -1
m2.foo";
            check_script_output(script, -1);
        }

        #[test]
        fn copy() {
            let script = "
m = {foo: 42}
m2 = copy m
m.foo = -1
m2.foo";
            check_script_output(script, 42);
        }

        #[test]
        fn addition() {
            let script = "
a = {foo: 42}
b = {bar: 99}
c = a + b
c.foo + c.bar
";
            check_script_output(script, 141);
        }

        #[test]
        fn indexing() {
            let script = "
m = {foo: 42, bar: 'xyz'}
m[0]
";
            check_script_output(script, tuple(&["foo".into(), 42.into()]));
        }
    }

    mod chains {
        use super::*;

        #[test]
        fn list_in_map() {
            let script = "
m = {x: [100, 200]}
m.x[1]";
            check_script_output(script, 200);
        }

        #[test]
        fn map_in_list() {
            let script = "
m = {foo: 99}
l = [m, m, m]
l[2].foo";
            check_script_output(script, 99);
        }

        #[test]
        fn assign_to_map_in_list() {
            let script = "
m = {bar: 0}
l = [m, m, m]
l[1].bar = -1
l[1].bar";
            check_script_output(script, -1);
        }

        #[test]
        fn assign_to_list_in_map_in_list() {
            let script = "
m = {foo: [1, 2, 3]}
l = [m, m, m]
l[2].foo[0] = 99
l[2].foo[0]";
            check_script_output(script, 99);
        }

        #[test]
        fn add_assign_with_map_entry() {
            let script = "
m = {foo: 99}
m.foo += 1
m.foo";
            check_script_output(script, 100);
        }

        #[test]
        fn subtract_assign_with_string_key() {
            let script = "
m = {foo: 42}
m.'foo' -= 1
m.'foo'";
            check_script_output(script, 41);
        }

        #[test]
        fn multiply_assign_with_list_entry() {
            let script = "
m = [1, 2, 3]
m[1] *= 10
m[1]";
            check_script_output(script, 20);
        }

        #[test]
        fn function_call() {
            let script = "
m = {get_map: || { foo: -1 }}
m.get_map().foo";
            check_script_output(script, -1);
        }

        #[test]
        fn function_call_variadic() {
            let script = "
m =
  foo: |x, xs...|
    xs.fold x, |a, b| a + b
m.foo 1, 2, 3
";
            check_script_output(script, 6);
        }

        #[test]
        fn instance_function_call_variadic() {
            let script = "
m =
  foo: |x, xs...|
    self.offset + xs.fold x, |a, b| a + b
  offset: 10
m.foo 1, 2, 3
";
            check_script_output(script, 16);
        }

        #[test]
        fn instance_function_call_variadic_generator() {
            let script = "
m =
  foo: |first, xs...|
    for x in xs
      yield self.offset + first + x
  offset: 100
m.foo(10, 1, 2, 3).to_tuple()
";
            check_script_output(script, number_tuple(&[111, 112, 113]));
        }

        #[test]
        fn negation_of_chain_result() {
            let script = "
x =
  nested: {foo: 42}
  get_nested: || self.nested
-x.get_nested().foo
";
            check_script_output(script, -42);
        }

        #[test]
        fn deep_copy_list() {
            let script = "
x = [0, [1, {foo: 2}]]
x2 = koto.deep_copy x
x[1][1].foo = 42
x2[1][1].foo";
            check_script_output(script, 2);
        }

        #[test]
        fn deep_copy_tuple() {
            let script = "
list = [1, [2]]
x = (0, list)
x2 = koto.deep_copy x
list[1][0] = 42
x2[1][1][0]";
            check_script_output(script, 2);
        }

        #[test]
        fn deep_copy_map() {
            let script = "
m = {foo: {bar: -1}}
m2 = koto.deep_copy m
m.foo.bar = 99
m2.foo.bar";
            check_script_output(script, -1);
        }

        #[test]
        fn copy_from_expression() {
            let script = "
m = {foo: {bar: 88}, get_foo: || self.foo}
m2 = copy m.get_foo()
m.get_foo().bar = 99
m2.bar";
            check_script_output(script, 88);
        }

        #[test]
        fn capture_in_map_block() {
            let script = "
x = 42
make_map = ||
  foo: x
m = make_map()
m.foo
";
            check_script_output(script, 42);
        }

        #[test]
        fn function_body_in_iterator_chain() {
            // The result.insert() call in a function block, followed by a continued iterator chain
            // at a lower indentation level caused a parser error.
            let script = "
result = {}
(1..=5)
  .each |x|
    result.insert(x, x * x)
  .consume()
size result
";
            check_script_output(script, 5);
        }

        #[test]
        fn inline_function_body_in_call_args() {
            let script = "
equal = |x, y| x == y
equal
  (0..10).position(|n| n == 5),
  5
";
            check_script_output(script, true);
        }

        #[test]
        fn range_in_call_args() {
            let script = "
foo = |range, x| (size range) + x
min, max = 0, 10
foo min..max, 20
";
            check_script_output(script, 30);
        }

        #[test]
        fn default_value_for_second_arg() {
            let script = "
foo = |a, b = 99| b
foo 42
";
            check_script_output(script, 99);
        }

        #[test]
        fn default_arg_after_list() {
            let script = "
foo = |a, b = 3| b
foo [42]
";
            check_script_output(script, 3);
        }

        #[test]
        fn default_arg_with_capture() {
            let script = "
x = 100
foo = |a, b = 23| b + x
foo [42]
";
            check_script_output(script, 123);
        }

        #[test]
        fn default_arg_before_unused_variadic() {
            let script = "
foo = |a, b = 99, rest...| (a, b) + rest
foo 1
";
            check_script_output(script, number_tuple(&[1, 99]));
        }

        #[test]
        fn default_arg_before_filled_variadic() {
            let script = "
foo = |a, b = -1, rest...| (a, b) + rest
foo 1, 2, 3, 4, 5
";
            check_script_output(script, number_tuple(&[1, 2, 3, 4, 5]));
        }

        #[test]
        fn default_arg_for_generator() {
            let script = "
foo = |a, b = 123|
  for x in a
    yield x
  yield b
foo([42, 99]).to_tuple()
";
            check_script_output(script, number_tuple(&[42, 99, 123]));
        }

        #[test]
        fn if_else_used_in_map_block() {
            let script = "
foo =
  x: if 1 == 2
       99
     else
       42
foo.x
";
            check_script_output(script, 42);
        }

        #[test]
        fn chained_call() {
            let script = "
f = ||
  |x| x * x
f()(8)
";
            check_script_output(script, 64);
        }

        mod optional_chaining {
            use super::*;

            #[test]
            fn check_after_lookup() {
                let script = "
m = {foo: null}
x = m.foo?.nested
x
";
                check_script_output(script, KValue::Null);
            }

            #[test]
            fn check_after_previous_assignment() {
                let script = "
m = {foo: null}
x = 99
x = m.foo?.nested
x
";
                check_script_output(script, KValue::Null);
            }

            #[test]
            fn checks_between_calls() {
                let script = "
f = || null
x = f()?()
x
";
                check_script_output(script, KValue::Null);
            }

            #[test]
            fn check_before_assignment() {
                let script = "
m = {foo: null}
m.foo? = 1
m.foo
";
                check_script_output(script, KValue::Null);
            }

            #[test]
            fn check_before_compound_assignment() {
                let script = "
m = {foo: 42}
m.foo? += 1
m.foo
";
                check_script_output(script, 43);
            }

            #[test]
            fn several_checks_pass() {
                let script = "
m = || {foo: [{bar: {baz: 99}}]}
m?()?.foo?[0]?.bar?.baz? += 1
";
                check_script_output(script, 100);
            }

            #[test]
            fn several_checks_with_short_circuit() {
                let script = "
m = || {foo: [{bar: {}}]}
m?()?.foo?[0]?.bar?.get('baz')? += 1
";
                check_script_output(script, KValue::Null);
            }

            #[test]
            fn check_into_piped_call_pass() {
                let script = "
m = {foo: |x| x}
42 -> m.foo?
";
                check_script_output(script, 42);
            }

            #[test]
            fn check_into_piped_call_after_call_pass() {
                let script = "
m = {foo: || |x| x}
42 -> m.foo()?
";
                check_script_output(script, 42);
            }

            #[test]
            fn check_into_piped_call_with_short_circuit() {
                let script = "
m = {foo: null}
42 -> m.foo?
";
                check_script_output(script, KValue::Null);
            }

            #[test]
            fn check_before_piped_call_pass() {
                let script = "
f = |x|
  x = x or 0
  x * x
m = {foo: || 10}
m.foo?() -> f
";
                check_script_output(script, 100);
            }

            #[test]
            fn check_short_circuited_before_piped_call() {
                let script = "
f = |x|
  x = x or 0
  x * x
m = {foo: null}
m.foo?() -> f
";
                check_script_output(script, 0);
            }

            #[test]
            fn check_after_call() {
                let script = "
m = {foo: || 42}
m.foo()? # The check is redundant, but shouldn't error
";
                check_script_output(script, 42);
            }

            #[test]
            fn check_when_result_is_temporary() {
                // The result should be 0 due to `m` not containing a value for `bar`
                // If the temporary register used by the match expression isn't cleared correctly,
                // then the `other` arm will be matched instead of the `null` arm.
                let script = "
m = {foo: 42.0}
m.get('foo')?.floor() # Fill temporary registers with non-null values
match m.get('bar')?.floor()
  null then 0
  other then other
";
                check_script_output(script, 0);
            }
        }
    }

    mod placeholders {
        use super::*;

        #[test]
        fn placeholder_in_multi_assignment() {
            let script = "
f = || 1, 2, 3
a, _, c = f()
a, c";
            check_script_output(script, number_tuple(&[1, 3]));
        }

        #[test]
        fn placeholder_argument() {
            let script = "
fold = |xs, f|
  result = 0
  for x in xs
    result = f result, x
  result
fold 0..5, |n, _| n + 1";
            check_script_output(script, 5);
        }
    }

    mod generators {
        use super::*;

        #[test]
        fn generator_two_values() {
            let script = "
gen = ||
  yield 1
  yield 2
gen().to_tuple()";
            check_script_output(script, number_tuple(&[1, 2]));
        }

        #[test]
        fn generator_loop() {
            let script = "
gen = || -> Number
  x = 1
  while x <= 5
    yield x
    x += 1
gen().to_tuple()";
            check_script_output(script, number_tuple(&[1, 2, 3, 4, 5]));
        }

        #[test]
        fn generator_with_arg() {
            let script = "
gen = |xs|
  for x in xs
    yield x
gen(1..=5).to_tuple()";
            check_script_output(script, number_tuple(&[1, 2, 3, 4, 5]));
        }

        #[test]
        fn generator_with_default_arg() {
            let script = "
gen = |xs = (1, 2, 3)|
  for x in xs
    yield x
gen().to_tuple()";
            check_script_output(script, number_tuple(&[1, 2, 3]));
        }

        #[test]
        fn generator_variadic() {
            let script = "
gen = |offset, xs...|
  for x in xs
    yield x + offset
gen(10, 1, 2, 3).to_tuple()";
            check_script_output(script, number_tuple(&[11, 12, 13]));
        }

        #[test]
        fn generator_returning_multiple_values() {
            let script = "
gen = |xs| -> Tuple
  for i, x in xs.enumerate()
    yield i, x
z = gen(1..=5).to_tuple()
z[1]";
            check_script_output(script, number_tuple(&[1, 2]));
        }

        #[test]
        fn generator_with_captured_data() {
            let script = "
x = 1, 2, 3
gen = ||
  for y in x
    yield y
gen().to_tuple()
";
            check_script_output(script, number_tuple(&[1, 2, 3]));
        }

        #[test]
        fn generator_with_captured_data_and_default_arg() {
            let script = "
x = 1, 2, 3
gen = |offset = 10, bar...|
  for y in x
    yield y + offset
gen().to_tuple()
";
            check_script_output(script, number_tuple(&[11, 12, 13]));
        }

        #[test]
        fn generator_as_iterator_adaptor() {
            let script = "
iterator.every_other = ||
  n = 0
  for output in self
    if n % 2 == 0 then yield output
    n += 1
(1..=5).every_other().to_tuple()
";
            check_script_output(script, number_tuple(&[1, 3, 5]));
        }

        #[test]
        fn yielding_null() {
            let script = "
gen = ||
  yield 1
  yield null
  yield 3
gen().to_tuple()
";
            check_script_output(script, tuple(&[1.into(), KValue::Null, 3.into()]));
        }

        #[test]
        fn exhausted_generator_returns_null() {
            let script = "
gen = || yield 1
x = gen()

x.next() # -> IteratorOutput(1)
x.next() # -> null

# The generator is now exhausted, calling .next() again should return null
x.next()
";
            check_script_output(script, KValue::Null);
        }
    }

    mod strings {
        use super::*;

        #[test]
        fn addition() {
            check_script_output(r#""Hello, " + "World!""#, "Hello, World!");
        }

        #[test]
        fn less() {
            check_script_output(r#""abc" < "abd""#, true);
            check_script_output(r#""abx" < "abc""#, false);
        }

        #[test]
        fn less_or_equal() {
            check_script_output(r#""abc" <= "abc""#, true);
            check_script_output(r#""xyz" <= "abd""#, false);
        }

        #[test]
        fn greater() {
            check_script_output(r#""hello42" > "hello1""#, true);
            check_script_output(r#""hello1" > "hellø1""#, false);
        }

        #[test]
        fn greater_or_equal() {
            check_script_output(r#""hello42" >= "hello11""#, true);
            check_script_output(r#""hello1" >= "hello42""#, false);
        }

        #[test]
        fn index_single_index() {
            check_script_output("'hello'[1]", "e");
        }

        #[test]
        fn index_start_and_end() {
            check_script_output("'hello'[1..2]", "e");
            check_script_output("'hello'[1..3]", "el");
            check_script_output("'föo'[1..3]", "ö");
        }

        #[test]
        fn index_from_start() {
            check_script_output("'hello'[2..]", "llo");
            check_script_output("'hello'[3..]", "lo");
        }

        #[test]
        fn index_to_end() {
            check_script_output("'hello'[..1]", "h");
            check_script_output("'hello'[..=2]", "hel");
        }

        #[test]
        fn index_from_one_past_the_end() {
            check_script_output("'x'[0..1]", "x");
            check_script_output("'x'[1..]", "");
            check_script_output("'x'[1..1]", "");
            check_script_output("'hello'[5..]", "");
        }

        #[test]
        fn index_whole_string() {
            check_script_output("'hello'[..]", "hello");
        }

        #[test]
        fn index_sub_string() {
            check_script_output("'hello'[3..][..]", "lo");
            check_script_output("'hello'[3..][1]", "o");
        }

        #[test]
        fn escaped_backslash() {
            check_script_output(r#""\\""#, "\\");
        }
    }

    mod string_interpolation {
        use super::*;

        #[test]
        fn id() {
            let script = "
x = 1
'{x} + {x}'
";
            check_script_output(script, "1 + 1");
        }

        #[test]
        fn id_from_capture() {
            let script = "
x = 1
f = || '{x}.{x}'
f()
";
            check_script_output(script, "1.1");
        }

        #[test]
        fn expression() {
            let script = "
x = 100
'sqrt(x): {x.sqrt()}'
";
            check_script_output(script, "sqrt(x): 10.0");
        }

        #[test]
        fn nested_expression() {
            let script = "
'foo{': {42}'}'
";
            check_script_output(script, "foo: 42");
        }

        #[test]
        fn inline_map() {
            let script = "
foo = |m| size m
'{foo {bar: 42, baz: 99}}!'
";
            check_script_output(script, "2!");
        }

        #[test]
        fn using_capture() {
            let script = "
x = 10
f = || 'x * 2 == {x * 2}'
f()
";
            check_script_output(script, "x * 2 == 20");
        }

        #[test]
        fn interpolated_string_as_map_key() {
            let script = "
x = 99
m =
  'key{x}': 'foo'
m.key99
";
            check_script_output(script, "foo");
        }

        #[test]
        fn interpolated_string_in_chain() {
            let script = "
x = 99
m =
  'key{x}': 'foo'
m.'key{x}'
";
            check_script_output(script, "foo");
        }

        #[test]
        fn interpolated_string_in_chain_assignment() {
            let script = "
x = 99
m =
  'key{x}': 'foo'
m.'key{x}' = 123
m.'key{x}'
";
            check_script_output(script, 123);
        }

        #[test]
        fn value_with_overridden_display() {
            let script = "
foo = {@display: || 'Foo'}
'{foo}'
";
            check_script_output(script, "Foo");
        }

        #[test]
        fn multiple_expressions() {
            let script = "
'{1, 2, 3}'
";
            check_script_output(script, "(1, 2, 3)");
        }

        #[test]
        fn recursive_list() {
            let script = "
x = [1, 2]
x.push x
'{x}'
";
            check_script_output(script, "[1, 2, [...]]");
        }

        #[test]
        fn recursive_map() {
            let script = "
x = {foo: 1, bar: 2}
x.baz = x
'{x}'
";
            check_script_output(script, "{foo: 1, bar: 2, baz: {...}}");
        }

        #[test]
        fn strings_in_tuples() {
            let script = "
x = ('foo', 'bar')
'{x}'
";
            check_script_output(script, "('foo', 'bar')");
        }

        #[test]
        fn escaped_curly_brace() {
            let script = r#"
'\{x}'
"#;
            check_script_output(script, "{x}");
        }

        use test_case::test_case;

        #[test_case("'{42:10}'", "        42"; "min width with integer")]
        #[test_case("'{42:06}'", "000042"; "min width with zero-prefixed integer")]
        #[test_case("'{42:1<4}'", "4211"; "min width with integer fill")]
        #[test_case("'{42:-<10}'", "42--------"; "min width with left-aligned integer")]
        #[test_case("'{1/3:_^11.3}'", "___0.333___"; "fill with centered float")]
        #[test_case("'{'hello':.2}'", "he"; "precision with string")]
        #[test_case("'{'hello':10}'", "hello     "; "min width with string")]
        #[test_case("'{'hello':~>4.2}'", "~~he"; "right-aligned truncated string")]
        fn formatted_expression(input: &str, expected: &str) {
            check_script_output(input, expected);
        }
    }

    mod raw_strings {
        use super::*;

        #[test]
        fn unescaped_backslashes() {
            let script = r"
r'\r\n\\\{\'
";
            check_script_output(script, r"\r\n\\\{\");
        }

        #[test]
        fn uninterpolated_expressions() {
            let script = r"
foo, bar = 42, 99
r'{foo} + {bar} == {foo + bar}'
";
            check_script_output(script, r"{foo} + {bar} == {foo + bar}");
        }

        #[test]
        fn multiline() {
            let script = r#"
r"
{foo}
\n
{bar}
"
"#;
            check_script_output(
                script,
                r"
{foo}
\n
{bar}
",
            );
        }
    }

    mod iterators {
        use super::*;

        #[test]
        fn iterator_copy() {
            let script = "
x = (1..10).iter()
z = copy x
x.next()
x.next()
z.next()
z.next().get()
";
            check_script_output(script, 2);
        }

        #[test]
        fn iterators_in_a_deep_copy() {
            let script = "
r = 1..10
x = [r.iter()]
z = koto.deep_copy x
x[0].next()
x[0].next()
z[0].next()
z[0].next().get()
";
            check_script_output(script, 2);
        }

        #[test]
        fn copy_of_a_generator() {
            let script = "
generator = ||
  for x in (1, 2, 3, 4, 5)
    yield x
x = generator()
x.next() # 1
y = copy x
x.next() # 2
x.next() # 3
y.next().get()
";
            check_script_output(script, 2);
        }
    }

    mod error_recovery {
        use super::*;

        #[test]
        fn try_catch() {
            let script = "
x = 1
try
  x += 1
  x += y
catch _
  x + 1
";
            check_script_output(script, 3);
        }

        #[test]
        fn try_catch_with_throw_string() {
            let script = "
x = 1
try
  x += 1
  throw '{x}'
catch error
  error
";
            check_script_output(script, "2");
        }

        #[test]
        fn try_catch_with_throw_map() {
            let script = r#"
x = 1
try
  x += 1
  throw
    data: x
    @display: || "error!"
catch error
  error.data
"#;
            check_script_output(script, 2);
        }

        #[test]
        fn try_catch_finally() {
            let script = "
try
  x
catch _e
  -1
finally
  99
";
            check_script_output(script, 99);
        }

        #[test]
        fn try_catch_with_type_checks() {
            let script = "
x = 1
try
  x += 1
  throw '{x}'
catch error: Bool
  throw 'Caught bool'
catch _error: Number
  throw 'Caught number'
catch error: String
  error
catch error
  throw 'Fallback'
";
            check_script_output(script, "2");
        }

        #[test]
        fn try_catch_nested() {
            let script = "
x = 0
try
  x += 1
  try
    x += 1
    x += y
  catch _ignored
    x += 1
  x += y
catch _
  x += 1
";
            check_script_output(script, 4);
        }

        #[test]
        fn catch_throw_from_map_creation() {
            // This would be a strange thing to do, but the compiler previously melted down while
            // trying to compile the throw expression as map value, which it shouldn't do.
            let script = "
try
  x =
    foo: throw 'foo'
catch _
  99
";
            check_script_output(script, 99);
        }
    }

    mod overridden_operators {
        use super::*;

        #[test]
        fn arithmetic() {
            let script = "
locals = {}
foo = |x| {x}.with_meta locals.foo_meta
locals.foo_meta =
  @+: |other| foo self.x + other.x
  @-: |other| foo self.x - other.x
  @*: |other| foo self.x * other.x
  @/: |other| foo self.x / other.x
  @%: |other| foo self.x % other.x

z = ((foo 2) * (foo 10) / (foo 4) + (foo 1) - (foo 2)) % foo 3
z.x
";
            check_script_output(script, 1);
        }

        #[test]
        fn compound_assignment() {
            let script = "
locals = {}
foo = |x| {x}.with_meta locals.foo_meta
locals.foo_meta =
  @+=: |y| self.x += y
  @-=: |y| self.x -= y
  @*=: |y| self.x *= y
  @/=: |y| self.x /= y
  @%=: |y| self.x %= y

z = foo 2
z += 10 # 12
z *= 3  # 36
z /= 2  # 18
z -= 3  # 15
z %= 4  # 3
z.x
";
            check_script_output(script, 3);
        }

        #[test]
        fn less() {
            let script = "
foo = |x|
  x: x
  @<: |other| self.x < other.x

(foo 10) < (foo 20) and not (foo 30) < (foo 30)
";
            check_script_output(script, true);
        }

        #[test]
        fn less_or_equal() {
            let script = "
foo = |x|
  x: x
  @<=: |other| self.x <= other.x

(foo 10) <= (foo 20) and (foo 30) <= (foo 30)
";
            check_script_output(script, true);
        }

        #[test]
        fn greater() {
            let script = "
foo = |x|
  x: x
  @>: |other| self.x > other.x

(foo 0) > (foo -1) and not (foo 0) > (foo 0)
";
            check_script_output(script, true);
        }

        #[test]
        fn greater_or_equal() {
            let script = "
foo = |x|
  x: x
  @>=: |other| self.x >= other.x

(foo 50) >= (foo 40) and (foo 50) >= (foo 50)
";
            check_script_output(script, true);
        }

        #[test]
        fn equal() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map equality behaviour to show its effect
    self.x != other.x

(foo 41) == (foo 42) and not (foo 42) == (foo 42)
";
            check_script_output(script, true);
        }

        #[test]
        fn not_equal() {
            let script = "
foo = |x|
  x: x
  @!=: |other|
    # Invert the default map inequality behaviour to show its effect
    self.x == other.x

(foo 99) != (foo 99) and not (foo 99) != (foo 100)
";
            check_script_output(script, true);
        }

        #[test]
        fn equality_of_list_containing_overridden_value() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map inequality behaviour to show its effect
    self.x != other.x

a = [foo(0), foo(1)]
b = [foo(1), foo(2)]
a == b # Should evaluate to true due to the inverted equality operator
";
            check_script_output(script, true);
        }

        #[test]
        fn equality_of_map_containing_overridden_value() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map inequality behaviour to show its effect
    self.x != other.x

a = { foo: foo(42) }
b = { foo: foo(99) }
a == b # Should evaluate to true due to the inverted equality operator
";
            check_script_output(script, true);
        }

        #[test]
        fn equality_of_tuple_containing_overridden_value() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map inequality behaviour to show its effect
    self.x != other.x

a = (foo(0), foo(1))
b = (foo(1), foo(2))
a == b # Should evaluate to true due to the inverted equality operator
";
            check_script_output(script, true);
        }

        #[test]
        fn inequality_of_list_containing_overridden_value() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map equality behaviour to show its effect
    self.x != other.x

a = [foo(0), foo(0)]
b = [foo(0), foo(0)]
a != b # Should evaluate to true due to the inverted equality operator
";
            check_script_output(script, true);
        }

        #[test]
        fn inequality_of_map_containing_overridden_value() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map equality behaviour to show its effect
    self.x != other.x

a = { foo: foo(42) }
b = { foo: foo(42) }
a != b # Should evaluate to true due to the inverted equality operator
";
            check_script_output(script, true);
        }

        #[test]
        fn inequality_of_tuple_containing_overridden_value() {
            let script = "
foo = |x|
  x: x
  @==: |other|
    # Invert the default map equality behaviour to show its effect
    self.x != other.x

a = (foo(1), foo(2))
b = (foo(1), foo(2))
a != b # Should evaluate to true due to the inverted equality operator
";
            check_script_output(script, true);
        }

        #[test]
        fn deep_copy_includes_meta_map() {
            let script = "
foo = |x|
  x: x
  @>=: |other| self.x >= other.x

a = foo 42
b = koto.deep_copy a
b >= a
";
            check_script_output(script, true);
        }

        #[test]
        fn equality_of_functions_with_overridden_captures() {
            let script = "
# Make two functions which capture different values of `foo`
f, g = (0, 1)
  .each |n|
    foo =
      x: n
      # Invert the equality comparison so that its effect is visible
      @==: |other| self.x != other.x
    || foo

# The overridden operator on the captured value of `foo` inverts the usual comparison logic
f == g, f != g
";
            check_script_output(script, tuple(&[true.into(), false.into()]));
        }

        #[test]
        fn map_addition_with_overridden_operators() {
            let script = "
foo = |a, b|
  @meta a: a
  @meta b: b
  get_a: || self.a
  get_b: || self.b

bar = |b|
  @meta b: b

a = foo 100, 42
b = bar 1000
c = a + b
c.get_a() + c.get_b()
";
            check_script_output(script, 1100);
        }
    }

    mod overridden_call {
        use super::*;

        #[test]
        fn basic_call() {
            let script = "
x = { @call: || 42 }
x()
";
            check_script_output(script, 42);
        }

        #[test]
        fn with_args() {
            let script = "
x = { @call: |a, b| a + b }
x 12, 34
";
            check_script_output(script, 46);
        }

        #[test]
        fn instance() {
            let script = "
x =
  data: 99
  @call: |z| self.data * z
x 10
";
            check_script_output(script, 990);
        }
    }

    mod overridden_index_and_size {
        use super::*;

        #[test]
        fn index() {
            let script = "
x =
  data: [1, 2, 3]
  @index: |i| self.data[i]
x[1] + x[2]
";
            check_script_output(script, 5);
        }

        #[test]
        fn index_mut() {
            let script = "
x =
  data: [1, 2, 3]
  @index: |i| self.data[i]
  @index_mut: |i, x| self.data[i] = x
x[1] = 99
x[2] = 1
x[1] + x[2]
";
            check_script_output(script, 100);
        }

        #[test]
        fn index_mut_result_is_rhs() {
            let script = "
x =
  @index_mut: |_i, _x|
    -1 # The result of @index_mut should be discarded
x[1] = 99
";
            check_script_output(script, 99);
        }

        #[test]
        fn size() {
            let script = "
foo = |n|
  n: n
  @size: || self.n
x = foo 99
size x
";
            check_script_output(script, 99);
        }

        #[test]
        fn argument_unpacking() {
            let script = "
foo = |data|
  data: data
  @index: |index| self.data[index]
  @size: || size self.data

f = |(a, b, others...)| a + b + size others
x = foo (10, 11, 12, 13)
f x # 10 + 11 + 2
";
            check_script_output(script, 23);
        }

        #[test]
        fn match_unpacking() {
            let script = "
foo = |data|
  data: data
  @index: |index| self.data[index]
  @size: || size self.data

match foo (10, 11, 12, 13)
  (a) then 99
  (a, b) then -1
  (a, b, c, others...) then
    # 10 + 11 + 12 + 1
    a + b + c + size others
";
            check_script_output(script, 34);
        }
    }

    mod overridden_iterator {
        use super::*;

        #[test]
        fn unpacking() {
            let script = "
x =
  @iterator: ||
    yield 10
    yield 20
a, b, c = x
a, b, c
";
            check_script_output(script, tuple(&[10.into(), 20.into(), KValue::Null]));
        }
    }

    mod overridden_next {
        use super::*;

        #[test]
        fn next() {
            let script = "
x =
  foo: 0
  @next: || self.foo += 1

x.take(3).to_tuple()
";
            check_script_output(script, number_tuple(&[1, 2, 3]));
        }

        #[test]
        fn next_back() {
            let script = "
x =
  foo: 0
  @next: || self.foo += 1
  @next_back: || self.foo -= 1

x.skip(3).reversed().take(3).to_tuple()
";
            check_script_output(script, number_tuple(&[2, 1, 0]));
        }
    }

    mod named_meta_entries {
        use super::*;

        #[test]
        fn basic_access() {
            let script = "
locals = {}
foo = |x| {x}.with_meta locals.foo_meta
locals.foo_meta =
  @meta get_x: || self.x
a = foo 10
a.x + a.get_x()
";
            check_script_output(script, 20);
        }

        #[test]
        fn access_order() {
            let script = "
locals = {}
foo = |x| {x, y: 100}.with_meta locals.foo_meta
locals.foo_meta =
  @meta y: 0
a = foo 10
a.x + a.y # The meta map's y entry is hidden by the data entry
";
            check_script_output(script, 110);
        }
    }

    mod base_access {
        use super::*;

        #[test]
        fn base_entry() {
            let script = "
animal = |name|
  name: name
  speak: || throw 'unimplemented'

dog = |name|
  @base: animal name
  speak: || 'Woof! My name is {self.name}'

dog('Fido').speak()
";
            check_script_output(script, "Woof! My name is Fido");
        }

        #[test]
        fn type_check_looks_at_base_when_no_type_found() {
            let script = "
animal = |name|
  @type: 'Animal'
  name: name

dog = |name|
  @base: animal name

let x: Animal = dog 'Fido'
type x
";
            check_script_output(script, "Animal");
        }

        #[test]
        fn type_check_looks_at_base_when_type_doesnt_match() {
            let script = "
animal = |name|
  @type: 'Animal'
  name: name

dog = |name|
  @base: animal name
  @type: 'Dog'

corgi = |name|
  @base: dog name
  @type: 'Corgi'

let x: Animal = corgi 'Fido'
type x
";
            check_script_output(script, "Corgi");
        }
    }

    mod import {
        use super::*;

        #[test]
        fn import_after_local_assignment() {
            let script = "
x = 123
y = from number import pi
x";
            check_script_output(script, 123);
        }

        #[test]
        fn import_with_same_local_name() {
            let script = "
x = 0
pi = number.pi
pi != x and pi == pi";
            check_script_output(script, true);
        }

        #[test]
        fn import_as_single_item() {
            let script = "
import number as num
num.abs -42";
            check_script_output(script, 42);
        }

        #[test]
        fn import_as_with_string_item() {
            let script = "
import 'number' as num
num.abs -42";
            check_script_output(script, 42);
        }

        #[test]
        fn from_import_as_single_item() {
            let script = "
from number import pi as 𝜋
number.pi == 𝜋";
            check_script_output(script, true);
        }

        #[test]
        fn from_import_as_with_string_item() {
            let script = "
from number import 'pi' as 𝜋
number.pi == 𝜋";
            check_script_output(script, true);
        }
    }

    mod export {
        use super::*;

        #[test]
        fn export_in_function() {
            let script = "
f = || export x = 42
f()
x";
            check_script_output(script, 42);
        }

        #[test]
        fn accessing_value_exported_after_function_creation() {
            let script = "
f = || x
export x = 99
f()";
            check_script_output(script, 99);
        }

        #[test]
        fn capture_of_value_exported_before_function_creation() {
            let script = "
export x = 123
f = || x
# Re-exporting x doesn't affect the value captured when f was created
export x = 99
f()";
            check_script_output(script, 123);
        }

        #[test]
        fn assignment_of_export() {
            let script = "
x = export y = 10
x + y";
            check_script_output(script, 20);
        }

        #[test]
        fn map_export() {
            let script = "
export
  x: 1
  y: x + 1
x + y
";
            check_script_output(script, 3);
        }

        #[test]
        fn map_export_of_previously_assigned_variable() {
            let script = "
x = 99
export {x: 1} # The local variable should be updated
x
";
            check_script_output(script, 1);
        }
    }

    mod meta_export {
        use super::*;

        #[test]
        fn assignment_of_meta_export() {
            let script = "
f = @main = || 42
f()";
            check_script_output(script, 42);
        }
    }

    mod debug {
        use super::*;

        #[test]
        fn debug_provides_expression_result() {
            let script = "
debug 1 + 1
";
            check_script_output(script, 2);
        }
    }
}
