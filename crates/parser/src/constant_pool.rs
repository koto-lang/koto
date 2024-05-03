use koto_memory::Ptr;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt,
    hash::{Hash, Hasher},
    num::TryFromIntError,
    ops::Range,
};

use crate::{error::InternalError, StringSlice};

/// The type used to refer to constants in the [ConstantPool]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConstantIndex(u32);

impl From<ConstantIndex> for u32 {
    fn from(value: ConstantIndex) -> Self {
        value.0
    }
}

impl From<ConstantIndex> for usize {
    fn from(value: ConstantIndex) -> Self {
        value.0 as usize
    }
}

impl From<u32> for ConstantIndex {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl TryFrom<usize> for ConstantIndex {
    type Error = TryFromIntError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(u32::try_from(value)?))
    }
}

impl fmt::Display for ConstantIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// An entry in the list of constants contained in a [ConstantPool]
#[derive(Clone, Debug, PartialEq)]
enum ConstantEntry {
    // An f64 constant
    F64(f64),
    // An i64 constant
    I64(i64),
    // The range in bytes in the ConstantPool's string data for a string constant
    Str(Range<usize>),
}

/// A constant provided by a [ConstantPool]
#[derive(Clone, Debug, PartialEq)]
pub enum Constant<'a> {
    /// An f64 constant
    F64(f64),
    /// An i64 constant
    I64(i64),
    /// A string constant
    Str(&'a str),
}

/// A constant pool produced by the [Parser](crate::Parser) for a Koto script
///
/// A `ConstantPoolBuilder` is used to prepare the pool.
#[derive(Clone, Debug)]
pub struct ConstantPool {
    // The list of constants in the pool
    //
    // A [ConstantIndex] is an index into this list, which then provides information to get the
    // constant itself.
    constants: Vec<ConstantEntry>,
    // A series of constant strings concatenated into a single string
    string_data: Ptr<str>,
    // A hash of the pool contents, incrementally prepared by the builder
    hash: u64,
}

impl Default for ConstantPool {
    fn default() -> Self {
        Self {
            constants: vec![],
            string_data: String::default().into(),
            hash: 0,
        }
    }
}

impl ConstantPool {
    /// Provides the number of constants in the pool
    pub fn size(&self) -> usize {
        self.constants.len()
    }

    /// Returns the constant corresponding to the provided index
    pub fn get(&self, index: usize) -> Option<Constant> {
        match self.constants.get(index) {
            Some(constant_info) => match constant_info {
                ConstantEntry::F64(n) => Some(Constant::F64(*n)),
                ConstantEntry::I64(n) => Some(Constant::I64(*n)),
                ConstantEntry::Str(range) => Some(Constant::Str(&self.string_data[range.clone()])),
            },
            None => None,
        }
    }

    /// Returns the concatenated string data stored in the pool
    pub fn string_data(&self) -> &Ptr<str> {
        &self.string_data
    }

    /// Returns the string corresponding to the provided index as a &str
    ///
    /// Warning! Panics if there isn't a string at the provided index
    #[inline]
    pub fn get_str(&self, index: ConstantIndex) -> &str {
        // Safety: The bounds have already been checked while the pool is being prepared
        unsafe { self.string_data.get_unchecked(self.get_str_bounds(index)) }
    }

    /// Returns the string corresponding to the provided index as a string slice
    ///
    /// Warning! Panics if there isn't a string at the provided index
    #[inline]
    pub fn get_string_slice(&self, index: ConstantIndex) -> StringSlice {
        // Safety: The bounds have already been checked while the pool is being prepared
        unsafe { StringSlice::new_unchecked(self.string_data.clone(), self.get_str_bounds(index)) }
    }

    fn get_str_bounds(&self, index: ConstantIndex) -> Range<usize> {
        match self.constants.get(usize::from(index)) {
            Some(ConstantEntry::Str(range)) => range.clone(),
            _ => panic!("Invalid index"),
        }
    }

    /// Returns the f64 corresponding to the provided constant index
    ///
    /// Warning! Panics if there isn't an f64 at the provided index
    pub fn get_f64(&self, index: ConstantIndex) -> f64 {
        match self.constants.get(usize::from(index)) {
            Some(ConstantEntry::F64(n)) => *n,
            _ => panic!("Invalid index"),
        }
    }

    /// Returns the i64 corresponding to the provided constant index
    ///
    /// Warning! Panics if there isn't an i64 at the provided index
    pub fn get_i64(&self, index: ConstantIndex) -> i64 {
        match self.constants.get(usize::from(index)) {
            Some(ConstantEntry::I64(n)) => *n,
            _ => panic!("Invalid index"),
        }
    }

    /// Provides an iterator that iterates over the pool's constants
    pub fn iter(&self) -> ConstantPoolIterator {
        ConstantPoolIterator::new(self)
    }
}

/// An iterator that iterates over a [ConstantPool]'s constants
pub struct ConstantPoolIterator<'a> {
    pool: &'a ConstantPool,
    index: usize,
}

impl<'a> ConstantPoolIterator<'a> {
    fn new(pool: &'a ConstantPool) -> Self {
        Self { pool, index: 0 }
    }
}

impl<'a> Iterator for ConstantPoolIterator<'a> {
    type Item = Constant<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.pool.get(self.index);
        self.index += 1;
        result
    }
}

impl fmt::Display for ConstantPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, constant) in self.iter().enumerate() {
            write!(f, "{i}\t")?;
            match constant {
                Constant::F64(n) => write!(f, "Float\t{n}")?,
                Constant::I64(n) => write!(f, "Int\t{n}")?,
                Constant::Str(s) => write!(f, "String\t{s}")?,
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl PartialEq for ConstantPool {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Hash for ConstantPool {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

/// A builder of [ConstantPool]s
///
/// The parser uses this builder to build up a pool of constants.
///
/// [ConstantPoolBuilder::build]() is called when parsing is finished to produce a finalized
/// ConstantPool.
#[derive(Default)]
pub(crate) struct ConstantPoolBuilder {
    // The list of constants
    constants: Vec<ConstantEntry>,
    // The concatenated string constants
    string_data: String,
    // A hash of the pool contents, incrementally built up as constants are added
    hasher: DefaultHasher,
    // A map that keeps track of which string constants have already been added
    string_map: HashMap<String, ConstantIndex>,
    // A map that keeps track of which float constants have already been added
    float_map: HashMap<u64, ConstantIndex>,
    // A map that keeps track of which integer constants have already been added
    int_map: HashMap<i64, ConstantIndex>,
}

impl ConstantPoolBuilder {
    pub fn add_string(&mut self, s: &str) -> Result<ConstantIndex, InternalError> {
        match self.string_map.get(s) {
            Some(index) => Ok(*index),
            None => {
                let result = ConstantIndex::try_from(self.constants.len())
                    .map_err(|_| InternalError::ConstantPoolCapacityOverflow)?;

                let start = self.string_data.len();
                let end = start + s.len();
                self.string_data.push_str(s);
                self.constants.push(ConstantEntry::Str(start..end));
                s.hash(&mut self.hasher);

                self.string_map.insert(s.to_string(), result);

                Ok(result)
            }
        }
    }

    pub fn add_f64(&mut self, n: f64) -> Result<ConstantIndex, InternalError> {
        let n_u64 = n.to_bits();

        match self.float_map.get(&n_u64) {
            Some(index) => Ok(*index),
            None => {
                let result = ConstantIndex::try_from(self.constants.len())
                    .map_err(|_| InternalError::ConstantPoolCapacityOverflow)?;
                self.constants.push(ConstantEntry::F64(n));
                n_u64.hash(&mut self.hasher);
                self.float_map.insert(n_u64, result);
                Ok(result)
            }
        }
    }

    pub fn add_i64(&mut self, n: i64) -> Result<ConstantIndex, InternalError> {
        match self.int_map.get(&n) {
            Some(index) => Ok(*index),
            None => {
                let result = ConstantIndex::try_from(self.constants.len())
                    .map_err(|_| InternalError::ConstantPoolCapacityOverflow)?;
                self.constants.push(ConstantEntry::I64(n));
                n.hash(&mut self.hasher);
                self.int_map.insert(n, result);
                Ok(result)
            }
        }
    }

    pub fn get_str(&self, index: ConstantIndex) -> &str {
        match self.constants.get(usize::from(index)) {
            Some(ConstantEntry::Str(range)) => {
                // Safety: The bounds have already been checked while the pool is being prepared
                unsafe { self.string_data.get_unchecked(range.clone()) }
            }
            _ => panic!("Invalid index"),
        }
    }

    pub fn build(self) -> ConstantPool {
        ConstantPool {
            constants: self.constants,
            string_data: self.string_data.into(),
            hash: self.hasher.finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floats_are_equal(a: f64, b: f64) -> bool {
        (a - b).abs() < f64::EPSILON
    }

    #[test]
    fn test_adding_strings() {
        let mut builder = ConstantPoolBuilder::default();

        let s1 = "test";
        let s2 = "test2";

        // 1 byte for string length
        assert_eq!(ConstantIndex(0), builder.add_string(s1).unwrap());
        assert_eq!(ConstantIndex(1), builder.add_string(s2).unwrap());

        // don't duplicate string_data
        assert_eq!(ConstantIndex(0), builder.add_string(s1).unwrap());
        assert_eq!(ConstantIndex(1), builder.add_string(s2).unwrap());

        let pool = builder.build();

        assert_eq!(s1, pool.get_str(0.into()));
        assert_eq!(s2, pool.get_str(1.into()));

        assert_eq!(2, pool.size());
    }

    #[test]
    fn test_adding_numbers() {
        let mut builder = ConstantPoolBuilder::default();

        let n1 = 3;
        let n2 = 9.87654321;

        assert_eq!(ConstantIndex(0), builder.add_i64(n1).unwrap());
        assert_eq!(ConstantIndex(1), builder.add_f64(n2).unwrap());

        // don't duplicate numbers
        assert_eq!(ConstantIndex(0), builder.add_i64(n1).unwrap());
        assert_eq!(ConstantIndex(1), builder.add_f64(n2).unwrap());

        let pool = builder.build();

        assert_eq!(n1, pool.get_i64(0.into()));
        assert!(floats_are_equal(n2, pool.get_f64(1.into())));

        assert_eq!(2, pool.size());
    }

    #[test]
    fn test_adding_numbers_and_strings() {
        let mut builder = ConstantPoolBuilder::default();

        let n1 = -1.1;
        let n2 = 99;
        let s1 = "O_o";
        let s2 = "^_^";

        assert_eq!(ConstantIndex(0), builder.add_f64(n1).unwrap());
        assert_eq!(ConstantIndex(1), builder.add_string(s1).unwrap());
        assert_eq!(ConstantIndex(2), builder.add_i64(n2).unwrap());
        assert_eq!(ConstantIndex(3), builder.add_string(s2).unwrap());

        let pool = builder.build();

        assert!(floats_are_equal(n1, pool.get_f64(0.into())));
        assert_eq!(s1, pool.get_str(1.into()));
        assert_eq!(n2, pool.get_i64(2.into()));
        assert_eq!(s2, pool.get_str(3.into()));

        assert_eq!(4, pool.size());
    }

    #[test]
    fn test_iter() {
        let mut builder = ConstantPoolBuilder::default();

        let n1 = -1;
        let n2 = 99.9;
        let s1 = "O_o";
        let s2 = "^_^";

        builder.add_i64(n1).unwrap();
        builder.add_string(s1).unwrap();
        builder.add_f64(n2).unwrap();
        builder.add_string(s2).unwrap();

        let pool = builder.build();

        let mut iter = pool.iter();
        assert_eq!(iter.next(), Some(Constant::I64(-1)));
        assert_eq!(iter.next(), Some(Constant::Str("O_o")));
        assert_eq!(iter.next(), Some(Constant::F64(99.9)));
        assert_eq!(iter.next(), Some(Constant::Str("^_^")));
        assert_eq!(iter.next(), None);
    }
}
