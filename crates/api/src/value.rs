use crate::KotoBackend;
use std::{fmt, ops::Range};

/// Shared value operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoValue<B: KotoBackend>: Clone + KotoDisplay<B> {
    /// Returns `true` if the value is null.
    fn is_null(&self) -> bool;

    /// Returns the contained boolean, if the value is a boolean.
    fn as_bool(&self) -> Option<bool>;

    /// Returns the contained number, if the value is a number.
    fn as_number(&self) -> Option<B::Number>;

    /// Returns the contained range, if the value is a range.
    fn as_range(&self) -> Option<B::Range>;

    /// Returns the contained string slice, if the value is a string.
    fn as_str(&self) -> Option<&str>;

    /// Returns the contained list, if the value is a list.
    fn as_list(&self) -> Option<B::List>;

    /// Returns the contained tuple, if the value is a tuple.
    fn as_tuple(&self) -> Option<B::Tuple>;

    /// Returns the contained map, if the value is a map.
    fn as_map(&self) -> Option<B::Map>;

    /// Returns the contained object, if the value is an object.
    fn as_object(&self) -> Option<B::Object>;

    /// Returns the contained iterator, if the value is an iterator.
    fn as_iterator(&self) -> Option<B::Iterator>;

    /// Returns the contained function, if the value is a function.
    fn as_function(&self) -> Option<B::Function>;

    /// Returns the contained native function, if the value is a native function.
    fn as_native_function(&self) -> Option<B::NativeFunction>;

    /// Returns the value's type as a string-like value.
    fn type_as_string(&self) -> B::String;
}

/// Shared numeric operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoNumber: Copy {
    /// Returns `true` if the number is represented by an `f64`.
    fn is_f64(self) -> bool;

    /// Returns `true` if the number is represented by an `i64`.
    fn is_i64(self) -> bool;

    /// Returns the numeric value as raw bits.
    fn to_bits(self) -> u64;
}

/// Shared range operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoRange {
    /// Returns the start of the range, if present.
    fn start(&self) -> Option<i64>;

    /// Returns the end of the range and its inclusivity, if present.
    fn end(&self) -> Option<(i64, bool)>;

    /// Returns the range with missing boundaries replaced by min/max values.
    fn as_bounded_range(&self) -> Range<i64>;
}

/// Writes a [`KotoRange`] to the provided formatter.
pub fn write_koto_range(output: &mut impl fmt::Write, range: &impl KotoRange) -> fmt::Result {
    if let Some(start) = range.start() {
        write!(output, "{start}")?;
    }

    output.write_str("..")?;

    if let Some((end, inclusive)) = range.end() {
        if inclusive {
            output.write_str("=")?;
        }
        write!(output, "{end}")?;
    }

    Ok(())
}

/// Shared display operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoDisplay<B: KotoBackend> {
    /// Renders the value into the provided display context.
    fn display(&self, ctx: &mut B::DisplayContext<'_>) -> Result<(), B::Error>;
}

impl<B, T> KotoDisplay<B> for T
where
    B: KotoBackend,
    T: KotoRange,
{
    fn display(&self, ctx: &mut B::DisplayContext<'_>) -> Result<(), B::Error> {
        let _ = write_koto_range(ctx, self);

        Ok(())
    }
}

/// Shared meta-map operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoMetaMap {
    /// The meta key type used by the map.
    type MetaKey;

    /// The map's value type.
    type Value: Clone;

    /// Returns `true` if the map contains the given meta key.
    fn contains_meta_key(&self, key: &Self::MetaKey) -> bool;

    /// Returns a cloned meta value for the given key.
    fn get_meta_value(&self, key: &Self::MetaKey) -> Option<Self::Value>;
}

/// Shared string operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoString: AsRef<str> + for<'a> From<&'a str> {
    /// Returns the string as `&str`.
    fn as_str(&self) -> &str {
        self.as_ref()
    }

    /// Returns the string length in bytes.
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// Returns `true` if the string is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
impl<T> KotoString for T where T: AsRef<str> + for<'a> From<&'a str> {}

/// Shared helper constructors for backend iterator wrapper types.
pub trait KotoIteratorBuilder: Sized {
    /// The iterator output item.
    type Item;

    /// Creates an iterator from any `DoubleEndedIterator`.
    fn with_std_iter<T>(iter: T) -> Self
    where
        T: DoubleEndedIterator<Item = Self::Item> + Clone + Send + Sync + 'static;

    /// Creates an iterator from any forward-only `Iterator`.
    fn with_std_forward_iter<T>(iter: T) -> Self
    where
        T: Iterator<Item = Self::Item> + Clone + Send + Sync + 'static;
}
