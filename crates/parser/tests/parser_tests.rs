mod parser {
    use koto_parser::{Node::*, *};

    fn check_ast(source: &str, expected_ast: &[Node], expected_constants: Option<&[Constant]>) {
        println!("{source}");

        match Parser::parse(source) {
            Ok(ast) => compare_ast(expected_ast, expected_constants, &ast),
            Err(error) => panic!("{error} - {:?}", error.span.start),
        }
    }

    fn compare_ast(expected_ast: &[Node], expected_constants: Option<&[Constant<'_>]>, ast: &Ast) {
        for (i, (ast_node, expected_node)) in
            ast.nodes().iter().zip(expected_ast.iter()).enumerate()
        {
            assert_eq!(*expected_node, ast_node.node, "Mismatch at position {i}");
        }
        assert_eq!(
            expected_ast.len(),
            ast.nodes().len(),
            "Node list length mismatch"
        );

        if let Some(expected_constants) = expected_constants {
            for (constant, expected_constant) in
                ast.constants().iter().zip(expected_constants.iter())
            {
                assert_eq!(*expected_constant, constant);
            }
            assert_eq!(
                expected_constants.len(),
                ast.constants().size(),
                "Constant pool size mismatch"
            );
        } else {
            assert_eq!(0, ast.constants().size());
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

    fn id_with_type_hint(constant: u32, type_hint: u32) -> Node {
        Node::Id(constant.into(), Some(type_hint.into()))
    }

    fn type_hint(constant: u32) -> Node {
        Node::Type {
            type_index: constant.into(),
            allow_null: false,
        }
    }

    fn optional_type_hint(constant: u32) -> Node {
        Node::Type {
            type_index: constant.into(),
            allow_null: true,
        }
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

    fn nodes(indices: &[u32]) -> AstVec<AstIndex> {
        indices.iter().map(|i| AstIndex::from(*i)).collect()
    }

    fn tuple_with_parens(elements: &[u32]) -> Node {
        Node::Tuple {
            elements: nodes(elements),
            parentheses: true,
        }
    }

    fn tuple_no_parens(elements: &[u32]) -> Node {
        Node::Tuple {
            elements: nodes(elements),
            parentheses: false,
        }
    }

    fn constants(indices: &[u32]) -> AstVec<ConstantIndex> {
        indices.iter().map(|i| ConstantIndex::from(*i)).collect()
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
            let_assignment: false,
        }
    }

    fn let_assign(target: u32, expression: u32) -> Node {
        Node::Assign {
            target: target.into(),
            expression: expression.into(),
            let_assignment: true,
        }
    }

    fn map_with_braces(entries: &[u32]) -> Node {
        Node::Map {
            entries: nodes(entries),
            braces: true,
        }
    }

    fn map_block(entries: &[u32]) -> Node {
        Node::Map {
            entries: nodes(entries),
            braces: false,
        }
    }

    fn map_entry(key: u32, value: u32) -> Node {
        Node::MapEntry(key.into(), value.into())
    }

    fn map_pattern(entries: &[u32]) -> Node {
        Node::MapPattern {
            entries: nodes(entries),
            type_hint: None,
        }
    }

    fn map_pattern_with_type_hint(entries: &[u32], type_hint: u32) -> Node {
        Node::MapPattern {
            entries: nodes(entries),
            type_hint: Some(type_hint.into()),
        }
    }

    fn map_key_rebind(key: u32, id_or_ignored: u32) -> Node {
        Node::MapKeyRebind {
            key: key.into(),
            id_or_ignored: id_or_ignored.into(),
        }
    }

    fn range(start: u32, end: u32, inclusive: bool) -> Node {
        Node::Range {
            start: start.into(),
            end: end.into(),
            inclusive,
        }
    }

    fn chain_call(args: &[u32], with_parens: bool, next: Option<u32>) -> Node {
        Node::Chain((
            ChainNode::Call {
                args: args.iter().map(AstIndex::from).collect(),
                with_parens,
            },
            next.map(AstIndex::from),
        ))
    }

    fn chain_id(id: u32, next: Option<u32>) -> Node {
        Node::Chain((ChainNode::Id(id.into()), next.map(AstIndex::from)))
    }

    fn chain_index(index: u32, next: Option<u32>) -> Node {
        Node::Chain((ChainNode::Index(index.into()), next.map(AstIndex::from)))
    }

    fn chain_root(index: u32, next: Option<u32>) -> Node {
        Node::Chain((ChainNode::Root(index.into()), next.map(AstIndex::from)))
    }

    fn chain_null_check(next: u32) -> Node {
        Node::Chain((ChainNode::NullCheck, Some(next.into())))
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
                        body: nodes(&[0, 1, 2, 3, 4, 5, 6, 7]),
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
0xABAD_CAFE
0o1
0o100
0b1
0b1_0_0
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
                        body: nodes(&[0, 1, 2, 3, 4, 5, 6, 7]),
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
                        body: nodes(&[0, 1]),
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
                        body: nodes(&[0, 1]),
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
                        body: nodes(&[1, 3, 6]),
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
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::I64(456), Constant::Str("!")]),
            )
        }

        #[test]
        fn string_with_formatted_expression() {
            let source = "
'!{a:_>3.2x}!'
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
                                    representation: Some(StringFormatRepresentation::HexLower),
                                },
                            },
                            StringNode::Literal(0.into()),
                        ]),
                    }),
                    MainBlock {
                        body: nodes(&[1]),
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
                        body: nodes(&[0, 1, 2, 3]),
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
                    chain_index(4, None), // 5
                    chain_root(3, Some(5)),
                    unary_op(AstUnaryOp::Negate, 6),
                    SmallInt(1),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Add, 8, 9), // 10
                    Nested(10.into()),
                    unary_op(AstUnaryOp::Negate, 11),
                    MainBlock {
                        body: nodes(&[0, 2, 7, 12]),
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
                    List(nodes(&[0, 1, 2, 3, 4])),
                    List(nodes(&[])),
                    MainBlock {
                        body: nodes(&[5, 6]),
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
                    List(nodes(&[1, 2])),
                    SmallInt(2),
                    List(nodes(&[0, 3, 4])), // 5
                    MainBlock {
                        body: nodes(&[5]),
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
x =
  [
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
                    List(nodes(&[1, 2, 3, 4, 5])),
                    assign(0, 6),
                    MainBlock {
                        body: nodes(&[7]),
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
        fn maps_with_braces() {
            let sources = [
                "
{}
x = {'foo': 42, bar, baz: 'hello', @+: 99}",
                "
{
}
x = { 'foo': 42
  , bar
  , baz   : 'hello'
  , @+: 99
}
",
                "
{ }
x =
  { 'foo': 42, bar
    , baz: 'hello'
    , @+  : 99
    }
",
                "
{
  }

x =
  { 'foo' : 42, bar,
     baz: 'hello'
  , @+: 99
}
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    map_with_braces(&[]),
                    id(0),                                  // x
                    string_literal(1, StringQuote::Single), // 'foo'
                    SmallInt(42),
                    map_entry(2, 3),
                    id(2),                                  // 5 - bar
                    id(3),                                  // baz
                    string_literal(4, StringQuote::Single), // 'hello'
                    map_entry(6, 7),
                    Meta(MetaKeyId::Add, None),
                    SmallInt(99), // 10
                    map_entry(9, 10),
                    map_with_braces(&[4, 5, 8, 11]),
                    assign(1, 12),
                    MainBlock {
                        body: nodes(&[0, 13]),
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
        fn map_block_first_entry_with_string_key() {
            let source = r#"
x =
  "foo": 42
"#;
            check_ast(
                source,
                &[
                    id(0), // x
                    string_literal(1, StringQuote::Double),
                    SmallInt(42),
                    map_entry(1, 2),
                    map_block(&[3]),
                    assign(0, 4), // 5
                    MainBlock {
                        body: nodes(&[5]),
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
                    id(1), // foo
                    id(2), // bar
                    SmallInt(42),
                    map_entry(2, 3),
                    map_block(&[4]), // 5
                    Block(nodes(&[5])),
                    map_entry(1, 6),
                    map_block(&[7]),
                    assign(0, 8),
                    MainBlock {
                        body: nodes(&[9]),
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
    foo: 10, 20,
         30,
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(0), // x
                    id(1), // foo
                    SmallInt(10),
                    SmallInt(20),
                    SmallInt(30),
                    tuple_no_parens(&[2, 3, 4]), // 5
                    map_entry(1, 5),
                    map_block(&[6]),
                    assign(0, 7),
                    MainBlock {
                        body: nodes(&[8]),
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
  bar:
    baz 42
",
                "
x =
  foo: 1
  bar:
    baz
      42
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(0), // x
                    id(1), // foo
                    SmallInt(1),
                    map_entry(1, 2),
                    id(2), // bar
                    id(3), // 5 - baz
                    SmallInt(42),
                    chain_call(&[6], false, None),
                    chain_root(5, Some(7)),
                    Block(nodes(&[8])),
                    map_entry(4, 9), // 10
                    map_block(&[3, 10]),
                    assign(0, 11),
                    MainBlock {
                        body: nodes(&[12]),
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
                    Meta(MetaKeyId::Add, None),
                    SmallInt(0),
                    map_entry(1, 2),
                    Meta(MetaKeyId::Subtract, None),
                    SmallInt(1), // 5
                    map_entry(4, 5),
                    Meta(MetaKeyId::Named, Some(1.into())),
                    SmallInt(0),
                    map_entry(7, 8),
                    map_block(&[3, 6, 9]),
                    assign(0, 10),
                    MainBlock {
                        body: nodes(&[11]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
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
                        body: nodes(&[2, 5]),
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
                        body: nodes(&[6]),
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
                        body: nodes(&[2, 5, 8]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("min"), Constant::Str("max")]),
            )
        }

        #[test]
        fn range_from_chains() {
            let source = "foo.bar..foo.baz";
            check_ast(
                source,
                &[
                    id(0),
                    chain_id(1, None),
                    chain_root(0, Some(1)),
                    id(0),
                    chain_id(2, None),
                    chain_root(3, Some(4)), // 5
                    range(2, 5, false),
                    MainBlock {
                        body: nodes(&[6]),
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
                    List(nodes(&[2])),
                    SmallInt(0),
                    SmallInt(10), // 5
                    range(4, 5, false),
                    SmallInt(10),
                    SmallInt(0),
                    range(7, 8, true),
                    List(nodes(&[6, 9])),
                    MainBlock {
                        body: nodes(&[3, 10]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn ranges_in_tuple() {
            let source = "\
1..2, 3..4
";
            check_ast(
                source,
                &[
                    SmallInt(1),
                    SmallInt(2),
                    range(0, 1, false),
                    SmallInt(3),
                    SmallInt(4),
                    range(3, 4, false), // 5
                    tuple_no_parens(&[2, 5]),
                    MainBlock {
                        body: nodes(&[6]),
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
                    tuple_no_parens(&[0, 1, 2]),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn tuple_with_missing_value() {
            let source = "8, , 5";
            check_ast(
                source,
                &[
                    SmallInt(8),
                    Null,
                    SmallInt(5),
                    tuple_no_parens(&[0, 1, 2]),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn empty_parentheses() {
            let source = "()";
            check_ast(
                source,
                &[
                    tuple_with_parens(&[]),
                    MainBlock {
                        body: nodes(&[0]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn single_comma() {
            let source = "(,)";
            check_ast(
                source,
                &[
                    Null,
                    tuple_with_parens(&[0]),
                    MainBlock {
                        body: nodes(&[1]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn two_commas() {
            let source = "(,,)";
            check_ast(
                source,
                &[
                    Null,
                    Null,
                    tuple_with_parens(&[0, 1]),
                    MainBlock {
                        body: nodes(&[2]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn nested_empty_tuple() {
            let source = "(())";
            check_ast(
                source,
                &[
                    tuple_with_parens(&[]),
                    Nested(0.into()),
                    MainBlock {
                        body: nodes(&[1]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn empty_tuple_inside_tuple() {
            let source = "((),)";
            check_ast(
                source,
                &[
                    tuple_with_parens(&[]),
                    tuple_with_parens(&[0]),
                    MainBlock {
                        body: nodes(&[1]),
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
                    tuple_with_parens(&[0]),
                    MainBlock {
                        body: nodes(&[1]),
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
                    tuple_with_parens(&[0, 1, 2]),
                    MainBlock {
                        body: nodes(&[3]),
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
                        body: nodes(&[2]),
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
                    tuple_no_parens(&[1, 2]),
                    assign(0, 3),
                    MainBlock {
                        body: nodes(&[4]),
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
                    tuple_with_parens(&[1, 2]),
                    SmallInt(2),
                    SmallInt(3), // 5
                    tuple_with_parens(&[4, 5]),
                    tuple_no_parens(&[3, 6]),
                    assign(0, 7),
                    MainBlock {
                        body: nodes(&[8]),
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
                    chain_index(2, None),
                    chain_root(1, Some(3)),
                    SmallInt(1), // 5
                    SmallInt(0),
                    TempTuple(nodes(&[5, 6])),
                    MultiAssign {
                        targets: nodes(&[0, 4]),
                        expression: 7.into(),
                        let_assignment: false,
                    },
                    MainBlock {
                        body: nodes(&[8]),
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
                    TempTuple(nodes(&[2, 3])),
                    MultiAssign {
                        targets: nodes(&[0, 1]),
                        expression: 4.into(),
                        let_assignment: false,
                    }, // 5
                    id(0),
                    MainBlock {
                        body: nodes(&[5, 6]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn multi_1_to_3_with_ignored_ids() {
            let source = "x, _, _y = f()";
            check_ast(
                source,
                &[
                    id(0),
                    Ignored(None, None),
                    Ignored(Some(1.into()), None),
                    id(2),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_root(3, Some(4)), // 5
                    MultiAssign {
                        targets: nodes(&[0, 1, 2]),
                        expression: 5.into(),
                        let_assignment: false,
                    },
                    MainBlock {
                        body: nodes(&[6]),
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
x %= 4
x ^= 5";
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
                    id(0), // 15
                    SmallInt(5),
                    binary_op(AstBinaryOp::PowerAssign, 15, 16),
                    MainBlock {
                        body: nodes(&[2, 5, 8, 11, 14, 17]),
                        local_count: 0,
                    }, // 15
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn list_with_chain_as_first_element() {
            let source = "
[foo.bar()]
";
            check_ast(
                source,
                &[
                    id(0),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(1, Some(1)),
                    chain_root(0, Some(2)),
                    List(nodes(&[3])),
                    MainBlock {
                        body: nodes(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn map_one() {
            let source = "
{x} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    map_with_braces(&[0]),
                    id(0),
                    SmallInt(1),
                    map_entry(2, 3),
                    map_with_braces(&[4]),
                    assign(1, 5),
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn map_two() {
            let source = "
{x, y} = {x: 1, y: 2}
";
            check_ast(
                source,
                &[
                    id(0),
                    id(1),
                    map_with_braces(&[0, 1]),
                    id(0),
                    SmallInt(1),
                    map_entry(3, 4),
                    id(1),
                    SmallInt(2),
                    map_entry(6, 7),
                    map_with_braces(&[5, 8]),
                    assign(2, 9),
                    MainBlock {
                        body: nodes(&[10]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn map_multiple() {
            let source = "
{x}, {y} = {x: 1}, {y: 2}
";
            check_ast(
                source,
                &[
                    id(0),
                    map_with_braces(&[0]),
                    id(1),
                    map_with_braces(&[2]),
                    id(0),
                    SmallInt(1),
                    map_entry(4, 5),
                    map_with_braces(&[6]),
                    id(1),
                    SmallInt(2),
                    map_entry(8, 9),
                    map_with_braces(&[10]),
                    TempTuple(nodes(&[7, 11])),
                    MultiAssign {
                        targets: nodes(&[1, 3]),
                        expression: 12.into(),
                        let_assignment: false,
                    },
                    MainBlock {
                        body: nodes(&[13]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn map_with_as() {
            let source = "
{x as y} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    id(1),
                    map_key_rebind(0, 1),
                    map_with_braces(&[2]),
                    id(0),
                    SmallInt(1),
                    map_entry(4, 5),
                    map_with_braces(&[6]),
                    assign(3, 7),
                    MainBlock {
                        body: nodes(&[8]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn map_ignored_key() {
            let source = "
{x as _} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    Ignored(None, None),
                    map_key_rebind(0, 1),
                    map_with_braces(&[2]),
                    id(0),
                    SmallInt(1),
                    map_entry(4, 5),
                    map_with_braces(&[6]),
                    assign(3, 7),
                    MainBlock {
                        body: nodes(&[8]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }

    mod let_expression {
        use super::*;

        #[test]
        fn number() {
            let source = "let a = 1";

            check_ast(
                source,
                &[
                    id(0), // a
                    SmallInt(1),
                    let_assign(0, 1),
                    MainBlock {
                        body: nodes(&[2]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn number_with_type_hint() {
            let source = "let a: Int = 1";

            check_ast(
                source,
                &[
                    type_hint(1),            // Int
                    id_with_type_hint(0, 0), // a
                    SmallInt(1),
                    let_assign(1, 2),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("Int")]),
            )
        }

        #[test]
        fn string_with_optional_type_hint() {
            let source = "let a: String? = 'hello'";

            check_ast(
                source,
                &[
                    optional_type_hint(1),   // String?
                    id_with_type_hint(0, 0), // a
                    string_literal(2, StringQuote::Single),
                    let_assign(1, 2),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Str("String"),
                    Constant::Str("hello"),
                ]),
            )
        }

        #[test]
        fn multiple_targets() {
            let source = "let foo: String, bar: Int = baz";

            check_ast(
                source,
                &[
                    type_hint(1),            // String
                    id_with_type_hint(0, 0), // foo
                    type_hint(3),            // Int
                    id_with_type_hint(2, 2), // bar
                    id(4),                   // baz
                    MultiAssign {
                        targets: nodes(&[1, 3]),
                        expression: 4.into(),
                        let_assignment: true,
                    }, // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("foo"),
                    Constant::Str("String"),
                    Constant::Str("bar"),
                    Constant::Str("Int"),
                    Constant::Str("baz"),
                ]),
            )
        }

        #[test]
        fn ignored_number_with_type_hint() {
            let source = "let _: Int = 1";

            check_ast(
                source,
                &[
                    type_hint(0),
                    Ignored(None, Some(0.into())),
                    SmallInt(1),
                    let_assign(1, 2),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("Int")]),
            )
        }

        #[test]
        fn number_with_ignored_id_and_type_hint() {
            let source = "let _a: Int = 1";

            check_ast(
                source,
                &[
                    type_hint(1),
                    Ignored(Some(0.into()), Some(0.into())),
                    SmallInt(1),
                    let_assign(1, 2),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("Int")]),
            )
        }

        #[test]
        fn multi_1_to_3_with_ignored_ids_and_type_hint() {
            let source = "let x: Int, _: Int, _y: Int = f()";
            check_ast(
                source,
                &[
                    type_hint(1),
                    id_with_type_hint(0, 0),
                    type_hint(1),
                    Ignored(None, Some(2.into())),
                    type_hint(1),
                    Ignored(Some(2.into()), Some(4.into())),
                    id(3),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_root(6, Some(7)), // 5
                    MultiAssign {
                        targets: nodes(&[1, 3, 5]),
                        expression: 8.into(),
                        let_assignment: true,
                    },
                    MainBlock {
                        body: nodes(&[9]),
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("Int"),
                    Constant::Str("y"),
                    Constant::Str("f"),
                ]),
            )
        }

        #[test]
        fn map_one() {
            let source = "
let {x} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    map_pattern(&[0]),
                    id(0),
                    SmallInt(1),
                    map_entry(2, 3),
                    map_with_braces(&[4]),
                    let_assign(1, 5),
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn map_two() {
            let source = "
let {x, y} = {x: 1, y: 2}
";
            check_ast(
                source,
                &[
                    id(0),
                    id(1),
                    map_pattern(&[0, 1]),
                    id(0),
                    SmallInt(1),
                    map_entry(3, 4),
                    id(1),
                    SmallInt(2),
                    map_entry(6, 7),
                    map_with_braces(&[5, 8]),
                    let_assign(2, 9),
                    MainBlock {
                        body: nodes(&[10]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn map_multiple() {
            let source = "
let {x}, {y} = {x: 1}, {y: 2}
";
            check_ast(
                source,
                &[
                    id(0),
                    map_pattern(&[0]),
                    id(1),
                    map_pattern(&[2]),
                    id(0),
                    SmallInt(1),
                    map_entry(4, 5),
                    map_with_braces(&[6]),
                    id(1),
                    SmallInt(2),
                    map_entry(8, 9),
                    map_with_braces(&[10]),
                    TempTuple(nodes(&[7, 11])),
                    MultiAssign {
                        targets: nodes(&[1, 3]),
                        expression: 12.into(),
                        let_assignment: true,
                    },
                    MainBlock {
                        body: nodes(&[13]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn map_with_type_hint() {
            let source = "
let {x}: Foo = {@type: 'Foo'}
";
            check_ast(
                source,
                &[
                    id(0),
                    type_hint(1),
                    map_pattern_with_type_hint(&[0], 1),
                    Meta(MetaKeyId::Type, None),
                    string_literal(1, StringQuote::Single),
                    map_entry(3, 4),
                    map_with_braces(&[5]),
                    let_assign(2, 6),
                    MainBlock {
                        body: nodes(&[7]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("Foo")]),
            )
        }

        #[test]
        fn map_with_as() {
            let source = "
let {x as y} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    id(1),
                    map_key_rebind(0, 1),
                    map_pattern(&[2]),
                    id(0),
                    SmallInt(1),
                    map_entry(4, 5),
                    map_with_braces(&[6]),
                    let_assign(3, 7),
                    MainBlock {
                        body: nodes(&[8]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn map_with_type() {
            let source = "
let {x: Number} = {x: 1}
";
            check_ast(
                source,
                &[
                    type_hint(1),
                    id_with_type_hint(0, 0),
                    map_pattern(&[1]),
                    id(0),
                    SmallInt(1),
                    map_entry(3, 4),
                    map_with_braces(&[5]),
                    let_assign(2, 6),
                    MainBlock {
                        body: nodes(&[7]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("Number")]),
            )
        }

        #[test]
        fn map_with_type_optional() {
            let source = "
let {x: Number?} = {x: 1}
";
            check_ast(
                source,
                &[
                    optional_type_hint(1),
                    id_with_type_hint(0, 0),
                    map_pattern(&[1]),
                    id(0),
                    SmallInt(1),
                    map_entry(3, 4),
                    map_with_braces(&[5]),
                    let_assign(2, 6),
                    MainBlock {
                        body: nodes(&[7]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("Number")]),
            )
        }

        #[test]
        fn map_with_as_and_type() {
            let source = "
let {x as y: Number} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    type_hint(2),
                    id_with_type_hint(1, 1),
                    map_key_rebind(0, 2),
                    map_pattern(&[3]),
                    id(0),
                    SmallInt(1),
                    map_entry(5, 6),
                    map_with_braces(&[7]),
                    let_assign(4, 8),
                    MainBlock {
                        body: nodes(&[9]),
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("Number"),
                ]),
            )
        }

        #[test]
        fn map_with_as_and_type_optional() {
            let source = "
let {x as y: Number?} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    optional_type_hint(2),
                    id_with_type_hint(1, 1),
                    map_key_rebind(0, 2),
                    map_pattern(&[3]),
                    id(0),
                    SmallInt(1),
                    map_entry(5, 6),
                    map_with_braces(&[7]),
                    let_assign(4, 8),
                    MainBlock {
                        body: nodes(&[9]),
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("Number"),
                ]),
            )
        }

        #[test]
        fn map_ignored_key() {
            let source = "
let {x as _} = {x: 1}
";
            check_ast(
                source,
                &[
                    id(0),
                    Ignored(None, None),
                    map_key_rebind(0, 1),
                    map_pattern(&[2]),
                    id(0),
                    SmallInt(1),
                    map_entry(4, 5),
                    map_with_braces(&[6]),
                    let_assign(3, 7),
                    MainBlock {
                        body: nodes(&[8]),
                        local_count: 0,
                    },
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
                "
export 
  a =
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
                        body: nodes(&[5]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn export_multi_assignment() {
            let sources = [
                "export a, b, c = foo",
                "
export a, b, c
  = foo",
                "
export a, b, c =
  foo",
                "
export 
  a, b, c = foo",
                "
export 
  a, b, c 
    = foo",
            ];

            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(0),
                    id(1),
                    id(2),
                    id(3),
                    MultiAssign {
                        targets: nodes(&[0, 1, 2]),
                        expression: 3.into(),
                        let_assignment: false,
                    },
                    Export(4.into()), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 3,
                    },
                ],
                Some(&[
                    Constant::Str("a"),
                    Constant::Str("b"),
                    Constant::Str("c"),
                    Constant::Str("foo"),
                ]),
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
                    id(0), // a
                    SmallInt(123),
                    map_entry(0, 1),
                    id(1), // b
                    SmallInt(99),
                    map_entry(3, 4), // 5
                    map_block(&[2, 5]),
                    Export(6.into()),
                    MainBlock {
                        body: nodes(&[7]),
                        local_count: 2,
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
                        body: nodes(&[4]),
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
                        body: nodes(&[6]),
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
                        body: nodes(&[8]),
                        local_count: 0,
                    },
                ],
                None,
            )
        }

        #[test]
        fn divide_then_remainder_with_power() {
            let source = "18 / 3 % 4 ^ 2"; // (18 / 3) % (4 ^ 2)
            check_ast(
                source,
                &[
                    SmallInt(18),
                    SmallInt(3),
                    binary_op(AstBinaryOp::Divide, 0, 1),
                    SmallInt(4),
                    SmallInt(2),
                    binary_op(AstBinaryOp::Power, 3, 4), // 5
                    binary_op(AstBinaryOp::Remainder, 2, 5),
                    MainBlock {
                        body: nodes(&[6]),
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
                        body: nodes(&[2]),
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
                    id(1), // f
                    id(2), // y
                    chain_call(&[3], false, None),
                    chain_root(2, Some(4)), // 5
                    binary_op(AstBinaryOp::Add, 1, 5),
                    assign(0, 6),
                    MainBlock {
                        body: nodes(&[7]),
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
                        body: nodes(&[6]),
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
                        body: nodes(&[7]),
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
                        body: nodes(&[8]),
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
                        body: nodes(&[4]),
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
                        else_if_blocks: astvec![],
                        else_node: Some(3.into()),
                        inline: true,
                    }),
                    binary_op(AstBinaryOp::Add, 0, 4),
                    MainBlock {
                        body: nodes(&[5]),
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
                    Block(nodes(&[2])),
                    BoolTrue,
                    SmallInt(1), // 5
                    Block(nodes(&[5])),
                    BoolFalse,
                    SmallInt(0),
                    Block(nodes(&[8])),
                    SmallInt(1), // 10
                    Block(nodes(&[10])),
                    If(AstIf {
                        condition: 1.into(),
                        then_node: 3.into(),
                        else_if_blocks: astvec![(4.into(), 6.into()), (7.into(), 9.into())],
                        else_node: Some(11.into()),
                        inline: false,
                    }),
                    assign(0, 12),
                    id(0),
                    MainBlock {
                        body: nodes(&[13, 14]),
                        local_count: 1,
                    }, // 15
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
                    tuple_no_parens(&[3, 4]), // 5
                    SmallInt(1),
                    SmallInt(0),
                    tuple_no_parens(&[6, 7]),
                    If(AstIf {
                        condition: 2.into(),
                        then_node: 5.into(),
                        else_if_blocks: astvec![],
                        else_node: Some(8.into()),
                        inline: true,
                    }),
                    MultiAssign {
                        targets: nodes(&[0, 1]),
                        expression: 9.into(),
                        let_assignment: false,
                    }, // 10
                    MainBlock {
                        body: nodes(&[10]),
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
        fn for_loop() {
            let source = "\
for x: String, _: Number, _y, z in foo
  x";
            check_ast(
                source,
                &[
                    type_hint(1),                  // String
                    id_with_type_hint(0, 0),       // x
                    type_hint(2),                  // Number
                    Ignored(None, Some(2.into())), // _
                    Ignored(Some(3.into()), None), // _y
                    id(4),                         // z - 5
                    id(5),                         // foo
                    id(0),                         // x
                    Block(nodes(&[7])),
                    For(AstFor {
                        args: nodes(&[1, 3, 4, 5]),
                        iterable: 6.into(),
                        body: 8.into(),
                    }),
                    MainBlock {
                        body: nodes(&[9]),
                        local_count: 2, // x, z
                    }, // 10
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("String"),
                    Constant::Str("Number"),
                    Constant::Str("y"),
                    Constant::Str("z"),
                    Constant::Str("foo"),
                ]),
            )
        }

        #[test]
        fn while_loop() {
            let source = "\
while x > y
  x";
            check_ast(
                source,
                &[
                    id(0), // x
                    id(1), // y
                    binary_op(AstBinaryOp::Greater, 0, 1),
                    id(0), // x
                    Block(nodes(&[3])),
                    While {
                        condition: 2.into(),
                        body: 4.into(),
                    }, // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn until_loop() {
            let source = "\
until x < y
  x";
            check_ast(
                source,
                &[
                    id(0), // x
                    id(1), // y
                    binary_op(AstBinaryOp::Less, 0, 1),
                    id(0), // x
                    Block(nodes(&[3])),
                    Until {
                        condition: 2.into(),
                        body: 4.into(),
                    }, // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn for_loop_after_array() {
            // A case that failed parsing at the start of the for block,
            // expecting an expression in the main block.
            let source = "\
[]
for x in y
  x";
            check_ast(
                source,
                &[
                    List(nodes(&[])),
                    id(0), // x
                    id(1), // y
                    id(0), // x
                    Block(nodes(&[3])),
                    For(AstFor {
                        args: nodes(&[1]),
                        iterable: 2.into(),
                        body: 4.into(),
                    }), // 5
                    MainBlock {
                        body: nodes(&[0, 5]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn for_with_range_from_chain_call() {
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
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[2]),
                            with_parens: false,
                        },
                        None,
                    )),
                    chain_id(2, Some(3)),
                    chain_root(1, Some(4)), // ast 5
                    id(0),                  // a
                    Block(nodes(&[6])),
                    For(AstFor {
                        args: nodes(&[0]),
                        iterable: 5.into(),
                        body: 7.into(),
                    }),
                    MainBlock {
                        body: nodes(&[8]),
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

        #[test]
        fn for_with_unpacked_map() {
            let source = "\
for {x} in [{x: 1}]
    x
";
            check_ast(
                source,
                &[
                    id(0),
                    map_pattern(&[0]),
                    id(0),
                    SmallInt(1),
                    map_entry(2, 3),
                    map_with_braces(&[4]),
                    List(nodes(&[5])),
                    id(0),
                    Block(nodes(&[7])),
                    For(AstFor {
                        args: nodes(&[1]),
                        iterable: 6.into(),
                        body: 8.into(),
                    }),
                    MainBlock {
                        body: nodes(&[9]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }

    mod functions {
        use super::*;

        #[test]
        fn two_args_with_type_hints() {
            let sources = [
                "
|x: String, y: Number = 42|
 x + y
",
                "
| x: String,
  y: Number = 42,
|
  x + y
",
                "
| x: String,
  y: Number=42
  |
    x + y
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    type_hint(1),            // String
                    id_with_type_hint(0, 0), // x
                    type_hint(3),            // Number
                    id_with_type_hint(2, 2), // y
                    SmallInt(42),
                    assign(3, 4), // - 5
                    FunctionArgs {
                        args: nodes(&[1, 5]),
                        variadic: false,
                        output_type: None,
                    },
                    id(0), // x
                    id(2), // y
                    binary_op(AstBinaryOp::Add, 7, 8),
                    Block(nodes(&[9])), // 10
                    Function(koto_parser::Function {
                        args: 6.into(),
                        local_count: 2,
                        accessed_non_locals: constants(&[]),
                        body: 10.into(),
                        is_generator: false,
                    }),
                    MainBlock {
                        body: nodes(&[11]),
                        local_count: 0,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("String"),
                    Constant::Str("y"),
                    Constant::Str("Number"),
                ]),
            )
        }

        #[test]
        fn output_type_hint() {
            let sources = [
                "
|x: String| -> String?
    x
",
                "
|x: String| -> String?
  x
",
                "
|x: String
| -> String?
  x
",
                "
|
  x: String
| -> String?
  x
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    type_hint(1),            // String
                    id_with_type_hint(0, 0), // x
                    optional_type_hint(1),   // String
                    FunctionArgs {
                        args: nodes(&[1]),
                        variadic: false,
                        output_type: Some(2.into()),
                    },
                    id(0),              // x
                    Block(nodes(&[4])), // 5
                    Function(koto_parser::Function {
                        args: 3.into(),
                        local_count: 1,
                        accessed_non_locals: constants(&[]),
                        body: 5.into(),
                        is_generator: false,
                    }),
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("String")]),
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
                    FunctionArgs {
                        args: nodes(&[1]),
                        variadic: false,
                        output_type: None,
                    },
                    id(2),        // y
                    id(1),        // x
                    assign(3, 4), // 5
                    id(2),
                    Block(nodes(&[5, 6])),
                    Function(koto_parser::Function {
                        args: 2.into(),
                        local_count: 2,
                        accessed_non_locals: constants(&[]),
                        body: 7.into(),
                        is_generator: false,
                    }),
                    assign(0, 8),
                    id(0), // 10 - f
                    SmallInt(42),
                    chain_call(&[11], false, None),
                    chain_root(10, Some(12)),
                    MainBlock {
                        body: nodes(&[9, 13]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x"), Constant::Str("y")]),
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
                    id(0), //foo
                    id(1), // x
                    id(2), // y
                    chain_call(&[1, 2], false, None),
                    chain_root(0, Some(3)),
                    MainBlock {
                        body: nodes(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("x"), Constant::Str("y")]),
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
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[1, 3]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_root(0, Some(4)),
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_negative_arg() {
            let source = "f x, -x";
            check_ast(
                source,
                &[
                    id(0),
                    id(1),
                    id(1),
                    unary_op(AstUnaryOp::Negate, 2),
                    chain_call(&[1, 3], false, None),
                    chain_root(0, Some(4)), // 5
                    MainBlock {
                        body: nodes(&[5]),
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
                    id(0), // f
                    id(1), // x
                    SmallInt(1),
                    binary_op(AstBinaryOp::Subtract, 1, 2),
                    chain_call(&[3], false, None),
                    chain_root(0, Some(4)), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn call_packed_arg_without_parentheses() {
            let source = "f a..., b";
            check_ast(
                source,
                &[
                    id(0),                      // f
                    id(1),                      // a
                    PackedExpression(1.into()), // a
                    id(2),                      // b
                    chain_call(&[2, 3], false, None),
                    chain_root(0, Some(4)), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("a"), Constant::Str("b")]),
            )
        }
        #[test]
        fn call_packed_arg_with_parentheses() {
            let source = "f(a..., b)";
            check_ast(
                source,
                &[
                    id(0),                      // f
                    id(1),                      // a
                    PackedExpression(1.into()), // a
                    id(2),                      // b
                    chain_call(&[2, 3], true, None),
                    chain_root(0, Some(4)), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("a"), Constant::Str("b")]),
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
                    FunctionArgs {
                        args: nodes(&[1]),
                        variadic: false,
                        output_type: None,
                    },
                    id(0),                         // f
                    id(1),                         // x
                    chain_call(&[4], false, None), // 5
                    chain_root(3, Some(5)),
                    Function(koto_parser::Function {
                        args: 2.into(),
                        local_count: 1,
                        accessed_non_locals: constants(&[0]),
                        body: 6.into(),
                        is_generator: false,
                    }),
                    assign(0, 7),
                    MainBlock {
                        body: nodes(&[8]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x")]),
            )
        }

        #[test]
        fn piped_call_chain() {
            let source = "f x -> g -> h";
            check_ast(
                source,
                &[
                    id(0), // f
                    id(1), // x
                    chain_call(&[1], false, None),
                    chain_root(0, Some(2)),
                    id(2),                              // g
                    binary_op(AstBinaryOp::Pipe, 3, 4), // 5
                    id(3),                              // h
                    binary_op(AstBinaryOp::Pipe, 5, 6),
                    MainBlock {
                        body: nodes(&[7]),
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
        fn indented_piped_calls_after_chain() {
            let source = "
foo.bar x
  -> y
  -> z
";
            check_ast(
                source,
                &[
                    id(0), // foo
                    id(2), // x
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    chain_id(1, Some(2)),
                    chain_root(0, Some(3)),
                    id(3), // 5 - y
                    binary_op(AstBinaryOp::Pipe, 4, 5),
                    id(4), // z
                    binary_op(AstBinaryOp::Pipe, 6, 7),
                    MainBlock {
                        body: nodes(&[8]),
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
        fn generator_function() {
            let source = "|| yield 1";
            check_ast(
                source,
                &[
                    FunctionArgs {
                        args: nodes(&[]),
                        variadic: false,
                        output_type: None,
                    },
                    SmallInt(1),
                    Yield(1.into()),
                    Function(koto_parser::Function {
                        args: 0.into(),
                        local_count: 0,
                        accessed_non_locals: constants(&[]),
                        body: 2.into(),
                        is_generator: true,
                    }),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                None,
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
                "
|
  a,
  (
    _,
    (others...,
        c,
    _d,
    ),
  ),
  _e,
|
 a
",
            ];
            check_ast_for_equivalent_sources(
                &sources,
                &[
                    id(0), // a
                    Ignored(None, None),
                    PackedId(Some(1.into())),      // others
                    id(2),                         // c
                    Ignored(Some(3.into()), None), // d
                    tuple_with_parens(&[2, 3, 4]), // ast index 5
                    tuple_with_parens(&[1, 5]),
                    Ignored(Some(4.into()), None), // e
                    FunctionArgs {
                        args: nodes(&[0, 6, 7]),
                        variadic: false,
                        output_type: None,
                    },
                    id(0),              // a
                    Block(nodes(&[9])), // 10
                    Function(koto_parser::Function {
                        args: 8.into(),
                        local_count: 3,
                        accessed_non_locals: constants(&[]),
                        body: 10.into(),
                        is_generator: false,
                    }),
                    MainBlock {
                        body: nodes(&[11]),
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
        fn multiline_comment_before_function_with_default_arg() {
            let source = "
#-
A bug previously broke default argument parsing when
the function was preceeded by a multiline comment.
-#

|foo = bar| foo
";
            check_ast(
                source,
                &[
                    id(0), // foo
                    id(1), // bar
                    assign(0, 1),
                    FunctionArgs {
                        args: nodes(&[2]),
                        variadic: false,
                        output_type: None,
                    },
                    id(0), // foo
                    Function(koto_parser::Function {
                        args: 3.into(),
                        local_count: 1,
                        accessed_non_locals: constants(&[]),
                        body: 4.into(),
                        is_generator: false,
                    }), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("bar")]),
            );
        }

        #[test]
        fn unpacked_map_argument() {
            let source = "\
f = |{x, y}|
  x + y
";
            check_ast(
                source,
                &[
                    id(0), // f
                    id(1), // x
                    id(2), // y
                    map_pattern(&[1, 2]),
                    FunctionArgs {
                        args: nodes(&[3]),
                        variadic: false,
                        output_type: None,
                    },
                    id(1), // x
                    id(2), // y
                    binary_op(AstBinaryOp::Add, 5, 6),
                    Block(nodes(&[7])),
                    Function(koto_parser::Function {
                        args: 4.into(),
                        local_count: 2,
                        accessed_non_locals: constants(&[]),
                        body: 8.into(),
                        is_generator: false,
                    }),
                    assign(0, 9),
                    MainBlock {
                        body: nodes(&[10]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x"), Constant::Str("y")]),
            )
        }
    }

    mod chains {
        use super::*;

        #[test]
        fn indexed_assignment() {
            let source = "a[0] = a[1]";

            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(0),
                    chain_index(1, None),
                    chain_root(0, Some(2)),
                    id(0),
                    SmallInt(1), // 5
                    chain_index(5, None),
                    chain_root(4, Some(6)),
                    assign(3, 7),
                    MainBlock {
                        body: nodes(&[8]),
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
                    chain_index(1, None),
                    chain_root(0, Some(2)),
                    MainBlock {
                        body: nodes(&[3]),
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
                    chain_index(2, None),
                    chain_root(0, Some(3)),
                    MainBlock {
                        body: nodes(&[4]),
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
                    chain_index(3, None),
                    chain_index(2, Some(4)), // 5
                    chain_root(0, Some(5)),
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn access_with_id() {
            let source = "x.foo";
            check_ast(
                source,
                &[
                    id(0),
                    chain_id(1, None),
                    chain_root(0, Some(1)),
                    MainBlock {
                        body: nodes(&[2]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn access_with_call() {
            let source = "x.bar()";
            check_ast(
                source,
                &[
                    id(0),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(1, Some(1)),
                    chain_root(0, Some(2)),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn access_call_arithmetic_arg() {
            let source = "x.bar() - 1";
            check_ast(
                source,
                &[
                    id(0),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(1, Some(1)),
                    chain_root(0, Some(2)),
                    SmallInt(1),
                    binary_op(AstBinaryOp::Subtract, 3, 4), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn access_assignment() {
            let source = r#"
x.bar()."baz" = 1
"#;
            check_ast(
                source,
                &[
                    id(0),
                    Chain((
                        ChainNode::Str(AstString {
                            quote: StringQuote::Double,
                            contents: StringContents::Literal(2.into()),
                        }),
                        None,
                    )),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        Some(1.into()),
                    )),
                    chain_id(1, Some(2)),
                    chain_root(0, Some(3)),
                    SmallInt(1), // 5
                    assign(4, 5),
                    MainBlock {
                        body: nodes(&[6]),
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
        fn access_space_separated_call() {
            let source = "x.foo 42";
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(42),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    chain_id(1, Some(2)),
                    chain_root(0, Some(3)),
                    MainBlock {
                        body: nodes(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn access_indentation_separated_call() {
            let source = "
x.foo
  42
";
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(42),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    chain_id(1, Some(2)),
                    chain_root(0, Some(3)),
                    MainBlock {
                        body: nodes(&[4]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn chain_indentation_separated_with_map_arg() {
            let source = "
x.takes_a_map
  foo: 42
";
            check_ast(
                source,
                &[
                    id(0), // x
                    id(2), // foo
                    SmallInt(42),
                    map_entry(1, 2),
                    map_block(&[3]),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[4]),
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    chain_id(1, Some(5)), // takes_a_map
                    chain_root(0, Some(6)),
                    MainBlock {
                        body: nodes(&[7]),
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
        fn map_access_in_list() {
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
                    chain_id(1, None),
                    chain_root(0, Some(1)),
                    id(0),
                    chain_id(2, None),
                    chain_root(3, Some(4)), // 5
                    List(nodes(&[2, 5])),
                    MainBlock {
                        body: nodes(&[6]),
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
        fn chain_on_call_result() {
            let source = "(f x).foo";
            check_ast(
                source,
                &[
                    id(0), // f
                    id(1), // x
                    chain_call(&[1], false, None),
                    chain_root(0, Some(2)),
                    Nested(3.into()),
                    chain_id(2, None), // 5
                    chain_root(4, Some(5)),
                    MainBlock {
                        body: nodes(&[6]),
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
                    id(0), // f
                    id(1), // x
                    chain_call(&[1], false, None),
                    chain_root(0, Some(2)),
                    Nested(3.into()),
                    SmallInt(0), // 5
                    chain_index(5, None),
                    chain_root(4, Some(6)),
                    MainBlock {
                        body: nodes(&[7]),
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
                    id(0), // f
                    id(1), // x
                    chain_call(&[1], false, None),
                    chain_root(0, Some(2)),
                    Nested(3.into()),
                    id(2), // 5 - y
                    chain_call(&[5], true, None),
                    chain_root(4, Some(6)),
                    MainBlock {
                        body: nodes(&[7]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("f"), Constant::Str("x"), Constant::Str("y")]),
            )
        }

        #[test]
        fn chain_on_number() {
            let source = "1.sin()";
            check_ast(
                source,
                &[
                    SmallInt(1),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(0, Some(1)),
                    chain_root(0, Some(2)),
                    MainBlock {
                        body: nodes(&[3]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("sin")]),
            )
        }

        #[test]
        fn chain_on_string() {
            let source = "'fox'.ends_with 'x'";
            check_ast(
                source,
                &[
                    string_literal(0, StringQuote::Single),
                    string_literal(2, StringQuote::Single),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[1]),
                            with_parens: false,
                        },
                        None,
                    )),
                    chain_id(1, Some(2)),
                    chain_root(0, Some(3)),
                    MainBlock {
                        body: nodes(&[4]),
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
        fn chain_on_tuple() {
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
                    tuple_with_parens(&[1, 2]),
                    id(2),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[4]),
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    chain_id(1, Some(5)),
                    chain_root(3, Some(6)),
                    assign(0, 7),
                    MainBlock {
                        body: nodes(&[8]),
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
        fn chain_on_list() {
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
                    List(nodes(&[1, 2])),
                    id(2),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[4]),
                            with_parens: false,
                        },
                        None,
                    )), // 5
                    chain_id(1, Some(5)),
                    chain_root(3, Some(6)),
                    assign(0, 7),
                    MainBlock {
                        body: nodes(&[8]),
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
        fn chain_on_map() {
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
                    id(1),
                    id(2),
                    map_with_braces(&[1, 2]),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(3, Some(4)), // 5 - values
                    chain_root(3, Some(5)),
                    assign(0, 6),
                    MainBlock {
                        body: nodes(&[7]),
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
        fn chain_on_range_same_line() {
            let source = "(0..1).size()";
            check_ast(
                source,
                &[
                    SmallInt(0),
                    SmallInt(1),
                    range(0, 1, false),
                    Nested(2.into()),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(0, Some(4)), // 5
                    chain_root(3, Some(5)),
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("size")]),
            )
        }

        #[test]
        fn chain_on_range_next_line() {
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
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(0, Some(3)),
                    chain_root(2, Some(4)), // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("size")]),
            )
        }

        #[test]
        fn nested_chain_call() {
            let source = "((x).contains y)";
            check_ast(
                source,
                &[
                    id(0),
                    Nested(0.into()),
                    id(2),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[2]),
                            with_parens: false,
                        },
                        None,
                    )),
                    chain_id(1, Some(3)),
                    chain_root(1, Some(4)), // 5
                    Nested(5.into()),
                    MainBlock {
                        body: nodes(&[6]),
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
        fn multiline_chain() {
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
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_id(3, Some(2)),
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[1]),
                            with_parens: false,
                        },
                        Some(3.into()),
                    )),
                    chain_id(2, Some(4)), // 5
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        Some(5.into()),
                    )),
                    chain_id(1, Some(6)),
                    chain_root(0, Some(7)),
                    MainBlock {
                        body: nodes(&[8]),
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
        fn chain_followed_by_continued_expression_on_next_line() {
            let source = "
foo.bar
  or foo.baz or
    false
";
            check_ast(
                source,
                &[
                    id(0),
                    chain_id(1, None),
                    chain_root(0, Some(1)),
                    id(0),
                    chain_id(2, None),
                    chain_root(3, Some(4)), // 5
                    binary_op(AstBinaryOp::Or, 2, 5),
                    BoolFalse,
                    binary_op(AstBinaryOp::Or, 6, 7),
                    MainBlock {
                        body: nodes(&[8]),
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

        mod null_checks {
            use super::*;

            #[test]
            fn after_root() {
                let source = "
foo?.bar
";
                check_ast(
                    source,
                    &[
                        id(0),
                        chain_id(1, None),
                        chain_null_check(1),
                        chain_root(0, Some(2)),
                        MainBlock {
                            body: nodes(&[3]),
                            local_count: 0,
                        },
                    ],
                    Some(&[Constant::Str("foo"), Constant::Str("bar")]),
                )
            }

            #[test]
            fn between_calls() {
                let source = "
foo()?()
";
                check_ast(
                    source,
                    &[
                        id(0),
                        chain_call(&[], true, None),
                        chain_null_check(1),
                        chain_call(&[], true, Some(2)),
                        chain_root(0, Some(3)),
                        MainBlock {
                            body: nodes(&[4]),
                            local_count: 0,
                        },
                    ],
                    Some(&[Constant::Str("foo")]),
                )
            }

            #[test]
            fn before_paren_free_call() {
                let source = "
foo? 42
";
                check_ast(
                    source,
                    &[
                        id(0),
                        SmallInt(42),
                        chain_call(&[1], false, None),
                        chain_null_check(2),
                        chain_root(0, Some(3)),
                        MainBlock {
                            body: nodes(&[4]),
                            local_count: 0,
                        },
                    ],
                    Some(&[Constant::Str("foo")]),
                )
            }
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
                        body: nodes(&[0, 1, 2, 4]),
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
                        body: nodes(&[1, 5]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("x + x")]),
            )
        }
    }

    mod semicolons {
        use super::*;

        #[test]
        fn separated_expressions_on_same_line() {
            let source = r#"
x = 1; x + x
"#;
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    assign(0, 1),
                    id(0),
                    id(0),
                    binary_op(AstBinaryOp::Add, 3, 4), // 5
                    MainBlock {
                        body: nodes(&[2, 5]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn separated_expressions_in_block() {
            let source = r#"
loop
  x = 1; y = x
"#;
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    assign(0, 1),
                    id(1),
                    id(0),
                    assign(3, 4), // 5
                    Block(nodes(&[2, 5])),
                    Loop { body: 6.into() },
                    MainBlock {
                        body: nodes(&[7]),
                        local_count: 2,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y")]),
            )
        }
    }

    mod import {
        use super::*;

        fn import_items(items: &[u32]) -> Vec<ImportItem> {
            items
                .iter()
                .map(|item| ImportItem {
                    item: item.into(),
                    name: None,
                })
                .collect()
        }

        #[test]
        fn import_single_item() {
            let source = "import foo";
            check_ast(
                source,
                &[
                    id(0), // foo
                    Import {
                        from: nodes(&[]),
                        items: import_items(&[0]),
                    },
                    MainBlock {
                        body: nodes(&[1]),
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
                    id(0), // foo
                    id(1), // bar
                    Import {
                        from: nodes(&[]),
                        items: vec![ImportItem {
                            item: 0.into(),
                            name: Some(1.into()),
                        }],
                    },
                    MainBlock {
                        body: nodes(&[2]),
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
                    id(0), // foo
                    id(1), // bar
                    Import {
                        from: nodes(&[0]),
                        items: import_items(&[1]),
                    },
                    MainBlock {
                        body: nodes(&[2]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("foo"), Constant::Str("bar")]),
            )
        }

        #[test]
        fn wildcard_import() {
            let source = "from foo import *";
            check_ast(
                source,
                &[
                    id(0), // foo
                    Import {
                        from: nodes(&[0]),
                        items: import_items(&[]),
                    },
                    MainBlock {
                        body: nodes(&[1]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("foo")]),
            )
        }

        #[test]
        fn import_item_used_in_assignment() {
            let source = "x = from foo import bar";
            check_ast(
                source,
                &[
                    id(0), // x
                    id(1), // foo
                    id(2), // bar
                    Import {
                        from: nodes(&[1]),
                        items: import_items(&[2]),
                    },
                    assign(0, 3),
                    MainBlock {
                        body: nodes(&[4]),
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
        fn import_multiple_items() {
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
                    id(0),                                  // foo
                    string_literal(1, StringQuote::Single), // bar
                    id(2),                                  // baz
                    Import {
                        from: nodes(&[]),
                        items: import_items(&[0, 1, 2]),
                    },
                    MainBlock {
                        body: nodes(&[3]),
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
                    id(0), // foo
                    id(1), // bar
                    id(2), // baz
                    Import {
                        from: nodes(&[0]),
                        items: import_items(&[1, 2]),
                    },
                    MainBlock {
                        body: nodes(&[3]),
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
                    string_literal(0, StringQuote::Single), // foo
                    id(1),                                  // bar
                    id(2),                                  // abc
                    id(3),                                  // xyz
                    Import {
                        from: nodes(&[0, 1]),
                        items: import_items(&[2, 3]),
                    },
                    MainBlock {
                        body: nodes(&[4]),
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
        fn try_catch_with_type_hints() {
            let source = "\
try
  f()
catch e: String
  e
catch x
  x
";
            check_ast(
                source,
                &[
                    id(0), // f
                    Chain((
                        ChainNode::Call {
                            args: nodes(&[]),
                            with_parens: true,
                        },
                        None,
                    )),
                    chain_root(0, Some(1)), // f
                    Block(nodes(&[2])),
                    type_hint(2),            // String
                    id_with_type_hint(1, 4), // 5 - e: String
                    id(1),                   // e
                    Block(nodes(&[6])),
                    id(3),              // x
                    id(3),              // x
                    Block(nodes(&[9])), // 10
                    Try(AstTry {
                        try_block: 3.into(),
                        catch_blocks: astvec![
                            AstCatch {
                                arg: 5.into(),
                                block: 7.into()
                            },
                            AstCatch {
                                arg: 8.into(),
                                block: 10.into()
                            }
                        ],
                        finally_block: None,
                    }),
                    MainBlock {
                        body: nodes(&[11]),
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("f"),
                    Constant::Str("e"),
                    Constant::Str("String"),
                    Constant::Str("x"),
                ]),
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
                        body: nodes(&[1]),
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
                        body: nodes(&[1]),
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
                    id(0), // data
                    id(1), // x
                    map_entry(0, 1),
                    id(2),                                  // message
                    string_literal(3, StringQuote::Double), // error!
                    map_entry(3, 4),                        // 5
                    map_block(&[2, 5]),
                    Throw(6.into()),
                    MainBlock {
                        body: nodes(&[7]),
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
                    MatchArm {
                        patterns: nodes(&[2, 3]),
                        condition: None,
                        expression: 4.into(),
                    }, // 5
                    id(2),
                    SmallInt(-1),
                    MatchArm {
                        patterns: nodes(&[6]),
                        condition: None,
                        expression: 7.into(),
                    },
                    Match {
                        expression: 1.into(),
                        arms: nodes(&[5, 8]),
                    },
                    assign(0, 9), // 10
                    MainBlock {
                        body: nodes(&[10]),
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
                    MatchArm {
                        patterns: nodes(&[1]),
                        condition: None,
                        expression: 2.into(),
                    },
                    string_literal(2, StringQuote::Double),
                    string_literal(3, StringQuote::Double), // 5
                    Break(None),
                    MatchArm {
                        patterns: nodes(&[4, 5]),
                        condition: None,
                        expression: 6.into(),
                    },
                    Match {
                        expression: 0.into(),
                        arms: nodes(&[3, 7]),
                    },
                    MainBlock {
                        body: nodes(&[8]),
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
        fn match_with_type_pattern() {
            let source = r#"
match x
  y: String then y
"#;
            check_ast(
                source,
                &[
                    id(0),                   // x
                    type_hint(2),            // String
                    id_with_type_hint(1, 1), // y
                    id(1),                   // y
                    MatchArm {
                        patterns: nodes(&[2]),
                        condition: None,
                        expression: 3.into(),
                    },
                    Match {
                        expression: 0.into(),
                        arms: nodes(&[4]),
                    }, // 5
                    MainBlock {
                        body: nodes(&[5]),
                        local_count: 1,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("String"),
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
                    tuple_with_parens(&[0, 1, 2]),
                    SmallInt(0),
                    id(3), // 5 - a
                    Ignored(None, None),
                    tuple_with_parens(&[4, 5, 6]),
                    id(3),
                    MatchArm {
                        patterns: nodes(&[7]),
                        condition: None,
                        expression: 8.into(),
                    },
                    Ignored(None, None), // 10
                    SmallInt(0),
                    id(4), // a
                    tuple_with_parens(&[11, 12]),
                    Ignored(Some(5.into()), None),
                    tuple_with_parens(&[10, 13, 14]), // 15
                    SmallInt(0),
                    MatchArm {
                        patterns: nodes(&[15]),
                        condition: None,
                        expression: 16.into(),
                    },
                    Match {
                        expression: 3.into(),
                        arms: nodes(&[9, 17]),
                    },
                    MainBlock {
                        body: nodes(&[18]),
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
                    PackedId(None),
                    SmallInt(0),
                    tuple_with_parens(&[1, 2]),
                    SmallInt(0),
                    MatchArm {
                        patterns: nodes(&[3]),
                        condition: None,
                        expression: 4.into(),
                    }, // 5
                    SmallInt(1),
                    PackedId(None),
                    tuple_with_parens(&[6, 7]),
                    SmallInt(1),
                    MatchArm {
                        patterns: nodes(&[8]),
                        condition: None,
                        expression: 9.into(),
                    }, // 10
                    Match {
                        expression: 0.into(),
                        arms: nodes(&[5, 10]),
                    },
                    MainBlock {
                        body: nodes(&[11]),
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
                    PackedId(Some(1.into())),
                    SmallInt(0),
                    SmallInt(1),
                    tuple_with_parens(&[1, 2, 3]),
                    SmallInt(0), // 5
                    MatchArm {
                        patterns: nodes(&[4]),
                        condition: None,
                        expression: 5.into(),
                    },
                    SmallInt(1),
                    SmallInt(0),
                    PackedId(Some(2.into())),
                    tuple_with_parens(&[7, 8, 9]), // 10
                    SmallInt(1),
                    MatchArm {
                        patterns: nodes(&[10]),
                        condition: None,
                        expression: 11.into(),
                    },
                    Match {
                        expression: 0.into(),
                        arms: nodes(&[6, 12]),
                    },
                    MainBlock {
                        body: nodes(&[13]),
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
                    TempTuple(nodes(&[0, 1])),
                    SmallInt(0),
                    SmallInt(1),
                    TempTuple(nodes(&[3, 4])), // 5
                    SmallInt(2),
                    SmallInt(3),
                    TempTuple(nodes(&[6, 7])),
                    id(2),
                    SmallInt(0), // 10
                    MatchArm {
                        patterns: nodes(&[5, 8]),
                        condition: Some(9.into()),
                        expression: 10.into(),
                    },
                    id(3), // a
                    Null,
                    TempTuple(nodes(&[12, 13])),
                    id(3), // a - 15
                    Block(nodes(&[15])),
                    MatchArm {
                        patterns: nodes(&[14]),
                        condition: None,
                        expression: 16.into(),
                    },
                    SmallInt(0),
                    MatchArm {
                        patterns: nodes(&[]),
                        condition: None,
                        expression: 18.into(),
                    },
                    Match {
                        expression: 2.into(),
                        arms: nodes(&[11, 17, 19]),
                    }, // 20
                    MainBlock {
                        body: nodes(&[20]),
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
        fn match_pattern_is_chain() {
            let source = "
match x
  y.foo then 0
";
            check_ast(
                source,
                &[
                    id(0),
                    id(1),
                    chain_id(2, None),
                    chain_root(1, Some(2)),
                    SmallInt(0),
                    MatchArm {
                        patterns: nodes(&[3]),
                        condition: None,
                        expression: 4.into(),
                    }, // 5
                    Match {
                        expression: 0.into(),
                        arms: nodes(&[5]),
                    },
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("y"), Constant::Str("foo")]),
            )
        }

        #[test]
        fn match_map() {
            let source = r#"
match {x: 1}
  {x} then x
"#;
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    map_entry(0, 1),
                    map_with_braces(&[2]),
                    id(0),
                    map_pattern(&[4]),
                    id(0),
                    MatchArm {
                        patterns: nodes(&[5]),
                        condition: None,
                        expression: 6.into(),
                    },
                    Match {
                        expression: 3.into(),
                        arms: nodes(&[7]),
                    },
                    MainBlock {
                        body: nodes(&[8]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }

        #[test]
        fn match_map_with_type() {
            let source = r#"
match {x: 1, @type: 'Foo'}
  {x}: Foo then x
"#;
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    map_entry(0, 1),
                    Meta(MetaKeyId::Type, None),
                    Str(simple_string(1, StringQuote::Single)),
                    map_entry(3, 4),
                    map_with_braces(&[2, 5]),
                    id(0),
                    type_hint(1),
                    map_pattern_with_type_hint(&[7], 8),
                    id(0),
                    MatchArm {
                        patterns: nodes(&[9]),
                        condition: None,
                        expression: 10.into(),
                    },
                    Match {
                        expression: 6.into(),
                        arms: nodes(&[11]),
                    },
                    MainBlock {
                        body: nodes(&[12]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("x"), Constant::Str("Foo")]),
            )
        }

        #[test]
        fn match_map_multi_line() {
            let source = "\
match {x: 1, y: 2}
  {
    x as a, 
    y as b
  } then 
    'ok'
";
            check_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    map_entry(0, 1),
                    id(1),
                    SmallInt(2),
                    map_entry(3, 4),
                    map_with_braces(&[2, 5]),
                    id(0),
                    id(2),
                    map_key_rebind(7, 8),
                    id(1),
                    id(3),
                    map_key_rebind(10, 11),
                    map_pattern(&[9, 12]),
                    Str(simple_string(4, StringQuote::Single)),
                    Block(nodes(&[14])),
                    MatchArm {
                        patterns: nodes(&[13]),
                        condition: None,
                        expression: 15.into(),
                    },
                    Match {
                        expression: 6.into(),
                        arms: nodes(&[16]),
                    },
                    MainBlock {
                        body: nodes(&[17]),
                        local_count: 2,
                    },
                ],
                Some(&[
                    Constant::Str("x"),
                    Constant::Str("y"),
                    Constant::Str("a"),
                    Constant::Str("b"),
                    Constant::Str("ok"),
                ]),
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
                    SwitchArm {
                        condition: Some(2.into()),
                        expression: 3.into(),
                    },
                    id(0), // 5
                    id(1),
                    binary_op(AstBinaryOp::Greater, 5, 6),
                    SmallInt(1),
                    SwitchArm {
                        condition: Some(7.into()),
                        expression: 8.into(),
                    },
                    id(0), // 10
                    SwitchArm {
                        condition: None,
                        expression: 10.into(),
                    },
                    Switch(nodes(&[4, 9, 11])),
                    MainBlock {
                        body: nodes(&[12]),
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
                    SwitchArm {
                        condition: Some(0.into()),
                        expression: 1.into(),
                    },
                    id(0),
                    Debug {
                        expression_string: 0.into(),
                        expression: 3.into(),
                    },
                    SwitchArm {
                        condition: None,
                        expression: 4.into(),
                    }, // 5
                    Switch(nodes(&[2, 5])),
                    MainBlock {
                        body: nodes(&[6]),
                        local_count: 0,
                    },
                ],
                Some(&[Constant::Str("x")]),
            )
        }
    }

    #[cfg(feature = "error_ast")]
    mod partial_ast_after_error {
        use super::*;

        fn check_error_ast(
            source: &str,
            expected_ast: &[Node],
            expected_constants: Option<&[Constant]>,
        ) {
            println!("{source}");

            match Parser::parse(source) {
                Ok(_) => panic!("Expected parsing to fail"),
                Err(error) => {
                    if let Some(ast) = error.ast {
                        compare_ast(expected_ast, expected_constants, &ast);
                    } else {
                        panic!("Missing AST after error ({error})");
                    }
                }
            }
        }

        #[test]
        fn after_assign() {
            let source = "
a = 1
!
123
";
            check_error_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    assign(0, 1),
                    MainBlock {
                        body: nodes(&[2]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a")]),
            )
        }

        #[test]
        fn mid_assignment_on_second_line() {
            let source = "
a = 1
b = !
x()
";
            check_error_ast(
                source,
                &[
                    id(0),
                    SmallInt(1),
                    assign(0, 1),
                    id(1),
                    MainBlock {
                        body: nodes(&[2, 3]),
                        local_count: 1,
                    },
                ],
                Some(&[Constant::Str("a"), Constant::Str("b")]),
            )
        }

        #[test]
        fn error_in_function() {
            let source = "\
f = |x|
  y = x
  z = y
  !
f 99
";
            check_error_ast(
                source,
                &[
                    id(0), // f
                    id(1), // x
                    FunctionArgs {
                        args: nodes(&[1]),
                        variadic: false,
                        output_type: None,
                    },
                    id(2),        // y
                    id(1),        // x
                    assign(3, 4), // 5
                    id(3),        // z
                    id(2),        // y
                    assign(6, 7),
                    Block(nodes(&[5, 8])),
                    Function(koto_parser::Function {
                        args: 2.into(),
                        local_count: 3,
                        accessed_non_locals: constants(&[]),
                        body: 9.into(),
                        is_generator: false,
                    }), // 10
                    MainBlock {
                        body: nodes(&[10]),
                        local_count: 0,
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
    }
}
