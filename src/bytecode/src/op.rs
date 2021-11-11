/// The operations used in Koto bytecode
///
/// Each operation is made up of a byte, followed by N additional bytes that define its behaviour.
/// The combined operation bytes are interpreted as an [Instruction](crate::Instruction) by the
/// [InstructionReader](crate::InstructionReader).
///
/// In the comments for each operation, the additional bytes are specified inside square brackets.
/// Bytes prefixed with * show that the byte is referring to a register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
#[allow(missing_docs)] // Allowed for the UnusedX ops
pub enum Op {
    /// Copies the source value to the target register
    ///
    /// `[Copy, *target, *source]`
    Copy,

    /// Sets a register to contain Empty
    ///
    /// `[*target]`
    SetEmpty,

    /// Sets a register to contain Bool(false)
    ///
    /// `[*target]`
    SetFalse,

    /// Sets a register to contain Bool(true)
    ///
    /// `[*target]`
    SetTrue,

    /// Sets a register to contain Int(0)
    ///
    /// `[*target]`
    Set0,

    /// Sets a register to contain Int(1)
    ///
    /// `[*target]`
    Set1,

    /// Sets a register to contain Int(n)
    ///
    /// `[*target, n]`
    SetNumberU8,

    /// Loads an f64 constant into a register
    ///
    /// `[*target, constant]`
    LoadFloat,

    /// Loads an f64 constant with a u16 index into a register
    ///
    /// `[*target, constant[2]]`
    LoadFloat16,

    /// Loads an f64 constant with a u24 index into a register
    ///
    /// `[*target, constant[3]]`
    LoadFloat24,

    /// Loads an i64 constant into a register
    ///
    /// `[*target, constant]`
    LoadInt,

    /// Loads an i64 constant with a u16 index into a register
    ///
    /// `[*target, constant[2]]`
    LoadInt16,

    /// Loads an i64 constant with a u24 index into a register
    ///
    /// `[*target, constant[3]]`
    LoadInt24,

    /// Loads a string constant into a register
    ///
    /// `[*target, constant]`
    LoadString,

    /// Loads a string constant with a u16 index into a register
    ///
    /// `[*target, constant[2]]`
    LoadString16,

    /// Loads a string constant with a u24 index into a register
    ///
    /// `[*target, constant[3]]`
    LoadString24,

    /// Loads a non-local value into a register
    ///
    /// `[*target, constant]`
    LoadNonLocal,

    /// Loads a non-local value with a u16 id index into a register
    ///
    /// `[*target, constant[2]]`
    LoadNonLocal16,

    /// Loads a non-local value with a u24 id index into a register
    ///
    /// `[*target, constant[3]]`
    LoadNonLocal24,

    /// Imports a value
    ///
    /// The name of the value to be imported will be placed in the register before running this op,
    /// the imported value will then be placed in the same register.
    ///
    /// `[*register]`
    Import,

    /// Makes a temporary tuple out of values stored in consecutive registers
    ///
    /// Used when a tuple is made which won't be assigned to a value,
    /// e.g. in multiple-assignment: `x, y, z = 1, 2, 3`
    ///
    /// `[*target, *start, value count]`
    MakeTempTuple,

    /// Makes an empty map with the given size hint
    ///
    /// `[*target, size hint]`
    MakeMap,

    /// Makes an empty map with the given u32 size hint
    ///
    /// `[*target, size hint[4]]`
    MakeMap32,

    /// Makes a Num2 in the target register
    ///
    /// `[*target, *start, value count]`
    MakeNum2,

    /// Makes a Num4 in the target register
    ///
    /// `[*target, *start, value count]`
    MakeNum4,

    /// Makes an Iterator out of an iterable value
    ///
    /// `[*target, *iterable]`
    MakeIterator,

    /// Makes a SequenceBuilder with a u8 size hint
    ///
    /// `[*target, size hint]`
    SequenceStart,

    /// Makes a SequenceBuilder with a u32 size hint
    ///
    /// `[*target, size hint[4]]`
    SequenceStart32,

    /// Pushes a single value to the end of a SequenceBuilder
    ///
    /// `[*target, *value]`
    SequencePush,

    /// Pushes values from consecutive registers to the end of a SequenceBuilder
    ///
    /// `[*target, *start, value count]`
    SequencePushN,

    /// Converts a SequenceBuilder into a List
    ///
    /// `[*register]`
    SequenceToList,

    /// Converts a SequenceBuilder into a Tuple
    ///
    /// `[*register]`
    SequenceToTuple,

    /// Makes a StringBuilder with a u8 size hint
    ///
    /// `[*target, size hint]`
    StringStart,

    /// Makes a StringBuilder with a u32 size hint
    ///
    /// `[*target, size hint[4]]`
    StringStart32,

    /// Pushes a value to the end of a StringBuilder
    ///
    /// Strings will have their contents added directly to the StringBuilder
    /// Other values will be formatted to a string and then added to the StrignBuilder.
    ///
    /// `[*target, *value]`
    StringPush,

    /// Replaces a StringBuilder with a String containing the builder's contents
    ///
    /// `[*target]`
    StringFinish,

    /// Makes a SimpleFunction
    ///
    /// The N size bytes following this instruction make up the body of the function.
    ///
    /// `[*target, arg count, function size[2]]`
    SimpleFunction,

    /// Makes a Function
    ///
    /// Like a SimpleFunction, but with extended properties and captured values.
    ///
    /// The flags are a bitfield constructed from [FunctionFlags](crate::FunctionFlags).
    /// The N size bytes following this instruction make up the body of the function.
    ///
    /// `[*target, arg count, capture count, flags, function size[2]]`
    Function,

    /// Captures a value for a Function
    ///
    /// The value gets cloned to the Function's captures list at the given index.
    ///
    /// `[*function, capture index, *value]`
    Capture,

    /// Makes a Range with defined start and end values
    ///
    /// `[*target, *start, *end]`
    Range,

    /// Makes an inclusive Range with defined start and end values
    ///
    /// `[*target, *start, *end]`
    RangeInclusive,

    /// Makes a Range with a defined end value and no start
    ///
    /// `[*target, *end]`
    RangeTo,

    /// Makes an inclusive Range with a defined end value and no start
    ///
    /// `[*target, *end]`
    RangeToInclusive,

    /// Makes a Range with a defined start value and no end
    ///
    /// `[*target, *start]`
    RangeFrom,

    /// Makes a full Range with undefined start and end
    ///
    /// `[*target]`
    RangeFull,

    /// Negates a value
    ///
    /// Used for the unary negation operator, i.e. `x = -y`
    ///
    /// `[*target, *source]`
    Negate,

    /// Flips the value of a boolean
    ///
    /// `[*target, *source]`
    Not,

    /// Adds lhs and rhs together
    ///
    /// `[*result, *lhs, *rhs]`
    Add,

    /// Subtracts rhs from lhs
    ///
    /// `[*result, *lhs, *rhs]`
    Subtract,

    /// Multiplies lhs and rhs together
    ///
    /// `[*result, *lhs, *rhs]`
    Multiply,

    /// Divides lhs by rhs
    ///
    /// `[*result, *lhs, *rhs]`
    Divide,

    /// Performs the modulo operation with lhs and rhs
    ///
    /// `[*result, *lhs, *rhs]`
    Modulo,

    /// Compares lhs and rhs using the '<' operator
    ///
    /// `[*result, *lhs, *rhs]`
    Less,

    /// Compares lhs and rhs using the '<=' operator
    ///
    /// `[*result, *lhs, *rhs]`
    LessOrEqual,

    /// Compares lhs and rhs using the '>' operator
    ///
    /// `[*result, *lhs, *rhs]`
    Greater,

    /// Compares lhs and rhs using the '>=' operator
    ///
    /// `[*result, *lhs, *rhs]`
    GreaterOrEqual,

    /// Compares lhs and rhs using the '==' operator
    ///
    /// `[*result, *lhs, *rhs]`
    Equal,

    /// Compares lhs and rhs using the '!=' operator
    ///
    /// `[*result, *lhs, *rhs]`
    NotEqual,

    /// Causes the instruction pointer to jump forward by a number of bytes
    ///
    /// `[offset[2]]`
    Jump,

    /// Causes the instruction pointer to jump forward, if a condition is true
    ///
    /// `[*condition, offset[2]]`
    JumpTrue,

    /// Causes the instruction pointer to jump forward, if a condition is false
    ///
    /// `[*condition, offset[2]]`
    JumpFalse,

    /// Causes the instruction pointer to jump back by a number of bytes
    ///
    /// `[offset[2]]`
    JumpBack,

    /// Calls a function
    ///
    /// `[*result, *function, *first arg, arg count]`
    Call,

    /// Calls an instance function
    ///
    /// `[*result, *function, *first arg, arg count, *instance]`
    CallInstance,

    /// Returns from the current frame with the given result
    ///
    /// `[*result]`
    Return,

    /// Yields a value from the current generator
    ///
    /// `[*value]`
    Yield,

    /// Throws an error
    ///
    /// `[*error]`
    Throw,

    /// Gets the next value from an Iterator
    ///
    /// The output from the iterator is placed in the output register.
    /// If the iterator is finished then the instruction jumps forward by the given offset.
    ///
    /// `[*output, *iterator, offset[2]]`
    IterNext,

    /// Gets the next value from an Iterator, used when the output is treated as temporary
    ///
    /// The output from the iterator is placed in the output register.
    /// The output is treated as temporary, with assigned values being unpacked from the output.
    ///   - e.g. `for key, value in map`
    /// If the iterator is finished then the instruction jumps forward by the given offset.
    ///
    /// `[*output, *iterator, offset[2]]`
    IterNextTemp,

    /// Gets the next value from an Iterator, used when the output can be ignored
    ///
    /// If the iterator is finished then the instruction jumps forward by the given offset.
    ///
    /// `[*iterator, offset[2]]`
    IterNextQuiet,

    /// Accesses a contained value from a temporary value using a u8 index
    ///
    /// This is used for internal indexing operations.
    /// e.g. when unpacking a temporary value in multi-assignment
    ///
    /// `[*result, *value, index]`
    TempIndex,

    /// Takes a slice from the end of a given List or Tuple, starting from a u8 index
    ///
    /// Used in unpacking expressions, e.g. in a match arm
    ///
    /// `[*result, *value, index]`
    SliceFrom,

    /// Takes a slice from the start of a given List or Tuple, ending at a u8 index
    ///
    /// Used in unpacking expressions, e.g. in a match arm
    ///
    /// `[*result, *value, index]`
    SliceTo,

    /// Accesses a contained value via index
    ///
    /// `[*result, *indexable, *index]`
    Index,

    /// Sets a contained value via index
    ///
    /// `[*indexable, *value, *index]`
    SetIndex,

    /// Inserts a key/value entry into a map
    ///
    /// `[*map, *key, *value]`
    MapInsert,

    /// Inserts a key/value entry into a map's metamap
    ///
    /// `[*map, *key, *value]`
    MetaInsert,

    /// Inserts a named key/value entry into a map's metamap
    ///
    /// Used for meta keys that take a name as part of the key, like @test or @meta
    ///
    /// `[*map, *key, *name, *value]`
    MetaInsertNamed,

    /// Adds a key/value entry into the module's exported metamap
    ///
    /// Used for expressions like `export @tests = ...`
    ///
    /// `[*key, *value]`
    MetaExport,

    /// Adds a named key/value entry into the module's exported metamap
    ///
    /// Used for expressions like `export @tests = ...`
    ///
    /// `[*key, *name, *value]`
    MetaExportNamed,

    /// Exports a value by adding it to the module's exports map
    ///
    /// Used for expressions like `export foo = ...`
    ///
    /// `[*name, *value]`
    ValueExport,

    /// Access a contained value via key
    ///
    /// Used in '.' access operations.
    ///
    /// `[*result, *value, *key]`
    Access,

    /// Sets the result register to true if the value is a List
    ///
    /// `[*result, *value]`
    IsList,

    /// Sets the result register to true if the value is a Tuple
    ///
    /// `[*result, *value]`
    IsTuple,

    /// Gets the size of a value
    ///
    /// `[*result, *value]`
    Size,

    /// Starts a try block
    ///
    /// If an error is thrown in the try block then the error will be placed in the error register
    /// and the instruction pointer will be jumped forward to the location referred to by the catch
    /// offset.
    ///
    /// `[*error, catch offset[2]]`
    TryStart,

    /// Ends a try block
    ///
    /// `[]`
    TryEnd,

    /// Displays the contents of a value along with the source expression that produced it
    ///
    /// `[*value, expression constant[3]]`
    Debug,

    /// Throws an error if the value doesn't match the expected type
    ///
    /// Used when matching function arguments.
    ///
    /// See [TypeId](crate::TypeId) for the list of types that are checked against.
    ///
    /// `[*value, type]`
    CheckType,

    /// Throws an error if the value doesn't match the expected size
    ///
    /// Used when matching function arguments.
    ///
    /// `[*value, size]`
    CheckSize,

    // Unused opcodes, allowing for a direct transmutation from a byte to an Op.
    Unused90,
    Unused91,
    Unused92,
    Unused93,
    Unused94,
    Unused95,
    Unused96,
    Unused97,
    Unused98,
    Unused99,
    Unused100,
    Unused101,
    Unused102,
    Unused103,
    Unused104,
    Unused105,
    Unused106,
    Unused107,
    Unused108,
    Unused109,
    Unused110,
    Unused111,
    Unused112,
    Unused113,
    Unused114,
    Unused115,
    Unused116,
    Unused117,
    Unused118,
    Unused119,
    Unused120,
    Unused121,
    Unused122,
    Unused123,
    Unused124,
    Unused125,
    Unused126,
    Unused127,
    Unused128,
    Unused129,
    Unused130,
    Unused131,
    Unused132,
    Unused133,
    Unused134,
    Unused135,
    Unused136,
    Unused137,
    Unused138,
    Unused139,
    Unused140,
    Unused141,
    Unused142,
    Unused143,
    Unused144,
    Unused145,
    Unused146,
    Unused147,
    Unused148,
    Unused149,
    Unused150,
    Unused151,
    Unused152,
    Unused153,
    Unused154,
    Unused155,
    Unused156,
    Unused157,
    Unused158,
    Unused159,
    Unused160,
    Unused161,
    Unused162,
    Unused163,
    Unused164,
    Unused165,
    Unused166,
    Unused167,
    Unused168,
    Unused169,
    Unused170,
    Unused171,
    Unused172,
    Unused173,
    Unused174,
    Unused175,
    Unused176,
    Unused177,
    Unused178,
    Unused179,
    Unused180,
    Unused181,
    Unused182,
    Unused183,
    Unused184,
    Unused185,
    Unused186,
    Unused187,
    Unused188,
    Unused189,
    Unused190,
    Unused191,
    Unused192,
    Unused193,
    Unused194,
    Unused195,
    Unused196,
    Unused197,
    Unused198,
    Unused199,
    Unused200,
    Unused201,
    Unused202,
    Unused203,
    Unused204,
    Unused205,
    Unused206,
    Unused207,
    Unused208,
    Unused209,
    Unused210,
    Unused211,
    Unused212,
    Unused213,
    Unused214,
    Unused215,
    Unused216,
    Unused217,
    Unused218,
    Unused219,
    Unused220,
    Unused221,
    Unused222,
    Unused223,
    Unused224,
    Unused225,
    Unused226,
    Unused227,
    Unused228,
    Unused229,
    Unused230,
    Unused231,
    Unused232,
    Unused233,
    Unused234,
    Unused235,
    Unused236,
    Unused237,
    Unused238,
    Unused239,
    Unused240,
    Unused241,
    Unused242,
    Unused243,
    Unused244,
    Unused245,
    Unused246,
    Unused247,
    Unused248,
    Unused249,
    Unused250,
    Unused251,
    Unused252,
    Unused253,
    Unused254,
    Unused255,
}

impl From<u8> for Op {
    fn from(op: u8) -> Op {
        // Safety:
        //  - Op is repr(u8)
        //  - All 256 possible values are represented in the enum
        unsafe { std::mem::transmute(op) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_op_count() {
        assert_eq!(
            Op::Unused255 as u8,
            255,
            "Op should have 256 entries (see impl From<u8> for Op)"
        );
    }
}
