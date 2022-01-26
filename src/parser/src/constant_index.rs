use std::fmt;

const CONSTANT_INDEX_MAX: usize = 2_usize.pow(24) - 1;

/// A 24 bit index for constants
///
/// Values are stored as little-endian 24 bit values.
///
/// Q: Why not just use a u32?
/// A: Minimizing the memory footprint for a complex script with lots of constants seems
///    like a healthy idea. Having a dedicated u24 type also forces the validity of a
///    value to be checked when its first created; after creation an in-range index is
///    guaranteed.
/// Q: What if we need more than 2^24 constants in a script?
/// A: Let's wait and see, ConstantIndex can be transitioned to a u32 (along with the
///    corresponding constant loading ops) if it really turns out to be necessary.
#[derive(Clone, Copy, Hash, PartialEq)]
pub struct ConstantIndex(pub u8, pub u8, pub u8);

impl ConstantIndex {
    /// The raw bytes representing the constant index
    pub fn bytes(&self) -> [u8; 3] {
        [self.0, self.1, self.2]
    }
}

impl From<u8> for ConstantIndex {
    fn from(x: u8) -> Self {
        Self(x, 0, 0)
    }
}

impl From<[u8; 2]> for ConstantIndex {
    fn from(x: [u8; 2]) -> Self {
        Self(x[0], x[1], 0)
    }
}

impl From<[u8; 3]> for ConstantIndex {
    fn from(x: [u8; 3]) -> Self {
        Self(x[0], x[1], x[2])
    }
}

impl TryFrom<usize> for ConstantIndex {
    type Error = ConstantIndexTryFromOutOfRange;

    fn try_from(x: usize) -> Result<Self, Self::Error> {
        if x <= CONSTANT_INDEX_MAX {
            let bytes = x.to_le_bytes();
            Ok(Self(bytes[0], bytes[1], bytes[2]))
        } else {
            Err(ConstantIndexTryFromOutOfRange())
        }
    }
}

impl From<ConstantIndex> for usize {
    fn from(x: ConstantIndex) -> Self {
        (x.0 as usize) | (x.1 as usize) << 8 | (x.2 as usize) << 16
    }
}

impl From<&ConstantIndex> for usize {
    fn from(x: &ConstantIndex) -> Self {
        usize::from(*x)
    }
}

impl Eq for ConstantIndex {}

impl PartialEq<ConstantIndex> for usize {
    fn eq(&self, other: &ConstantIndex) -> bool {
        self == &usize::from(other)
    }
}

impl fmt::Display for ConstantIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", usize::from(self))
    }
}

impl fmt::Debug for ConstantIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// The error returned from TryFrom implementations for ConstantIndex
#[derive(Debug)]
pub struct ConstantIndexTryFromOutOfRange();

impl fmt::Display for ConstantIndexTryFromOutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_from_usize() {
        let x = usize::from_le_bytes([12, 34, 56, 0, 0, 0, 0, 0]);
        let constant = ConstantIndex::try_from(x).unwrap();
        assert_eq!(constant.bytes(), [12, 34, 56]);
    }
}
