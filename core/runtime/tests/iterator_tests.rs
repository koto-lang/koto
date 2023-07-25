mod runtime_test_utils;

use {crate::runtime_test_utils::*, koto_runtime::Value};

mod iterator {
    use super::*;

    mod chain {
        use super::*;

        #[test]
        fn make_copy_in_first_iter() {
            let script = "
x = (10..12).chain 12..15
x.next() # 10
y = copy x
x.next() # 11
x.next() # 12
y.next()
";
            test_script(script, 11);
        }

        #[test]
        fn make_copy_in_second_iter() {
            let script = "
x = (0..2).chain 2..5
x.next() # 0
x.next() # 1
x.next() # 2
y = copy x
x.next() # 3
x.next() # 4
y.next()
";
            test_script(script, 3);
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
            test_script(script, number_tuple(&[3, 4]));
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
            test_script(
                script,
                value_tuple(&[
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
        fn make_copy() {
            let script = "
x = (1..=3).cycle()
x.next() # 1
y = copy x
x.next() # 2
x.next() # 3
y.next()
";
            test_script(script, 2);
        }
    }

    mod each {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (3, 4, 5, 6).each |x| x * x
x.next() # 9
y = copy x
x.next() # 16
x.next() # 25
y.next()
";
            test_script(script, 16);
        }

        #[test]
        fn each_reversed() {
            let script = "
x = (2, 4, 6)
 .each |x| x * x
 .reversed()
x.next()
";
            test_script(script, 36);
        }
    }

    mod enumerate {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (10..20).enumerate()
x.next() # 0, 10
y = copy x
x.next() # 1, 11
x.next() # 2, 12
y.next()
";
            test_script(script, value_tuple(&[1.into(), 11.into()]));
        }
    }

    mod intersperse {
        use super::*;

        #[test]
        fn intersperse_by_value_make_copy() {
            let script = "
x = (1, 2, 3).intersperse 0
x.next() # 1
x.next() # 0
y = copy x
x.next() # 2
x.next() # 0
y.next()
";
            test_script(script, 2);
        }

        #[test]
        fn intersperse_with_function_make_copy() {
            let script = "
x = (10, 20, 30).intersperse || 42
x.next() # 10
x.next() # 42
y = copy x
x.next() # 20
x.next() # 42
y.next()
";
            test_script(script, 20);
        }
    }

    mod keep {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abcdef'.chars().keep |c| 'bef'.contains c
x.next() # 'b'
y = copy x
x.next() # 'e'
y.next()
";
            test_script(script, "e");
        }
    }

    mod peekable {
        use super::*;
        use Value::Null;

        #[test]
        fn peek() {
            use Value::Null;

            let script = "
i = (1, 2, 3).peekable()
result = []
result.push i.peek() # 1
result.push i.next() # 1
result.push i.next() # 2
result.push i.peek() # 3
result.push i.next() # 3
result.push i.peek() # null
result.push i.next() # null
result
";
            test_script(
                script,
                value_list(&[1.into(), 1.into(), 2.into(), 3.into(), 3.into(), Null, Null]),
            );
        }

        #[test]
        fn peek_back_forwards() {
            let script = "
i = (1, 2, 3).peekable()
result = []
result.push i.peek() # 1
result.push i.peek_back() # 3
result.push i.peek_back() # 3
result.push i.next() # 1
result.push i.next() # 2
result.push i.peek() # 3
result.push i.next() # 3
result.push i.next() # null
result.push i.next_back() # null
result.push i.peek_back() # null
result
";
            test_script(
                script,
                value_list(&[
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
result.push i.peek() # 1
result.push i.peek_back() # 3
result.push i.peek_back() # 3
result.push i.next_back() # 3
result.push i.next_back() # 2
result.push i.peek_back() # 1
result.push i.next_back() # 1
result.push i.peek_back() # null
result.push i.next_back() # null
result.push i.next() # null
result
";
            test_script(
                script,
                value_list(&[
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
            test_script(script, value_tuple(&[]));
        }
    }

    mod take {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abcdef'.chars().take 4
x.next() # 'a'
x.next() # 'b'
y = copy x
x.next() # 'c'
y.next()
";
            test_script(script, "c");
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
            test_script(script, number_tuple(&[1, 2, 2, 3, 3, 4]));
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
            test_script(script, number_list(&[1, 2, 2, 3, 3, 4, 4, 5]));
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
            test_script(
                script,
                value_tuple(&[
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
x.next() # (1, 11)
x.next() # (2, 12)
y = copy x
x.next() # (3, 13)
y.next()
";
            test_script(script, number_tuple(&[3, 13]));
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
x.next() # foo
y = copy x
x.next() # bar
y.next()
";
            test_script(script, "bar");
        }
    }

    mod values {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = {foo: 42, bar: 99, baz: -1}.values()
x.next() # 42
y = copy x
x.next() # 99
y.next()
";
            test_script(script, 99);
        }
    }
}

mod string {
    use super::*;

    mod bytes {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abc'.bytes()
x.next() # 97
y = copy x
x.next() # 98
y.next()
";
            test_script(script, 98);
        }
    }

    mod lines {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abc\ndef\nxyz'.lines()
x.next() # abc
y = copy x
x.next() # def
y.next()
";
            test_script(script, "def");
        }

        #[test]
        fn crlf_line_endings() {
            let script = "
'abc\r\ndef\r\nxyz\r\n\r\n'.lines().to_tuple()
";
            test_script(
                script,
                value_tuple(&["abc".into(), "def".into(), "xyz".into(), "".into()]),
            );
        }
    }

    mod split {
        use super::*;

        #[test]
        fn make_copy_pattern() {
            let script = "
x = '1-2-3'.split '-'
x.next() # 1
y = copy x
x.next() # 2
y.next()
";
            test_script(script, "2");
        }

        #[test]
        fn make_copy_predicate() {
            let script = "
x = '1-2_3'.split |c| '-_'.contains c
x.next() # 1
y = copy x
x.next() # 2
y.next()
";
            test_script(script, "2");
        }
    }
}
