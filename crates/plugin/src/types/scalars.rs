use super::callable::{KFunction, KNativeFunction};
use crate::{DisplayContext, KIterator, KList, KMap, KObject, KString, KTuple};
use koto_api::{
    KotoCollection, KotoDisplay, KotoIdentity, KotoNumber, KotoObjectOps, KotoRange, KotoSequence,
    KotoSlice, KotoValue, KotoVmTrait, UnaryOp, write_koto_range,
};
use koto_ffi as abi;
use std::{
    cmp::Ordering,
    fmt,
    ops::{Range, RangeInclusive},
};

/// A range value used by the plugin helpers.
#[derive(Clone, Copy, Debug)]
pub struct KRange {
    start: Option<i64>,
    end: Option<(i64, bool)>,
}

impl KRange {
    /// Creates a range from explicit bounds.
    pub fn new(start: Option<i64>, end: Option<(i64, bool)>) -> Self {
        Self { start, end }
    }

    /// Returns the start of the range, if present.
    pub fn start(&self) -> Option<i64> {
        self.start
    }

    /// Returns the end of the range and its inclusivity, if present.
    pub fn end(&self) -> Option<(i64, bool)> {
        self.end
    }

    /// Returns the range with missing boundaries replaced by min/max values.
    pub fn as_bounded_range(&self) -> Range<i64> {
        let start = self.start.unwrap_or(i64::MIN);
        let (end, inclusive) = self.end.unwrap_or((i64::MAX, false));
        let end = if inclusive {
            end.saturating_add(1)
        } else {
            end
        };
        start..end.max(start)
    }
}

impl KotoRange for KRange {
    fn start(&self) -> Option<i64> {
        self.start()
    }

    fn end(&self) -> Option<(i64, bool)> {
        self.end()
    }

    fn as_bounded_range(&self) -> Range<i64> {
        self.as_bounded_range()
    }
}

impl fmt::Display for KRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_koto_range(f, self)
    }
}

impl From<abi::KotoRange> for KRange {
    fn from(value: abi::KotoRange) -> Self {
        Self {
            start: value.has_start.then_some(value.start),
            end: value.has_end.then_some((value.end, value.end_inclusive)),
        }
    }
}

impl From<KRange> for abi::KotoRange {
    fn from(value: KRange) -> Self {
        Self {
            start: value.start.unwrap_or_default(),
            has_start: value.start.is_some(),
            end: value.end.map_or(0, |(end, _)| end),
            has_end: value.end.is_some(),
            end_inclusive: value.end.is_some_and(|(_, inclusive)| inclusive),
        }
    }
}

impl From<Range<i64>> for KRange {
    fn from(value: Range<i64>) -> Self {
        Self::new(Some(value.start), Some((value.end, false)))
    }
}

impl From<Range<usize>> for KRange {
    fn from(value: Range<usize>) -> Self {
        Self::new(Some(value.start as i64), Some((value.end as i64, false)))
    }
}

impl From<RangeInclusive<i64>> for KRange {
    fn from(value: RangeInclusive<i64>) -> Self {
        Self::new(Some(*value.start()), Some((*value.end(), true)))
    }
}

impl From<RangeInclusive<usize>> for KRange {
    fn from(value: RangeInclusive<usize>) -> Self {
        Self::new(
            Some(*value.start() as i64),
            Some((*value.end() as i64, true)),
        )
    }
}

/// The supported plugin value types.
#[derive(Clone, Debug)]
pub enum KValue {
    /// The null value.
    Null,
    /// A boolean value.
    Bool(bool),
    /// A numeric value.
    Number(KNumber),
    /// A range value.
    Range(KRange),
    /// A string value.
    Str(KString),
    /// A list value.
    List(KList),
    /// A tuple value.
    Tuple(KTuple),
    /// A map value.
    Map(KMap),
    /// A function value.
    Function(KFunction),
    /// A native function value.
    NativeFunction(KNativeFunction),
    /// An iterator value.
    Iterator(KIterator),
    /// An object value.
    Object(KObject),
}

impl KValue {
    /// Returns the value's type as a string.
    pub fn type_as_string(&self) -> &'static str {
        match self {
            KValue::Null => "Null",
            KValue::Bool(_) => "Bool",
            KValue::Number(_) => "Number",
            KValue::Range(_) => "Range",
            KValue::Str(_) => "String",
            KValue::List(_) => "List",
            KValue::Tuple(_) => "Tuple",
            KValue::Map(_) => "Map",
            KValue::Function(value) => {
                if value.is_generator() {
                    "Generator"
                } else {
                    "Function"
                }
            }
            KValue::NativeFunction(_) => "Function",
            KValue::Iterator(_) => "Iterator",
            KValue::Object(_) => "Object",
        }
    }

    /// Returns `true` if both values refer to the same underlying runtime instance.
    pub fn is_same_instance(&self, other: &Self) -> bool {
        use KValue::*;

        match (self, other) {
            (Map(a), Map(b)) => a.is_same_instance(b),
            (Object(a), Object(b)) => a.is_same_instance(b),
            (List(a), List(b)) => a.is_same_instance(b),
            (Tuple(a), Tuple(b)) => a.is_same_instance(b),
            _ => false,
        }
    }
}

fn display_sequence(
    ctx: &mut DisplayContext<'_>,
    id: usize,
    open: char,
    close: char,
    values: impl IntoIterator<Item = KValue>,
) -> crate::Result<()> {
    ctx.append(open);

    if ctx.is_in_parents(id) {
        ctx.append("...");
    } else {
        ctx.push_container(id);

        for (i, value) in values.into_iter().enumerate() {
            if i > 0 {
                ctx.append(", ");
            }
            value.display(ctx)?;
        }

        ctx.pop_container();
    }

    ctx.append(close);
    Ok(())
}

fn display_map(map: &KMap, ctx: &mut DisplayContext<'_>) -> crate::Result<()> {
    if let Some(vm) = ctx.vm().as_ref().copied() {
        let mut vm = *vm;
        match vm.run_unary_op(UnaryOp::Display, map.clone().into())? {
            KValue::Str(display_result) => {
                ctx.append(display_result);
                return Ok(());
            }
            unexpected => return crate::unexpected_type("String from @display", &unexpected),
        }
    }

    ctx.append('{');

    let id = map.display_id();
    if ctx.is_in_parents(id) {
        ctx.append("...");
    } else {
        ctx.push_container(id);

        for index in 0..map.len() {
            if index > 0 {
                ctx.append(", ");
            }

            let Some((key, value)) = map.get_index(index) else {
                continue;
            };

            let mut key_ctx = DisplayContext::default();
            key.display(&mut key_ctx)?;
            ctx.append(key_ctx.result());
            ctx.append(": ");
            value.display(ctx)?;
        }

        ctx.pop_container();
    }

    ctx.append('}');
    Ok(())
}

impl KotoValue<crate::PluginBackend> for KValue {
    fn is_null(&self) -> bool {
        matches!(self, KValue::Null)
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            KValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_number(&self) -> Option<KNumber> {
        match self {
            KValue::Number(value) => Some(*value),
            _ => None,
        }
    }

    fn as_range(&self) -> Option<KRange> {
        match self {
            KValue::Range(value) => Some(*value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            KValue::Str(value) => Some(value),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<KList> {
        match self {
            KValue::List(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn as_tuple(&self) -> Option<KTuple> {
        match self {
            KValue::Tuple(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn as_map(&self) -> Option<KMap> {
        match self {
            KValue::Map(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn as_object(&self) -> Option<KObject> {
        match self {
            KValue::Object(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn as_iterator(&self) -> Option<KIterator> {
        match self {
            KValue::Iterator(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn as_function(&self) -> Option<KFunction> {
        match self {
            KValue::Function(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn as_native_function(&self) -> Option<KNativeFunction> {
        match self {
            KValue::NativeFunction(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn type_as_string(&self) -> KString {
        self.type_as_string().into()
    }
}

impl KotoDisplay<crate::PluginBackend> for KValue {
    fn display(&self, ctx: &mut DisplayContext<'_>) -> crate::Result<()> {
        match self {
            KValue::Null => ctx.append("null"),
            KValue::Bool(value) => ctx.append(value.to_string()),
            KValue::Number(value) => ctx.append(value.to_string()),
            KValue::Range(value) => {
                return <KRange as KotoDisplay<crate::PluginBackend>>::display(value, ctx);
            }
            KValue::Str(value) => {
                if ctx.is_contained() || ctx.debug_enabled() {
                    ctx.append('\'');
                    ctx.append(value);
                    ctx.append('\'');
                } else {
                    ctx.append(value);
                }
            }
            KValue::List(value) => {
                return display_sequence(ctx, value.display_id(), '[', ']', value.data().iter());
            }
            KValue::Tuple(value) => {
                return display_sequence(ctx, value.display_id(), '(', ')', value.data().iter());
            }
            KValue::Map(value) => return display_map(value, ctx),
            KValue::Function(_) | KValue::NativeFunction(_) => ctx.append("||"),
            KValue::Iterator(_) => ctx.append("Iterator"),
            KValue::Object(value) => return value.try_borrow()?.display(ctx),
        }

        Ok(())
    }
}

impl KotoIdentity for KValue {
    fn is_same_instance(&self, other: &Self) -> bool {
        KValue::is_same_instance(self, other)
    }
}

impl From<()> for KValue {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl From<bool> for KValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<String> for KValue {
    fn from(value: String) -> Self {
        Self::Str(value.into())
    }
}

impl From<&str> for KValue {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

impl From<i64> for KValue {
    fn from(value: i64) -> Self {
        Self::Number(KNumber::I64(value))
    }
}

impl From<i32> for KValue {
    fn from(value: i32) -> Self {
        Self::Number(KNumber::I64(value as i64))
    }
}

impl From<usize> for KValue {
    fn from(value: usize) -> Self {
        Self::Number(KNumber::I64(value as i64))
    }
}

impl From<f32> for KValue {
    fn from(value: f32) -> Self {
        Self::Number(KNumber::F64(value as f64))
    }
}

impl From<f64> for KValue {
    fn from(value: f64) -> Self {
        Self::Number(KNumber::F64(value))
    }
}

impl From<KRange> for KValue {
    fn from(value: KRange) -> Self {
        Self::Range(value)
    }
}

impl From<KList> for KValue {
    fn from(value: KList) -> Self {
        Self::List(value)
    }
}

impl From<KTuple> for KValue {
    fn from(value: KTuple) -> Self {
        Self::Tuple(value)
    }
}

impl From<KMap> for KValue {
    fn from(value: KMap) -> Self {
        Self::Map(value)
    }
}

impl From<KFunction> for KValue {
    fn from(value: KFunction) -> Self {
        Self::Function(value)
    }
}

impl From<KNativeFunction> for KValue {
    fn from(value: KNativeFunction) -> Self {
        Self::NativeFunction(value)
    }
}

impl From<KIterator> for KValue {
    fn from(value: KIterator) -> Self {
        Self::Iterator(value)
    }
}

impl From<KObject> for KValue {
    fn from(value: KObject) -> Self {
        Self::Object(value)
    }
}

/// The number type used by plugin values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KNumber {
    /// A signed 64-bit integer.
    I64(i64),
    /// A 64-bit float.
    F64(f64),
}

impl KNumber {
    /// Returns `true` if the number is represented by an `f64`.
    pub fn is_f64(self) -> bool {
        matches!(self, Self::F64(_))
    }

    /// Returns `true` if the number is represented by an `i64`.
    pub fn is_i64(self) -> bool {
        matches!(self, Self::I64(_))
    }

    /// Returns the numeric value as raw bits suitable for seeding random generators.
    pub fn to_bits(self) -> u64 {
        match self {
            KNumber::I64(value) => value as u64,
            KNumber::F64(value) => value.to_bits(),
        }
    }
}

impl KotoNumber for KNumber {
    fn is_f64(self) -> bool {
        self.is_f64()
    }

    fn is_i64(self) -> bool {
        self.is_i64()
    }

    fn to_bits(self) -> u64 {
        self.to_bits()
    }
}

impl fmt::Display for KNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KNumber::F64(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{n:.1}")
                } else {
                    write!(f, "{n}")
                }
            }
            KNumber::I64(n) => write!(f, "{n}"),
        }
    }
}

impl From<KNumber> for KValue {
    fn from(value: KNumber) -> Self {
        KValue::Number(value)
    }
}

impl From<KNumber> for i64 {
    fn from(value: KNumber) -> Self {
        match value {
            KNumber::I64(n) => n,
            KNumber::F64(n) => n as i64,
        }
    }
}

impl From<KNumber> for usize {
    fn from(value: KNumber) -> Self {
        match value {
            KNumber::I64(n) => n.max(0) as usize,
            KNumber::F64(n) => n.max(0.0) as usize,
        }
    }
}

impl From<KNumber> for u32 {
    fn from(value: KNumber) -> Self {
        match value {
            KNumber::I64(n) => n.clamp(0, u32::MAX as i64) as u32,
            KNumber::F64(n) => n.clamp(0.0, u32::MAX as f64) as u32,
        }
    }
}

impl From<&KNumber> for i64 {
    fn from(value: &KNumber) -> Self {
        (*value).into()
    }
}

impl From<&KNumber> for usize {
    fn from(value: &KNumber) -> Self {
        (*value).into()
    }
}

impl From<&KNumber> for u32 {
    fn from(value: &KNumber) -> Self {
        (*value).into()
    }
}

impl From<KNumber> for f32 {
    fn from(value: KNumber) -> Self {
        match value {
            KNumber::I64(n) => n as f32,
            KNumber::F64(n) => n as f32,
        }
    }
}

impl From<&KNumber> for f32 {
    fn from(value: &KNumber) -> Self {
        (*value).into()
    }
}

impl From<KNumber> for f64 {
    fn from(value: KNumber) -> Self {
        match value {
            KNumber::I64(n) => n as f64,
            KNumber::F64(n) => n,
        }
    }
}

impl From<&KNumber> for f64 {
    fn from(value: &KNumber) -> Self {
        (*value).into()
    }
}

macro_rules! number_cmp {
    ($($type:ty),+ $(,)?) => {
        $(
            impl PartialEq<$type> for KNumber {
                fn eq(&self, other: &$type) -> bool {
                    match self {
                        KNumber::I64(n) => (*n as f64) == (*other as f64),
                        KNumber::F64(n) => *n == (*other as f64),
                    }
                }
            }

            impl PartialOrd<$type> for KNumber {
                fn partial_cmp(&self, other: &$type) -> Option<Ordering> {
                    match self {
                        KNumber::I64(n) => (*n as f64).partial_cmp(&(*other as f64)),
                        KNumber::F64(n) => n.partial_cmp(&(*other as f64)),
                    }
                }
            }
        )+
    };
}

number_cmp!(i32, i64, usize, u32, f32, f64);
