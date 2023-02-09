use {
    crate::Value,
    std::{
        cmp::Ordering,
        fmt,
        hash::{Hash, Hasher},
        ops,
    },
};

/// The Number type used by the Koto runtime
///
/// The number can be either an `f64` or an `i64` depending on usage.
#[allow(missing_docs)]
#[derive(Clone, Copy)]
pub enum ValueNumber {
    F64(f64),
    I64(i64),
}

impl ValueNumber {
    /// Returns the absolute value of the number
    #[must_use]
    pub fn abs(self) -> Self {
        match self {
            Self::F64(n) => Self::F64(n.abs()),
            Self::I64(n) => Self::I64(n.abs()),
        }
    }

    /// Returns the smallest integer greater than or equal to the number
    #[must_use]
    pub fn ceil(self) -> Self {
        match self {
            Self::F64(n) => Self::I64(n.ceil() as i64),
            Self::I64(n) => Self::I64(n),
        }
    }

    /// Returns the largest integer less than or equal to the number
    #[must_use]
    pub fn floor(self) -> Self {
        Self::I64(self.as_i64())
    }

    /// Returns the integer closest to the number
    ///
    /// Half-way values get rounded away from zero.
    #[must_use]
    pub fn round(self) -> Self {
        match self {
            Self::F64(n) => Self::I64(n.round() as i64),
            Self::I64(n) => Self::I64(n),
        }
    }

    /// Returns true if the number is represented by an `f64`
    pub fn is_f64(self) -> bool {
        matches!(self, Self::F64(_))
    }

    /// Returns true if the integer version of the number is representable by an `f64`
    pub fn is_i64_in_f64_range(&self) -> bool {
        if let Self::I64(n) = *self {
            (n as f64 as i64) == n
        } else {
            false
        }
    }

    /// Returns true if the number is not NaN or infinity
    pub fn is_finite(self) -> bool {
        match self {
            Self::F64(n) => n.is_finite(),
            Self::I64(_) => true,
        }
    }

    /// Returns true if the number is NaN
    pub fn is_nan(self) -> bool {
        match self {
            Self::F64(n) => n.is_nan(),
            Self::I64(_) => false,
        }
    }

    /// Returns the result of raising self to the power of `other`
    ///
    /// If both inputs are i64s then the result will also be an i64,
    /// otherwise the result will be an f64.
    #[must_use]
    pub fn pow(self, other: Self) -> Self {
        use ValueNumber::*;

        match (self, other) {
            (F64(a), F64(b)) => F64(a.powf(b)),
            (F64(a), I64(b)) => F64(a.powf(b as f64)),
            (I64(a), F64(b)) => F64((a as f64).powf(b)),
            (I64(a), I64(b)) => I64(a.pow(b as u32)),
        }
    }

    /// Returns the value transmuted to a `u64`
    pub fn to_bits(self) -> u64 {
        match self {
            Self::F64(n) => n.to_bits(),
            Self::I64(n) => n as u64,
        }
    }

    /// Returns the number as an `i64`, calling `floor` if the number is an `f64`
    pub fn as_i64(self) -> i64 {
        match self {
            Self::F64(n) => n.floor() as i64,
            Self::I64(n) => n,
        }
    }
}

impl fmt::Debug for ValueNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueNumber::F64(n) => write!(f, "Float({n})"),
            ValueNumber::I64(n) => write!(f, "Int({n})"),
        }
    }
}

impl fmt::Display for ValueNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueNumber::F64(n) => {
                if n.fract() > 0.0 {
                    write!(f, "{n}")
                } else {
                    write!(f, "{n:.1}")
                }
            }
            ValueNumber::I64(n) => write!(f, "{n}"),
        }
    }
}

impl Hash for ValueNumber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_bits())
    }
}

impl PartialEq for ValueNumber {
    fn eq(&self, other: &Self) -> bool {
        use ValueNumber::*;

        match (self, other) {
            (F64(a), F64(b)) => a == b,
            (F64(a), I64(b)) => *a == *b as f64,
            (I64(a), F64(b)) => *a as f64 == *b,
            (I64(a), I64(b)) => a == b,
        }
    }
}

impl Eq for ValueNumber {}

impl PartialOrd for ValueNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValueNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        use ValueNumber::*;

        let result = match (self, other) {
            (F64(a), F64(b)) => a.partial_cmp(b),
            (F64(a), I64(b)) => a.partial_cmp(&(*b as f64)),
            (I64(a), F64(b)) => (*a as f64).partial_cmp(b),
            (I64(a), I64(b)) => a.partial_cmp(b),
        };

        match result {
            Some(result) => result,
            None => match (self.is_nan(), other.is_nan()) {
                (false, true) => Ordering::Less,
                (true, false) => Ordering::Greater,
                _ => Ordering::Equal,
            },
        }
    }
}

impl ops::Neg for ValueNumber {
    type Output = ValueNumber;

    fn neg(self) -> ValueNumber {
        use ValueNumber::*;

        match self {
            F64(n) => F64(-n),
            I64(n) => I64(-n),
        }
    }
}

impl ops::Neg for &ValueNumber {
    type Output = ValueNumber;

    fn neg(self) -> ValueNumber {
        use ValueNumber::*;

        match *self {
            F64(n) => F64(-n),
            I64(n) => I64(-n),
        }
    }
}

macro_rules! number_traits_float {
    ($type:ident) => {
        impl From<$type> for ValueNumber {
            fn from(n: $type) -> ValueNumber {
                ValueNumber::F64(n as f64)
            }
        }

        impl From<&$type> for ValueNumber {
            fn from(n: &$type) -> ValueNumber {
                ValueNumber::F64(*n as f64)
            }
        }

        impl From<$type> for Value {
            fn from(value: $type) -> Self {
                Self::Number(value.into())
            }
        }

        impl From<&$type> for Value {
            fn from(value: &$type) -> Self {
                Self::Number(value.into())
            }
        }

        impl PartialEq<$type> for ValueNumber {
            fn eq(&self, b: &$type) -> bool {
                let b = *b as f64;
                match self {
                    ValueNumber::F64(a) => *a == b,
                    ValueNumber::I64(a) => *a as f64 == b,
                }
            }
        }

        impl PartialOrd<$type> for ValueNumber {
            fn partial_cmp(&self, b: &$type) -> Option<Ordering> {
                let b = *b as f64;
                match self {
                    ValueNumber::F64(a) => a.partial_cmp(&b),
                    ValueNumber::I64(a) => (*a as f64).partial_cmp(&b),
                }
            }
        }
    };
}

macro_rules! number_traits_int {
    ($type:ident) => {
        impl From<$type> for ValueNumber {
            fn from(n: $type) -> ValueNumber {
                ValueNumber::I64(n as i64)
            }
        }

        impl From<&$type> for ValueNumber {
            fn from(n: &$type) -> ValueNumber {
                ValueNumber::I64(*n as i64)
            }
        }

        impl From<$type> for Value {
            fn from(value: $type) -> Self {
                Self::Number(value.into())
            }
        }

        impl From<&$type> for Value {
            fn from(value: &$type) -> Self {
                Self::Number(value.into())
            }
        }

        impl PartialEq<$type> for ValueNumber {
            fn eq(&self, b: &$type) -> bool {
                let b = *b as i64;
                match self {
                    ValueNumber::F64(a) => (*a as i64) == b,
                    ValueNumber::I64(a) => *a == b,
                }
            }
        }

        impl PartialOrd<$type> for ValueNumber {
            fn partial_cmp(&self, b: &$type) -> Option<Ordering> {
                let b = *b as i64;
                match self {
                    ValueNumber::F64(a) => (*a as i64).partial_cmp(&b),
                    ValueNumber::I64(a) => a.partial_cmp(&b),
                }
            }
        }
    };
}

number_traits_float!(f32);
number_traits_float!(f64);

number_traits_int!(i8);
number_traits_int!(u8);
number_traits_int!(i16);
number_traits_int!(u16);
number_traits_int!(i32);
number_traits_int!(u32);
number_traits_int!(i64);
number_traits_int!(u64);
number_traits_int!(isize);
number_traits_int!(usize);

macro_rules! from_number {
    ($type:ident) => {
        impl From<ValueNumber> for $type {
            fn from(n: ValueNumber) -> $type {
                match n {
                    ValueNumber::F64(f) => f as $type,
                    ValueNumber::I64(i) => i as $type,
                }
            }
        }

        impl From<&ValueNumber> for $type {
            fn from(n: &ValueNumber) -> $type {
                match n {
                    ValueNumber::F64(f) => *f as $type,
                    ValueNumber::I64(i) => *i as $type,
                }
            }
        }
    };
}

from_number!(f32);
from_number!(f64);

from_number!(i32);
from_number!(u32);
from_number!(i64);
from_number!(u64);
from_number!(isize);
from_number!(usize);

macro_rules! number_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl ops::$trait for ValueNumber {
            type Output = ValueNumber;

            fn $fn(self, other: ValueNumber) -> ValueNumber {
                use ValueNumber::*;

                match (self, other) {
                    (F64(a), F64(b)) => F64(a $op b),
                    (F64(a), I64(b)) => F64(a $op b as f64),
                    (I64(a), F64(b)) => F64(a as f64 $op b),
                    (I64(a), I64(b)) => I64(a $op b),
                }
            }
        }

        impl ops::$trait for &ValueNumber {
            type Output = ValueNumber;

            fn $fn(self, other: &ValueNumber) -> ValueNumber {
                use ValueNumber::*;

                match (*self, *other) {
                    (F64(a), F64(b)) => F64(a $op b),
                    (F64(a), I64(b)) => F64(a $op b as f64),
                    (I64(a), F64(b)) => F64(a as f64 $op b),
                    (I64(a), I64(b)) => I64(a $op b),
                }
            }
        }
    };
}

number_op!(Add, add, +);
number_op!(Sub, sub, -);
number_op!(Mul, mul, *);
number_op!(Rem, rem, %);

impl ops::Div for ValueNumber {
    type Output = ValueNumber;

    fn div(self, other: ValueNumber) -> ValueNumber {
        use ValueNumber::*;

        match (self, other) {
            (F64(a), F64(b)) => F64(a / b),
            (F64(a), I64(b)) => F64(a / b as f64),
            (I64(a), F64(b)) => F64(a as f64 / b),
            (I64(a), I64(b)) => F64(a as f64 / b as f64),
        }
    }
}

impl ops::Div for &ValueNumber {
    type Output = ValueNumber;

    fn div(self, other: &ValueNumber) -> ValueNumber {
        use ValueNumber::*;

        match (*self, *other) {
            (F64(a), F64(b)) => F64(a / b),
            (F64(a), I64(b)) => F64(a / b as f64),
            (I64(a), F64(b)) => F64(a as f64 / b),
            (I64(a), I64(b)) => F64(a as f64 / b as f64),
        }
    }
}
