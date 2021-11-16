mod runtime_test_utils;

use crate::runtime_test_utils::{number_tuple, test_script, value_tuple};

mod iterator {
    use super::*;

    mod chain {
        use super::*;

        #[test]
        fn make_copy_in_first_iter() {
            let script = "
x = (10..12).chain 12..15
x.next() # 10
y = x.copy()
x.next() # 11
x.next() # 12
y.next()
";
            test_script(script, 11.into());
        }

        #[test]
        fn make_copy_in_second_iter() {
            let script = "
x = (0..2).chain 2..5
x.next() # 0
x.next() # 1
x.next() # 2
y = x.copy()
x.next() # 3
x.next() # 4
y.next()
";
            test_script(script, 3.into());
        }
    }

    mod cycle {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (1..=3).cycle()
x.next() # 1
y = x.copy()
x.next() # 2
x.next() # 3
y.next()
";
            test_script(script, 2.into());
        }
    }

    mod each {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (3, 4, 5, 6).each |x| x * x
x.next() # 9
y = x.copy()
x.next() # 16
x.next() # 25
y.next()
";
            test_script(script, 16.into());
        }
    }

    mod enumerate {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = (10..20).enumerate()
x.next() # 0, 10
y = x.copy()
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
y = x.copy()
x.next() # 2
x.next() # 0
y.next()
";
            test_script(script, 2.into());
        }

        #[test]
        fn intersperse_with_function_make_copy() {
            let script = "
x = (10, 20, 30).intersperse || 42
x.next() # 10
x.next() # 42
y = x.copy()
x.next() # 20
x.next() # 42
y.next()
";
            test_script(script, 20.into());
        }
    }

    mod keep {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abcdef'.chars().keep |c| 'bef'.contains c
x.next() # 'b'
y = x.copy()
x.next() # 'e'
y.next()
";
            test_script(script, "e".into());
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
y = x.copy()
x.next() # 'c'
y.next()
";
            test_script(script, "c".into());
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
y = x.copy()
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
y = x.copy()
x.next() # bar
y.next()
";
            test_script(script, "bar".into());
        }
    }

    mod values {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = {foo: 42, bar: 99, baz: -1}.values()
x.next() # 42
y = x.copy()
x.next() # 99
y.next()
";
            test_script(script, 99.into());
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
y = x.copy()
x.next() # 98
y.next()
";
            test_script(script, 98.into());
        }
    }

    mod lines {
        use super::*;

        #[test]
        fn make_copy() {
            let script = "
x = 'abc\ndef\nxyz'.lines()
x.next() # abc
y = x.copy()
x.next() # def
y.next()
";
            test_script(script, "def".into());
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
y = x.copy()
x.next() # 2
y.next()
";
            test_script(script, "2".into());
        }

        #[test]
        fn make_copy_predicate() {
            let script = "
x = '1-2_3'.split |c| '-_'.contains c
x.next() # 1
y = x.copy()
x.next() # 2
y.next()
";
            test_script(script, "2".into());
        }
    }
}
