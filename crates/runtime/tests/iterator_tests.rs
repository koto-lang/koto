use koto_runtime::KValue;
use koto_test_utils::*;

mod iterator {
    use super::*;

    mod next {
        use super::*;

        #[test]
        fn null_in_output() {
            let script = "
x = (1, null, 'x').iter()
x.next().get() # 1
x.next().get() # null
";
            check_script_output(script, KValue::Null);
        }
    }

    mod next_back {
        use super::*;

        #[test]
        fn null_in_output() {
            let script = "
x = (1, null, 'x').iter()
x.next_back().get() # 'x'
x.next_back().get() # null
";
            check_script_output(script, KValue::Null);
        }
    }

    mod chain {
        use super::*;

        #[test]
        fn make_copy_in_first_iter() {
            let script = "
x = (10..12).chain 12..15
x.next().get() # 10
y = copy x
x.next().get() # 11
x.next().get() # 12
y.next().get()
";
            check_script_output(script, 11);
        }

        #[test]
        fn make_copy_in_second_iter() {
            let script = "
x = (0..2).chain 2..5
x.next().get() # 0
x.next().get() # 1
x.next().get() # 2
y = copy x
x.next().get() # 3
x.next().get() # 4
y.next().get()
";
            check_script_output(script, 3);
        }
    }

    mod chunks {
        use super::*;

        #[test]
        fn with_generator() {
            let script = "
generator = || 
  for i in 1..=4
    yield i
generator()
  .chunks 2
  .skip 1
  .flatten()
  .to_tuple()
";
            check_script_output(script, number_tuple(&[3, 4]));
        }

        #[test]
        fn with_peekable() {
            let script = "
(1..=5)
  .peekable()
  .chunks 2
  .each |w| w.to_tuple()
  .to_tuple()
";
            check_script_output(
                script,
                tuple(&[
                    number_tuple(&[1, 2]),
                    number_tuple(&[3, 4]),
                    number_tuple(&[5]),
                ]),
            );
        }
    }

    mod cycle {
        use super::*;

        #[test]
        fn empty_input() {
            let script = "
[].cycle().next()
";
            check_script_output(script, KValue::Null);
        }

        #[test]
        fn make_copy() {
            let script = "
x = (1..=3).cycle()
x.next().get() # 1
y = copy x
x.next().get() # 2
x.next().get() # 3
y.next().get()
";
            check_script_output(script, 2);
        }

        #[test]
        fn with_peekable() {
            let script = "
(1..=3)
  .peekable()
  .cycle()
  .take 7
  .to_tuple()
";
            check_script_output(script, number_tuple(&[1, 2, 3, 1, 2, 3, 1]));
        }
    }

    mod each {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (3, 4, 5, 6).each |x| x * x
x.next().get() # 9
y = copy x
x.next().get() # 16
x.next().get() # 25
y.next().get()
";
            check_script_output(script, 16);
        }

        #[test]
        fn each_reversed() {
            let script = "
x = (2, 4, 6)
 .each |x| x * x
 .reversed()
x.next().get()
";
            check_script_output(script, 36);
        }
    }

    mod enumerate {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (10..20).enumerate()
x.next().get() # 0, 10
y = copy x
x.next().get() # 1, 11
x.next().get() # 2, 12
y.next().get()
";
            check_script_output(script, tuple(&[1.into(), 11.into()]));
        }
    }

    mod intersperse {
        use super::*;

        #[test]
        fn intersperse_by_value_make_copy() {
            let script = "
x = (1, 2, 3).intersperse 0
x.next().get() # 1
x.next().get() # 0
y = copy x
x.next().get() # 2
x.next().get() # 0
y.next().get()
";
            check_script_output(script, 2);
        }

        #[test]
        fn intersperse_with_function_make_copy() {
            let script = "
x = (10, 20, 30).intersperse || 42
x.next().get() # 10
x.next().get() # 42
y = copy x
x.next().get() # 20
x.next().get() # 42
y.next().get()
";
            check_script_output(script, 20);
        }
    }

    mod keep {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abcdef'.chars().keep |c| 'bef'.contains c
x.next().get() # 'b'
y = copy x
x.next().get() # 'e'
y.next().get()
";
            check_script_output(script, "e");
        }
    }

    mod peekable {
        use super::*;
        use KValue::Null;

        #[test]
        fn peek() {
            let script = "
i = (1, 2, 3).peekable()
result = []
result.push i.peek().get() # 1
result.push i.next().get() # 1
result.push i.next().get() # 2
result.push i.peek().get() # 3
result.push i.next().get() # 3
result.push i.peek() # null
result.push i.next() # null
result
";
            check_script_output(
                script,
                list(&[1.into(), 1.into(), 2.into(), 3.into(), 3.into(), Null, Null]),
            );
        }

        #[test]
        fn peek_back_forwards() {
            let script = "
i = (1, 2, 3).peekable()
result = []
result.push i.peek().get() # 1
result.push i.peek_back().get() # 3
result.push i.peek_back().get() # 3
result.push i.next().get() # 1
result.push i.next().get() # 2
result.push i.peek().get() # 3
result.push i.next().get() # 3
result.push i.next() # null
result.push i.next_back() # null
result.push i.peek_back() # null
result
";
            check_script_output(
                script,
                list(&[
                    1.into(),
                    3.into(),
                    3.into(),
                    1.into(),
                    2.into(),
                    3.into(),
                    3.into(),
                    Null,
                    Null,
                    Null,
                ]),
            );
        }

        #[test]
        fn peek_back_backwards() {
            let script = "
i = (1, 2, 3).peekable()
result = []
result.push i.peek().get() # 1
result.push i.peek_back().get() # 3
result.push i.peek_back().get() # 3
result.push i.next_back().get() # 3
result.push i.next_back().get() # 2
result.push i.peek_back().get() # 1
result.push i.next_back().get() # 1
result.push i.peek_back() # null
result.push i.next_back() # null
result.push i.next() # null
result
";
            check_script_output(
                script,
                list(&[
                    1.into(),
                    3.into(),
                    3.into(),
                    3.into(),
                    2.into(),
                    1.into(),
                    1.into(),
                    Null,
                    Null,
                    Null,
                ]),
            );
        }
    }

    mod skip {
        use super::*;

        #[test]
        fn skip_past_end_then_collect_shouldnt_panic() {
            let script = "
[].skip(1).to_tuple()
";
            check_script_output(script, tuple(&[]));
        }
    }

    mod take {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abcdef'.chars().take 4
x.next().get() # 'a'
x.next().get() # 'b'
y = copy x
x.next().get() # 'c'
y.next().get()
";
            check_script_output(script, "c");
        }
    }

    mod windows {
        use super::*;

        #[test]
        fn with_a_generator() {
            let script = "
generator = ||
  for i in 1..=4
    yield i
generator()
  .windows 2
  .flatten()
  .to_tuple()
";
            check_script_output(script, number_tuple(&[1, 2, 2, 3, 3, 4]));
        }

        #[test]
        fn in_for_loop() {
            let script = "
result = []
for a, b in (1..=5).windows(2)
  result.push a
  result.push b
result
";
            check_script_output(script, number_list(&[1, 2, 2, 3, 3, 4, 4, 5]));
        }

        #[test]
        fn with_peekable() {
            let script = "
(1..=5)
  .peekable()
  .windows 3
  .each |w| w.to_tuple()
  .to_tuple()
";
            check_script_output(
                script,
                tuple(&[
                    number_tuple(&[1, 2, 3]),
                    number_tuple(&[2, 3, 4]),
                    number_tuple(&[3, 4, 5]),
                ]),
            );
        }
    }

    mod zip {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (1..5).zip 11..15
x.next().get() # (1, 11)
x.next().get() # (2, 12)
y = copy x
x.next().get() # (3, 13)
y.next().get()
";
            check_script_output(script, number_tuple(&[3, 13]));
        }
    }
}

mod map {
    use super::*;

    mod keys {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = {foo: 42, bar: 99, baz: -1}.keys()
x.next().get() # foo
y = copy x
x.next().get() # bar
y.next().get()
";
            check_script_output(script, "bar");
        }
    }

    mod values {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = {foo: 42, bar: 99, baz: -1}.values()
x.next().get() # 42
y = copy x
x.next().get() # 99
y.next().get()
";
            check_script_output(script, 99);
        }
    }
}

mod string {
    use super::*;

    mod graphemes {
        use super::*;

        #[test]
        fn next_back() {
            let script = "
'abc'.next_back().get()
";
            check_script_output(script, "c");
        }
    }

    mod bytes {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abc'.bytes()
x.next().get() # 97
y = copy x
x.next().get() # 98
y.next().get()
";
            check_script_output(script, 98);
        }
    }

    mod char_indices {
        use super::*;

        #[test]
        fn text() {
            let script = "
'xßîहिं'.char_indices().to_tuple()
";
            check_script_output(
                script,
                tuple(&[range(0..1), range(1..3), range(3..5), range(5..14)]),
            );
        }

        #[test]
        fn emojis() {
            let script = "
'👍🫶🏽🫱🏼‍🫲🏾'.char_indices().to_tuple()
";
            check_script_output(script, tuple(&[range(0..4), range(4..12), range(12..31)]));
        }
    }

    mod lines {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abc\ndef\nxyz'.lines()
x.next().get() # abc
y = copy x
x.next().get() # def
y.next().get()
";
            check_script_output(script, "def");
        }

        #[test]
        fn crlf_line_endings() {
            let script = "
'abc\r\ndef\r\nxyz\r\n\r\n'.lines().to_tuple()
";
            check_script_output(
                script,
                tuple(&["abc".into(), "def".into(), "xyz".into(), "".into()]),
            );
        }
    }

    mod split {
        use super::*;

        #[test]
        fn make_copy_pattern() {
            let script = "
x = '1-2-3'.split '-'
x.next().get() # 1
y = copy x
x.next().get() # 2
y.next().get()
";
            check_script_output(script, "2");
        }

        #[test]
        fn make_copy_predicate() {
            let script = "
x = '1-2_3'.split |c| '-_'.contains c
x.next().get() # 1
y = copy x
x.next().get() # 2
y.next().get()
";
            check_script_output(script, "2");
        }
    }
}
