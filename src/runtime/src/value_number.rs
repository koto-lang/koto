use {
    crate::Value,
    std::{
        cmp::Ordering,
        fmt,
        hash::{Hash, Hasher},
        ops,
    },
};

#[derive(Clone, Copy)]
pub enum ValueNumber {
    F64(f64),
    I64(i64),
}

impl ValueNumber {
    #[must_use]
    pub fn abs(self) -> Self {
        match self {
            Self::F64(n) => Self::F64(n.abs()),
            Self::I64(n) => Self::I64(n.abs()),
        }
    }

    #[must_use]
    pub fn ceil(self) -> Self {
        match self {
            Self::F64(n) => Self::I64(n.ceil() as i64),
            Self::I64(n) => Self::I64(n),
        }
    }

    #[must_use]
    pub fn floor(self) -> Self {
        Self::I64(self.as_i64())
    }

    #[must_use]
    pub fn round(self) -> Self {
        match self {
            Self::F64(n) => Self::I64(n.round() as i64),
            Self::I64(n) => Self::I64(n),
        }
    }

    pub fn is_f64(self) -> bool {
        matches!(self, Self::F64(_))
    }

    pub fn is_i64_in_f64_range(&self) -> bool {
        if let Self::I64(n) = *self {
            (n as f64 as i64) == n
        } else {
            false
        }
    }

    pub fn is_nan(self) -> bool {
        match self {
            Self::F64(n) => n.is_nan(),
            Self::I64(_) => false,
        }
    }

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

    pub fn to_bits(self) -> u64 {
        match self {
            Self::F64(n) => n.to_bits(),
            Self::I64(n) => n as u64,
        }
    }

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
            ValueNumber::F64(n) => write!(f, "Float({})", n),
            ValueNumber::I64(n) => write!(f, "Int({})", n),
        }
    }
}

impl fmt::Display for ValueNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueNumber::F64(n) => {
                if n.fract() > 0.0 {
                    write!(f, "{}", n)
                } else {
                    write!(f, "{:.1}", n)
                }
            }
            ValueNumber::I64(n) => write!(f, "{}", n),
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
