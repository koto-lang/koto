use koto_test_utils::*;

mod range {
    use super::*;
    use test_case::test_case;

    #[test_case("1..=10", "15", "1..=15")]
    #[test_case("1..10", "15", "1..16")]
    #[test_case("1..=10", "-5", "-5..=10")]
    #[test_case("1..=10", "15..=20", "1..=20")]
    #[test_case("10..=1", "20..15", "20..=1")]
    #[test_case("11..1", "20..=13", "20..1")]
    #[test_case("10..20", "15..20", "10..20")]
    #[test_case("1..1", "3..3", "1..3")]
    fn union(a: &str, b: &str, expected: &str) {
        let script = format!(
            "
assert_eq ({a}).union({b}), {expected}
"
        );
        check_script_output(&script, ());
    }

    #[test_case("1..=3", "(1, 2, 3)")]
    #[test_case("(1..=3).reversed()", "(3, 2, 1)")]
    #[test_case("-3..0", "(-3, -2, -1)")]
    #[test_case("(-3..0).reversed()", "(-1, -2, -3)")]
    #[test_case("4..=1", "(4, 3, 2, 1)")]
    #[test_case("(4..=1).reversed()", "(1, 2, 3, 4)")]
    fn as_iterator(range: &str, expected: &str) {
        let script = format!(
            "
assert_eq ({range}).to_tuple(), {expected}"
        );
        check_script_output(&script, ());
    }
}
