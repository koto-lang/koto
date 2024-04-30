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
            Err(error) => panic!("{error} - {:?}", error.span.start),
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

    fn simple_string(literal_index: u32, quotation_mark: StringQuote) -> AstString {
        AstString {
            quote: quotation_mark,
            contents: StringContents::Literal(literal_index.into()),
        }
    }

    fn id(constant: u32) -> Node {
        Node::Id(constant.into(), None)
    }

    fn int(constant: u32) -> Node {
        Node::Int(constant.into())
    }

    fn float(constant: u32) -> Node {
        Node::Float(constant.into())
    }

    fn string_literal(literal_index: u32, quotation_mark: StringQuote) -> Node {
        Node::Str(simple_string(literal_index, quotation_mark))
    }

    fn expressions(ast_indices: &[u32]) -> Vec<AstIndex> {
        ast_indices.iter().map(|i| AstIndex::from(*i)).collect()
    }

    fn unary_op(op: AstUnaryOp, value: u32) -> Node {
        Node::UnaryOp {
            op,
            value: value.into(),
        }
    }

    fn binary_op(op: AstBinaryOp, lhs: u32, rhs: u32) -> Node {
        Node::BinaryOp {
            op,
            lhs: lhs.into(),
            rhs: rhs.into(),
        }
    }

    fn assign(target: u32, expression: u32) -> Node {
        Node::Assign {
            target: target.into(),
            expression: expression.into(),
        }
    }

    fn string_entry(
        literal_index: u32,
        quotation_mark: StringQuote,
        value: Option<u32>,
    ) -> (MapKey, Option<AstIndex>) {
        (
            MapKey::Str(simple_string(literal_index, quotation_mark)),
            value.map(AstIndex::from),
        )
    }

    fn id_entry(key: u32, value: Option<u32>) -> (MapKey, Option<AstIndex>) {
        (MapKey::Id(key.into()), value.map(AstIndex::from))
    }

    fn range(start: u32, end: u32, inclusive: bool) -> Node {
        Node::Range {
            start: start.into(),
            end: end.into(),
            inclusive,
        }
    }

    fn lookup_id(id: u32, next: Option<u32>) -> Node {
        Lookup((LookupNode::Id(id.into()), next.map(AstIndex::from)))
    }

    fn lookup_index(index: u32, next: Option<u32>) -> Node {
        Lookup((LookupNode::Index(index.into()), next.map(AstIndex::from)))
    }

    fn lookup_root(index: u32, next: Option<u32>) -> Node {
        Lookup((LookupNode::Root(index.into()), next.map(AstIndex::from)))
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
                    float(0),
                    string_literal(1, StringQuote::Double),
                    string_literal(2, StringQuote::Single),
                    id(3),
                    Null,
                    MainBlock {
                        body: expressions(&[0, 1, 2, 3, 4, 5, 6, 7]),
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
                    int(0),
                    int(1),
                    SmallInt(1),
                    SmallInt(64),
                    SmallInt(1),
                    SmallInt(4),
                    MainBlock {
                        body: expressions(&[0, 1, 2, 3, 4, 5, 6, 7]),
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
                    string_literal(0, StringQuote::Double),
                    string_literal(1, StringQuote::Double),
                    MainBlock {
                        body: expressions(&[0, 1]),
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
                    string_literal(0, StringQuote::Double),
                    string_literal(1, StringQuote::Single),
                    MainBlock {
                        body: expressions(&[0, 1]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("\t\nM."), Constant::Str("🤗🌍")]),
            )
        }

        #[test]
        fn strings_with_interpolated_ids() {
            let source = r#"
'Hello, {name}!'
"{foo}"
'{x} {y}'
"#;
            check_ast(
                source,
                &[
                    id(1),
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Interpolated(vec![
                            StringNode::Literal(0.into()),
                            StringNode::Expression {
                                expression: 0.into(),
                                format: StringFormatOptions::default(),
                            },
                            StringNode::Literal(2.into()),
                        ]),
                    }),
                    id(3),
                    Str(AstString {
                        quote: StringQuote::Double,
                        contents: StringContents::Interpolated(vec![StringNode::Expression {
                            expression: 2.into(),
                            format: StringFormatOptions::default(),
                        }]),
                    }),
                    id(4),
                    id(6), // 5
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Interpolated(vec![
                            StringNode::Expression {
                                expression: 4.into(),
                                format: StringFormatOptions::default(),
                            },
                            StringNode::Literal(5.into()),
                            StringNode::Expression {
                                expression: 5.into(),
                                format: StringFormatOptions::default(),
                            },
                        ]),
                    }),
                    MainBlock {
                        body: expressions(&[1, 3, 6]),
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
'{123 + 456}!'
";
            check_ast(
                source,
                &[
                    SmallInt(123),
                    int(0),
                    binary_op(AstBinaryOp::Add, 0, 1),
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Interpolated(vec![
                            StringNode::Expression {
                                expression: 2.into(),
                                format: StringFormatOptions::default(),
                            },
                            StringNode::Literal(1.into()),
                        ]),
                    }),
                    MainBlock {
                        body: expressions(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::I64(456), Constant::Str("!")]),
            )
        }

        #[test]
        fn string_with_formatted_expression() {
            let source = "
'!{a:_>3.2}!'
";
            check_ast(
                source,
                &[
                    id(1),
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Interpolated(vec![
                            StringNode::Literal(0.into()),
                            StringNode::Expression {
                                expression: 0.into(),
                                format: StringFormatOptions {
                                    alignment: StringAlignment::Right,
                                    min_width: Some(3),
                                    precision: Some(2),
                                    fill_character: Some(2.into()),
                                },
                            },
                            StringNode::Literal(0.into()),
                        ]),
                    }),
                    MainBlock {
                        body: expressions(&[1]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("!"), Constant::Str("a"), Constant::Str("_")]),
            )
        }
        #[test]
        fn raw_strings() {
            let source = r###"
r'$foo ${bar}'
r"[\r?\n]\"
r#''$foo''#
r##'#$bar'##
"###;

            check_ast(
                source,
                &[
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Raw {
                            constant: 0.into(),
                            hash_count: 0,
                        },
                    }),
                    Str(AstString {
                        quote: StringQuote::Double,
                        contents: StringContents::Raw {
                            constant: 1.into(),
                            hash_count: 0,
                        },
                    }),
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Raw {
                            constant: 2.into(),
                            hash_count: 1,
                        },
                    }),
                    Str(AstString {
                        quote: StringQuote::Single,
                        contents: StringContents::Raw {
                            constant: 3.into(),
                            hash_count: 2,
                        },
                    }),
                    MainBlock {
                        body: expressions(&[0, 1, 2, 3]),
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("$foo ${bar}"),
                    Constant::Str(r"[\r?\n]\"),
                    Constant::Str("'$foo'"),
                    Constant::Str("#$bar"),
                ]),
            )
        }

        #[test]
        fn negatives() {
            let source = "
-12.0
-a
-x[0]
-(1 + 1)";
            check_ast(
                source,
                &[
                    float(0),
                    id(1),
                    unary_op(AstUnaryOp::Negate, 1),
                    id(2),
                    SmallInt(0),
                    lookup_index(4, None), // 5
                    lookup_root(3, Some(5)),
                    unary_op(AstUnaryOp::Negate, 6),
                    SmallInt(1),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Add, 8, 9), // 10
                    Nested(10.into()),
                    unary_op(AstUnaryOp::Negate, 11),
                    MainBlock {
                        body: expressions(&[0, 2, 7, 12]),
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
                    id(0),
                    string_literal(1, StringQuote::Double),
                    id(0),
                    SmallInt(-1),
                    List(expressions(&[0, 1, 2, 3, 4])),
                    List(expressions(&[])),
                    MainBlock {
                        body: expressions(&[5, 6]),
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
                    List(expressions(&[1, 2])),
                    SmallInt(2),
                    List(expressions(&[0, 3, 4])), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0), // 5
                    List(expressions(&[1, 2, 3, 4, 5])),
                    assign(0, 6),
                    MainBlock {
                        body: expressions(&[7]),
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
                    id(0),
                    SmallInt(42),
                    string_literal(4, StringQuote::Single),
                    SmallInt(99),
                    Map(vec![
                        string_entry(1, StringQuote::Single, Some(2)),
                        id_entry(2, None),
                        id_entry(3, Some(3)),
                        (MapKey::Meta(MetaKeyId::Add, None), Some(4.into())),
                    ]), // 5
                    assign(1, 5),
                    MainBlock {
                        body: expressions(&[0, 6]),
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
                    string_literal(3, StringQuote::Double),
                    Map(vec![
                        string_entry(0, StringQuote::Single, Some(0)),
                        id_entry(1, None),
                        id_entry(2, Some(1)),
                    ]),
                    MainBlock {
                        body: expressions(&[2]),
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
                    id(0), // x
                    SmallInt(42),
                    SmallInt(0),
                    Map(vec![id_entry(1, Some(2))]), // foo, 0
                    SmallInt(-1),
                    Map(vec![
                        id_entry(1, Some(1)),                                      // foo: 42
                        string_entry(2, StringQuote::Double, Some(3)), // "baz": nested map
                        (MapKey::Meta(MetaKeyId::Subtract, None), Some(4.into())), // @-: -1
                    ]), // 5
                    assign(0, 5),
                    id(0),
                    MainBlock {
                        body: expressions(&[6, 7]),
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
                    id(0), // x
                    SmallInt(42),
                    Map(vec![string_entry(1, StringQuote::Double, Some(1))]), // "foo": 42
                    assign(0, 2),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0), // x
                    SmallInt(42),
                    Map(vec![
                        id_entry(2, Some(1)), // bar: 42
                    ]),
                    Map(vec![
                        id_entry(1, Some(2)), // foo: ...
                    ]),
                    assign(0, 3),
                    MainBlock {
                        body: expressions(&[4]),
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
        fn map_block_first_entry_is_comma_separated_tuple() {
            let sources = [
                "
x =
    foo: 10, 20, 30
",
                "
x =
    foo: 10,
         20,
         30,
",
                "
x =
    foo:
      10, 20, 30,
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(0), // x
                    SmallInt(10),
                    SmallInt(20),
                    SmallInt(30),
                    Tuple(expressions(&[1, 2, 3])),
                    Map(vec![id_entry(1, Some(4))]), // 5 - foo: 10, 20, 30
                    assign(0, 5),
                    MainBlock {
                        body: expressions(&[6]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn map_block_second_entry_is_paren_free_call() {
            let sources = [
                "
x =
  foo: 1
  bar: baz 42
",
                "
x =
  foo: 1
  bar:
    baz 42
",
                "
x =
  foo: 1
  bar: baz
    42
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(0), // x
                    SmallInt(1),
                    SmallInt(42),
                    NamedCall {
                        id: 3.into(),
                        args: expressions(&[2]),
                    },
                    Map(vec![
                        id_entry(1, Some(1)), // foo: 1
                        id_entry(2, Some(3)), // bar: baz 42
                    ]),
                    assign(0, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
                        local_count: 1,
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
                    id(0), // x
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(0),
                    Map(vec![
                        (MapKey::Meta(MetaKeyId::Add, None), Some(1.into())),
                        (MapKey::Meta(MetaKeyId::Subtract, None), Some(2.into())),
                        (
                            MapKey::Meta(MetaKeyId::Named, Some(1.into())),
                            Some(3.into()),
                        ),
                    ]),
                    assign(0, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                        (MapKey::Meta(MetaKeyId::PreTest, None), Some(1.into())),
                        (MapKey::Meta(MetaKeyId::PostTest, None), Some(2.into())),
                        (
                            MapKey::Meta(MetaKeyId::Test, Some(0.into())),
                            Some(3.into()),
                        ),
                    ]),
                    assign(0, 4), // 5
                    Export(5.into()),
                    MainBlock {
                        body: expressions(&[6]),
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
                    range(0, 1, false),
                    SmallInt(0),
                    SmallInt(1),
                    range(3, 4, true), // 5
                    MainBlock {
                        body: expressions(&[2, 5]),
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
                    binary_op(AstBinaryOp::Add, 0, 1),
                    SmallInt(1),
                    SmallInt(0),
                    binary_op(AstBinaryOp::Add, 3, 4), // 5
                    range(2, 5, false),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    SmallInt(0),
                    assign(0, 1),
                    id(1),
                    SmallInt(10),
                    assign(3, 4), // 5
                    id(0),
                    id(1),
                    range(6, 7, false),
                    MainBlock {
                        body: expressions(&[2, 5, 8]),
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
                    id(0),
                    lookup_id(1, None),
                    lookup_root(0, Some(1)),
                    id(0),
                    lookup_id(2, None),
                    lookup_root(3, Some(4)), // 5
                    range(2, 5, false),
                    MainBlock {
                        body: expressions(&[6]),
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
                    range(0, 1, false),
                    List(expressions(&[2])),
                    SmallInt(0),
                    SmallInt(10), // 5
                    range(4, 5, false),
                    SmallInt(10),
                    SmallInt(0),
                    range(7, 8, true),
                    List(expressions(&[6, 9])),
                    MainBlock {
                        body: expressions(&[3, 10]),
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
                    Tuple(expressions(&[0, 1, 2])),
                    MainBlock {
                        body: expressions(&[3]),
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
                    Tuple(expressions(&[])),
                    MainBlock {
                        body: expressions(&[0]),
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
                        body: expressions(&[0]),
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
                    Tuple(expressions(&[0, 1, 2])),
                    MainBlock {
                        body: expressions(&[3]),
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
                    Tuple(expressions(&[0])),
                    MainBlock {
                        body: expressions(&[1]),
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
                    id(0),
                    SmallInt(1),
                    assign(0, 1),
                    MainBlock {
                        body: expressions(&[2]),
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
                    id(0),
                    SmallInt(1),
                    SmallInt(0),
                    Tuple(expressions(&[1, 2])),
                    assign(0, 3),
                    MainBlock {
                        body: expressions(&[4]),
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
                    id(0),
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(expressions(&[1, 2])),
                    SmallInt(2),
                    SmallInt(3), // 5
                    Tuple(expressions(&[4, 5])),
                    Tuple(expressions(&[3, 6])),
                    assign(0, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    id(1),
                    SmallInt(0),
                    lookup_index(2, None),
                    lookup_root(1, Some(3)),
                    SmallInt(1), // 5
                    SmallInt(0),
                    TempTuple(expressions(&[5, 6])),
                    MultiAssign {
                        targets: expressions(&[0, 4]),
                        expression: 7.into(),
                    },
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    id(1),
                    SmallInt(1),
                    SmallInt(0),
                    TempTuple(expressions(&[2, 3])),
                    MultiAssign {
                        targets: expressions(&[0, 1]),
                        expression: 4.into(),
                    }, // 5
                    id(0),
                    MainBlock {
                        body: expressions(&[5, 6]),
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
                    id(0),
                    Wildcard(None),
                    Wildcard(Some(1.into())),
                    id(2),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_root(3, Some(4)), // 5
                    MultiAssign {
                        targets: expressions(&[0, 1, 2]),
                        expression: 5.into(),
                    },
                    MainBlock {
                        body: expressions(&[6]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("f")]),
            )
        }

        #[test]
        fn compound_assignment() {
            let source = "\
x += 0
x -= 1
x *= 2
x /= 3
x %= 4";
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(0),
                    binary_op(AstBinaryOp::AddAssign, 0, 1),
                    id(0),
                    SmallInt(1),
                    binary_op(AstBinaryOp::SubtractAssign, 3, 4), // 5
                    id(0),
                    SmallInt(2),
                    binary_op(AstBinaryOp::MultiplyAssign, 6, 7),
                    id(0),
                    SmallInt(3), // 10
                    binary_op(AstBinaryOp::DivideAssign, 9, 10),
                    id(0),
                    SmallInt(4),
                    binary_op(AstBinaryOp::RemainderAssign, 12, 13),
                    MainBlock {
                        body: expressions(&[2, 5, 8, 11, 14]),
                        local_count: 0,
                    }, // 15
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn list_with_lookup_as_first_element() {
            let source = "
[foo.bar()]
";
            check_ast(
                source,
                &[
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(1, Some(1)),
                    lookup_root(0, Some(2)),
                    List(expressions(&[3])),
                    MainBlock {
                        body: expressions(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("bar")]),
            )
        }
    }

    mod variable_declaration {
        use super::*;

        #[test]
        fn single() {
            let source = "let a = 1";

            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    Assign {
                        target: 0.into(),
                        expression: 1.into(),
                    },
                    MainBlock {
                        body: vec![2.into()],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn single_with_type_hint() {
            let source = "let a: Number = 1";

            check_ast(
                source,
                &[
                    Id(
                        0.into(),
                        Some(TypeHint {
                            type_name: 1.into(),
                            inner: vec![],
                        }),
                    ),
                    SmallInt(1),
                    Assign {
                        target: 0.into(),
                        expression: 1.into(),
                    },
                    MainBlock {
                        body: vec![2.into()],
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("Number")]),
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
                    id(0),
                    SmallInt(1),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Add, 1, 2),
                    assign(0, 3),
                    Export(4.into()), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                        id_entry(0, Some(0)), // a: 123
                        id_entry(1, Some(1)), // b: 99
                    ]),
                    Export(2.into()),
                    MainBlock {
                        body: expressions(&[3]),
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
                    binary_op(AstBinaryOp::Subtract, 0, 1),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Add, 2, 3),
                    MainBlock {
                        body: expressions(&[4]),
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
                    binary_op(AstBinaryOp::Multiply, 1, 2),
                    binary_op(AstBinaryOp::Add, 0, 3),
                    SmallInt(0), // 5
                    binary_op(AstBinaryOp::Add, 4, 5),
                    MainBlock {
                        body: expressions(&[6]),
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
                    binary_op(AstBinaryOp::Add, 0, 1),
                    Nested(2.into()),
                    SmallInt(1),
                    SmallInt(0), // 5
                    binary_op(AstBinaryOp::Add, 4, 5),
                    Nested(6.into()),
                    binary_op(AstBinaryOp::Multiply, 3, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    binary_op(AstBinaryOp::Divide, 0, 1),
                    SmallInt(4),
                    binary_op(AstBinaryOp::Remainder, 2, 3),
                    MainBlock {
                        body: expressions(&[4]),
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
                    string_literal(0, StringQuote::Single),
                    id(1),
                    binary_op(AstBinaryOp::Add, 0, 1),
                    MainBlock {
                        body: expressions(&[2]),
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
                    id(0), // x
                    SmallInt(1),
                    id(2), // y
                    NamedCall {
                        id: 1.into(), // f
                        args: expressions(&[2]),
                    },
                    binary_op(AstBinaryOp::Add, 1, 3),
                    assign(0, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("f"), Constant::Str("y")]),
            )
        }

        #[test]
        fn arithmetic_assignment_chained() {
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
                    id(0),
                    SmallInt(1),
                    SmallInt(2),
                    SmallInt(3),
                    binary_op(AstBinaryOp::Multiply, 2, 3),
                    binary_op(AstBinaryOp::Add, 1, 4), // 5
                    assign(0, 5),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    SmallInt(1),
                    SmallInt(2),
                    binary_op(AstBinaryOp::Add, 1, 2),
                    Nested(3.into()),
                    SmallInt(3), // 5
                    binary_op(AstBinaryOp::Multiply, 4, 5),
                    assign(0, 6),
                    MainBlock {
                        body: expressions(&[7]),
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
                    binary_op(AstBinaryOp::Less, 0, 1),
                    SmallInt(1),
                    SmallInt(0),
                    binary_op(AstBinaryOp::Greater, 3, 4),
                    binary_op(AstBinaryOp::And, 2, 5),
                    BoolTrue,
                    binary_op(AstBinaryOp::Or, 6, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    binary_op(AstBinaryOp::LessOrEqual, 1, 2),
                    binary_op(AstBinaryOp::Less, 0, 3),
                    MainBlock {
                        body: expressions(&[4]),
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
                        condition: 1.into(),
                        then_node: 2.into(),
                        else_if_blocks: vec![],
                        else_node: Some(3.into()),
                    }),
                    binary_op(AstBinaryOp::Add, 0, 4),
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    BoolFalse,
                    SmallInt(0),
                    BoolTrue,
                    SmallInt(1),
                    BoolFalse, // 5
                    SmallInt(0),
                    SmallInt(1),
                    If(AstIf {
                        condition: 1.into(),
                        then_node: 2.into(),
                        else_if_blocks: vec![(3.into(), 4.into()), (5.into(), 6.into())],
                        else_node: Some(7.into()),
                    }),
                    assign(0, 8),
                    id(0),
                    MainBlock {
                        body: expressions(&[9, 10]),
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
                    id(0),
                    id(1),
                    BoolTrue,
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(expressions(&[3, 4])), // 5
                    SmallInt(1),
                    SmallInt(0),
                    Tuple(expressions(&[6, 7])),
                    If(AstIf {
                        condition: 2.into(),
                        then_node: 5.into(),
                        else_if_blocks: vec![],
                        else_node: Some(8.into()),
                    }),
                    MultiAssign {
                        targets: expressions(&[0, 1]),
                        expression: 9.into(),
                    }, // 10
                    MainBlock {
                        body: expressions(&[10]),
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
                        condition: 0.into(),
                        then_node: 1.into(),
                        else_if_blocks: vec![],
                        else_node: None,
                    }),
                    id(0),
                    Block(expressions(&[2, 3])),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![0.into()],
                        body: 4.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0), // x
                    Wildcard(None),
                    Wildcard(Some(1.into())), // _y
                    id(2),                    // z
                    id(3),                    // foo
                    id(0),                    // x - 5
                    NamedCall {
                        id: 2.into(), // z
                        args: expressions(&[5]),
                    },
                    For(AstFor {
                        args: expressions(&[0, 1, 2, 3]),
                        iterable: 4.into(),
                        body: 6.into(),
                    }),
                    MainBlock {
                        body: expressions(&[7]),
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
                    id(0), // x
                    id(1), // y
                    binary_op(AstBinaryOp::Greater, 0, 1),
                    id(0), // x
                    NamedCall {
                        id: 2.into(), // f
                        args: expressions(&[3]),
                    },
                    While {
                        condition: 2.into(),
                        body: 4.into(),
                    }, // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0), // x
                    id(1), // y
                    binary_op(AstBinaryOp::Less, 0, 1),
                    id(1), // y
                    NamedCall {
                        id: 2.into(), // f
                        args: expressions(&[3]),
                    },
                    Until {
                        condition: 2.into(),
                        body: 4.into(),
                    }, // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    List(expressions(&[])),
                    id(0), // x
                    id(1), // y
                    id(0), // x
                    For(AstFor {
                        args: expressions(&[1]),
                        iterable: 2.into(),
                        body: 3.into(),
                    }),
                    MainBlock {
                        body: expressions(&[0, 4]),
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
                    id(0), // a
                    id(1), // x
                    id(3), // y
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[2]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(2, Some(3)),
                    lookup_root(1, Some(4)), // ast 5
                    id(0),                   // a
                    For(AstFor {
                        args: expressions(&[0]),
                        iterable: 5.into(),
                        body: 6.into(),
                    }),
                    MainBlock {
                        body: expressions(&[7]),
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
                    id(0),
                    SmallInt(42),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 1.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    assign(0, 2),
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )), // 5
                    lookup_root(4, Some(5)),
                    MainBlock {
                        body: expressions(&[3, 6]),
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
                    id(0),
                    id(1),
                    id(0),
                    id(1),
                    binary_op(AstBinaryOp::Add, 2, 3),
                    Function(koto_parser::Function {
                        args: expressions(&[0, 1]),
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 4.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    id(1),
                    id(0),
                    id(1),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(2, Some(4)), // 5
                    lookup_root(3, Some(5)),
                    binary_op(AstBinaryOp::Add, 2, 6),
                    Function(koto_parser::Function {
                        args: expressions(&[0, 1]),
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 7.into(),
                        is_variadic: true,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0), // f
                    id(1), // x
                    id(2), // y
                    id(1), // x
                    assign(2, 3),
                    id(2), // 5
                    Block(expressions(&[4, 5])),
                    Function(koto_parser::Function {
                        args: expressions(&[1]),
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 6.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    assign(0, 7),
                    SmallInt(42),
                    NamedCall {
                        id: 0.into(),
                        args: expressions(&[9]),
                    }, // 10
                    MainBlock {
                        body: expressions(&[8, 10]),
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
                    id(0), // f
                    id(1), // x
                    id(2), // y
                    id(3), // z
                    id(3), // z
                    Function(koto_parser::Function {
                        args: expressions(&[3]),
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 4.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    assign(2, 5),
                    id(1), // x
                    NamedCall {
                        id: 2.into(), // y
                        args: expressions(&[7]),
                    },
                    Block(expressions(&[6, 8])),
                    Function(koto_parser::Function {
                        args: expressions(&[1]),
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 9.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 10
                    assign(0, 10),
                    SmallInt(42),
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[12]),
                    },
                    MainBlock {
                        body: expressions(&[11, 13]),
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
                    id(1),
                    id(1),
                    unary_op(AstUnaryOp::Negate, 1),
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[0, 2]),
                    },
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(1),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Subtract, 0, 1),
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[2]),
                    },
                    MainBlock {
                        body: expressions(&[3]),
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
f(x,-x)
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
                    id(0),
                    id(1),
                    id(1),
                    unary_op(AstUnaryOp::Negate, 2),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1, 3]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_root(0, Some(4)),
                    MainBlock {
                        body: expressions(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_without_parentheses() {
            let sources = [
                "
foo x, y
",
                "
foo x,y
",
                "
foo
  x,
  y
",
                "
foo x,
    y
",
            ];

            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(1),
                    id(2),
                    NamedCall {
                        id: 0.into(), // foo
                        args: expressions(&[0, 1]),
                    },
                    MainBlock {
                        body: expressions(&[2]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn call_with_indented_function_arg() {
            let source = "
foo
  x,
  |y| y";
            check_ast(
                source,
                &[
                    id(1),
                    id(2),
                    id(2),
                    Function(koto_parser::Function {
                        args: expressions(&[1]),
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 2.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    NamedCall {
                        id: 0.into(), // foo
                        args: expressions(&[0, 3]),
                    },
                    MainBlock {
                        body: expressions(&[4]),
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
                    id(1),
                    NamedCall {
                        id: 0.into(),
                        args: expressions(&[0]),
                    },
                    id(1),
                    NamedCall {
                        id: 0.into(),
                        args: expressions(&[2]),
                    }, // 5
                    MainBlock {
                        body: expressions(&[1, 3]),
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
                    id(0), // f
                    id(1), // x
                    id(1), // x
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[2]),
                    },
                    Function(koto_parser::Function {
                        args: expressions(&[1]),
                        local_count: 1,
                        accessed_non_locals: vec![0.into()],
                        body: 3.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    assign(0, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0), // f
                    id(1), // g
                    id(2), // x
                    id(2),
                    NamedCall {
                        id: 0.into(),
                        args: expressions(&[3]),
                    },
                    Function(koto_parser::Function {
                        args: expressions(&[2]),
                        local_count: 1,
                        accessed_non_locals: vec![0.into()],
                        body: 4.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    Nested(5.into()),
                    id(2), // x
                    id(2), // x
                    NamedCall {
                        id: 1.into(), // g
                        args: expressions(&[8]),
                    },
                    Function(koto_parser::Function {
                        args: expressions(&[7]),
                        local_count: 1,
                        accessed_non_locals: vec![1.into()],
                        body: 9.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 10
                    Nested(10.into()),
                    TempTuple(expressions(&[6, 11])),
                    MultiAssign {
                        targets: expressions(&[0, 1]),
                        expression: 12.into(),
                    },
                    MainBlock {
                        body: expressions(&[13]),
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
                    id(1), // x
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[0]),
                    },
                    id(2), // g
                    binary_op(AstBinaryOp::Pipe, 1, 2),
                    id(3),                              // h
                    binary_op(AstBinaryOp::Pipe, 3, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0), // foo
                    id(2), // x
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(2)),
                    lookup_root(0, Some(3)),
                    id(3), // 5 - y
                    binary_op(AstBinaryOp::Pipe, 4, 5),
                    id(4), // z
                    binary_op(AstBinaryOp::Pipe, 6, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(2), // x
                    Self_, // self
                    lookup_id(0, None),
                    lookup_root(2, Some(3)),
                    id(2), // 5
                    assign(4, 5),
                    Function(koto_parser::Function {
                        args: expressions(&[1]),
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 6.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    Map(vec![id_entry(0, Some(0)), id_entry(1, Some(7))]),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    id(2),
                    SmallInt(0),
                    Map(vec![id_entry(1, Some(1)), id_entry(3, Some(2))]),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![2.into()],
                        body: 3.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    assign(0, 4),
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0), // f
                    id(3), // x
                    Map(vec![
                        id_entry(2, Some(1)), // bar: x
                    ]),
                    SmallInt(0),
                    Map(vec![
                        id_entry(1, Some(2)), // foo: ...
                        id_entry(4, Some(3)), // baz: 0
                    ]),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![3.into()],
                        body: 4.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 5
                    assign(0, 5),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    SmallInt(42),
                    id(3), // x
                    Self_,
                    lookup_id(1, None),
                    lookup_root(3, Some(4)), // 5
                    id(3),
                    assign(5, 6),
                    Function(koto_parser::Function {
                        args: expressions(&[2]),
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 7.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // 10
                    Map(vec![id_entry(1, Some(1)), id_entry(2, Some(8))]),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 9.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    assign(0, 10),
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )), // 15
                    lookup_root(12, Some(13)),
                    MainBlock {
                        body: expressions(&[11, 14]),
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
                    id(0), // f
                    id(1), // n
                    id(2), // f2
                    id(1),
                    id(3),       // i
                    SmallInt(0), // ast 5
                    SmallInt(1),
                    range(5, 6, false),
                    id(3), // i
                    id(1),
                    binary_op(AstBinaryOp::Equal, 8, 9), // ast 10
                    id(3),
                    Return(Some(11.into())),
                    If(AstIf {
                        condition: 10.into(),
                        then_node: 12.into(),
                        else_if_blocks: vec![],
                        else_node: None,
                    }),
                    For(AstFor {
                        args: expressions(&[4]),
                        iterable: 7.into(),
                        body: 13.into(),
                    }),
                    Function(koto_parser::Function {
                        args: expressions(&[3]),
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 14.into(),
                        is_variadic: false,
                        is_generator: false,
                    }), // ast 15
                    assign(2, 15),
                    id(2),
                    Block(expressions(&[16, 17])),
                    Function(koto_parser::Function {
                        args: expressions(&[1]),
                        local_count: 2,
                        accessed_non_locals: vec![],
                        body: 18.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    assign(0, 19), // ast 20
                    MainBlock {
                        body: expressions(&[20]),
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
                    id(0),
                    id(0),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Add, 1, 2),
                    assign(0, 3),
                    id(0), // 5
                    Block(expressions(&[4, 5])),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 1,
                        accessed_non_locals: vec![0.into()], // initial read of x via capture
                        body: 6.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: expressions(&[7]),
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
                    id(0),
                    id(1),
                    SmallInt(1),
                    assign(1, 2),
                    Nested(3.into()),
                    id(1), // 5
                    Tuple(expressions(&[4, 5])),
                    assign(0, 6),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 2,
                        accessed_non_locals: vec![], // b is locally assigned when accessed
                        body: 7.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    SmallInt(1),
                    binary_op(AstBinaryOp::AddAssign, 0, 1),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![0.into()], // initial read of x via capture
                        body: 2.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0), // z
                    SmallInt(0),
                    SmallInt(20),
                    range(1, 2, false),
                    List(expressions(&[3])),
                    id(2), // 5 - x
                    id(2),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Greater, 6, 7),
                    Function(koto_parser::Function {
                        args: expressions(&[5]),
                        local_count: 1,
                        accessed_non_locals: vec![],
                        body: 8.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    NamedCall {
                        id: 1.into(), // y
                        args: expressions(&[4, 9]),
                    }, // 10
                    assign(0, 10),
                    id(0), // z
                    NamedCall {
                        id: 1.into(), // y
                        args: expressions(&[12]),
                    },
                    MainBlock {
                        body: expressions(&[11, 13]),
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
                    Yield(0.into()),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 1.into(),
                        is_variadic: false,
                        is_generator: true,
                    }),
                    MainBlock {
                        body: expressions(&[2]),
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
                    Tuple(expressions(&[0, 1])),
                    Yield(2.into()),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 3.into(),
                        is_variadic: false,
                        is_generator: true,
                    }),
                    MainBlock {
                        body: expressions(&[4]),
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
                    Map(vec![id_entry(0, Some(0))]),
                    Yield(1.into()),
                    Function(koto_parser::Function {
                        args: expressions(&[]),
                        local_count: 0,
                        accessed_non_locals: vec![],
                        body: 2.into(),
                        is_variadic: false,
                        is_generator: true,
                    }),
                    MainBlock {
                        body: expressions(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo")]),
            )
        }

        #[test]
        fn unpack_call_args() {
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
                    id(0), // a
                    Wildcard(None),
                    Ellipsis(Some(1.into())),       // others
                    id(2),                          // c
                    Wildcard(Some(3.into())),       // d
                    Tuple(expressions(&[2, 3, 4])), // ast index 5
                    Tuple(expressions(&[1, 5])),
                    Wildcard(Some(4.into())), // e
                    id(0),
                    Function(koto_parser::Function {
                        args: expressions(&[0, 6, 7]),
                        local_count: 3,
                        accessed_non_locals: vec![],
                        body: 8.into(),
                        is_variadic: false,
                        is_generator: false,
                    }),
                    MainBlock {
                        body: expressions(&[9]),
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
    }

    mod lookups {
        use super::*;

        #[test]
        fn indexed_assignment() {
            let source = "a[0] = a[1]";

            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(0),
                    lookup_index(1, None),
                    lookup_root(0, Some(2)),
                    id(0),
                    SmallInt(1), // 5
                    lookup_index(5, None),
                    lookup_root(4, Some(6)),
                    assign(3, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    RangeFull,
                    lookup_index(1, None),
                    lookup_root(0, Some(2)),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0),
                    SmallInt(3),
                    RangeTo {
                        end: 1.into(),
                        inclusive: false,
                    },
                    lookup_index(2, None),
                    lookup_root(0, Some(3)),
                    MainBlock {
                        body: expressions(&[4]),
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
                    id(0),
                    SmallInt(10),
                    RangeFrom { start: 1.into() },
                    SmallInt(0),
                    lookup_index(3, None),
                    lookup_index(2, Some(4)), // 5
                    lookup_root(0, Some(5)),
                    MainBlock {
                        body: expressions(&[6]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn lookup_with_id() {
            let source = "x.foo";
            check_ast(
                source,
                &[
                    id(0),
                    lookup_id(1, None),
                    lookup_root(0, Some(1)),
                    MainBlock {
                        body: expressions(&[2]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn lookup_with_call() {
            let source = "x.bar()";
            check_ast(
                source,
                &[
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(1, Some(1)),
                    lookup_root(0, Some(2)),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(1, Some(1)),
                    lookup_root(0, Some(2)),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Subtract, 3, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    Lookup((
                        LookupNode::Str(AstString {
                            quote: StringQuote::Double,
                            contents: StringContents::Literal(2.into()),
                        }),
                        None,
                    )),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        Some(1.into()),
                    )),
                    lookup_id(1, Some(2)),
                    lookup_root(0, Some(3)),
                    SmallInt(1), // 5
                    assign(4, 5),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    SmallInt(42),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(2)),
                    lookup_root(0, Some(3)),
                    MainBlock {
                        body: expressions(&[4]),
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
                    id(0),
                    SmallInt(42),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(2)),
                    lookup_root(0, Some(3)),
                    MainBlock {
                        body: expressions(&[4]),
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
                    id(0), // x
                    SmallInt(42),
                    Map(vec![
                        id_entry(2, Some(1)), // foo: 42
                    ]),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[2]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(3)),   // takes_a_map
                    lookup_root(0, Some(4)), // @5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    lookup_id(1, None),
                    lookup_root(0, Some(1)),
                    id(0),
                    lookup_id(2, None),
                    lookup_root(3, Some(4)), // 5
                    List(expressions(&[2, 5])),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(1), // x
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[0]),
                    },
                    Nested(1.into()),
                    lookup_id(2, None),
                    lookup_root(2, Some(3)),
                    MainBlock {
                        body: expressions(&[4]),
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
                    id(1), // x
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[0]),
                    },
                    Nested(1.into()),
                    SmallInt(0),
                    lookup_index(3, None),
                    lookup_root(2, Some(4)), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(1), // x
                    NamedCall {
                        id: 0.into(), // f
                        args: expressions(&[0]),
                    },
                    Nested(1.into()),
                    id(2), // y
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[3]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_root(2, Some(4)), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(0, Some(1)),
                    lookup_root(0, Some(2)),
                    MainBlock {
                        body: expressions(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("sin")]),
            )
        }

        #[test]
        fn lookup_on_string() {
            let source = "'fox'.ends_with 'x'";
            check_ast(
                source,
                &[
                    string_literal(0, StringQuote::Single),
                    string_literal(2, StringQuote::Single),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(2)),
                    lookup_root(0, Some(3)),
                    MainBlock {
                        body: expressions(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("fox"),
                    Constant::Str("ends_with"),
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
                    id(0),
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(expressions(&[1, 2])),
                    id(2),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[4]),
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    lookup_id(1, Some(5)),
                    lookup_root(3, Some(6)),
                    assign(0, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    SmallInt(0),
                    SmallInt(1),
                    List(expressions(&[1, 2])),
                    id(2),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[4]),
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    lookup_id(1, Some(5)),
                    lookup_root(3, Some(6)),
                    assign(0, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    Map(vec![id_entry(1, None), id_entry(2, None)]),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(3, Some(2)),
                    lookup_root(1, Some(3)),
                    assign(0, 4), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    range(0, 1, false),
                    Nested(2.into()),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(0, Some(4)), // 5
                    lookup_root(3, Some(5)),
                    MainBlock {
                        body: expressions(&[6]),
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
                    range(0, 1, false),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(0, Some(3)),
                    lookup_root(2, Some(4)), // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    Nested(0.into()),
                    id(2),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[2]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(3)),
                    lookup_root(1, Some(4)), // 5
                    Nested(5.into()),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    SmallInt(1),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_id(3, Some(2)),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1]),
                            with_parens: false,
                        },
                        Some(3.into()),
                    )),
                    lookup_id(2, Some(4)), // 5
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        Some(5.into()),
                    )),
                    lookup_id(1, Some(6)),
                    lookup_root(0, Some(7)),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    lookup_id(1, None),
                    lookup_root(0, Some(1)),
                    id(0),
                    lookup_id(2, None),
                    lookup_root(3, Some(4)), // 5
                    binary_op(AstBinaryOp::Or, 2, 5),
                    BoolFalse,
                    binary_op(AstBinaryOp::Or, 6, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    Return(Some(3.into())),
                    MainBlock {
                        body: expressions(&[0, 1, 2, 4]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn keywords_with_args() {
            let source = r#"
not true
debug x + x
"#;
            check_ast(
                source,
                &[
                    BoolTrue,
                    unary_op(AstUnaryOp::Not, 0),
                    id(0),
                    id(0),
                    binary_op(AstBinaryOp::Add, 2, 3),
                    Debug {
                        expression_string: 1.into(),
                        expression: 4.into(),
                    }, // 5
                    MainBlock {
                        body: expressions(&[1, 5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("x + x")]),
            )
        }
    }

    mod import {
        use super::*;

        fn import_id(id: u32) -> ImportItem {
            ImportItem {
                item: id.into(),
                name: None,
            }
        }

        fn import_string(literal_index: u32, quotation_mark: StringQuote) -> ImportItem {
            ImportItem {
                item: simple_string(literal_index, quotation_mark).into(),
                name: None,
            }
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
                        body: expressions(&[0]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("foo")]),
            )
        }

        #[test]
        fn import_item_as() {
            let source = "import foo as bar";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![],
                        items: vec![ImportItem {
                            item: 0.into(),
                            name: Some(1.into()),
                        }],
                    },
                    MainBlock {
                        body: expressions(&[0]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn import_from_module() {
            let source = "from foo import bar";
            check_ast(
                source,
                &[
                    Import {
                        from: vec![0.into()],
                        items: vec![import_id(1)],
                    },
                    MainBlock {
                        body: expressions(&[0]),
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
                    id(0),
                    Import {
                        from: vec![1.into()],
                        items: vec![import_id(2)],
                    },
                    assign(0, 1),
                    MainBlock {
                        body: expressions(&[2]),
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
                            import_string(1, StringQuote::Single),
                            import_id(2),
                        ],
                    },
                    MainBlock {
                        body: expressions(&[0]),
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
                        from: vec![0.into()],
                        items: vec![import_id(1), import_id(2)],
                    },
                    MainBlock {
                        body: expressions(&[0]),
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
                        from: vec![simple_string(0, StringQuote::Single).into(), 1.into()],
                        items: vec![import_id(2), import_id(3)],
                    },
                    MainBlock {
                        body: expressions(&[0]),
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
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_root(0, Some(1)),
                    id(1), // e
                    id(1),
                    Debug {
                        expression_string: 1.into(),
                        expression: 4.into(),
                    }, // ast 5
                    Try(AstTry {
                        try_block: 2.into(),
                        catch_arg: 3.into(),
                        catch_block: 5.into(),
                        finally_block: None,
                    }),
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    Wildcard(None),
                    id(1),
                    Try(AstTry {
                        try_block: 0.into(),
                        catch_arg: 1.into(),
                        catch_block: 2.into(),
                        finally_block: None,
                    }),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0),                    // x
                    Wildcard(Some(1.into())), // error
                    id(2),                    // y
                    Try(AstTry {
                        try_block: 0.into(),
                        catch_arg: 1.into(),
                        catch_block: 2.into(),
                        finally_block: None,
                    }),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    lookup_root(0, Some(1)),
                    id(1), // e
                    id(1),
                    Debug {
                        expression_string: 1.into(),
                        expression: 4.into(),
                    }, // ast 5
                    SmallInt(0),
                    Try(AstTry {
                        try_block: 2.into(),
                        catch_arg: 3.into(),
                        catch_block: 5.into(),
                        finally_block: Some(6.into()),
                    }),
                    MainBlock {
                        body: expressions(&[7]),
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
                    id(0),
                    Throw(0.into()),
                    MainBlock {
                        body: expressions(&[1]),
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
                    string_literal(0, StringQuote::Single),
                    Throw(0.into()),
                    MainBlock {
                        body: expressions(&[1]),
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
                    id(1),
                    string_literal(3, StringQuote::Double),
                    Map(vec![id_entry(0, Some(0)), id_entry(2, Some(1))]),
                    Throw(2.into()),
                    MainBlock {
                        body: expressions(&[3]),
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
                    id(0),
                    id(1),
                    SmallInt(0),
                    SmallInt(1),
                    SmallInt(42),
                    id(2), // 5
                    SmallInt(-1),
                    Match {
                        expression: 1.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[2, 3]),
                                condition: None,
                                expression: 4.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[5]),
                                condition: None,
                                expression: 6.into(),
                            },
                        ],
                    },
                    assign(0, 7),
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    string_literal(1, StringQuote::Single),
                    SmallInt(99),
                    string_literal(2, StringQuote::Double),
                    string_literal(3, StringQuote::Double),
                    Break(None), // 5
                    Match {
                        expression: 0.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[1]),
                                condition: None,
                                expression: 2.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[3, 4]),
                                condition: None,
                                expression: 5.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[6]),
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
                    id(0),
                    id(1),
                    id(2),
                    Tuple(expressions(&[0, 1, 2])),
                    SmallInt(0),
                    id(3), // 5
                    Wildcard(None),
                    Tuple(expressions(&[4, 5, 6])),
                    id(3),
                    Wildcard(None),
                    SmallInt(0), // 10
                    id(4),
                    Tuple(expressions(&[10, 11])),
                    Wildcard(Some(5.into())),
                    Tuple(expressions(&[9, 12, 13])),
                    SmallInt(0), // 15
                    Match {
                        expression: 3.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[7]),
                                condition: None,
                                expression: 8.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[14]),
                                condition: None,
                                expression: 15.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[16]),
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
                    id(0),
                    Ellipsis(None),
                    SmallInt(0),
                    Tuple(expressions(&[1, 2])),
                    SmallInt(0),
                    SmallInt(1), // 5
                    Ellipsis(None),
                    Tuple(expressions(&[5, 6])),
                    SmallInt(1),
                    Match {
                        expression: 0.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[3]),
                                condition: None,
                                expression: 4.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[7]),
                                condition: None,
                                expression: 8.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[9]),
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
                    id(0),
                    Ellipsis(Some(1.into())),
                    SmallInt(0),
                    SmallInt(1),
                    Tuple(expressions(&[1, 2, 3])),
                    SmallInt(0), // 5
                    SmallInt(1),
                    SmallInt(0),
                    Ellipsis(Some(2.into())),
                    Tuple(expressions(&[6, 7, 8])),
                    SmallInt(1), // 10
                    Match {
                        expression: 0.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[4]),
                                condition: None,
                                expression: 5.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[9]),
                                condition: None,
                                expression: 10.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[11]),
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
                    id(0),
                    id(1),
                    id(1),
                    SmallInt(5),
                    binary_op(AstBinaryOp::Greater, 2, 3),
                    SmallInt(0), // 5
                    id(1),
                    id(1),
                    SmallInt(10),
                    binary_op(AstBinaryOp::Less, 7, 8),
                    SmallInt(1), // 10
                    id(1),
                    SmallInt(-1),
                    Match {
                        expression: 0.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[1]),
                                condition: Some(4.into()),
                                expression: 5.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[6]),
                                condition: Some(9.into()),
                                expression: 10.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[11]),
                                condition: None,
                                expression: 12.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[13]),
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
                    id(0),
                    id(1),
                    TempTuple(expressions(&[0, 1])),
                    SmallInt(0),
                    SmallInt(1),
                    TempTuple(expressions(&[3, 4])), // 5
                    SmallInt(2),
                    SmallInt(3),
                    TempTuple(expressions(&[6, 7])),
                    id(2),
                    SmallInt(0), // 10
                    id(3),
                    Null,
                    TempTuple(expressions(&[11, 12])),
                    id(3),
                    SmallInt(0), // 15
                    Match {
                        expression: 2.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[5, 8]),
                                condition: Some(9.into()),
                                expression: 10.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[13]),
                                condition: None,
                                expression: 14.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[]),
                                condition: None,
                                expression: 15.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[16]),
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
                    id(0),
                    SmallInt(42),
                    Lookup((
                        LookupNode::Call {
                            args: expressions(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    lookup_id(1, Some(2)),
                    lookup_root(0, Some(3)),
                    Null, // 5
                    SmallInt(0),
                    SmallInt(1),
                    Match {
                        expression: 4.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[5]),
                                condition: None,
                                expression: 6.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[]),
                                condition: None,
                                expression: 7.into(),
                            },
                        ],
                    },
                    MainBlock {
                        body: expressions(&[8]),
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
                    id(0),
                    id(1),
                    lookup_id(2, None),
                    lookup_root(1, Some(2)),
                    SmallInt(0),
                    Match {
                        expression: 0.into(),
                        arms: vec![MatchArm {
                            patterns: expressions(&[3]),
                            condition: None,
                            expression: 4.into(),
                        }],
                    },
                    MainBlock {
                        body: expressions(&[5]),
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
                    id(0),
                    SmallInt(0),
                    SmallInt(1),
                    string_literal(1, StringQuote::Single),
                    Throw(3.into()),
                    Match {
                        expression: 0.into(),
                        arms: vec![
                            MatchArm {
                                patterns: expressions(&[1]),
                                condition: None,
                                expression: 2.into(),
                            },
                            MatchArm {
                                patterns: expressions(&[]),
                                condition: None,
                                expression: 4.into(),
                            },
                        ],
                    }, // 5
                    MainBlock {
                        body: expressions(&[5]),
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
                    binary_op(AstBinaryOp::Equal, 0, 1),
                    SmallInt(0),
                    id(0),
                    id(1), // 5
                    binary_op(AstBinaryOp::Greater, 4, 5),
                    SmallInt(1),
                    id(0),
                    Switch(vec![
                        SwitchArm {
                            condition: Some(2.into()),
                            expression: 3.into(),
                        },
                        SwitchArm {
                            condition: Some(6.into()),
                            expression: 7.into(),
                        },
                        SwitchArm {
                            condition: None,
                            expression: 8.into(),
                        },
                    ]),
                    MainBlock {
                        body: expressions(&[9]),
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
                    id(0),
                    Debug {
                        expression_string: 0.into(),
                        expression: 2.into(),
                    },
                    Switch(vec![
                        SwitchArm {
                            condition: Some(0.into()),
                            expression: 1.into(),
                        },
                        SwitchArm {
                            condition: None,
                            expression: 3.into(),
                        },
                    ]),
                    MainBlock {
                        body: expressions(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }
}
