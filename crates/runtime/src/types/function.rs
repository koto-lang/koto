use crate::KList;
use koto_bytecode::Chunk;
use koto_memory::Ptr;

/// A Koto function
///
/// See also:
/// * [KCaptureFunction]
/// * [KNativeFunction](crate::KNativeFunction)
/// * [KValue::Function](crate::KValue::Function)
#[derive(Clone, Debug, PartialEq)]
pub struct KFunction {
    /// The [Chunk] in which the function can be found.
    pub chunk: Ptr<Chunk>,
    /// The start ip of the function.
    pub ip: u32,
    /// The expected number of arguments for the function
    pub arg_count: u8,
    /// If the function is variadic, then extra args will be captured in a tuple.
    pub variadic: bool,
    /// If the function has a single arg, and that arg is an unpacked tuple
    ///
    /// This is used to optimize calls where the caller has a series of args that might be unpacked
    /// by the function, and it would be wasteful to create a Tuple when it's going to be
    /// immediately unpacked and discarded.
    pub arg_is_unpacked_tuple: bool,
    /// If the function is a generator, then calling the function will yield an iterator that
    /// executes the function's body for each iteration step, pausing when a yield instruction is
    /// encountered. See Vm::call_generator and Iterable::Generator.
    pub generator: bool,
}

/// A Koto function with captured values
///
/// See also:
/// * [KFunction]
/// * [KNativeFunction](crate::KNativeFunction)
/// * [KValue::CaptureFunction](crate::KValue::CaptureFunction)
#[derive(Clone)]
pub struct KCaptureFunction {
    /// The function's properties
    pub info: KFunction,
    /// The optional list of captures that should be copied into scope when the function is called.
    //
    // Q. Why use a KList?
    // A. Because capturing values currently works by assigning by index, after the function
    //    itself has been created, and the captured function and the assigned function both need to
    //    share the same captures list. Currently the only way for this to work is to allow mutation
    //    of the shared list after the creation of the function, so a KList is a reasonable choice.
    // Q. After capturing is complete, what about using Ptr<[Value]> for non-recursive functions,
    //    or Option<Value> for non-recursive functions with a single capture?
    // A. These could be worth investigating as optimizations, but a KList will do for now.
    pub captures: KList,
}
