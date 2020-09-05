use std::{
    fmt,
    hash::{Hash, Hasher},
    ops,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Num4(pub f32, pub f32, pub f32, pub f32);

impl Num4 {
    pub fn abs(&self) -> Self {
        Self(self.0.abs(), self.1.abs(), self.2.abs(), self.3.abs())
    }
}

impl Hash for Num4 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.0.to_bits());
        state.write_u32(self.1.to_bits());
        state.write_u32(self.2.to_bits());
        state.write_u32(self.3.to_bits());
    }
}

impl fmt::Display for Num4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} {} {} {})", self.0, self.1, self.2, self.3)
    }
}

impl ops::Index<usize> for Num4 {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.0,
            1 => &self.1,
            2 => &self.2,
            3 => &self.3,
            _ => panic!("Invalid index for Num4"),
        }
    }
}

impl ops::IndexMut<usize> for Num4 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.0,
            1 => &mut self.1,
            2 => &mut self.2,
            3 => &mut self.3,
            _ => panic!("Invalid index for Num4"),
        }
    }
}

impl ops::Neg for Num4 {
    type Output = Num4;

    fn neg(self) -> Self::Output {
        Self(-self.0, -self.1, -self.2, -self.3)
    }
}

impl ops::Neg for &Num4 {
    type Output = Num4;

    fn neg(self) -> Self::Output {
        Num4(-self.0, -self.1, -self.2, -self.3)
    }
}

macro_rules! num4_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl ops::$trait for Num4 {
            type Output = Num4;

            fn $fn(self, other: Num4) -> Num4 {
                Num4(
                    self.0 $op other.0,
                    self.1 $op other.1,
                    self.2 $op other.2,
                    self.3 $op other.3,
                )
            }
        }

        impl ops::$trait<&Num4> for Num4 {
            type Output = Num4;

            fn $fn(self, other: &Num4) -> Num4 {
                Num4(
                    self.0 $op other.0,
                    self.1 $op other.1,
                    self.2 $op other.2,
                    self.3 $op other.3,
                )
            }
        }

        impl ops::$trait<&Num4> for &Num4 {
            type Output = Num4;

            fn $fn(self, other: &Num4) -> Num4 {
                Num4(
                    self.0 $op other.0,
                    self.1 $op other.1,
                    self.2 $op other.2,
                    self.3 $op other.3,
                )
            }
        }

        impl ops::$trait<f32> for Num4 {
            type Output = Num4;

            fn $fn(self, other: f32) -> Num4 {
                Num4(
                    self.0 $op other,
                    self.1 $op other,
                    self.2 $op other,
                    self.3 $op other,
                )
            }
        }

        impl ops::$trait<Num4> for f32 {
            type Output = Num4;

            fn $fn(self, other: Num4) -> Num4 {
                Num4(
                    self $op other.0,
                    self $op other.1,
                    self $op other.2,
                    self $op other.3,
                )
            }
        }

        impl ops::$trait<f64> for Num4 {
            type Output = Self;

            fn $fn(self, other: f64) -> Num4 {
                Num4(
                    self.0 $op other as f32,
                    self.1 $op other as f32,
                    self.2 $op other as f32,
                    self.3 $op other as f32,
                )
            }
        }

        impl ops::$trait<&f64> for &Num4 {
            type Output = Num4;

            fn $fn(self, other: &f64) -> Num4 {
                Num4(
                    self.0 $op *other as f32,
                    self.1 $op *other as f32,
                    self.2 $op *other as f32,
                    self.3 $op *other as f32,
                )
            }
        }

        impl ops::$trait<Num4> for f64 {
            type Output = Num4;

            fn $fn(self, other: Num4) -> Num4 {
                Num4(
                    self as f32 $op other.0,
                    self as f32 $op other.1,
                    self as f32 $op other.2,
                    self as f32 $op other.3,
                )
            }
        }

        impl ops::$trait<&Num4> for &f64 {
            type Output = Num4;

            fn $fn(self, other: &Num4) -> Num4 {
                Num4(
                    *self as f32 $op other.0,
                    *self as f32 $op other.1,
                    *self as f32 $op other.2,
                    *self as f32 $op other.3,
                )
            }
        }
    };
}

num4_op!(Add, add, +);
num4_op!(Sub, sub, -);
num4_op!(Mul, mul, *);
num4_op!(Div, div, /);
num4_op!(Rem, rem, %);
