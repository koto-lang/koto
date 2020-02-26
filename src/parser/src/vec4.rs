use std::ops;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Vec4 ( pub f32, pub f32, pub f32, pub f32);

macro_rules! vec4_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl ops::$trait for Vec4 {
            type Output = Vec4;

            fn $fn(self, other: Vec4) -> Vec4 {
                Vec4(
                    self.0 $op other.0,
                    self.1 $op other.1,
                    self.2 $op other.2,
                    self.3 $op other.3,
                )
            }
        }

        impl ops::$trait<f32> for Vec4 {
            type Output = Vec4;

            fn $fn(self, other: f32) -> Vec4 {
                Vec4(
                    self.0 $op other,
                    self.1 $op other,
                    self.2 $op other,
                    self.3 $op other,
                )
            }
        }

        impl ops::$trait<Vec4> for f32 {
            type Output = Vec4;

            fn $fn(self, other: Vec4) -> Vec4 {
                Vec4(
                    self $op other.0,
                    self $op other.1,
                    self $op other.2,
                    self $op other.3,
                )
            }
        }

        impl ops::$trait<f64> for Vec4 {
            type Output = Self;

            fn $fn(self, other: f64) -> Vec4 {
                Vec4(
                    self.0 $op other as f32,
                    self.1 $op other as f32,
                    self.2 $op other as f32,
                    self.3 $op other as f32,
                )
            }
        }

        impl ops::$trait<Vec4> for f64 {
            type Output = Vec4;

            fn $fn(self, other: Vec4) -> Vec4 {
                Vec4(
                    self as f32 $op other.0,
                    self as f32 $op other.1,
                    self as f32 $op other.2,
                    self as f32 $op other.3,
                )
            }
        }

    };
}

vec4_op!(Add, add, +);
vec4_op!(Sub, sub, -);
vec4_op!(Mul, mul, *);
vec4_op!(Div, div, /);
vec4_op!(Rem, rem, %);
