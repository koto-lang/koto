mod parser {
    use koto_parser::{Node::*, *};

    fn check_ast(source: &str, expected_ast: &[Node], expected_constants: Option<&[Constant]>) {
        println!("{}", source);

        match Parser::parse(source) {
            Ok((ast, constants)) => {
                for (i, (ast_node, expected_node)) in
                    ast.nodes().iter().zip(expected_ast.iter()).enumerate()
                {
                    assert_eq!(ast_node.node, *expected_node, "Mismatch at position {}", i);
                }
                assert_eq!(
                    ast.nodes().len(),
                    expected_ast.len(),
                    "Node list length mismatch"
                );

                if let Some(expected_constants) = expected_constants {
                    for (constant, expected_constant) in
                        constants.iter().zip(expected_constants.iter())
                    {
                        assert_eq!(constant, *expected_constant);
                    }
                    assert_eq!(
                        constants.len(),
                        expected_constants.len(),
                        "Constant list length mismatch"
                    );
                }
            }
            Err(error) => panic!("{} - {}", error, error.span.start),
        }
    }

    mod values {
        use super::*;

        #[test]
        fn literals() {
            let source = "
true
false
1
1.5
\"hello\"
a
()";
            check_ast(
                source,
                &[
                    BoolTrue,
                    BoolFalse,
                    Number1,
                    Number(0),
                    Str(1),
                    Id(2),
                    Empty,
                    MainBlock {
                        body: vec![0, 1, 2, 3, 4, 5, 6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Number(1.5),
                    Constant::Str("hello"),
                    Constant::Str("a"),
                ]),
            )
        }

        #[test]
        fn multiline_strings() {
            let source = r#"
"    foo
     bar
"
"foo \
     bar\
"
"#;
            check_ast(
                source,
                &[
                    Str(0),
                    Str(1),
                    MainBlock {
                        body: vec![0, 1],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("    foo\n     bar\n"),
                    Constant::Str("foo bar"),
                ]),
            )
        }

        #[test]
        fn negatives() {
            let source = "\
-12.0
-a
-x[0]
-(1 + 1)";
            check_ast(
                source,
                &[
                    Number(0),
                    Id(1),
                    Negate(1),
                    Id(2),
                    Number0,
                    Lookup((LookupNode::Index(4), None)), // 5
                    Lookup((LookupNode::Root(3), Some(5))),
                    Negate(6),
                    Number1,
                    Number1,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 8,
                        rhs: 9,
                    }, // 10
                    Negate(10),
                    MainBlock {
                        body: vec![0, 2, 7, 11],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Number(-12.0),
                    Constant::Str("a"),
                    Constant::Str("x"),
                ]),
            )
        }

        #[test]
        fn list() {
            let source = r#"
[0, n, "test", n, -1]
[]
"#;
            check_ast(
                source,
                &[
                    Number0,
                    Id(0),
                    Str(1),
                    Id(0),
                    Number(2),
                    List(vec![0, 1, 2, 3, 4]),
                    List(vec![]),
                    MainBlock {
                        body: vec![5, 6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("n"),
                    Constant::Str("test"),
                    Constant::Number(-1.0),
                ]),
            )
        }

        #[test]
        fn list_with_line_breaks() {
            let source = "\
x = [
  0,
  1, 0, 1,
  0
]";
            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Number1,
                    Number0,
                    Number1,
                    Number0, // 5
                    List(vec![1, 2, 3, 4, 5]),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 6,
                    },
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn map_inline() {
            let source = r#"
{}
{"foo": 42, bar, baz: "hello"}"#;
            check_ast(
                source,
                &[
                    Map(vec![]),
                    Number(1),
                    Str(4),
                    // map entries are constant/ast index pairs
                    Map(vec![(0, Some(1)), (2, None), (3, Some(2))]),
                    MainBlock {
                        body: vec![0, 3],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                    Constant::Str("hello"),
                ]),
            )
        }

        #[test]
        fn map_inline_multiline() {
            let source = r#"
{
  "foo": 42,
  bar,
  baz:
    "hello",
}"#;
            check_ast(
                source,
                &[
                    Number(1),
                    Str(4),
                    // map entries are constant/ast index pairs
                    Map(vec![(0, Some(0)), (2, None), (3, Some(1))]),
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                    Constant::Str("hello"),
                ]),
            )
        }

        #[test]
        fn map_block() {
            let source = r#"
x =
  foo: 42
  bar
  "baz":
    foo: 0
x"#;
            check_ast(
                source,
                &[
                    Id(0),     // x
                    Number(2), // 42
                    Number0,
                    // map entries are constant/ast pairs
                    Map(vec![(1, Some(2))]), // baz, nested map
                    Map(vec![(1, Some(1)), (3, None), (4, Some(3))]),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 4,
                    }, // 5
                    Id(0),
                    MainBlock {
                        body: vec![5, 6],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn ranges_from_literals() {
            let source = "
0..1
0..=1";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    Number0,
                    Number1,
                    Range {
                        start: 3,
                        end: 4,
                        inclusive: true,
                    }, // 5
                    MainBlock {
                        body: vec![2, 5],
                        local_count: 0,
                    },
                ],
                Some(&[]),
            )
        }

        #[test]
        fn range_from_expressions() {
            let source = "(0 + 1)..(1 + 1)";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number1,
                    Number1,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 3,
                        rhs: 4,
                    }, // 5
                    Range {
                        start: 2,
                        end: 5,
                        inclusive: false,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[]),
            )
        }

        #[test]
        fn range_from_values() {
            let source = "
min = 0
max = 10
min..max
";
            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 1,
                    },
                    Id(1),
                    Number(2),
                    Assign {
                        target: AssignTarget {
                            target_index: 3,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 4,
                    }, // 5
                    Id(0),
                    Id(1),
                    Range {
                        start: 6,
                        end: 7,
                        inclusive: false,
                    },
                    MainBlock {
                        body: vec![2, 5, 8],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("min"),
                    Constant::Str("max"),
                    Constant::Number(10.0),
                ]),
            )
        }

        #[test]
        fn range_from_lookups() {
            let source = "foo.bar..foo.baz";
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Id(1), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(0),
                    Lookup((LookupNode::Id(2), None)),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    Range {
                        start: 2,
                        end: 5,
                        inclusive: false,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn lists_from_ranges() {
            let source = "\
[0..1]
[0..10, 10..=0]";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    List(vec![2]),
                    Number0,
                    Number(0), // 5
                    Range {
                        start: 4,
                        end: 5,
                        inclusive: false,
                    },
                    Number(0),
                    Number0,
                    Range {
                        start: 7,
                        end: 8,
                        inclusive: true,
                    },
                    List(vec![6, 9]),
                    MainBlock {
                        body: vec![3, 10],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Number(10.0)]),
            )
        }

        #[test]
        fn num2() {
            let source = "\
num2 0
num2
  1
  x";
            check_ast(
                source,
                &[
                    Number0,
                    Num2(vec![0]),
                    Number1,
                    Id(0),
                    Num2(vec![2, 3]),
                    MainBlock {
                        body: vec![1, 4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn num4() {
            let source = "\
num4 0
num4 1 x
num4
  x 0
  1 x";
            check_ast(
                source,
                &[
                    Number0,
                    Num4(vec![0]),
                    Number1,
                    Id(0),
                    Num4(vec![2, 3]),
                    Id(0), // 5
                    Number0,
                    Number1,
                    Id(0),
                    Num4(vec![5, 6, 7, 8]),
                    MainBlock {
                        body: vec![1, 4, 9],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn tuple() {
            let source = "0, 1, 0";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Number0,
                    Tuple(vec![0, 1, 2]),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn tuple_in_parens() {
            let source = "(0, 1, 0)";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Number0,
                    Tuple(vec![0, 1, 2]),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                None,
            )
        }
    }

    mod assignment {
        use super::*;

        #[test]
        fn single() {
            let source = "a = 1";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 1,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn single_export() {
            let source = "export a = 1 + 1";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Number1,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 1,
                        rhs: 2,
                    },
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Global,
                        },
                        op: AssignOp::Equal,
                        expression: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn tuple() {
            let source = "x = 1, 0";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Number0,
                    Tuple(vec![1, 2]),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn tuple_of_tuples() {
            let source = "x = (0, 1), (2, 3)";
            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Number1,
                    Tuple(vec![1, 2]),
                    Number(1),
                    Number(2), // 5
                    Tuple(vec![4, 5]),
                    Tuple(vec![3, 6]),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Number(2.0),
                    Constant::Number(3.0),
                ]),
            )
        }

        #[test]
        fn unpack_tuple() {
            let source = "x, y[0] = 1, 0";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Number0,
                    Lookup((LookupNode::Index(2), None)),
                    Lookup((LookupNode::Root(1), Some(3))),
                    Number1, // 5
                    Number0,
                    Tuple(vec![5, 6]),
                    MultiAssign {
                        targets: vec![
                            AssignTarget {
                                target_index: 0,
                                scope: Scope::Local,
                            },
                            AssignTarget {
                                target_index: 4,
                                scope: Scope::Local,
                            },
                        ],
                        expressions: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 1, // y is assumed to be non-local
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn tuple_with_linebreaks() {
            let source = "\
x, y =
  1,
  0,
x";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Number1,
                    Number0,
                    Tuple(vec![2, 3]),
                    MultiAssign {
                        targets: vec![
                            AssignTarget {
                                target_index: 0,
                                scope: Scope::Local,
                            },
                            AssignTarget {
                                target_index: 1,
                                scope: Scope::Local,
                            },
                        ],
                        expressions: 4,
                    }, // 5
                    Id(0),
                    MainBlock {
                        body: vec![5, 6],
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn multi_1_to_3_with_wildcard() {
            let source = "x, _, y = f()";
            check_ast(
                source,
                &[
                    Id(0),
                    Wildcard,
                    Id(1),
                    Id(2),
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    MultiAssign {
                        targets: vec![
                            AssignTarget {
                                target_index: 0,
                                scope: Scope::Local,
                            },
                            AssignTarget {
                                target_index: 1,
                                scope: Scope::Local,
                            },
                            AssignTarget {
                                target_index: 2,
                                scope: Scope::Local,
                            },
                        ],
                        expressions: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("f")]),
            )
        }

        #[test]
        fn modify_assign() {
            let source = "\
x += 0
x -= 1
x *= 2
x /= 3
x %= 4";
            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Add,
                        expression: 1,
                    },
                    Id(0),
                    Number1,
                    Assign {
                        target: AssignTarget {
                            target_index: 3,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Subtract,
                        expression: 4,
                    }, // 5
                    Id(0),
                    Number(1),
                    Assign {
                        target: AssignTarget {
                            target_index: 6,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Multiply,
                        expression: 7,
                    },
                    Id(0),
                    Number(2), // 10
                    Assign {
                        target: AssignTarget {
                            target_index: 9,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Divide,
                        expression: 10,
                    },
                    Id(0),
                    Number(3),
                    Assign {
                        target: AssignTarget {
                            target_index: 12,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Modulo,
                        expression: 13,
                    },
                    MainBlock {
                        body: vec![2, 5, 8, 11, 14],
                        local_count: 0,
                    }, // 15
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Number(2.0),
                    Constant::Number(3.0),
                    Constant::Number(4.0),
                ]),
            )
        }
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn addition_subtraction() {
            let source = "1 - 0 + 1";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    BinaryOp {
                        op: AstOp::Subtract,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number1,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 2,
                        rhs: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn add_multiply() {
            let source = "1 + 0 * 1 + 0";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    Number1,
                    BinaryOp {
                        op: AstOp::Multiply,
                        lhs: 1,
                        rhs: 2,
                    },
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 3,
                    },
                    Number0, // 5
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 4,
                        rhs: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn with_parentheses() {
            let source = "(1 + 0) * (1 + 0)";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number1,
                    Number0,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 3,
                        rhs: 4,
                    },
                    BinaryOp {
                        op: AstOp::Multiply,
                        lhs: 2,
                        rhs: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn divide_modulo() {
            let source = "18 / 3 % 4";
            check_ast(
                source,
                &[
                    Number(0),
                    Number(1),
                    BinaryOp {
                        op: AstOp::Divide,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number(2),
                    BinaryOp {
                        op: AstOp::Modulo,
                        lhs: 2,
                        rhs: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Number(18.0),
                    Constant::Number(3.0),
                    Constant::Number(4.0),
                ]),
            )
        }

        #[test]
        fn string_and_id() {
            let source = "\"hello\" + x";
            check_ast(
                source,
                &[
                    Str(0),
                    Id(1),
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("hello"), Constant::Str("x")]),
            )
        }

        #[test]
        fn function_call_on_rhs() {
            let source = "x = 1 + f y";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Id(1),
                    Id(2),
                    Call {
                        function: 2,
                        args: vec![3],
                    },
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 1,
                        rhs: 4,
                    }, // 5
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("f"), Constant::Str("y")]),
            )
        }

        #[test]
        fn multiline_trailing_operators() {
            let source = "
a = 1 +
    2 *
    3
";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Number(1),
                    Number(2),
                    BinaryOp {
                        op: AstOp::Multiply,
                        lhs: 2,
                        rhs: 3,
                    },
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 1,
                        rhs: 4,
                    }, // 5
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Number(2.0),
                    Constant::Number(3.0),
                ]),
            )
        }

        #[test]
        fn multiline_preceding_operators() {
            let source = "
a = 1
  + 2
  * 3
";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Number(1),
                    Number(2),
                    BinaryOp {
                        op: AstOp::Multiply,
                        lhs: 2,
                        rhs: 3,
                    },
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 1,
                        rhs: 4,
                    }, // 5
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Number(2.0),
                    Constant::Number(3.0),
                ]),
            )
        }
    }

    mod logic {
        use super::*;

        #[test]
        fn and_or() {
            let source = "0 < 1 and 1 > 0 or true";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    BinaryOp {
                        op: AstOp::Less,
                        lhs: 0,
                        rhs: 1,
                    },
                    Number1,
                    Number0,
                    BinaryOp {
                        op: AstOp::Greater,
                        lhs: 3,
                        rhs: 4,
                    },
                    BinaryOp {
                        op: AstOp::And,
                        lhs: 2,
                        rhs: 5,
                    },
                    BoolTrue,
                    BinaryOp {
                        op: AstOp::Or,
                        lhs: 6,
                        rhs: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn chained_comparisons() {
            let source = "0 < 1 <= 1";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    Number1,
                    BinaryOp {
                        op: AstOp::LessOrEqual,
                        lhs: 1,
                        rhs: 2,
                    },
                    BinaryOp {
                        op: AstOp::Less,
                        lhs: 0,
                        rhs: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                None,
            )
        }
    }

    mod control_flow {
        use super::*;

        #[test]
        fn if_inline() {
            let source = "1 + if true then 0 else 1";
            check_ast(
                source,
                &[
                    Number1,
                    BoolTrue,
                    Number0,
                    Number1,
                    If(AstIf {
                        condition: 1,
                        then_node: 2,
                        else_if_blocks: vec![],
                        else_node: Some(3),
                    }),
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 4,
                    },
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn if_block() {
            let source = "\
a = if false
  0
else if true
  1
else if false
  0
else
  1
a";
            check_ast(
                source,
                &[
                    Id(0),
                    BoolFalse,
                    Number0,
                    BoolTrue,
                    Number1,
                    BoolFalse, // 5
                    Number0,
                    Number1,
                    If(AstIf {
                        condition: 1,
                        then_node: 2,
                        else_if_blocks: vec![(3, 4), (5, 6)],
                        else_node: Some(7),
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 8,
                    },
                    Id(0),
                    MainBlock {
                        body: vec![9, 10],
                        local_count: 1,
                    }, // 10
                ],
                None,
            )
        }

        #[test]
        fn if_inline_multi_expressions() {
            let source = "a, b = if true then 0, 1 else 1, 0";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    BoolTrue,
                    Number0,
                    Number1,
                    Tuple(vec![3, 4]), // 5
                    Number1,
                    Number0,
                    Tuple(vec![6, 7]),
                    If(AstIf {
                        condition: 2,
                        then_node: 5,
                        else_if_blocks: vec![],
                        else_node: Some(8),
                    }),
                    MultiAssign {
                        targets: vec![
                            AssignTarget {
                                target_index: 0,
                                scope: Scope::Local,
                            },
                            AssignTarget {
                                target_index: 1,
                                scope: Scope::Local,
                            },
                        ],
                        expressions: 9,
                    }, // 10
                    MainBlock {
                        body: vec![10],
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("b")]),
            )
        }
    }

    mod loops {
        use super::*;

        #[test]
        fn for_inline() {
            let source = "x for x in 0..1";
            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Number1,
                    Range {
                        start: 1,
                        end: 2,
                        inclusive: false,
                    },
                    For(AstFor {
                        args: vec![Some(0)],
                        ranges: vec![3],
                        condition: None,
                        body: 0,
                    }),
                    MainBlock {
                        body: vec![4],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn for_inline_call_in_body() {
            let source = "f x for x in 0..1";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Call {
                        function: 0,
                        args: vec![1],
                    },
                    Number0,
                    Number1,
                    Range {
                        start: 3,
                        end: 4,
                        inclusive: false,
                    }, // 5
                    For(AstFor {
                        args: vec![Some(1)],
                        ranges: vec![5],
                        condition: None,
                        body: 2,
                    }),
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn for_inline_multi() {
            let source = "x, y for x, y in a, b";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(2),
                    Id(3),
                    Tuple(vec![0, 1]),
                    For(AstFor {
                        args: vec![Some(0), Some(1)],
                        ranges: vec![2, 3],
                        condition: None,
                        body: 4,
                    }), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("a"),
                    Constant::Str("b"),
                ]),
            )
        }

        #[test]
        fn for_inline_conditional() {
            let source = "x for x in f y if x == 0";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(2),
                    Call {
                        function: 1,
                        args: vec![2],
                    },
                    Id(0),
                    Number0, // 5
                    BinaryOp {
                        op: AstOp::Equal,
                        lhs: 4,
                        rhs: 5,
                    },
                    For(AstFor {
                        args: vec![Some(0)],
                        ranges: vec![3],
                        condition: Some(6),
                        body: 0,
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("f"), Constant::Str("y")]),
            )
        }

        #[test]
        fn for_block() {
            let source = "\
for x in y if x > 0
  f x";
            check_ast(
                source,
                &[
                    Id(1),
                    Id(0),
                    Number0,
                    BinaryOp {
                        op: AstOp::Greater,
                        lhs: 1,
                        rhs: 2,
                    },
                    Id(2),
                    Id(0), // 5
                    Call {
                        function: 4,
                        args: vec![5],
                    },
                    For(AstFor {
                        args: vec![Some(0)], // constant 0
                        ranges: vec![0],     // ast 0
                        condition: Some(3),
                        body: 6,
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("f")]),
            )
        }

        #[test]
        fn while_inline() {
            let source = "x while true";
            check_ast(
                source,
                &[
                    Id(0),
                    BoolTrue,
                    While {
                        condition: 1,
                        body: 0,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn until_inline() {
            let source = "y until false";
            check_ast(
                source,
                &[
                    Id(0),
                    BoolFalse,
                    Until {
                        condition: 1,
                        body: 0,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("y")]),
            )
        }

        #[test]
        fn while_block() {
            let source = "\
while x > y
  f x";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    BinaryOp {
                        op: AstOp::Greater,
                        lhs: 0,
                        rhs: 1,
                    },
                    Id(2),
                    Id(0),
                    Call {
                        function: 3,
                        args: vec![4],
                    }, // 5
                    While {
                        condition: 2,
                        body: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("f")]),
            )
        }

        #[test]
        fn until_block() {
            let source = "\
until x < y
  f y";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    BinaryOp {
                        op: AstOp::Less,
                        lhs: 0,
                        rhs: 1,
                    },
                    Id(2),
                    Id(1),
                    Call {
                        function: 3,
                        args: vec![4],
                    }, // 5
                    Until {
                        condition: 2,
                        body: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("f")]),
            )
        }

        #[test]
        fn for_block_after_array() {
            // A case that failed parsing at the start of the for block,
            // expecting an expression in the main block.
            let source = "\
[]
for x in y
  x";
            check_ast(
                source,
                &[
                    List(vec![]),
                    Id(1),
                    Id(0),
                    For(AstFor {
                        args: vec![Some(0)], // constant 0
                        ranges: vec![1],     // ast 1
                        condition: None,
                        body: 2,
                    }),
                    MainBlock {
                        body: vec![0, 3],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }
    }

    mod functions {
        use super::*;

        #[test]
        fn inline_no_args() {
            let source = "
a = || 42
a()";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(1),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 1,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 2,
                    },
                    Id(0),
                    Lookup((LookupNode::Call(vec![]), None)), // 5
                    Lookup((LookupNode::Root(4), Some(5))),
                    MainBlock {
                        body: vec![3, 6],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Number(42.0)]),
            )
        }

        #[test]
        fn inline_two_args() {
            let source = "|x y| x + y";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Function(koto_parser::Function {
                        args: vec![Some(0), Some(1)],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 2,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn with_body() {
            let source = "\
f = |x|
  y = x
  y = y + 1
  y
f 42";
            check_ast(
                source,
                &[
                    Id(0), // f
                    Id(2), // y
                    Id(1), // x
                    Assign {
                        target: AssignTarget {
                            target_index: 1,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 2,
                    },
                    Id(2), // y
                    Id(2), // y // 5
                    Number1,
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 5,
                        rhs: 6,
                    },
                    Assign {
                        target: AssignTarget {
                            target_index: 4,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 7,
                    },
                    Id(2),                // y
                    Block(vec![3, 8, 9]), // 10
                    Function(koto_parser::Function {
                        args: vec![Some(1)],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 10,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 11,
                    },
                    Id(0),
                    Number(3),
                    Call {
                        function: 13,
                        args: vec![14],
                    }, // 15
                    MainBlock {
                        body: vec![12, 15],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Number(42.0),
                ]),
            )
        }

        #[test]
        fn with_body_nested() {
            let source = "\
f = |x|
  y = |z|
    z
  y x
f 42";
            check_ast(
                source,
                &[
                    Id(0), // f
                    Id(2), // y
                    Id(3), // z
                    Function(koto_parser::Function {
                        args: vec![Some(3)],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 2,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 1,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 3,
                    },
                    Id(2), // y // 5
                    Id(1), // x
                    Call {
                        function: 5,
                        args: vec![6],
                    },
                    Block(vec![4, 7]),
                    Function(koto_parser::Function {
                        args: vec![Some(1)],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 8,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 9,
                    }, // 10
                    Id(0), // f
                    Number(4),
                    Call {
                        function: 11,
                        args: vec![12],
                    },
                    MainBlock {
                        body: vec![10, 13],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Number(42.0),
                ]),
            )
        }

        #[test]
        fn call_negative_arg() {
            let source = "f x -x";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(1),
                    Negate(2),
                    Call {
                        function: 0,
                        args: vec![1, 3],
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_with_parentheses() {
            let source = "f(x, -x)";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(1),
                    Negate(2),
                    Lookup((LookupNode::Call(vec![1, 3]), None)),
                    Lookup((LookupNode::Root(0), Some(4))),
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_over_lines() {
            let source = "
foo
  x
  y";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Id(2),
                    Call {
                        function: 0,
                        args: vec![1, 2],
                    },
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn calls_with_comment_between() {
            let source = "
f x
  # Indented comment shouldn't break parsing
f x";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Call {
                        function: 0,
                        args: vec![1],
                    },
                    Id(0),
                    Id(1),
                    Call {
                        function: 3,
                        args: vec![4],
                    }, // 5
                    MainBlock {
                        body: vec![2, 5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn instance_function() {
            let source = "{foo: 42, bar: |self x| self.foo = x}";
            check_ast(
                source,
                &[
                    Number(1),
                    Id(3), // self
                    Lookup((LookupNode::Id(0), None)),
                    Lookup((LookupNode::Root(1), Some(2))),
                    Id(4), // x
                    Assign {
                        target: AssignTarget {
                            target_index: 3,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 4,
                    }, // 5
                    Function(koto_parser::Function {
                        args: vec![Some(3), Some(4)],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 5,
                        is_generator: false,
                    }),
                    // Map entries are constant/ast index pairs
                    Map(vec![(0, Some(0)), (2, Some(6))]),
                    MainBlock {
                        body: vec![7],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                    Constant::Str("bar"),
                    Constant::Str("self"),
                    Constant::Str("x"),
                ]),
            )
        }

        #[test]
        fn function_map_block() {
            let source = "
f = ||
  foo: x
  bar: 0
";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(2),
                    Number0,
                    // Map entries are constant/ast index pairs
                    Map(vec![(1, Some(1)), (3, Some(2))]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![2],
                        body: 3,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 4,
                    },
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("foo"),
                    Constant::Str("x"),
                    Constant::Str("bar"),
                ]),
            )
        }

        #[test]
        fn instance_function_block() {
            let source = "
f = ||
  foo: 42
  bar: |self x| self.foo = x
f()";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(2),
                    Id(4),
                    Lookup((LookupNode::Id(1), None)),
                    Lookup((LookupNode::Root(2), Some(3))),
                    Id(5), // 5 - x
                    Assign {
                        target: AssignTarget {
                            target_index: 4,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 5,
                    }, // 5
                    Function(koto_parser::Function {
                        args: vec![Some(4), Some(5)],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 6,
                        is_generator: false,
                    }),
                    // Map entries are constant/ast index pairs
                    Map(vec![(1, Some(1)), (3, Some(7))]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 8,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 9,
                    }, // 10
                    Id(0),
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Root(11), Some(12))),
                    MainBlock {
                        body: vec![10, 13],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                    Constant::Str("bar"),
                    Constant::Str("self"),
                    Constant::Str("x"),
                ]),
            )
        }

        #[test]
        fn nested_function_with_loops_and_ifs() {
            let source = "\
f = |n|
  f2 = |n|
    for i in 0..1
      if i == n
        return i

  for x in 0..1
    if x == n
      return f2 n
f 1
";
            check_ast(
                source,
                &[
                    Id(0), // f
                    Id(2), // f2
                    Number0,
                    Number1,
                    Range {
                        start: 2,
                        end: 3,
                        inclusive: false,
                    },
                    Id(3), // 5 - i
                    Id(1), // n
                    BinaryOp {
                        op: AstOp::Equal,
                        lhs: 5,
                        rhs: 6,
                    },
                    Id(3),
                    ReturnExpression(8),
                    If(AstIf {
                        condition: 7,
                        then_node: 9,
                        else_if_blocks: vec![],
                        else_node: None,
                    }), // 10
                    For(AstFor {
                        args: vec![Some(3)],
                        ranges: vec![4],
                        condition: None,
                        body: 10,
                    }),
                    Function(koto_parser::Function {
                        args: vec![Some(1)],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 11,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 1,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 12,
                    },
                    Number0,
                    Number1, // 15
                    Range {
                        start: 14,
                        end: 15,
                        inclusive: false,
                    },
                    Id(4), // x
                    Id(1), // n
                    BinaryOp {
                        op: AstOp::Equal,
                        lhs: 17,
                        rhs: 18,
                    },
                    Id(2), // 20 - f2
                    Id(1), // n
                    Call {
                        function: 20,
                        args: vec![21],
                    },
                    ReturnExpression(22),
                    If(AstIf {
                        condition: 19,
                        then_node: 23,
                        else_if_blocks: vec![],
                        else_node: None,
                    }),
                    For(AstFor {
                        args: vec![Some(4)], // x
                        ranges: vec![16],
                        condition: None,
                        body: 24,
                    }), // 25
                    Block(vec![13, 25]),
                    Function(koto_parser::Function {
                        args: vec![Some(1)],
                        local_count: 3,
                        accessed_non_locals: vec![],
                        body: 26,
                        is_generator: false,
                    }),
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 27,
                    },
                    Id(0),   // f
                    Number1, // 30
                    Call {
                        function: 29,
                        args: vec![30],
                    },
                    MainBlock {
                        body: vec![28, 31],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("n"),
                    Constant::Str("f2"),
                    Constant::Str("i"),
                    Constant::Str("x"),
                ]),
            )
        }

        #[test]
        fn non_local_access() {
            let source = "|| x += 1";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Add,
                        expression: 1,
                    },
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![0], // initial read of x via capture
                        body: 2,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn call_with_functor() {
            let source = "\
z = y [0..20] |x| x > 1
y z";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Number0,
                    Number(2),
                    Range {
                        start: 2,
                        end: 3,
                        inclusive: false,
                    },
                    List(vec![4]), // 5
                    Id(3),
                    Number1,
                    BinaryOp {
                        op: AstOp::Greater,
                        lhs: 6,
                        rhs: 7,
                    },
                    Function(koto_parser::Function {
                        args: vec![Some(3)],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 8,
                        is_generator: false,
                    }),
                    Call {
                        function: 1,
                        args: vec![5, 9],
                    }, // 10
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 10,
                    },
                    Id(1),
                    Id(0),
                    Call {
                        function: 12,
                        args: vec![13],
                    },
                    MainBlock {
                        body: vec![11, 14],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("z"),
                    Constant::Str("y"),
                    Constant::Number(20.0),
                    Constant::Str("x"),
                ]),
            )
        }

        #[test]
        fn generator_function() {
            let source = "|| yield 1";
            check_ast(
                source,
                &[
                    Number1,
                    Yield(0),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 1,
                        is_generator: true,
                    }),
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn generator_multiple_values() {
            let source = "|| yield 1, 0";
            check_ast(
                source,
                &[
                    Number1,
                    Number0,
                    Tuple(vec![0, 1]),
                    Yield(2),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 3,
                        is_generator: true,
                    }),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                None,
            )
        }
    }

    mod lookups {
        use super::*;

        #[test]
        fn indexed_assignment() {
            let source = "a[0] = a[1]";

            check_ast(
                source,
                &[
                    Id(0),
                    Number0,
                    Lookup((LookupNode::Index(1), None)),
                    Lookup((LookupNode::Root(0), Some(2))),
                    Id(0),
                    Number1, // 5
                    Lookup((LookupNode::Index(5), None)),
                    Lookup((LookupNode::Root(4), Some(6))),
                    Assign {
                        target: AssignTarget {
                            target_index: 3,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn index_range_full() {
            let source = "x[..]";
            check_ast(
                source,
                &[
                    Id(0),
                    RangeFull,
                    Lookup((LookupNode::Index(1), None)),
                    Lookup((LookupNode::Root(0), Some(2))),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn index_range_to() {
            let source = "x[..3]";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(1),
                    RangeTo {
                        end: 1,
                        inclusive: false,
                    },
                    Lookup((LookupNode::Index(2), None)),
                    Lookup((LookupNode::Root(0), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Number(3.0)]),
            )
        }

        #[test]
        fn index_range_from_and_sub_index() {
            let source = "x[10..][0]";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(1),
                    RangeFrom { start: 1 },
                    Number0,
                    Lookup((LookupNode::Index(3), None)),
                    Lookup((LookupNode::Index(2), Some(4))), // 5
                    Lookup((LookupNode::Root(0), Some(5))),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Number(10.0)]),
            )
        }

        #[test]
        fn lookup_id() {
            let source = "x.foo";
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Id(1), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn lookup_call() {
            let source = "x.bar()";
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Id(1), Some(1))),
                    Lookup((LookupNode::Root(0), Some(2))),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn lookup_assignment() {
            let source = r#"
x.bar()."baz" = 1
"#;
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Id(2), None)),
                    Lookup((LookupNode::Call(vec![]), Some(1))),
                    Lookup((LookupNode::Id(1), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    Number1, // 5
                    Assign {
                        target: AssignTarget {
                            target_index: 4,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn lookup_space_separated_call() {
            let source = "x.foo 42";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(2),
                    Lookup((LookupNode::Call(vec![1]), None)),
                    Lookup((LookupNode::Id(1), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                ]),
            )
        }

        #[test]
        fn map_lookup_in_list() {
            let source = "[m.foo, m.bar]";
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Id(1), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(0),
                    Lookup((LookupNode::Id(2), None)),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    List(vec![2, 5]),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("m"),
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                ]),
            )
        }

        #[test]
        fn lookups_on_call_result() {
            let source = "(f x).foo";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Call {
                        function: 0,
                        args: vec![1],
                    },
                    Lookup((LookupNode::Id(2), None)),
                    Lookup((LookupNode::Root(2), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn index_on_call_result() {
            let source = "(f x)[0]";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Call {
                        function: 0,
                        args: vec![1],
                    },
                    Number0,
                    Lookup((LookupNode::Index(3), None)),
                    Lookup((LookupNode::Root(2), Some(4))), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }
        #[test]
        fn call_on_call_result() {
            let source = "(f x)(y)";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Call {
                        function: 0,
                        args: vec![1],
                    },
                    Id(2),
                    Lookup((LookupNode::Call(vec![3]), None)),
                    Lookup((LookupNode::Root(2), Some(4))),
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn lookup_on_number() {
            let source = "1.sin()";
            check_ast(
                source,
                &[
                    Number1,
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Id(0), Some(1))),
                    Lookup((LookupNode::Root(0), Some(2))),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("sin")]),
            )
        }

        #[test]
        fn lookup_on_string() {
            let source = r#"
"{}".format x
"#;
            check_ast(
                source,
                &[
                    Str(0),
                    Id(2),
                    Lookup((LookupNode::Call(vec![1]), None)),
                    Lookup((LookupNode::Id(1), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("{}"),
                    Constant::Str("format"),
                    Constant::Str("x"),
                ]),
            )
        }

        #[test]
        fn lookup_on_list() {
            let source = "[0, 1].contains y";
            check_ast(
                source,
                &[
                    Number0,
                    Number1,
                    List(vec![0, 1]),
                    Id(1),
                    Lookup((LookupNode::Call(vec![3]), None)),
                    Lookup((LookupNode::Id(0), Some(4))), // 5
                    Lookup((LookupNode::Root(2), Some(5))),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("contains"), Constant::Str("y")]),
            )
        }

        #[test]
        fn lookup_on_map() {
            let source = "{x}.values()";
            check_ast(
                source,
                &[
                    Map(vec![(0, None)]),
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Id(1), Some(1))),
                    Lookup((LookupNode::Root(0), Some(2))),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("values")]),
            )
        }

        #[test]
        fn nested_lookup_call() {
            let source = "((x).contains y)";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(2),
                    Lookup((LookupNode::Call(vec![1]), None)),
                    Lookup((LookupNode::Id(1), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("contains"),
                    Constant::Str("y"),
                ]),
            )
        }

        #[test]
        fn multiline_lookup() {
            let source = "
x.iter()
  .skip 1
  .to_tuple()
";
            check_ast(
                source,
                &[
                    Id(0),
                    Number1,
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Id(3), Some(2))),
                    Lookup((LookupNode::Call(vec![1]), Some(3))),
                    Lookup((LookupNode::Id(2), Some(4))), // 5
                    Lookup((LookupNode::Call(vec![]), Some(5))),
                    Lookup((LookupNode::Id(1), Some(6))),
                    Lookup((LookupNode::Root(0), Some(7))),
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("iter"),
                    Constant::Str("skip"),
                    Constant::Str("to_tuple"),
                ]),
            )
        }
    }

    mod keywords {
        use super::*;

        #[test]
        fn flow() {
            let source = "\
break
continue
return
return 1";
            check_ast(
                source,
                &[
                    Break,
                    Continue,
                    Return,
                    Number1,
                    ReturnExpression(3),
                    MainBlock {
                        body: vec![0, 1, 2, 4],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn expressions() {
            let source = "\
copy x
not true
debug x + x
assert_eq (type true) \"bool\"
";
            check_ast(
                source,
                &[
                    Id(0),
                    CopyExpression(0),
                    BoolTrue,
                    Negate(2),
                    Id(0),
                    Id(0), // 5
                    BinaryOp {
                        op: AstOp::Add,
                        lhs: 4,
                        rhs: 5,
                    },
                    Debug {
                        expression_string: 1,
                        expression: 6,
                    },
                    Id(2),
                    BoolTrue,
                    Type(9), // 10
                    Str(3),
                    Call {
                        function: 8,
                        args: vec![10, 11],
                    },
                    MainBlock {
                        body: vec![1, 3, 7, 12],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("x + x"),
                    Constant::Str("assert_eq"),
                    Constant::Str("bool"),
                ]),
            )
        }
    }

    mod import {
        use super::*;

        #[test]
        fn import_module() {
            let source = "import foo";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![],
                        items: vec![vec![0]],
                    },
                    MainBlock {
                        body: vec![0],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("foo")]),
            )
        }

        #[test]
        fn import_item() {
            let source = "import foo.bar";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![],
                        items: vec![vec![0, 1]],
                    },
                    MainBlock {
                        body: vec![0],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn import_item_used_in_assignment() {
            let source = "x = import foo.bar";
            check_ast(
                source,
                &[
                    Id(0),
                    Import {
                        from: vec![],
                        items: vec![vec![1, 2]],
                    },
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 1,
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 2, // x and bar both assigned locally
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                ]),
            )
        }

        #[test]
        fn import_items() {
            let source = "from foo import bar baz";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![0],
                        items: vec![vec![1], vec![2]],
                    },
                    MainBlock {
                        body: vec![0],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn import_nested_items() {
            let source = "from foo.bar import abc.def xyz";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![0, 1],
                        items: vec![vec![2, 3], vec![4]],
                    },
                    MainBlock {
                        body: vec![0],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Str("abc"),
                    Constant::Str("def"),
                    Constant::Str("xyz"),
                ]),
            )
        }
    }

    mod error_handling {
        use super::*;

        #[test]
        fn try_catch() {
            let source = "\
try
  f()
catch e
  debug e
";
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(1),
                    Debug {
                        expression_string: 1,
                        expression: 3,
                    },
                    Try(AstTry {
                        try_block: 2,
                        catch_arg: Some(1),
                        catch_block: 4,
                        finally_block: None,
                    }), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("e")]),
            )
        }

        #[test]
        fn try_catch_ignored_catch_arg() {
            let source = "\
try
  x
catch _
  y
";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Try(AstTry {
                        try_block: 0,
                        catch_arg: None,
                        catch_block: 1,
                        finally_block: None,
                    }), // 5
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn try_catch_finally() {
            let source = "\
try
  f()
catch e
  debug e
finally
  0
";
            check_ast(
                source,
                &[
                    Id(0),
                    Lookup((LookupNode::Call(vec![]), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(1),
                    Debug {
                        expression_string: 1,
                        expression: 3,
                    },
                    Number0, // 5
                    Try(AstTry {
                        try_block: 2,
                        catch_arg: Some(1),
                        catch_block: 4,
                        finally_block: Some(5),
                    }),
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("e")]),
            )
        }
    }

    mod match_expression {
        use super::*;

        #[test]
        fn match_single_expression() {
            let source = r#"
x = match y
  0 or 1 then 42
  "foo" or ["bar"] then 99
  "baz" then break
  z if z < 10
    123
  z then -1
"#;
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Number0,
                    Number1,
                    Number(2),
                    Str(3), // 5
                    Str(4),
                    List(vec![6]),
                    Number(5),
                    Str(6),
                    Break, // 10
                    Id(7),
                    Id(7),
                    Number(8),
                    BinaryOp {
                        op: AstOp::Less,
                        lhs: 12,
                        rhs: 13,
                    },
                    Number(9), // 15
                    Id(7),
                    Number(10),
                    Match {
                        expression: 1,
                        arms: vec![
                            MatchArm {
                                patterns: vec![2, 3],
                                condition: None,
                                expression: 4,
                            },
                            MatchArm {
                                patterns: vec![5, 7],
                                condition: None,
                                expression: 8,
                            },
                            MatchArm {
                                patterns: vec![9],
                                condition: None,
                                expression: 10,
                            },
                            MatchArm {
                                patterns: vec![11],
                                condition: Some(14),
                                expression: 15,
                            },
                            MatchArm {
                                patterns: vec![16],
                                condition: None,
                                expression: 17,
                            },
                        ],
                    },
                    Assign {
                        target: AssignTarget {
                            target_index: 0,
                            scope: Scope::Local,
                        },
                        op: AssignOp::Equal,
                        expression: 18,
                    },
                    MainBlock {
                        body: vec![19],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Number(42.0),
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Number(99.0), // 5
                    Constant::Str("baz"),
                    Constant::Str("z"),
                    Constant::Number(10.0),
                    Constant::Number(123.0),
                    Constant::Number(-1.0), // 10
                ]),
            )
        }

        #[test]
        fn match_multi_expression() {
            let source = "
match x, y
  0, 1 or 2, 3 if z then 0
  a, ()
    a
  _ then 0
";
            check_ast(
                source,
                &[
                    Id(0),
                    Id(1),
                    Tuple(vec![0, 1]),
                    Number0,
                    Number1,
                    Tuple(vec![3, 4]), // 5
                    Number(2),
                    Number(3),
                    Tuple(vec![6, 7]),
                    Id(4),
                    Number0, // 10
                    Id(5),
                    Empty,
                    Tuple(vec![11, 12]),
                    Id(5),
                    Wildcard, // 15
                    Number0,
                    Match {
                        expression: 2,
                        arms: vec![
                            MatchArm {
                                patterns: vec![5, 8],
                                condition: Some(9),
                                expression: 10,
                            },
                            MatchArm {
                                patterns: vec![13],
                                condition: None,
                                expression: 14,
                            },
                            MatchArm {
                                patterns: vec![15],
                                condition: None,
                                expression: 16,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![17],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Number(2.0),
                    Constant::Number(3.0),
                    Constant::Str("z"),
                    Constant::Str("a"),
                ]),
            )
        }

        #[test]
        fn match_condition_is_lookup_call() {
            let source = "
match x.foo 42
  () then 0
  _ then 1
";
            check_ast(
                source,
                &[
                    Id(0),
                    Number(2),
                    Lookup((LookupNode::Call(vec![1]), None)),
                    Lookup((LookupNode::Id(1), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    Empty, // 5
                    Number0,
                    Wildcard,
                    Number1,
                    Match {
                        expression: 4,
                        arms: vec![
                            MatchArm {
                                patterns: vec![5],
                                condition: None,
                                expression: 6,
                            },
                            MatchArm {
                                patterns: vec![7],
                                condition: None,
                                expression: 8,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![9],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
                    Constant::Number(42.0),
                ]),
            )
        }
    }
}
