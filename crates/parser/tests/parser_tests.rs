mod parser {
    use koto_parser::{Node::*, *};

    fn check_ast(source: &str, expected_ast: &[Node], expected_constants: Option<&[Constant]>) {
        println!("{source}");

        match Parser::parse(source) {
            Ok(ast) => {
                for (i, (ast_node, expected_node)) in
                    ast.nodes().iter().zip(expected_ast.iter()).enumerate()
                {
                    assert_eq!(ast_node.node, *expected_node, "Mismatch at position {i}");
                }
                assert_eq!(
                    ast.nodes().len(),
                    expected_ast.len(),
                    "Node list length mismatch"
                );

                if let Some(expected_constants) = expected_constants {
                    for (constant, expected_constant) in
                        ast.constants().iter().zip(expected_constants.iter())
                    {
                        assert_eq!(constant, *expected_constant);
                    }
                    assert_eq!(
                        ast.constants().size(),
                        expected_constants.len(),
                        "Constant pool size mismatch"
                    );
                } else {
                    assert_eq!(ast.constants().size(), 0);
                }
            }
            Err(error) => panic!("{error} - {}", error.span.start),
        }
    }

    fn check_ast_for_equivalent_sources(
        sources: &[&str],
        expected_ast: &[Node],
        expected_constants: Option<&[Constant]>,
    ) {
        for source in sources {
            check_ast(source, expected_ast, expected_constants)
        }
    }

    fn constant(index: u8) -> u32 {
        ConstantIndex::from(index)
    }

    fn string_literal(literal_index: u8, quotation_mark: QuotationMark) -> Node {
        Node::Str(AstString {
            quotation_mark,
            nodes: vec![StringNode::Literal(constant(literal_index))],
        })
    }

    fn string_literal_map_key(literal_index: u8, quotation_mark: QuotationMark) -> MapKey {
        MapKey::Str(AstString {
            quotation_mark,
            nodes: vec![StringNode::Literal(constant(literal_index))],
        })
    }

    mod values {
        use super::*;

        #[test]
        fn literals() {
            let source = r#"
true
false
1
1.0
"hello"
'world'
a
null"#;
            check_ast(
                source,
                &[
                    BoolTrue,
                    BoolFalse,
                    SmallInt(1),
                    Float(constant(0)),
                    string_literal(1, QuotationMark::Double),
                    string_literal(2, QuotationMark::Single),
                    Id(constant(3)),
                    Null,
                    MainBlock {
                        body: vec![0, 1, 2, 3, 4, 5, 6, 7],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::F64(1.0),
                    Constant::Str("hello"),
                    Constant::Str("world"),
                    Constant::Str("a"),
                ]),
            )
        }

        #[test]
        fn number_notation() {
            let source = "
1
0x1
0x100
0xABADCAFE
0o1
0o100
0b1
0b100
";
            check_ast(
                source,
                &[
                    SmallInt(1),
                    SmallInt(1),
                    Int(constant(0)),
                    Int(constant(1)),
                    SmallInt(1),
                    SmallInt(64),
                    SmallInt(1),
                    SmallInt(4),
                    MainBlock {
                        body: vec![0, 1, 2, 3, 4, 5, 6, 7],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::I64(256), Constant::I64(2880293630)]),
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
                    string_literal(0, QuotationMark::Double),
                    string_literal(1, QuotationMark::Double),
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
        fn strings_with_escape_codes() {
            let source = r#"
"\t\n\x4d\x2E"
'\u{1F917}\u{1f30d}'
"#;
            check_ast(
                source,
                &[
                    string_literal(0, QuotationMark::Double),
                    string_literal(1, QuotationMark::Single),
                    MainBlock {
                        body: vec![0, 1],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("\t\nM."), Constant::Str("🤗🌍")]),
            )
        }

        #[test]
        fn strings_with_interpolated_ids() {
            let source = r#"
'Hello, $name!'
"$foo"
'$x $y'
"#;
            check_ast(
                source,
                &[
                    Id(constant(1)),
                    Str(AstString {
                        quotation_mark: QuotationMark::Single,
                        nodes: vec![
                            StringNode::Literal(constant(0)),
                            StringNode::Expr(0),
                            StringNode::Literal(constant(2)),
                        ],
                    }),
                    Id(constant(3)),
                    Str(AstString {
                        quotation_mark: QuotationMark::Double,
                        nodes: vec![StringNode::Expr(2)],
                    }),
                    Id(constant(4)),
                    Id(constant(6)), // 5
                    Str(AstString {
                        quotation_mark: QuotationMark::Single,
                        nodes: vec![
                            StringNode::Expr(4),
                            StringNode::Literal(constant(5)),
                            StringNode::Expr(5),
                        ],
                    }),
                    MainBlock {
                        body: vec![1, 3, 6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("Hello, "),
                    Constant::Str("name"),
                    Constant::Str("!"),
                    Constant::Str("foo"),
                    Constant::Str("x"),
                    Constant::Str(" "),
                    Constant::Str("y"),
                ]),
            )
        }

        #[test]
        fn string_with_interpolated_expression() {
            let source = "
'${123 + 456}!'
";
            check_ast(
                source,
                &[
                    SmallInt(123),
                    Int(constant(0)),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Str(AstString {
                        quotation_mark: QuotationMark::Single,
                        nodes: vec![StringNode::Expr(2), StringNode::Literal(constant(1))],
                    }),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::I64(456), Constant::Str("!")]),
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
                    Float(constant(0)),
                    Id(constant(1)),
                    UnaryOp {
                        op: AstUnaryOp::Negate,
                        value: 1,
                    },
                    Id(constant(2)),
                    SmallInt(0),
                    Lookup((LookupNode::Index(4), None)), // 5
                    Lookup((LookupNode::Root(3), Some(5))),
                    UnaryOp {
                        op: AstUnaryOp::Negate,
                        value: 6,
                    },
                    SmallInt(1),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 8,
                        rhs: 9,
                    }, // 10
                    Nested(10),
                    UnaryOp {
                        op: AstUnaryOp::Negate,
                        value: 11,
                    },
                    MainBlock {
                        body: vec![0, 2, 7, 12],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::F64(-12.0), Constant::Str("a"), Constant::Str("x")]),
            )
        }
    }

    mod lists {
        use super::*;

        #[test]
        fn basic_lists() {
            let source = r#"
[0, n, "test", n, -1]
[]
"#;
            check_ast(
                source,
                &[
                    SmallInt(0),
                    Id(constant(0)),
                    string_literal(1, QuotationMark::Double),
                    Id(constant(0)),
                    SmallInt(-1),
                    List(vec![0, 1, 2, 3, 4]),
                    List(vec![]),
                    MainBlock {
                        body: vec![5, 6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("n"), Constant::Str("test")]),
            )
        }

        #[test]
        fn nested_list() {
            let source = r#"
[0, [1, -1], 2]
"#;
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(-1),
                    List(vec![1, 2]),
                    SmallInt(2),
                    List(vec![0, 3, 4]), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn list_with_line_breaks() {
            let sources = [
                "
x = [
  0,
  1,
  0,
  1,
  0
]
",
                "
x = [
  0, 1,
  0, 1,
  0
]
",
                "
x = [ 0
    , 1
    , 0
    , 1
    , 0
    ]
",
                "
x = [
  0 ,
  1
  , 0 , 1
  , 0]
",
            ];

            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0), // 5
                    List(vec![1, 2, 3, 4, 5]),
                    Assign {
                        target: 0,
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
    }

    mod maps {
        use super::*;

        #[test]
        fn map_inline() {
            let sources = [
                "
{}
x = {'foo': 42, bar, baz: 'hello', @+: 99}",
                "
{
}
x = { 'foo': 42
  , bar
  , baz: 'hello'
  , @+: 99
}
",
                "
{ }
x =
  { 'foo': 42, bar
    , baz: 'hello'
    , @+: 99
    }
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Map(vec![]),
                    Id(constant(0)),
                    SmallInt(42),
                    string_literal(4, QuotationMark::Single),
                    SmallInt(99),
                    Map(vec![
                        (string_literal_map_key(1, QuotationMark::Single), Some(2)),
                        (MapKey::Id(constant(2)), None),
                        (MapKey::Id(constant(3)), Some(3)),
                        (MapKey::Meta(MetaKeyId::Add, None), Some(4)),
                    ]), // 5
                    Assign {
                        target: 1,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![0, 6],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
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
  'foo': 42,
  bar,
  baz:
    "hello",
}"#;
            check_ast(
                source,
                &[
                    SmallInt(42),
                    string_literal(3, QuotationMark::Double),
                    Map(vec![
                        (string_literal_map_key(0, QuotationMark::Single), Some(0)),
                        (MapKey::Id(constant(1)), None),
                        (MapKey::Id(constant(2)), Some(1)),
                    ]),
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
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
  "baz":
    foo: 0
  @-: -1
x"#;
            check_ast(
                source,
                &[
                    Id(constant(0)), // x
                    SmallInt(42),
                    SmallInt(0),
                    Map(vec![(MapKey::Id(constant(1)), Some(2))]), // foo, 0
                    SmallInt(-1),
                    Map(vec![
                        (MapKey::Id(constant(1)), Some(1)), // foo: 42
                        (string_literal_map_key(2, QuotationMark::Double), Some(3)), // "baz": nested map
                        (MapKey::Meta(MetaKeyId::Subtract, None), Some(4)),          // @-: -1
                    ]), // 5
                    Assign {
                        target: 0,
                        expression: 5,
                    },
                    Id(constant(0)),
                    MainBlock {
                        body: vec![6, 7],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn map_block_first_entry_with_string_key() {
            let source = r#"
x =
  "foo": 42
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)), // x
                    SmallInt(42),
                    Map(vec![(
                        string_literal_map_key(1, QuotationMark::Double),
                        Some(1),
                    )]), // "foo", 42
                    Assign {
                        target: 0,
                        expression: 2,
                    },
                    MainBlock {
                        body: vec![3],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn map_block_first_entry_is_nested_map_block() {
            let source = r#"
x =
  foo:
    bar: 42
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)), // x
                    SmallInt(42),
                    Map(vec![
                        (MapKey::Id(constant(2)), Some(1)), // bar: 42
                    ]),
                    Map(vec![
                        (MapKey::Id(constant(1)), Some(2)), // foo: ...
                    ]),
                    Assign {
                        target: 0,
                        expression: 3,
                    },
                    MainBlock {
                        body: vec![4],
                        local_count: 1,
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
        fn map_block_meta() {
            let source = r#"
x =
  @+: 0
  @-: 1
  @meta foo: 0
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)), // x
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
                    Map(vec![
                        (MapKey::Meta(MetaKeyId::Add, None), Some(1)),
                        (MapKey::Meta(MetaKeyId::Subtract, None), Some(2)),
                        (MapKey::Meta(MetaKeyId::Named, Some(constant(1))), Some(3)),
                    ]),
                    Assign {
                        target: 0,
                        expression: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn assigning_map_to_meta_key() {
            let source = r#"
@tests =
  @pre_test: 0
  @post_test: 1
  @test foo: 0
"#;
            check_ast(
                source,
                &[
                    Meta(MetaKeyId::Tests, None),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
                    Map(vec![
                        (MapKey::Meta(MetaKeyId::PreTest, None), Some(1)),
                        (MapKey::Meta(MetaKeyId::PostTest, None), Some(2)),
                        (MapKey::Meta(MetaKeyId::Test, Some(constant(0))), Some(3)),
                    ]),
                    Assign {
                        target: 0,
                        expression: 4,
                    }, // 5
                    Export(5),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo")]),
            )
        }
    }

    mod ranges {
        use super::*;

        #[test]
        fn ranges_from_literals() {
            let source = "
0..1
0..=1";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    SmallInt(0),
                    SmallInt(1),
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
                None,
            )
        }

        #[test]
        fn range_from_expressions() {
            let source = "0 + 1..1 + 0";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    SmallInt(1),
                    SmallInt(0),
                    BinaryOp {
                        op: AstBinaryOp::Add,
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
                None,
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
                    Id(constant(0)),
                    SmallInt(0),
                    Assign {
                        target: 0,
                        expression: 1,
                    },
                    Id(constant(1)),
                    SmallInt(10),
                    Assign {
                        target: 3,
                        expression: 4,
                    }, // 5
                    Id(constant(0)),
                    Id(constant(1)),
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
                Some(&[Constant::Str("min"), Constant::Str("max")]),
            )
        }

        #[test]
        fn range_from_lookups() {
            let source = "foo.bar..foo.baz";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(1)), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(2)), None)),
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
        fn ranges_in_lists() {
            let source = "\
[0..1]
[0..10, 10..=0]";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    List(vec![2]),
                    SmallInt(0),
                    SmallInt(10), // 5
                    Range {
                        start: 4,
                        end: 5,
                        inclusive: false,
                    },
                    SmallInt(10),
                    SmallInt(0),
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
                None,
            )
        }
    }

    mod tuples {
        use super::*;

        #[test]
        fn tuple() {
            let source = "0, 1, 0";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
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
        fn empty_tuple() {
            let source = "(,)";
            check_ast(
                source,
                &[
                    Tuple(vec![]),
                    MainBlock {
                        body: vec![0],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn empty_parentheses_without_comma() {
            let source = "()";
            check_ast(
                source,
                &[
                    Null,
                    MainBlock {
                        body: vec![0],
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn tuple_in_parens() {
            let sources = [
                "(0, 1, 0)",
                "
( 0,
  1,
  0
)
",
                "
( 0
  , 1
  , 0
  )
",
            ];

            check_ast_for_equivalent_sources(
                &sources,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
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
        fn single_entry_tuple() {
            let source = "(1,)";
            check_ast(
                source,
                &[
                    SmallInt(1),
                    Tuple(vec![0]),
                    MainBlock {
                        body: vec![1],
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
                    Id(constant(0)),
                    SmallInt(1),
                    Assign {
                        target: 0,
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
        fn tuple() {
            let source = "x = 1, 0";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(1),
                    SmallInt(0),
                    Tuple(vec![1, 2]),
                    Assign {
                        target: 0,
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
                    Id(constant(0)),
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(vec![1, 2]),
                    SmallInt(2),
                    SmallInt(3), // 5
                    Tuple(vec![4, 5]),
                    Tuple(vec![3, 6]),
                    Assign {
                        target: 0,
                        expression: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn unpack_tuple() {
            let source = "x, y[0] = 1, 0";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    SmallInt(0),
                    Lookup((LookupNode::Index(2), None)),
                    Lookup((LookupNode::Root(1), Some(3))),
                    SmallInt(1), // 5
                    SmallInt(0),
                    TempTuple(vec![5, 6]),
                    MultiAssign {
                        targets: vec![0, 4],
                        expression: 7,
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
                    Id(constant(0)),
                    Id(constant(1)),
                    SmallInt(1),
                    SmallInt(0),
                    TempTuple(vec![2, 3]),
                    MultiAssign {
                        targets: vec![0, 1],
                        expression: 4,
                    }, // 5
                    Id(constant(0)),
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
            let source = "x, _, _y = f()";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Wildcard(None),
                    Wildcard(Some(constant(1))),
                    Id(constant(2)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    MultiAssign {
                        targets: vec![0, 1, 2],
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
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
                    Id(constant(0)),
                    SmallInt(0),
                    BinaryOp {
                        op: AstBinaryOp::AddAssign,
                        lhs: 0,
                        rhs: 1,
                    },
                    Id(constant(0)),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::SubtractAssign,
                        lhs: 3,
                        rhs: 4,
                    }, // 5
                    Id(constant(0)),
                    SmallInt(2),
                    BinaryOp {
                        op: AstBinaryOp::MultiplyAssign,
                        lhs: 6,
                        rhs: 7,
                    },
                    Id(constant(0)),
                    SmallInt(3), // 10
                    BinaryOp {
                        op: AstBinaryOp::DivideAssign,
                        lhs: 9,
                        rhs: 10,
                    },
                    Id(constant(0)),
                    SmallInt(4),
                    BinaryOp {
                        op: AstBinaryOp::RemainderAssign,
                        lhs: 12,
                        rhs: 13,
                    },
                    MainBlock {
                        body: vec![2, 5, 8, 11, 14],
                        local_count: 0,
                    }, // 15
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }

    mod export {
        use super::*;

        #[test]
        fn export_assignment() {
            let sources = [
                "export a = 1 + 1",
                "
export a
  = 1 + 1",
                "
export a =
  1 + 1",
            ];

            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    SmallInt(1),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 1,
                        rhs: 2,
                    },
                    Assign {
                        target: 0,
                        expression: 3,
                    },
                    Export(4), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn export_map_block() {
            let source = "
export 
  a: 123
  b: 99
";

            check_ast(
                source,
                &[
                    SmallInt(123),
                    SmallInt(99),
                    Map(vec![
                        (MapKey::Id(constant(0)), Some(0)), // a: 123
                        (MapKey::Id(constant(1)), Some(1)), // b: 99
                    ]),
                    Export(2),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("b")]),
            )
        }
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn addition_subtraction() {
            let sources = [
                "
1 - 0 + 1
",
                "
1 - 0
  + 1
",
                "
1
  - 0
    + 1
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    SmallInt(1),
                    SmallInt(0),
                    BinaryOp {
                        op: AstBinaryOp::Subtract,
                        lhs: 0,
                        rhs: 1,
                    },
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Add,
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
                    SmallInt(1),
                    SmallInt(0),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Multiply,
                        lhs: 1,
                        rhs: 2,
                    },
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 0,
                        rhs: 3,
                    },
                    SmallInt(0), // 5
                    BinaryOp {
                        op: AstBinaryOp::Add,
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
                    SmallInt(1),
                    SmallInt(0),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 0,
                        rhs: 1,
                    },
                    Nested(2),
                    SmallInt(1),
                    SmallInt(0), // 5
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 4,
                        rhs: 5,
                    },
                    Nested(6),
                    BinaryOp {
                        op: AstBinaryOp::Multiply,
                        lhs: 3,
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
        fn divide_then_remainder() {
            let source = "18 / 3 % 4";
            check_ast(
                source,
                &[
                    SmallInt(18),
                    SmallInt(3),
                    BinaryOp {
                        op: AstBinaryOp::Divide,
                        lhs: 0,
                        rhs: 1,
                    },
                    SmallInt(4),
                    BinaryOp {
                        op: AstBinaryOp::Remainder,
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
        fn string_and_id() {
            let source = "'hello' + x";
            check_ast(
                source,
                &[
                    string_literal(0, QuotationMark::Single),
                    Id(constant(1)),
                    BinaryOp {
                        op: AstBinaryOp::Add,
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
                    Id(constant(0)), // x
                    SmallInt(1),
                    Id(constant(2)), // y
                    NamedCall {
                        id: constant(1), // f
                        args: vec![2],
                    },
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 1,
                        rhs: 3,
                    },
                    Assign {
                        target: 0,
                        expression: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("f"), Constant::Str("y")]),
            )
        }

        #[test]
        fn arithmetic_assignment() {
            let sources = [
                "
a = 1 +
    2 *
    3
",
                "
a = 1
  + 2
  * 3
",
                "
a =
  1
  + 2
  * 3
",
                "
a =
  1
  + 2
    * 3
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    SmallInt(1),
                    SmallInt(2),
                    SmallInt(3),
                    BinaryOp {
                        op: AstBinaryOp::Multiply,
                        lhs: 2,
                        rhs: 3,
                    },
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 1,
                        rhs: 4,
                    }, // 5
                    Assign {
                        target: 0,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn arithmetic_assignment_with_nested_expression() {
            let sources = [
                "
a = (1 + 2) * 3
",
                "
a =
  (1 + 2)
  * 3
",
                "
a =
  (1 +
     2)
  * 3
",
                "
a = (1
       + 2)
  * 3
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    SmallInt(1),
                    SmallInt(2),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 1,
                        rhs: 2,
                    },
                    Nested(3),
                    SmallInt(3), // 5
                    BinaryOp {
                        op: AstBinaryOp::Multiply,
                        lhs: 4,
                        rhs: 5,
                    },
                    Assign {
                        target: 0,
                        expression: 6,
                    },
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
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
                    SmallInt(0),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Less,
                        lhs: 0,
                        rhs: 1,
                    },
                    SmallInt(1),
                    SmallInt(0),
                    BinaryOp {
                        op: AstBinaryOp::Greater,
                        lhs: 3,
                        rhs: 4,
                    },
                    BinaryOp {
                        op: AstBinaryOp::And,
                        lhs: 2,
                        rhs: 5,
                    },
                    BoolTrue,
                    BinaryOp {
                        op: AstBinaryOp::Or,
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
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::LessOrEqual,
                        lhs: 1,
                        rhs: 2,
                    },
                    BinaryOp {
                        op: AstBinaryOp::Less,
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
                    SmallInt(1),
                    BoolTrue,
                    SmallInt(0),
                    SmallInt(1),
                    If(AstIf {
                        condition: 1,
                        then_node: 2,
                        else_if_blocks: vec![],
                        else_node: Some(3),
                    }),
                    BinaryOp {
                        op: AstBinaryOp::Add,
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
            let sources = [
                "
a = if false
  0
else if true
  1
else if false
  0
else
  1
a",
                "
a =
  if false
    0
  else if true
    1
  else if false
    0
  else
    1
a",
                "
a = if false
      0
    else if true
      1
    else if false
      0
    else
      1
a",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    BoolFalse,
                    SmallInt(0),
                    BoolTrue,
                    SmallInt(1),
                    BoolFalse, // 5
                    SmallInt(0),
                    SmallInt(1),
                    If(AstIf {
                        condition: 1,
                        then_node: 2,
                        else_if_blocks: vec![(3, 4), (5, 6)],
                        else_node: Some(7),
                    }),
                    Assign {
                        target: 0,
                        expression: 8,
                    },
                    Id(constant(0)),
                    MainBlock {
                        body: vec![9, 10],
                        local_count: 1,
                    }, // 10
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn if_inline_multi_expressions() {
            let source = "a, b = if true then 0, 1 else 1, 0";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    BoolTrue,
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(vec![3, 4]), // 5
                    SmallInt(1),
                    SmallInt(0),
                    Tuple(vec![6, 7]),
                    If(AstIf {
                        condition: 2,
                        then_node: 5,
                        else_if_blocks: vec![],
                        else_node: Some(8),
                    }),
                    MultiAssign {
                        targets: vec![0, 1],
                        expression: 9,
                    }, // 10
                    MainBlock {
                        body: vec![10],
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("b")]),
            )
        }

        #[test]
        fn if_block_in_function_followed_by_id() {
            let source = "
||
  if true
    return
  x
";

            check_ast(
                source,
                &[
                    BoolTrue,
                    Return(None),
                    If(AstIf {
                        condition: 0,
                        then_node: 1,
                        else_if_blocks: vec![],
                        else_node: None,
                    }),
                    Id(constant(0)),
                    Block(vec![2, 3]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![constant(0)],
                        body: 4,
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }

    mod loops {
        use super::*;

        #[test]
        fn for_block() {
            let source = "\
for x, _, _y, z in foo
  z x";
            check_ast(
                source,
                &[
                    Id(constant(0)), // x
                    Wildcard(None),
                    Wildcard(Some(constant(1))), // _y
                    Id(constant(2)),             // z
                    Id(constant(3)),             // foo
                    Id(constant(0)),             // x - 5
                    NamedCall {
                        id: constant(2), // z
                        args: vec![5],
                    },
                    For(AstFor {
                        args: vec![0, 1, 2, 3],
                        iterable: 4,
                        body: 6,
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 2, // x, z
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Str("foo"),
                ]),
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
                    Id(constant(0)), // x
                    Id(constant(1)), // y
                    BinaryOp {
                        op: AstBinaryOp::Greater,
                        lhs: 0,
                        rhs: 1,
                    },
                    Id(constant(0)), // x
                    NamedCall {
                        id: constant(2), // f
                        args: vec![3],
                    },
                    While {
                        condition: 2,
                        body: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
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
                    Id(constant(0)), // x
                    Id(constant(1)), // y
                    BinaryOp {
                        op: AstBinaryOp::Less,
                        lhs: 0,
                        rhs: 1,
                    },
                    Id(constant(1)), // y
                    NamedCall {
                        id: constant(2), // f
                        args: vec![3],
                    },
                    Until {
                        condition: 2,
                        body: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
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
                    Id(constant(0)), // x
                    Id(constant(1)), // y
                    Id(constant(0)), // x
                    For(AstFor {
                        args: vec![1],
                        iterable: 2,
                        body: 3,
                    }),
                    MainBlock {
                        body: vec![0, 4],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn for_with_range_from_lookup_call() {
            let source = "\
for a in x.zip y
  a
";
            check_ast(
                source,
                &[
                    Id(constant(0)), // a
                    Id(constant(1)), // x
                    Id(constant(3)), // y
                    Lookup((
                        LookupNode::Call {
                            args: vec![2],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(2)), Some(3))),
                    Lookup((LookupNode::Root(1), Some(4))), // ast 5
                    Id(constant(0)),                        // a
                    For(AstFor {
                        args: vec![0],
                        iterable: 5,
                        body: 6,
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Str("x"),
                    Constant::Str("zip"),
                    Constant::Str("y"),
                ]),
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
                    Id(constant(0)),
                    SmallInt(42),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 1,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Assign {
                        target: 0,
                        expression: 2,
                    },
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )), // 5
                    Lookup((LookupNode::Root(4), Some(5))),
                    MainBlock {
                        body: vec![3, 6],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn inline_two_args() {
            let sources = [
                "
|x, y| x + y
",
                "
| x,
  y,
|
  x + y
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    Id(constant(0)),
                    Id(constant(1)),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 2,
                        rhs: 3,
                    },
                    Function(koto_parser::Function {
                        args: vec![0, 1],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 4,
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn inline_var_args() {
            let source = "|x, y...| x + y.size()";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    Id(constant(0)),
                    Id(constant(1)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(2)), Some(4))), // 5
                    Lookup((LookupNode::Root(3), Some(5))),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 2,
                        rhs: 6,
                    },
                    Function(koto_parser::Function {
                        args: vec![0, 1],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 7,
                        is_variadic: true,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("size"),
                ]),
            )
        }

        #[test]
        fn with_body() {
            let source = "\
f = |x|
  y = x
  y
f 42";
            check_ast(
                source,
                &[
                    Id(constant(0)), // f
                    Id(constant(1)), // x
                    Id(constant(2)), // y
                    Id(constant(1)), // x
                    Assign {
                        target: 2,
                        expression: 3,
                    },
                    Id(constant(2)), // 5
                    Block(vec![4, 5]),
                    Function(koto_parser::Function {
                        args: vec![1],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 6,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Assign {
                        target: 0,
                        expression: 7,
                    },
                    SmallInt(42),
                    NamedCall {
                        id: constant(0),
                        args: vec![9],
                    }, // 10
                    MainBlock {
                        body: vec![8, 10],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x"), Constant::Str("y")]),
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
                    Id(constant(0)), // f
                    Id(constant(1)), // x
                    Id(constant(2)), // y
                    Id(constant(3)), // z
                    Id(constant(3)), // z
                    Function(koto_parser::Function {
                        args: vec![3],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 4,
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    Assign {
                        target: 2,
                        expression: 5,
                    },
                    Id(constant(1)), // x
                    NamedCall {
                        id: constant(2), // y
                        args: vec![7],
                    },
                    Block(vec![6, 8]),
                    Function(koto_parser::Function {
                        args: vec![1],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 9,
                        is_variadic: false,
                        is_generator: false,
                    }), // 10
                    Assign {
                        target: 0,
                        expression: 10,
                    },
                    SmallInt(42),
                    NamedCall {
                        id: constant(0), // f
                        args: vec![12],
                    },
                    MainBlock {
                        body: vec![11, 13],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                ]),
            )
        }

        #[test]
        fn call_negative_arg() {
            let source = "f x, -x";
            check_ast(
                source,
                &[
                    Id(constant(1)),
                    Id(constant(1)),
                    UnaryOp {
                        op: AstUnaryOp::Negate,
                        value: 1,
                    },
                    NamedCall {
                        id: constant(0), // f
                        args: vec![0, 2],
                    },
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_arithmetic_arg() {
            let source = "f x - 1";
            check_ast(
                source,
                &[
                    Id(constant(1)),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Subtract,
                        lhs: 0,
                        rhs: 1,
                    },
                    NamedCall {
                        id: constant(0), // f
                        args: vec![2],
                    },
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_with_parentheses() {
            let sources = [
                "
f(x, -x)
",
                "
f(
  x,
  -x
)
",
                "
f(x,
  -x)
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    Id(constant(1)),
                    UnaryOp {
                        op: AstUnaryOp::Negate,
                        value: 2,
                    },
                    Lookup((
                        LookupNode::Call {
                            args: vec![1, 3],
                            with_parens: true,
                        },
                        None,
                    )),
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
        fn call_with_indentated_args() {
            let source = "
foo
  x,
  y";
            check_ast(
                source,
                &[
                    Id(constant(1)),
                    Id(constant(2)),
                    NamedCall {
                        id: constant(0), // foo
                        args: vec![0, 1],
                    },
                    MainBlock {
                        body: vec![2],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn call_with_indentated_function_arg() {
            let source = "
foo
  x,
  |y| y";
            check_ast(
                source,
                &[
                    Id(constant(1)),
                    Id(constant(2)),
                    Id(constant(2)),
                    Function(koto_parser::Function {
                        args: vec![1],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 2,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    NamedCall {
                        id: constant(0), // foo
                        args: vec![0, 3],
                    },
                    MainBlock {
                        body: vec![4],
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
                    Id(constant(1)),
                    NamedCall {
                        id: constant(0),
                        args: vec![0],
                    },
                    Id(constant(1)),
                    NamedCall {
                        id: constant(0),
                        args: vec![2],
                    }, // 5
                    MainBlock {
                        body: vec![1, 3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn recursive_call() {
            let source = "f = |x| f x";
            check_ast(
                source,
                &[
                    Id(constant(0)), // f
                    Id(constant(1)), // x
                    Id(constant(1)), // x
                    NamedCall {
                        id: constant(0), // f
                        args: vec![2],
                    },
                    Function(koto_parser::Function {
                        args: vec![1],
                        local_count: 1,
                        accessed_non_locals: vec![constant(0)],
                        body: 3,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Assign {
                        target: 0,
                        expression: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn recursive_calls_multi_assign() {
            let source = "f, g = (|x| f x), (|x| g x)";
            check_ast(
                source,
                &[
                    Id(constant(0)), // f
                    Id(constant(1)), // g
                    Id(constant(2)), // x
                    Id(constant(2)),
                    NamedCall {
                        id: constant(0),
                        args: vec![3],
                    },
                    Function(koto_parser::Function {
                        args: vec![2],
                        local_count: 1,
                        accessed_non_locals: vec![constant(0)],
                        body: 4,
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    Nested(5),
                    Id(constant(2)), // x
                    Id(constant(2)), // x
                    NamedCall {
                        id: constant(1), // g
                        args: vec![8],
                    },
                    Function(koto_parser::Function {
                        args: vec![7],
                        local_count: 1,
                        accessed_non_locals: vec![constant(1)],
                        body: 9,
                        is_variadic: false,
                        is_generator: false,
                    }), // 10
                    Nested(10),
                    TempTuple(vec![6, 11]),
                    MultiAssign {
                        targets: vec![0, 1],
                        expression: 12,
                    },
                    MainBlock {
                        body: vec![13],
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("g"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_with_pipe() {
            let source = "f x >> g >> h";
            check_ast(
                source,
                &[
                    Id(constant(1)), // x
                    NamedCall {
                        id: constant(0), // f
                        args: vec![0],
                    },
                    Id(constant(2)), // g
                    BinaryOp {
                        op: AstBinaryOp::Pipe,
                        lhs: 1,
                        rhs: 2,
                    },
                    Id(constant(3)), // h
                    BinaryOp {
                        op: AstBinaryOp::Pipe,
                        lhs: 3,
                        rhs: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("x"),
                    Constant::Str("g"),
                    Constant::Str("h"),
                ]),
            )
        }

        #[test]
        fn indented_piped_calls_after_lookup() {
            let source = "
foo.bar x
  >> y
  >> z
";
            check_ast(
                source,
                &[
                    Id(constant(0)), // foo
                    Id(constant(2)), // x
                    Lookup((
                        LookupNode::Call {
                            args: vec![1],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    Id(constant(3)), // 5 - y
                    BinaryOp {
                        op: AstBinaryOp::Pipe,
                        lhs: 4,
                        rhs: 5,
                    },
                    Id(constant(4)), // z
                    BinaryOp {
                        op: AstBinaryOp::Pipe,
                        lhs: 6,
                        rhs: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                ]),
            )
        }

        #[test]
        fn instance_function() {
            let source = "{foo: 42, bar: |x| self.foo = x}";
            check_ast(
                source,
                &[
                    SmallInt(42),
                    Id(constant(2)), // x
                    Self_,           // self
                    Lookup((LookupNode::Id(constant(0)), None)),
                    Lookup((LookupNode::Root(2), Some(3))),
                    Id(constant(2)), // 5
                    Assign {
                        target: 4,
                        expression: 5,
                    },
                    Function(koto_parser::Function {
                        args: vec![1],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 6,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Map(vec![
                        (MapKey::Id(constant(0)), Some(0)),
                        (MapKey::Id(constant(1)), Some(7)),
                    ]),
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Str("bar"),
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
                    Id(constant(0)),
                    Id(constant(2)),
                    SmallInt(0),
                    Map(vec![
                        (MapKey::Id(constant(1)), Some(1)),
                        (MapKey::Id(constant(3)), Some(2)),
                    ]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![constant(2)],
                        body: 3,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Assign {
                        target: 0,
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
        fn function_map_block_with_nested_map_as_first_entry() {
            let source = "
f = ||
  foo:
    bar: x
  baz: 0
";
            check_ast(
                source,
                &[
                    Id(constant(0)), // f
                    Id(constant(3)), // x
                    Map(vec![
                        (MapKey::Id(constant(2)), Some(1)), // bar: x
                    ]),
                    SmallInt(0),
                    Map(vec![
                        (MapKey::Id(constant(1)), Some(2)), // foo: ...
                        (MapKey::Id(constant(4)), Some(3)), // baz: 0
                    ]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![constant(3)],
                        body: 4,
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    Assign {
                        target: 0,
                        expression: 5,
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Str("x"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn instance_function_block() {
            let source = "
f = ||
  foo: 42
  bar: |x| self.foo = x
f()";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(42),
                    Id(constant(3)), // x
                    Self_,
                    Lookup((LookupNode::Id(constant(1)), None)),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    Id(constant(3)),
                    Assign {
                        target: 5,
                        expression: 6,
                    },
                    Function(koto_parser::Function {
                        args: vec![2],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 7,
                        is_variadic: false,
                        is_generator: false,
                    }), // 10
                    Map(vec![
                        (MapKey::Id(constant(1)), Some(1)),
                        (MapKey::Id(constant(2)), Some(8)),
                    ]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 9,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Assign {
                        target: 0,
                        expression: 10,
                    },
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )), // 15
                    Lookup((LookupNode::Root(12), Some(13))),
                    MainBlock {
                        body: vec![11, 14],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("foo"),
                    Constant::Str("bar"),
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
  f2
";
            check_ast(
                source,
                &[
                    Id(constant(0)), // f
                    Id(constant(1)), // n
                    Id(constant(2)), // f2
                    Id(constant(1)),
                    Id(constant(3)), // i
                    SmallInt(0),     // ast 5
                    SmallInt(1),
                    Range {
                        start: 5,
                        end: 6,
                        inclusive: false,
                    },
                    Id(constant(3)), // i
                    Id(constant(1)),
                    BinaryOp {
                        op: AstBinaryOp::Equal,
                        lhs: 8,
                        rhs: 9,
                    }, // ast 10
                    Id(constant(3)),
                    Return(Some(11)),
                    If(AstIf {
                        condition: 10,
                        then_node: 12,
                        else_if_blocks: vec![],
                        else_node: None,
                    }),
                    For(AstFor {
                        args: vec![4],
                        iterable: 7,
                        body: 13,
                    }),
                    Function(koto_parser::Function {
                        args: vec![3],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 14,
                        is_variadic: false,
                        is_generator: false,
                    }), // ast 15
                    Assign {
                        target: 2,
                        expression: 15,
                    },
                    Id(constant(2)),
                    Block(vec![16, 17]),
                    Function(koto_parser::Function {
                        args: vec![1],
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 18,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Assign {
                        target: 0,
                        expression: 19,
                    }, // ast 20
                    MainBlock {
                        body: vec![20],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("n"),
                    Constant::Str("f2"),
                    Constant::Str("i"),
                ]),
            )
        }

        #[test]
        fn non_local_access() {
            let source = "
||
  x = x + 1
  x
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(0)),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 1,
                        rhs: 2,
                    },
                    Assign {
                        target: 0,
                        expression: 3,
                    },
                    Id(constant(0)), // 5
                    Block(vec![4, 5]),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 1,
                        accessed_non_locals: vec![constant(0)], // initial read of x via capture
                        body: 6,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn access_after_previous_assignment() {
            // In this example, b should not be counted as a non-local
            let source = "
|| a = (b = 1), b 
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    SmallInt(1),
                    Assign {
                        target: 1,
                        expression: 2,
                    },
                    Nested(3),
                    Id(constant(1)), // 5
                    Tuple(vec![4, 5]),
                    Assign {
                        target: 0,
                        expression: 6,
                    },
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 2,
                        accessed_non_locals: vec![], // b is locally assigned when accessed
                        body: 7,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("b")]),
            )
        }

        #[test]
        fn non_local_update_assignment() {
            let source = "
|| x += 1
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::AddAssign,
                        lhs: 0,
                        rhs: 1,
                    },
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![constant(0)], // initial read of x via capture
                        body: 2,
                        is_variadic: false,
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
        fn call_with_function() {
            let source = "\
z = y [0..20], |x| x > 1
y z";
            check_ast(
                source,
                &[
                    Id(constant(0)), // z
                    SmallInt(0),
                    SmallInt(20),
                    Range {
                        start: 1,
                        end: 2,
                        inclusive: false,
                    },
                    List(vec![3]),
                    Id(constant(2)), // 5 - x
                    Id(constant(2)),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Greater,
                        lhs: 6,
                        rhs: 7,
                    },
                    Function(koto_parser::Function {
                        args: vec![5],
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 8,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    NamedCall {
                        id: constant(1), // y
                        args: vec![4, 9],
                    }, // 10
                    Assign {
                        target: 0,
                        expression: 10,
                    },
                    Id(constant(0)), // z
                    NamedCall {
                        id: constant(1), // y
                        args: vec![12],
                    },
                    MainBlock {
                        body: vec![11, 13],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("z"), Constant::Str("y"), Constant::Str("x")]),
            )
        }

        #[test]
        fn generator_function() {
            let source = "|| yield 1";
            check_ast(
                source,
                &[
                    SmallInt(1),
                    Yield(0),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 1,
                        is_variadic: false,
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
                    SmallInt(1),
                    SmallInt(0),
                    Tuple(vec![0, 1]),
                    Yield(2),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 3,
                        is_variadic: false,
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

        #[test]
        fn generator_yielding_a_map() {
            let source = "
||
  yield
    foo: 42
";
            check_ast(
                source,
                &[
                    SmallInt(42),
                    Map(vec![(MapKey::Id(constant(0)), Some(0))]),
                    Yield(1),
                    Function(koto_parser::Function {
                        args: vec![],
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 2,
                        is_variadic: false,
                        is_generator: true,
                    }),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo")]),
            )
        }

        #[test]
        fn unpack_call_args_tuple() {
            let sources = [
                "
|a, (_, (others..., c, _d)), _e|
  a
",
                "
| a, 
  ( _, 
    (others..., c, _d)
  ), 
  _e
|
  a
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)), // a
                    Wildcard(None),
                    Ellipsis(Some(constant(1))), // others
                    Id(constant(2)),             // c
                    Wildcard(Some(constant(3))), // d
                    Tuple(vec![2, 3, 4]),        // ast index 5
                    Tuple(vec![1, 5]),
                    Wildcard(Some(constant(4))), // e
                    Id(constant(0)),
                    Function(koto_parser::Function {
                        args: vec![0, 6, 7],
                        local_count: 3,
                        accessed_non_locals: vec![],
                        body: 8,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![9],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Str("others"),
                    Constant::Str("c"),
                    Constant::Str("d"),
                    Constant::Str("e"),
                ]),
            )
        }

        #[test]
        fn unpack_call_args_list() {
            let sources = [
                "
|a, [_, [c, _d, ...]], e|
  a
",
                "
| a, 
  [ _, 
    [c, _d, ...]
  ], 
  e
|
  a
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)), // a
                    Wildcard(None),
                    Id(constant(1)),             // c
                    Wildcard(Some(constant(2))), // d
                    Ellipsis(None),              // ...
                    List(vec![2, 3, 4]),         // ast index 5
                    List(vec![1, 5]),
                    Id(constant(3)), // e
                    Id(constant(0)),
                    Function(koto_parser::Function {
                        args: vec![0, 6, 7],
                        local_count: 3,
                        accessed_non_locals: vec![],
                        body: 8,
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: vec![9],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Str("c"),
                    Constant::Str("d"),
                    Constant::Str("e"),
                ]),
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
                    Id(constant(0)),
                    SmallInt(0),
                    Lookup((LookupNode::Index(1), None)),
                    Lookup((LookupNode::Root(0), Some(2))),
                    Id(constant(0)),
                    SmallInt(1), // 5
                    Lookup((LookupNode::Index(5), None)),
                    Lookup((LookupNode::Root(4), Some(6))),
                    Assign {
                        target: 3,
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
                    Id(constant(0)),
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
                    Id(constant(0)),
                    SmallInt(3),
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
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn index_range_from_and_sub_index() {
            let source = "x[10..][0]";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(10),
                    RangeFrom { start: 1 },
                    SmallInt(0),
                    Lookup((LookupNode::Index(3), None)),
                    Lookup((LookupNode::Index(2), Some(4))), // 5
                    Lookup((LookupNode::Root(0), Some(5))),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn lookup_id() {
            let source = "x.foo";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(1)), None)),
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
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(1))),
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
        fn lookup_call_arithmetic_arg() {
            let source = "x.bar() - 1";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(1))),
                    Lookup((LookupNode::Root(0), Some(2))),
                    SmallInt(1),
                    BinaryOp {
                        op: AstBinaryOp::Subtract,
                        lhs: 3,
                        rhs: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
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
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Str(AstString {
                            quotation_mark: QuotationMark::Double,
                            nodes: vec![StringNode::Literal(constant(2))],
                        }),
                        None,
                    )),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        Some(1),
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    SmallInt(1), // 5
                    Assign {
                        target: 4,
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
                    Id(constant(0)),
                    SmallInt(42),
                    Lookup((
                        LookupNode::Call {
                            args: vec![1],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn lookup_indentation_separated_call() {
            let source = "
x.foo
  42
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(42),
                    Lookup((
                        LookupNode::Call {
                            args: vec![1],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn lookup_indentation_separated_with_map_arg() {
            let source = "
x.takes_a_map
  foo: 42
";
            check_ast(
                source,
                &[
                    Id(constant(0)), // x
                    SmallInt(42),
                    Map(vec![
                        (MapKey::Id(constant(2)), Some(1)), // foo: 42
                    ]),
                    Lookup((
                        LookupNode::Call {
                            args: vec![2],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(3))), // takes_a_map
                    Lookup((LookupNode::Root(0), Some(4))),         // @5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("takes_a_map"),
                    Constant::Str("foo"),
                ]),
            )
        }

        #[test]
        fn map_lookup_in_list() {
            let sources = [
                "[my_map.foo, my_map.bar]",
                "
[
  my_map
    .foo
  ,
  my_map
    .bar
]
",
                "
[ my_map.foo,
  my_map
    .bar
]
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(1)), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(2)), None)),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    List(vec![2, 5]),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("my_map"),
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
                    Id(constant(1)), // x
                    NamedCall {
                        id: constant(0), // f
                        args: vec![0],
                    },
                    Nested(1),
                    Lookup((LookupNode::Id(constant(2)), None)),
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
                    Id(constant(1)), // x
                    NamedCall {
                        id: constant(0), // f
                        args: vec![0],
                    },
                    Nested(1),
                    SmallInt(0),
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
                    Id(constant(1)), // x
                    NamedCall {
                        id: constant(0), // f
                        args: vec![0],
                    },
                    Nested(1),
                    Id(constant(2)), // y
                    Lookup((
                        LookupNode::Call {
                            args: vec![3],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Root(2), Some(4))), // 5
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
                    SmallInt(1),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(0)), Some(1))),
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
            let source = "'{}'.format x";
            check_ast(
                source,
                &[
                    string_literal(0, QuotationMark::Single),
                    Id(constant(2)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![1],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(2))),
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
        fn lookup_on_tuple() {
            let sources = [
                "
x = (0, 1).contains y
",
                "
x = (0, 1)
  .contains y
",
                "
x = ( 0
    , 1)
  .contains y
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(vec![1, 2]),
                    Id(constant(2)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![4],
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    Lookup((LookupNode::Id(constant(1)), Some(5))),
                    Lookup((LookupNode::Root(3), Some(6))),
                    Assign {
                        target: 0,
                        expression: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 1,
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
        fn lookup_on_list() {
            let sources = [
                "
x = [0, 1].contains y
",
                "
x = [0, 1]
  .contains y
",
                "
x = [ 0
    , 1]
  .contains y
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    SmallInt(0),
                    SmallInt(1),
                    List(vec![1, 2]),
                    Id(constant(2)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![4],
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    Lookup((LookupNode::Id(constant(1)), Some(5))),
                    Lookup((LookupNode::Root(3), Some(6))),
                    Assign {
                        target: 0,
                        expression: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 1,
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
        fn lookup_on_map() {
            let sources = [
                "
x = {y, z}.values()
",
                "
x = {y, z}
  .values()
",
                "
x =
  {y, z}
    .values()
",
                "
x = { y
    , z}
  .values()
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Id(constant(0)),
                    Map(vec![
                        (MapKey::Id(constant(1)), None),
                        (MapKey::Id(constant(2)), None),
                    ]),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(3)), Some(2))),
                    Lookup((LookupNode::Root(1), Some(3))),
                    Assign {
                        target: 0,
                        expression: 4,
                    }, // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Str("values"),
                ]),
            )
        }

        #[test]
        fn lookup_on_range_same_line() {
            let source = "(0..1).size()";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    Nested(2),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(0)), Some(4))), // 5
                    Lookup((LookupNode::Root(3), Some(5))),
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("size")]),
            )
        }

        #[test]
        fn lookup_on_range_next_line() {
            let source = "
0..1
  .size()
";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    Range {
                        start: 0,
                        end: 1,
                        inclusive: false,
                    },
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(0)), Some(3))),
                    Lookup((LookupNode::Root(2), Some(4))), // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("size")]),
            )
        }

        #[test]
        fn nested_lookup_call() {
            let source = "((x).contains y)";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Nested(0),
                    Id(constant(2)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![2],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(3))),
                    Lookup((LookupNode::Root(1), Some(4))), // 5
                    Nested(5),
                    MainBlock {
                        body: vec![6],
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
                    Id(constant(0)),
                    SmallInt(1),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(3)), Some(2))),
                    Lookup((
                        LookupNode::Call {
                            args: vec![1],
                            with_parens: false,
                        },
                        Some(3),
                    )),
                    Lookup((LookupNode::Id(constant(2)), Some(4))), // 5
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        Some(5),
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(6))),
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

        #[test]
        fn lookup_followed_by_continued_expression_on_next_line() {
            let source = "
foo.bar
  or foo.baz or
    false
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(1)), None)),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(constant(0)),
                    Lookup((LookupNode::Id(constant(2)), None)),
                    Lookup((LookupNode::Root(3), Some(4))), // 5
                    BinaryOp {
                        op: AstBinaryOp::Or,
                        lhs: 2,
                        rhs: 5,
                    },
                    BoolFalse,
                    BinaryOp {
                        op: AstBinaryOp::Or,
                        lhs: 6,
                        rhs: 7,
                    },
                    MainBlock {
                        body: vec![8],
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
                    Break(None),
                    Continue,
                    Return(None),
                    SmallInt(1),
                    Return(Some(3)),
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
            let source = r#"
not true
debug x + x
assert_eq x, "hello"
"#;
            check_ast(
                source,
                &[
                    BoolTrue,
                    UnaryOp {
                        op: AstUnaryOp::Not,
                        value: 0,
                    },
                    Id(constant(0)),
                    Id(constant(0)),
                    BinaryOp {
                        op: AstBinaryOp::Add,
                        lhs: 2,
                        rhs: 3,
                    },
                    Debug {
                        expression_string: constant(1),
                        expression: 4,
                    }, // 5
                    Id(constant(0)), // x
                    string_literal(3, QuotationMark::Double),
                    NamedCall {
                        id: constant(2),
                        args: vec![6, 7],
                    },
                    MainBlock {
                        body: vec![1, 5, 8],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("x + x"),
                    Constant::Str("assert_eq"),
                    Constant::Str("hello"),
                ]),
            )
        }
    }

    mod import {
        use super::*;

        fn import_id(id: u8) -> ImportItemNode {
            ImportItemNode::Id(constant(id))
        }

        fn import_string(literal_index: u8, quotation_mark: QuotationMark) -> ImportItemNode {
            ImportItemNode::Str(AstString {
                quotation_mark,
                nodes: vec![StringNode::Literal(constant(literal_index))],
            })
        }

        #[test]
        fn import_single_item() {
            let source = "import foo";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![],
                        items: vec![import_id(0)],
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
        fn import_from_module() {
            let source = "from foo import bar";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![import_id(0)],
                        items: vec![import_id(1)],
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
            let source = "x = from foo import bar";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Import {
                        from: vec![import_id(1)],
                        items: vec![import_id(2)],
                    },
                    Assign {
                        target: 0,
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
            let sources = [
                "import foo, 'bar', baz",
                "
import
  foo,
  'bar',
  baz,
",
                "
import foo,
  'bar', baz
",
            ];

            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Import {
                        from: vec![],
                        items: vec![
                            import_id(0),
                            import_string(1, QuotationMark::Single),
                            import_id(2),
                        ],
                    },
                    MainBlock {
                        body: vec![0],
                        local_count: 2, // foo and baz, bar needs to be assigned
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
        fn import_items_from() {
            let sources = [
                "from foo import bar, baz",
                "
from foo import
  bar, baz
",
                "
from foo import bar,
                baz,
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    Import {
                        from: vec![import_id(0)],
                        items: vec![import_id(1), import_id(2)],
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
            let source = "from 'foo'.bar import abc, xyz";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![import_string(0, QuotationMark::Single), import_id(1)],
                        items: vec![import_id(2), import_id(3)],
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
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(constant(1)), // e
                    Id(constant(1)),
                    Debug {
                        expression_string: constant(1),
                        expression: 4,
                    }, // ast 5
                    Try(AstTry {
                        try_block: 2,
                        catch_arg: 3,
                        catch_block: 5,
                        finally_block: None,
                    }),
                    MainBlock {
                        body: vec![6],
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
                    Id(constant(0)),
                    Wildcard(None),
                    Id(constant(1)),
                    Try(AstTry {
                        try_block: 0,
                        catch_arg: 1,
                        catch_block: 2,
                        finally_block: None,
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
        fn try_catch_ignored_catch_arg_with_name() {
            let source = "\
try
  x
catch _error
  y
";
            check_ast(
                source,
                &[
                    Id(constant(0)),             // x
                    Wildcard(Some(constant(1))), // error
                    Id(constant(2)),             // y
                    Try(AstTry {
                        try_block: 0,
                        catch_arg: 1,
                        catch_block: 2,
                        finally_block: None,
                    }),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("error"),
                    Constant::Str("y"),
                ]),
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
                    Id(constant(0)),
                    Lookup((
                        LookupNode::Call {
                            args: vec![],
                            with_parens: true,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Root(0), Some(1))),
                    Id(constant(1)), // e
                    Id(constant(1)),
                    Debug {
                        expression_string: constant(1),
                        expression: 4,
                    }, // ast 5
                    SmallInt(0),
                    Try(AstTry {
                        try_block: 2,
                        catch_arg: 3,
                        catch_block: 5,
                        finally_block: Some(6),
                    }),
                    MainBlock {
                        body: vec![7],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("e")]),
            )
        }

        #[test]
        fn throw_value() {
            let source = "throw x";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Throw(0),
                    MainBlock {
                        body: vec![1],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn throw_string() {
            let source = "throw 'error!'";
            check_ast(
                source,
                &[
                    string_literal(0, QuotationMark::Single),
                    Throw(0),
                    MainBlock {
                        body: vec![1],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("error!")]),
            )
        }

        #[test]
        fn throw_map() {
            let source = r#"
throw
  data: x
  message: "error!"
"#;
            check_ast(
                source,
                &[
                    Id(constant(1)),
                    string_literal(3, QuotationMark::Double),
                    Map(vec![
                        (MapKey::Id(constant(0)), Some(0)),
                        (MapKey::Id(constant(2)), Some(1)),
                    ]),
                    Throw(2),
                    MainBlock {
                        body: vec![3],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("data"),
                    Constant::Str("x"),
                    Constant::Str("message"),
                    Constant::Str("error!"),
                ]),
            )
        }
    }

    mod match_and_switch {
        use super::*;

        #[test]
        fn assign_from_match_with_alternative_patterns() {
            let source = r#"
x = match y
  0 or 1 then 42
  z then -1
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(42),
                    Id(constant(2)), // 5
                    SmallInt(-1),
                    Match {
                        expression: 1,
                        arms: vec![
                            MatchArm {
                                patterns: vec![2, 3],
                                condition: None,
                                expression: 4,
                            },
                            MatchArm {
                                patterns: vec![5],
                                condition: None,
                                expression: 6,
                            },
                        ],
                    },
                    Assign {
                        target: 0,
                        expression: 7,
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("z")]),
            )
        }

        #[test]
        fn match_string_literals() {
            let source = r#"
match x
  'foo' then 99
  "bar" or "baz" then break
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    string_literal(1, QuotationMark::Single),
                    SmallInt(99),
                    string_literal(2, QuotationMark::Double),
                    string_literal(3, QuotationMark::Double),
                    Break(None), // 5
                    Match {
                        expression: 0,
                        arms: vec![
                            MatchArm {
                                patterns: vec![1],
                                condition: None,
                                expression: 2,
                            },
                            MatchArm {
                                patterns: vec![3, 4],
                                condition: None,
                                expression: 5,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![6],
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("foo"),
                    Constant::Str("bar"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn match_tuple() {
            let source = r#"
match (x, y, z)
  (0, a, _) then a
  (_, (0, b), _foo) then 0
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    Id(constant(2)),
                    Tuple(vec![0, 1, 2]),
                    SmallInt(0),
                    Id(constant(3)), // 5
                    Wildcard(None),
                    Tuple(vec![4, 5, 6]),
                    Id(constant(3)),
                    Wildcard(None),
                    SmallInt(0), // 10
                    Id(constant(4)),
                    Tuple(vec![10, 11]),
                    Wildcard(Some(constant(5))),
                    Tuple(vec![9, 12, 13]),
                    SmallInt(0), // 15
                    Match {
                        expression: 3,
                        arms: vec![
                            MatchArm {
                                patterns: vec![7],
                                condition: None,
                                expression: 8,
                            },
                            MatchArm {
                                patterns: vec![14],
                                condition: None,
                                expression: 15,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![16],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Str("a"),
                    Constant::Str("b"),
                    Constant::Str("foo"),
                ]),
            )
        }

        #[test]
        fn match_tuple_subslice() {
            let source = r#"
match x
  (..., 0) then 0
  (1, ...) then 1
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Ellipsis(None),
                    SmallInt(0),
                    Tuple(vec![1, 2]),
                    SmallInt(0),
                    SmallInt(1), // 5
                    Ellipsis(None),
                    Tuple(vec![5, 6]),
                    SmallInt(1),
                    Match {
                        expression: 0,
                        arms: vec![
                            MatchArm {
                                patterns: vec![3],
                                condition: None,
                                expression: 4,
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
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn match_tuple_subslice_with_id() {
            let source = r#"
match y
  (rest..., 0, 1) then 0
  (1, 0, others...) then 1
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Ellipsis(Some(constant(1))),
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(vec![1, 2, 3]),
                    SmallInt(0), // 5
                    SmallInt(1),
                    SmallInt(0),
                    Ellipsis(Some(constant(2))),
                    Tuple(vec![6, 7, 8]),
                    SmallInt(1), // 10
                    Match {
                        expression: 0,
                        arms: vec![
                            MatchArm {
                                patterns: vec![4],
                                condition: None,
                                expression: 5,
                            },
                            MatchArm {
                                patterns: vec![9],
                                condition: None,
                                expression: 10,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![11],
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("y"),
                    Constant::Str("rest"),
                    Constant::Str("others"),
                ]),
            )
        }

        #[test]
        fn match_with_conditions_and_block() {
            let source = r#"
match x
  z if z > 5 then 0
  z if z < 10 then
    1
  z then
    -1
"#;
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    Id(constant(1)),
                    SmallInt(5),
                    BinaryOp {
                        op: AstBinaryOp::Greater,
                        lhs: 2,
                        rhs: 3,
                    },
                    SmallInt(0), // 5
                    Id(constant(1)),
                    Id(constant(1)),
                    SmallInt(10),
                    BinaryOp {
                        op: AstBinaryOp::Less,
                        lhs: 7,
                        rhs: 8,
                    },
                    SmallInt(1), // 10
                    Id(constant(1)),
                    SmallInt(-1),
                    Match {
                        expression: 0,
                        arms: vec![
                            MatchArm {
                                patterns: vec![1],
                                condition: Some(4),
                                expression: 5,
                            },
                            MatchArm {
                                patterns: vec![6],
                                condition: Some(9),
                                expression: 10,
                            },
                            MatchArm {
                                patterns: vec![11],
                                condition: None,
                                expression: 12,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![13],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("z")]),
            )
        }

        #[test]
        fn match_multi_expression() {
            let source = "
match x, y
  0, 1 or 2, 3 if z then 0
  a, () then
    a
  else 0
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    TempTuple(vec![0, 1]),
                    SmallInt(0),
                    SmallInt(1),
                    TempTuple(vec![3, 4]), // 5
                    SmallInt(2),
                    SmallInt(3),
                    TempTuple(vec![6, 7]),
                    Id(constant(2)),
                    SmallInt(0), // 10
                    Id(constant(3)),
                    Null,
                    TempTuple(vec![11, 12]),
                    Id(constant(3)),
                    SmallInt(0), // 15
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
                                patterns: vec![],
                                condition: None,
                                expression: 15,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![16],
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Str("a"),
                ]),
            )
        }

        #[test]
        fn match_expression_is_lookup_call() {
            let source = "
match x.foo 42
  null then 0
  else 1
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(42),
                    Lookup((
                        LookupNode::Call {
                            args: vec![1],
                            with_parens: false,
                        },
                        None,
                    )),
                    Lookup((LookupNode::Id(constant(1)), Some(2))),
                    Lookup((LookupNode::Root(0), Some(3))),
                    Null, // 5
                    SmallInt(0),
                    SmallInt(1),
                    Match {
                        expression: 4,
                        arms: vec![
                            MatchArm {
                                patterns: vec![5],
                                condition: None,
                                expression: 6,
                            },
                            MatchArm {
                                patterns: vec![],
                                condition: None,
                                expression: 7,
                            },
                        ],
                    },
                    MainBlock {
                        body: vec![8],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn match_pattern_is_lookup() {
            let source = "
match x
  y.foo then 0
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    Id(constant(1)),
                    Lookup((LookupNode::Id(constant(2)), None)),
                    Lookup((LookupNode::Root(1), Some(2))),
                    SmallInt(0),
                    Match {
                        expression: 0,
                        arms: vec![MatchArm {
                            patterns: vec![3],
                            condition: None,
                            expression: 4,
                        }],
                    },
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn match_arm_is_throw_expression() {
            let source = "
match x
  0 then 1
  else throw 'nope'
";
            check_ast(
                source,
                &[
                    Id(constant(0)),
                    SmallInt(0),
                    SmallInt(1),
                    string_literal(1, QuotationMark::Single),
                    Throw(3),
                    Match {
                        expression: 0,
                        arms: vec![
                            MatchArm {
                                patterns: vec![1],
                                condition: None,
                                expression: 2,
                            },
                            MatchArm {
                                patterns: vec![],
                                condition: None,
                                expression: 4,
                            },
                        ],
                    }, // 5
                    MainBlock {
                        body: vec![5],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("nope")]),
            )
        }

        #[test]
        fn switch_expression() {
            let source = "
switch
  1 == 0 then 0
  a > b then 1
  else a
";
            check_ast(
                source,
                &[
                    SmallInt(1),
                    SmallInt(0),
                    BinaryOp {
                        op: AstBinaryOp::Equal,
                        lhs: 0,
                        rhs: 1,
                    },
                    SmallInt(0),
                    Id(constant(0)),
                    Id(constant(1)), // 5
                    BinaryOp {
                        op: AstBinaryOp::Greater,
                        lhs: 4,
                        rhs: 5,
                    },
                    SmallInt(1),
                    Id(constant(0)),
                    Switch(vec![
                        SwitchArm {
                            condition: Some(2),
                            expression: 3,
                        },
                        SwitchArm {
                            condition: Some(6),
                            expression: 7,
                        },
                        SwitchArm {
                            condition: None,
                            expression: 8,
                        },
                    ]),
                    MainBlock {
                        body: vec![9],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("b")]),
            )
        }

        #[test]
        fn switch_arm_is_debug_expression() {
            let source = "
switch
  true then 1
  else debug x
";
            check_ast(
                source,
                &[
                    BoolTrue,
                    SmallInt(1),
                    Id(constant(0)),
                    Debug {
                        expression_string: constant(0),
                        expression: 2,
                    },
                    Switch(vec![
                        SwitchArm {
                            condition: Some(0),
                            expression: 1,
                        },
                        SwitchArm {
                            condition: None,
                            expression: 3,
                        },
                    ]),
                    MainBlock {
                        body: vec![4],
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }
}
