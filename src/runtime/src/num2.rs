use std::{
    fmt,
    hash::{Hash, Hasher},
    ops,
};

#[derive(Clone, Copy, Debug, Default, PartialOrd)]
pub struct Num2(pub f64, pub f64);

impl Num2 {
    pub fn abs(&self) -> Self {
        Num2(self.0.abs(), self.1.abs())
    }
}

impl PartialEq for Num2 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Hash for Num2 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.to_bits());
        state.write_u64(self.1.to_bits());
    }
}

impl fmt::Display for Num2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "num2({}, {})", self.0, self.1)
    }
}

impl ops::Index<usize> for Num2 {
    type Output = f64;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.0,
            1 => &self.1,
            _ => panic!("Invalid index for Num2"),
        }
    }
}

impl ops::IndexMut<usize> for Num2 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.0,
            1 => &mut self.1,
            _ => panic!("Invalid index for Num2"),
        }
    }
}

impl ops::Neg for Num2 {
    type Output = Num2;

    fn neg(self) -> Self::Output {
        Self(-self.0, -self.1)
    }
}

impl ops::Neg for &Num2 {
    type Output = Num2;

    fn neg(self) -> Self::Output {
        Num2(-self.0, -self.1)
    }
}

macro_rules! num2_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl ops::$trait for Num2 {
            type Output = Num2;

            fn $fn(self, other: Num2) -> Num2 {
                Num2(
                    self.0 $op other.0,
                    self.1 $op other.1,
                )
            }
        }

        impl ops::$trait<&Num2> for Num2 {
            type Output = Num2;

            fn $fn(self, other: &Num2) -> Num2 {
                Num2(
                    self.0 $op other.0,
                    self.1 $op other.1,
                )
            }
        }

        impl ops::$trait<&Num2> for &Num2 {
            type Output = Num2;

            fn $fn(self, other: &Num2) -> Num2 {
                Num2(
                    self.0 $op other.0,
                    self.1 $op other.1,
                )
            }
        }

        impl ops::$trait<f64> for Num2 {
            type Output = Num2;

            fn $fn(self, other: f64) -> Num2 {
                Num2(
                    self.0 $op other,
                    self.1 $op other,
                )
            }
        }

        impl ops::$trait<Num2> for f64 {
            type Output = Num2;

            fn $fn(self, other: Num2) -> Num2 {
                Num2(
                    self $op other.0,
                    self $op other.1,
                )
            }
        }

        impl ops::$trait<&f64> for &Num2 {
            type Output = Num2;

            fn $fn(self, other: &f64) -> Num2 {
                Num2(
                    self.0 $op *other as f64,
                    self.1 $op *other as f64,
                )
            }
        }

        impl ops::$trait<&Num2> for &f64 {
            type Output = Num2;

            fn $fn(self, other: &Num2) -> Num2 {
                Num2(
                    *self as f64 $op other.0,
                    *self as f64 $op other.1,
                )
            }
        }
    };
}

num2_op!(Add, add, +);
num2_op!(Sub, sub, -);
num2_op!(Mul, mul, *);
num2_op!(Div, div, /);
num2_op!(Rem, rem, %);
