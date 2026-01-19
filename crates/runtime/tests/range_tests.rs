use koto_test_utils::*;

mod range {
    use super::*;
    use test_case::test_case;

    #[test_case("1..1", "3..3", "1..3")]
    #[test_case("1..10", "10..20", "1..20")]
    #[test_case("10..20", "15..20", "10..20")]
    #[test_case("10..=20", "25..=30", "10..=30")]
    #[test_case("10..=10", "20..20", "10..20")]
    #[test_case("11..=11", "20..=20", "11..=20")]
    #[test_case("11..=11", "20..=19", "11..20")]
    #[test_case("12..12", "20..20", "12..20")]
    #[test_case("10..1", "20..15", "10..20")]
    #[test_case("11..1", "20..=13", "11..20")]
    #[test_case("100..=10", "200..150", "100..200")]
    #[test_case("100..=10", "200..=250", "100..=250")]
    fn union_with_range(a: &str, b: &str, expected: &str) {
        let script = format!(
            "
assert_eq ({a}).union({b}), {expected}
assert_eq ({b}).union({a}), {expected}
"
        );
        check_script_output(&script, ());
    }

    #[test_case("1..=10", "1", "1..=10")]
    #[test_case("1..=10", "10", "1..=10")]
    #[test_case("1..=10", "15", "1..=15")]
    #[test_case("1..=10", "-5", "-5..=10")]
    #[test_case("10..20", "0", "0..20")]
    #[test_case("10..20", "20", "10..=20")]
    #[test_case("10..20", "25", "10..=25")]
    fn union_with_scalar(a: &str, b: &str, expected: &str) {
        let script = format!(
            "
assert_eq ({a}).union({b}), {expected}
assert_eq ({b}..={b}).union({a}), {expected}
"
        );
        check_script_output(&script, ());
    }

    #[test_case("1..=3", "(1, 2, 3)")]
    #[test_case("(1..=3).reversed()", "(3, 2, 1)")]
    #[test_case("-3..0", "(-3, -2, -1)")]
    #[test_case("(-3..0).reversed()", "(-1, -2, -3)")]
    fn as_iterator(range: &str, expected: &str) {
        let script = format!(
            "
assert_eq ({range}).to_tuple(), {expected}"
        );
        check_script_output(&script, ());
    }
}
