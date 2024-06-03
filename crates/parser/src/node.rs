use crate::{ast::AstIndex, constant_pool::ConstantIndex, StringFormatOptions, StringQuote};
use smallvec::SmallVec;
use std::fmt;

/// The vec type used in the AST
//
//  Q. Why 4 elements in the small vec?
//  A. It's the maximum number of elements that can be used in [Node] without increasing its overall
//     size.
pub type AstVec<T> = SmallVec<[T; 4]>;

/// A convenience macro for initializing [AstVec]s
pub use smallvec::smallvec as astvec;

/// A parsed node that can be included in the [AST](crate::Ast).
///
/// Nodes refer to each other via [AstIndex]s, see [AstNode](crate::AstNode).
#[derive(Clone, Debug, Default, PartialEq, Eq, derive_name::VariantName)]
pub enum Node {
    /// The `null` keyword
    #[default]
    Null,

    /// A single expression wrapped in parentheses
    Nested(AstIndex),

    /// An identifer, and optionally the type hint node
    Id(ConstantIndex, Option<AstIndex>),

    /// A meta identifier, e.g. `@display` or `@test my_test`
    Meta(MetaKeyId, Option<ConstantIndex>),

    /// A chained expression, and optionally the node that follows it in the chain
    Chain((ChainNode, Option<AstIndex>)), // chain node, next node

    /// The `true` keyword
    BoolTrue,

    /// The `false` keyword
    BoolFalse,

    /// An integer in the range -255..=255
    SmallInt(i16),

    /// An integer outside of the range -255..=255
    Int(ConstantIndex),

    /// A float literal
    Float(ConstantIndex),

    /// A string literal
    Str(AstString),

    /// A list literal
    ///
    /// e.g. `[foo, bar, 42]`
    List(AstVec<AstIndex>),

    /// A tuple literal
    ///
    /// e.g. `(foo, bar, 42)`
    ///
    /// Note that this is also used for implicit tuples, e.g. in `x = 1, 2, 3`
    Tuple(AstVec<AstIndex>),

    /// A temporary tuple
    ///
    /// Used in contexts where the result won't be exposed directly to the use, e.g.
    /// `x, y = 1, 2` - here `x` and `y` are indexed from the temporary tuple.
    /// `match foo, bar...` - foo and bar will be stored in a temporary tuple for comparison.
    TempTuple(AstVec<AstIndex>),

    /// A range with a defined start and end
    Range {
        /// The start of the range
        start: AstIndex,
        /// The end of the range
        end: AstIndex,
        /// Whether or not the end of the range includes the end value itself
        ///
        /// e.g. `1..10` - a range from 1 up to but not including 10
        /// e.g. `1..=10` - a range from 1 up to and including 10
        inclusive: bool,
    },

    /// A range without a defined end
    RangeFrom {
        /// The start of the range
        start: AstIndex,
    },

    /// A range without a defined start
    RangeTo {
        /// The end of the range
        end: AstIndex,
        /// Whether or not the end of the range includes the end value itself
        inclusive: bool,
    },

    /// The range operator without defined start or end
    ///
    /// Used when indexing a list or tuple, and the full contents are to be returned.
    RangeFull,

    /// A map literal, with a series of keys and values
    ///
    /// Keys will either be Id, String, or Meta nodes.
    ///
    /// Values are optional for inline maps.
    Map(Vec<(AstIndex, Option<AstIndex>)>),

    /// The `self` keyword
    Self_,

    /// The main block node
    ///
    /// Typically all ASTs will have this node at the root.
    MainBlock {
        /// The main block's body as a series of expressions
        body: AstVec<AstIndex>,
        /// The number of locally assigned values in the main block
        ///
        /// This tells the compiler how many registers need to be reserved for locally
        /// assigned values.
        local_count: usize,
    },

    /// A block node
    ///
    /// Used for indented blocks that share the context of the frame they're in,
    /// e.g. if expressions, arms in match or switch experssions, loop bodies
    Block(AstVec<AstIndex>),

    /// A function node
    Function(Function),

    /// An import expression
    ///
    /// e.g. `from foo.bar import baz, 'qux'
    Import {
        /// Where the items should be imported from
        ///
        /// An empty list here implies that import without `from` has been used.
        from: AstVec<AstIndex>,
        /// The series of items to import
        items: Vec<ImportItem>,
    },

    /// An export expression
    ///
    /// The export item will be a map literal, with each map entry added to the exports map
    Export(AstIndex),

    /// An assignment expression
    ///
    /// Used for single-assignment, multiple-assignment is represented by [Node::MultiAssign].
    Assign {
        /// The target of the assignment
        target: AstIndex,
        /// The expression to be assigned
        expression: AstIndex,
    },

    /// A multiple-assignment expression
    ///
    /// e.g. `x, y = foo()`, or `foo, bar, baz = 1, 2, 3`
    MultiAssign {
        /// The targets of the assignment
        targets: AstVec<AstIndex>,
        /// The expression to be assigned
        expression: AstIndex,
    },

    /// A unary operation
    UnaryOp {
        /// The operator to use
        op: AstUnaryOp,
        /// The value used in the operation
        value: AstIndex,
    },

    /// A binary operation
    BinaryOp {
        /// The operator to use
        op: AstBinaryOp,
        /// The "left hand side" of the operation
        lhs: AstIndex,
        /// The "right hand side" of the operation
        rhs: AstIndex,
    },

    /// An if expression
    If(AstIf),

    /// A match expression
    Match {
        /// The expression that will be matched against
        expression: AstIndex,
        /// The series of arms that match against the provided expression
        arms: Vec<MatchArm>,
    },

    /// A switch expression
    Switch(AstVec<SwitchArm>),

    /// A `_` identifier
    ///
    /// Used as a placeholder for unused function arguments or unpacked values, or as a wildcard
    /// in match expressions.
    ///
    /// Comes with an optional name, e.g. `_foo` will have `foo` stored as a constant.
    Wildcard(Option<ConstantIndex>, Option<AstIndex>),

    /// The `...` operator
    ///
    /// Used when capturing variadic arguments, and when unpacking list or tuple values.
    Ellipsis(Option<ConstantIndex>),

    /// A `for` loop
    For(AstFor),

    /// A `loop` expression
    Loop {
        /// The loop's body
        body: AstIndex,
    },

    /// A `while` loop
    While {
        /// The condition for the while loop
        condition: AstIndex,
        /// The body of the while loop
        body: AstIndex,
    },

    /// An `until` expression
    Until {
        /// The condition for the until loop
        condition: AstIndex,
        /// The body of the until loop
        body: AstIndex,
    },

    /// The break keyword, with optional break value
    Break(Option<AstIndex>),

    /// The continue keyword
    Continue,

    /// A return expression, with optional return value
    Return(Option<AstIndex>),

    /// A try expression
    Try(AstTry),

    /// A throw expression
    Throw(AstIndex),

    /// A yield expression
    Yield(AstIndex),

    /// A debug expression
    Debug {
        /// The stored string of the debugged expression to be used when printing the result
        expression_string: ConstantIndex,
        /// The expression that should be debugged
        expression: AstIndex,
    },

    /// A type hint
    ///
    /// e.g. `let x: Number = 0`
    ///            ^~~ This is the beginning of the type hint
    Type(ConstantIndex),
}

/// A function definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    /// The function's arguments
    pub args: AstVec<AstIndex>,
    /// The number of locally assigned values
    ///
    /// Used by the compiler when reserving registers for local values at the start of the frame.
    pub local_count: usize,
    /// Any non-local values that are accessed in the function
    ///
    /// Any ID (or chain root) that's accessed in a function and which wasn't previously assigned
    /// locally, is either an export or the value needs to be captured. The compiler takes care of
    /// determining if an access is a capture or not at the moment the function is created.
    pub accessed_non_locals: AstVec<ConstantIndex>,
    /// The function's body
    pub body: AstIndex,
    /// A flag that indicates if the function arguments end with a variadic `...` argument
    pub is_variadic: bool,
    /// A flag that indicates if the function is a generator or not
    ///
    /// The presence of a `yield` expression in the function body will set this to true.
    pub is_generator: bool,
    /// The optional output type of the function
    pub output_type: Option<AstIndex>,
}

/// A string definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstString {
    /// Indicates if single or double quotation marks were used
    pub quote: StringQuote,
    /// The string's contents
    pub contents: StringContents,
}

/// The contents of an [AstString]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StringContents {
    /// A string literal
    Literal(ConstantIndex),
    /// A raw string literal
    Raw {
        /// The literal's constant index
        constant: ConstantIndex,
        /// The number of hashes associated with the raw string's delimiter
        hash_count: u8,
    },
    /// An interpolated string
    ///
    /// An interpolated string is made up of a series of literals and template expressions,
    /// which are then joined together using a string builder.
    Interpolated(Vec<StringNode>),
}

/// A node in a string definition
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StringNode {
    /// A string literal
    Literal(ConstantIndex),
    /// An expression that should be evaluated and inserted into the string
    Expression {
        /// The expressions AST index
        expression: AstIndex,
        /// Formatting options for the rendered expression
        format: StringFormatOptions,
    },
}

/// A for loop definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstFor {
    /// The ids that capture each iteration's output values, or wildcards that ignore them
    pub args: AstVec<AstIndex>,
    /// The expression that produces an iterable value
    pub iterable: AstIndex,
    /// The body of the for loop
    pub body: AstIndex,
}

/// An if expression definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstIf {
    /// The if expression's condition
    pub condition: AstIndex,
    /// The if expression's `then` branch
    pub then_node: AstIndex,
    /// An optional series of `else if` conditions and branches
    pub else_if_blocks: AstVec<(AstIndex, AstIndex)>,
    /// An optional `else` branch
    pub else_node: Option<AstIndex>,
}

/// An operation used in UnaryOp expressions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum AstUnaryOp {
    Negate,
    Not,
}

/// An operation used in BinaryOp expressions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum AstBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    AddAssign,
    SubtractAssign,
    MultiplyAssign,
    DivideAssign,
    RemainderAssign,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    And,
    Or,
    Pipe,
}

/// A try expression definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstTry {
    /// The block that's wrapped by the try
    pub try_block: AstIndex,
    /// The identifier that will receive a caught error, or a wildcard
    pub catch_arg: AstIndex,
    /// The try expression's catch block
    pub catch_block: AstIndex,
    /// An optional `finally` block
    pub finally_block: Option<AstIndex>,
}

/// A node in a chained expression
///
/// Chains are any expressions that contain two or more nodes in a sequence.
///
/// In other words, some series of operations involving indexing, `.` accesses, and function calls.
///
/// e.g.
/// `foo.bar."baz"[0](42)`
///  |  |   |     |  ^ Call {args: 42, with_parens: true}
///  |  |   |     ^ Index (0)
///  |  |   ^ Str (baz)
///  |  ^ Id (bar)
///  ^ Root (foo)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainNode {
    /// The root of the chain
    Root(AstIndex),
    /// A `.` access using an identifier
    Id(ConstantIndex),
    /// A `.` access using a string
    Str(AstString),
    /// An index operation using square `[]` brackets.
    Index(AstIndex),
    /// A function call
    Call {
        /// The arguments used in the function call
        args: AstVec<AstIndex>,
        /// Whether or not parentheses are present in the function call
        ///
        /// This is not cosmetic, as parentheses represent a 'closed call', which has an impact on
        /// function piping:
        /// e.g.
        ///   `99 -> foo.bar 42` is equivalent to `foo.bar(42, 99)`
        /// but:
        ///   `99 -> foo.bar(42)` is equivalent to `foo.bar(42)(99)`.
        with_parens: bool,
    },
}

/// An arm in a match expression
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchArm {
    /// A series of match patterns
    ///
    /// If `patterns` is empty then `else` is implied, and should always appear as the last arm.
    pub patterns: AstVec<AstIndex>,
    /// An optional condition for the match arm
    ///
    /// e.g.
    /// match foo
    ///   bar if check_condition bar then ...
    pub condition: Option<AstIndex>,
    /// The body of the match arm
    pub expression: AstIndex,
}

impl MatchArm {
    /// Returns true if the arm is `else`
    pub fn is_else(&self) -> bool {
        self.patterns.is_empty()
    }
}

/// An arm in a switch expression
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchArm {
    /// An optional condition for the switch arm
    ///
    /// None implies `else`, and should always appear as the last arm.
    pub condition: Option<AstIndex>,
    /// The body of the switch arm
    pub expression: AstIndex,
}

impl SwitchArm {
    /// Returns true if the arm is `else`
    pub fn is_else(&self) -> bool {
        self.condition.is_none()
    }
}

/// A meta key
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MetaKeyId {
    /// @+
    Add,
    /// @-
    Subtract,
    /// @*
    Multiply,
    /// @/
    Divide,
    /// @%
    Remainder,
    /// @+=
    AddAssign,
    /// @-=
    SubtractAssign,
    /// @*=
    MultiplyAssign,
    /// @/=
    DivideAssign,
    /// @%=
    RemainderAssign,
    /// @<
    Less,
    /// @<=
    LessOrEqual,
    /// @>
    Greater,
    /// @>=
    GreaterOrEqual,
    /// @==
    Equal,
    /// @!=
    NotEqual,
    /// @[]
    Index,

    /// @display
    Display,
    /// @iterator
    Iterator,
    /// @next
    Next,
    /// @next_back
    NextBack,
    /// @negate
    Negate,
    /// @size
    Size,
    /// @type
    Type,
    /// @base
    Base,

    /// @||
    Call,

    /// @tests
    Tests,
    /// @test test_name
    Test,
    /// @pre_test
    PreTest,
    /// @post_test
    PostTest,

    /// @main
    Main,

    /// @meta name
    Named,

    /// Unused
    ///
    /// This entry must be last, see `TryFrom<u7>` for [MetaKeyId]
    Invalid,
}

impl TryFrom<u8> for MetaKeyId {
    type Error = u8;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        if byte < Self::Invalid as u8 {
            // Safety: Any value less than Invalid is safe to transmute.
            Ok(unsafe { std::mem::transmute::<u8, Self>(byte) })
        } else {
            Err(byte)
        }
    }
}

// Display impl used by koto-ls
impl fmt::Display for MetaKeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use MetaKeyId::*;

        write!(
            f,
            "@{}",
            match self {
                Add => "+",
                Subtract => "-",
                Multiply => "*",
                Divide => "/",
                Remainder => "%",
                AddAssign => "+=",
                SubtractAssign => "-=",
                MultiplyAssign => "*=",
                DivideAssign => "/=",
                RemainderAssign => "%=",
                Less => "<",
                LessOrEqual => "<=",
                Greater => ">",
                GreaterOrEqual => ">=",
                Equal => "==",
                NotEqual => "!=",
                Index => "[]",
                Display => "display",
                Iterator => "iterator",
                Next => "next",
                NextBack => "next_back",
                Negate => "negate",
                Size => "size",
                Type => "type",
                Base => "base",
                Call => "||",
                Tests => "tests",
                Test => "test",
                PreTest => "pre_test",
                PostTest => "post_test",
                Main => "main",
                Named => "meta",
                Invalid => unreachable!(),
            }
        )
    }
}

/// A node in an import item, see [Node::Import]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportItem {
    /// The imported item
    pub item: AstIndex,
    /// An optional 'as' name for the imported item
    pub name: Option<AstIndex>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_size() {
        // Think carefully before allowing Node to increase in size
        let size = std::mem::size_of::<Node>();
        let maximum_size = 72;
        assert!(
            size <= maximum_size,
            "Node has a size of {size} bytes, the allowed maximum is {maximum_size} bytes"
        );
    }
}
