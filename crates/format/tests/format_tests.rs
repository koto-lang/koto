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
Mismatch in format output at char {}.
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
{}

---",
                            expected
                                .chars()
                                .zip(output.chars())
                                .take_while(|(a, b)| a == b)
                                .count(),
                            output.replace("\n", "⏎\n"),
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

    mod comments {
        use super::*;

        #[test]
        fn several_comments() {
            check_format_output(
                &["
# one
# two
# three
"],
                "\
# one
# two
# three
",
            );
        }

        #[test]
        fn multiline_comment_before_expression() {
            check_format_output(
                &["
#-
xyz
-#
print    'hello'
"],
                "\
#-
xyz
-#
print 'hello'
",
            );
        }

        #[test]
        fn multiline_comment_at_start_of_function_block() {
            check_format_output(
                &["
f   = ||
  #-
    abc
  -#
  return 42
"],
                "\
f = ||
  #-
    abc
  -#
  return 42
",
            );
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

    mod strings {
        use super::*;

        #[test]
        fn with_line_comment() {
            check_format_output(
                &["
'foo'    # abc
\"bar\"     # xyz
r###'raw!'###
'baz - { 1 +   #- hi -#     1:_^3.3}!'
"],
                "\
'foo' # abc
\"bar\" # xyz
r###'raw!'###
'baz - {1 + #- hi -# 1:_^3.3}!'
",
            );
        }

        #[test]
        fn with_escaped_characters() {
            check_format_output(
                &[r#"
x =   '\
\n
\u{1F44B}
'
"#],
                r#"x = '\
\n
\u{1F44B}
'
"#,
            );
        }
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
1 +     # abc
    2    *    3",
                ],
                "\
1 # abc
  + 2 * 3
",
            );
        }

        #[test]
        fn with_inline_comment() {
            check_format_output(
                &[
                    "\
x   =   1   +  #- abc -#  x- -3*2   ",
                    "\
x =   1+#- abc -#x    - -3 *2",
                ],
                "\
x = 1 + #- abc -# x - -3 * 2
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

        #[test]
        fn multi_assignment() {
            check_format_output_with_options(
                &["\
a,   b,   c =
    11+11, 22   + 22,    33   + 33
"],
                "\
a, b, c =
  11 + 11, 22 + 22,
  33 + 33
",
                FormatOptions {
                    line_length: 20,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn integers_with_alt_bases() {
            check_format_output_with_options(
                &["\
0b101   +     0xabad_cafe *   0o707
"],
                "\
0b101
  + 0xabad_cafe * 0o707
",
                FormatOptions {
                    line_length: 25,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn floats() {
            check_format_output_with_options(
                &["\
1.0e-3    * 2e99
"],
                "\
1.0e-3 * 2e99
",
                FormatOptions {
                    line_length: 25,
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

        #[test]
        fn list_broken_by_comment() {
            check_format_output(
                &["\
[  a , b ,# xyz
       c,
       d
]
"],
                "\
[
  a, b, # xyz
  c, d
]
",
            );
        }

        #[test]
        fn tuple_multi_line() {
            check_format_output_with_options(
                &["\
(11111  ,
    22222,33333,   #- foo -#     44444
)
"],
                "\
(
  11111, 22222,
  33333, #- foo -#
  44444
)
",
                FormatOptions {
                    line_length: 20,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn map_with_braces() {
            check_format_output_with_options(
                &["\
{ foo:42,bar,      baz: 99    }
"],
                "\
{
  foo: 42, bar,
  baz: 99
}
",
                FormatOptions {
                    line_length: 20,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn map_block_assignment() {
            check_format_output_with_options(
                &["\
x =
  # foo
  foo  :
    99     # abc

    # bar
  bar: some_long_function()
  'baz'  : #- xyz -# 1 + 1
x
"],
                "\
x =
  # foo
  foo:
    99 # abc

  # bar
  bar:
    some_long_function()
  'baz':
    #- xyz -# 1 + 1
x
",
                FormatOptions {
                    line_length: 20,
                    ..Default::default()
                },
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
        fn if_inline() {
            check_format_output(
                &["\
if   #- abc -#   x>   10 then x else y*y# bar
"],
                "\
if #- abc -# x > 10 then x else y * y # bar
",
            );
        }

        #[test]
        fn if_block_with_else_ifs() {
            check_format_output(
                &["\
if   #- abc -#   x >   10 # foo
   return x
else if   x   < 5 # ---
    return x     #- bar -#
else if     x ==   0 # xyz
      # x
     return x
else # baz
 x     =    42      # 42
 return x
"],
                "\
if #- abc -# x > 10 # foo
  return x
else if x < 5 # ---
  return x #- bar -#
else if x == 0 # xyz
  # x
  return x
else # baz
  x = 42 # 42
  return x
",
            );
        }

        #[test]
        fn if_block_nested() {
            check_format_output(
                &["\
f = ||
  if    true
      x + y
  else if    x > 100
    x
  else
        x * y
"],
                "\
f = ||
  if true
    x + y
  else if x > 100
    x
  else
    x * y
",
            );
        }

        #[test]
        fn switch() {
            check_format_output(
                &["\
switch
  x   ==   0   then x # abc
  y   ==0    then
    debug y
    f(y)
  else # xyz
    42
"],
                "\
switch
  x == 0 then x # abc
  y == 0 then
    debug y
    f(y)
  else # xyz
    42
",
            );
        }

        #[test]
        fn switch_always_break() {
            check_format_output_with_options(
                &["\
switch
  x   ==   0   then x # abc
  y   ==0    then
    debug y
    f(y)
  else  42 # xyz
"],
                "\
switch
  x == 0 then # abc
    x
  y == 0 then
    debug y
    f(y)
  else
    42 # xyz
",
                FormatOptions {
                    always_indent_arms: true,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn match_expression() {
            check_format_output_with_options(
                &["
match   foo()    # abc
  'hello'   then
                  'xyz'
  1   or   2   or 3   or   4 then   -1
  ('a', 'b'  )or(   'c', 'd')if bar()then baz()      # xyz
  else
    0
"],
                "\
match foo() # abc
  'hello' then
    'xyz'
  1 or 2 or 3 or 4 then -1
  ('a', 'b') or ('c', 'd') if bar() then
    baz() # xyz
  else
    0
",
                FormatOptions {
                    line_length: 50,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn match_expression_always_break() {
            check_format_output_with_options(
                &["
match   foo()    # abc
  'hello'   then
                  'xyz'
  1   or   2   or 3   or   4 then   -1
  ('a', 'b'  )or(   'c', 'd')if bar()then baz()      # xyz
  else
    0
"],
                "\
match foo() # abc
  'hello' then
    'xyz'
  1 or 2 or 3 or 4 then
    -1
  ('a', 'b') or ('c', 'd') if bar() then # xyz
    baz()
  else
    0
",
                FormatOptions {
                    always_indent_arms: true,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn try_catch_finally() {
            check_format_output_with_options(
                &["
try     # abc
  foo()
  bar()
catch i  :   Int
    debug i
    throw   '{i}'
catch    other
  throw other
finally
    print 'bye' # xyz"],
                "\
try # abc
  foo()
  bar()
catch i: Int
  debug i
  throw '{i}'
catch other
  throw other
finally
  print 'bye' # xyz
",
                FormatOptions {
                    always_indent_arms: true,
                    ..Default::default()
                },
            );
        }
    }

    mod chains {
        use super::*;

        #[test]
        fn call_without_parens() {
            check_format_output(
                &["\
f   1,   2,  3
"],
                "\
f 1, 2, 3
",
            );
        }

        #[test]
        fn single_line_with_parens() {
            check_format_output(
                &["\
foo.bar[  #- foo -# 1..  ]?.'baz'( x[..] ,  2 ,  3  )
"],
                "\
foo.bar[#- foo -# 1..]?.'baz'(x[..], 2, 3)
",
            );
        }

        #[test]
        fn multi_line_that_gets_collapsed() {
            check_format_output(
                &["\
foo
  .bar(
  )?[0]
"],
                "\
foo.bar()?[0]
",
            );
        }

        #[test]
        fn single_line_that_gets_broken() {
            check_format_output(
                &["\
foo.bar()?.'baz'().xyz[0]?
"],
                "\
foo
  .bar()?
  .'baz'()
  .xyz[0]?
",
            );
        }

        #[test]
        fn broken_by_line_length() {
            check_format_output_with_options(
                &["\
foo.bar[  #- foo -# ..9  ]?.baz( 1 ,  2 ,  3..=4  )
"],
                "\
foo
  .bar[#- foo -# ..9]?
  .baz(1, 2, 3..=4)
",
                FormatOptions {
                    line_length: 20,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn paren_free_call_before_end() {
            // The paren-free call prevents collaps
            check_format_output(
                &["\
foo
      .bar     |x| x+10
      .baz()
"],
                "\
foo
  .bar |x| x + 10
  .baz()
",
            );
        }

        #[test]
        fn paren_free_call_at_end() {
            // Paren-free calls at the end of the chain can be collapsed
            check_format_output(
                &["\
foo
      .bar     |x| x+10
"],
                "\
foo.bar |x| x + 10
",
            );
        }

        #[test]
        fn dont_collapse_pipe_operator() {
            check_format_output(
                &["
some
    .chained()
    .expression()
  -> piped_1
      -> piped_2
"],
                "\
some.chained().expression()
  -> piped_1
  -> piped_2
",
            );
        }

        #[test]
        fn dont_break_trailing_paren_free_call() {
            check_format_output(
                &["
foo.bar   ||
  baz
"],
                "\
foo.bar ||
  baz
",
            );
        }

        #[test]
        fn dont_break_call_with_long_multiline_string() {
            check_format_output_with_options(
                &["
x   =   foo   '
abcdefghijklmnopqrstuvwxyz
'
"],
                "\
x = foo '
abcdefghijklmnopqrstuvwxyz
'
",
                FormatOptions {
                    line_length: 25,
                    ..Default::default()
                },
            );
        }
    }

    mod import_and_export {
        use super::*;

        #[test]
        fn export() {
            check_format_output(
                &["\
export   #- abc -#   foo     # xyz
"],
                "\
export #- abc -# foo # xyz
",
            );
        }

        #[test]
        fn import_single_line() {
            check_format_output(
                &["\
from    foo   import #- abc -#   bar     # xyz
"],
                "\
from foo import #- abc -# bar # xyz
",
            );
        }

        #[test]
        fn import_without_from() {
            check_format_output(
                &["\
import #- abc -#   bar     # xyz
"],
                "\
import #- abc -# bar # xyz
",
            );
        }

        #[test]
        fn import_multiline() {
            check_format_output_with_options(
                &["\
from foo.bar.baz import     #- abc -#   bar as   aaa  , baz   as   bbb       # xyz
"],
                "\
from
  foo.bar.baz
import
  #- abc -# bar as aaa, baz as bbb # xyz
",
                FormatOptions {
                    line_length: 40,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn metakey_assignment() {
            check_format_output(
                &["\
@main =
    ||
        print
            'hello'
"],
                "\
@main = ||
  print 'hello'
",
            );
        }
    }

    mod functions {
        use super::*;

        #[test]
        fn inline() {
            check_format_output(
                &["\
f   =   |  a : Number ,  b: Number, c...  |   g(a  +  b, c...)
"],
                "\
f = |a: Number, b: Number, c...| g(a + b, c...)
",
            );
        }

        #[test]
        fn broken_args() {
            check_format_output(
                &["\
f   =   |  a,
b, # xyz
 c  | x a, b, c
"],
                "\
f = |
  a, b, # xyz
  c
| x a, b, c
",
            );
        }

        #[test]
        fn block_with_long_lines() {
            check_format_output_with_options(
                &["\
f   =   |  (aaaa,  bbbb, ( ..., c, d  ))  |   ->   Number   # abc
    x =   aaaa +  bbbb  +c+   d
    yield   x   *   2
"],
                "\
f = |
  (
    aaaa, bbbb,
    (..., c, d)
  )
| -> Number # abc
  x = aaaa
    + bbbb
    + c
    + d
  yield x * 2
",
                FormatOptions {
                    line_length: 20,
                    ..Default::default()
                },
            );
        }

        #[test]
        fn return_tuple() {
            check_format_output(
                &["\
f   =   |a,b,c|a,b,c
"],
                "\
f = |a, b, c| a, b, c
",
            );
        }

        #[test]
        fn return_map_block() {
            check_format_output(
                &["\
f   =  ||
  # foo
  foo:   42
  bar:     99
"],
                "\
f = ||
  # foo
  foo: 42
  bar: 99
",
            );
        }
    }
}
