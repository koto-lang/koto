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
                Err(error) => panic!(
                    "error while formatting (line: {}, column: {}): {error}\ninput:\n{input}",
                    error.span.start.line, error.span.start.column
                ),
            }
        }
    }

    mod keywords {
        use super::*;

        #[test]
        fn null_true_and_false() {
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

    mod arithmetic {
        use super::*;

        #[test]
        fn with_line_comment() {
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
        fn with_inline_comment() {
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
        fn expression_longer_than_line_length() {
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
    }

    mod containers {
        use super::*;

        #[test]
        fn tuple_single_line() {
            check_format_output(
                &[
                    "\
(1  ,
    #- foo -#    2,3,    4
)
",
                    "\
(1,#- foo -#2,3,4)
",
                ],
                "\
(1, #- foo -# 2, 3, 4)
",
            );
        }

        #[test]
        fn list_single_line() {
            check_format_output(
                &[
                    "\
[  #- bar -#   a
    ,
        b
            , c
]
",
                    "\
[#- bar -#a,b,c]
",
                ],
                "\
[#- bar -# a, b, c]
",
            );
        }
    }

    mod loops {
        use super::*;

        #[test]
        fn loop_() {
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

        #[test]
        fn while_() {
            check_format_output(
                &[
                    "\
while   x < 10     # abc
    # xyz
    x += 1

",
                    "\
while   x  <     10# abc
 # xyz
 x += 1

",
                ],
                "\
while x < 10 # abc
  # xyz
  x += 1
",
            );
        }

        #[test]
        fn for_single_arg() {
            check_format_output(
                &[
                    "\
for #- abc -#      x     in y      # xyz
  x     +=   99
",
                    "\
for     #- abc -#x in   y# xyz
  x     +=   99
",
                ],
                "\
for #- abc -# x in y # xyz
  x += 99
",
            );
        }
    }

    mod conditionals {
        use super::*;

        #[test]
        fn if_block() {
            check_format_output(
                &["\
if   #- abc -#   x >   10 # foo
   x = 1
   return x
else if   x   < 5
    x = 0
    return x     # bar
else if     x ==   0 # xyz
     x = -1
     return x
else # baz
 x     =    42      # 42
 return x
"],
                "\
if #- abc -# x > 10 # foo
  x = 1
  return x
else if x < 5
  x = 0
  return x # bar
else if x == 0 # xyz
  x = -1
  return x
else # baz
  x = 42 # 42
  return x
",
            );
        }
    }
}
