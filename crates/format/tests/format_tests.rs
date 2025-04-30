mod format {
    use koto_format::{FormatOptions, format};
    use std::iter::once;

    fn check_format_output(inputs: &[&str], expected: &str) {
        check_format_output_with_options(inputs, expected, FormatOptions::default());
    }

    fn check_format_output_with_options(inputs: &[&str], expected: &str, options: FormatOptions) {
        for input in inputs.iter().chain(once(&expected)) {
            match format(input, options) {
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
    fn return_with_inline_comment() {
        check_format_output(
            &[
                "\
return #- abc -#foo",
                "\
return  #- abc -#     foo",
                "\
  return  #- abc -#   foo",
                "
return  #- abc -#
  foo",
            ],
            "\
return #- abc -# foo
",
        );
    }

    #[test]
    fn return_with_line_comment() {
        check_format_output(
            &[
                "\
return      # abc
       foo",
                "\
return  # abc
  foo",
            ],
            "\
return # abc
  foo
",
        );
    }

    #[test]
    fn return_with_long_value() {
        check_format_output_with_options(
            &[
                "\
return #- abc -# xxxxxxxxxxxxxxxxxxxx
",
                "\
return  #- abc -#    xxxxxxxxxxxxxxxxxxxx
",
            ],
            "\
return
  #- abc -#
  xxxxxxxxxxxxxxxxxxxx
",
            FormatOptions {
                line_length: 20,
                ..Default::default()
            },
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
    fn arithmetic_with_line_comment() {
        check_format_output(
            &[
                "\
1   +  # abc
 2 * 3",
                "\
1 + # abc
    2
      * 3",
            ],
            "\
1 + # abc
  2 * 3
",
        );
    }

    #[test]
    fn arithmetic_with_inline_comment() {
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
    fn arithmetic_expression_longer_than_line_length() {
        check_format_output_with_options(
            &["\
1 + 2 * 3 - 4 / 5 % 6 + #- xyz -# 7 ^ 8 - (9 + a)
"],
            "\
1
  + 2 * 3
  - 4 / 5 % 6
  + #- xyz -# 7 ^ 8
  - (9 + a)
",
            FormatOptions {
                line_length: 20,
                ..Default::default()
            },
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
