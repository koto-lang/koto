mod runtime_test_utils;

use crate::runtime_test_utils::{test_script, value_tuple};

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
}
