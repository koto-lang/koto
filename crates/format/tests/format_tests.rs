mod format {
    use koto_format::{FormatOptions, format};
    use std::iter::once;

    fn check_format_output(inputs: &[&str], expected: &str) {
        for input in inputs.iter().chain(once(&expected)) {
            match format(input, FormatOptions::default()) {
                Ok(output) => {
                    if expected != output {
                        panic!(
                            "\
Mismatch in format output.
Input:
---
{input}
---

Expected:
---
{expected}
---

Output:
---
{output}
---"
                        )
                    }
                }
                Err(error) => panic!("error while formatting: {error}\ninput:\n{input}"),
            }
        }
    }

    #[test]
    fn comments_and_keywords() {
        check_format_output(
            &[
                "
null# null
true

false",
                "
null    # null
true

false",
                "\
null      # null
true




false




",
            ],
            "\
null # null
true

false
",
        );
    }

    #[test]
    fn return_foo() {
        check_format_output(
            &[
                "\
return foo",
                "\
return   foo",
                "\
  return     foo",
                "
return
  foo",
            ],
            "\
return foo
",
        );
    }

    #[test]
    fn nested() {
        check_format_output(
            &[
                "(null)",
                "(null )",
                "( null)",
                "
(
  null
)",
            ],
            "\
(null)
",
        );
    }

    #[test]
    fn nested_with_comment() {
        check_format_output(
            &[
                "( #- xyz -# null)",
                "(
                #- xyz -#
                null
                )",
            ],
            "\
(#- xyz -# null)
",
        );
    }

    #[test]
    fn binary_op_single_line() {
        check_format_output(
            &[
                "\
1   +  #- abc -#  x- -3*2   ",
                "\
1+#- abc -#x    - -3 *2",
            ],
            "\
1 + #- abc -# x - -3 * 2
",
        );
    }

    #[test]
    fn tuple_and_list() {
        check_format_output(
            &[
                "\
(1  ,   #- foo -#    2,3,    4)
[  #- bar -#   a
    ,
        b
            , c
]
",
                "\
(1,#- foo -#2,3,4)
[#- bar -#a,b,c]
",
            ],
            "\
(1, #- foo -# 2, 3, 4)
[#- bar -# a, b, c]
",
        );
    }

    #[test]
    fn loop_block() {
        check_format_output(
            &[
                "\
loop     # abc
    x =   1
    break  not    #- foo -#    true
    continue

",
                "\
loop# abc
 x =   1
 break not#- foo -#true
 continue
",
            ],
            "\
loop # abc
  x = 1
  break not #- foo -# true
  continue
",
        );
    }
}
