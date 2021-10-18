mod runtime_test_utils;

use crate::runtime_test_utils::test_script;

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
}
