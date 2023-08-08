#[macro_export]
macro_rules! impl_arithmetic_op {
    ($type:ident, $trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for $type {
            type Output = Self;

            fn $trait_fn(self, other: Self) -> Self {
                Self(self.0 $op other.0)
            }
        }

        impl ops::$trait<f64> for $type {
            type Output = Self;

            fn $trait_fn(self, other: f64) -> Self {
                Self(self.0 $op other)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_arithmetic_assign_op {
    ($type:ident, $trait:ident, $trait_fn:ident, $op:tt) => {
        impl ops::$trait for $type {
            fn $trait_fn(&mut self, other: $type) -> () {
                self.0 $op other.0;
            }
        }

        impl ops::$trait<f64> for $type {
            fn $trait_fn(&mut self, other: f64) -> () {
                self.0 $op other;
            }
        }
    };
}

#[macro_export]
macro_rules! impl_arithmetic_ops {
    ($type:ident)=> {
        use $crate::{impl_arithmetic_op, impl_arithmetic_assign_op};
        impl_arithmetic_op!($type, Add, add, +);
        impl_arithmetic_op!($type, Sub, sub, -);
        impl_arithmetic_op!($type, Mul, mul, *);
        impl_arithmetic_op!($type, Div, div, /);
        impl_arithmetic_assign_op!($type, AddAssign, add_assign, +=);
        impl_arithmetic_assign_op!($type, SubAssign, sub_assign, -=);
        impl_arithmetic_assign_op!($type, MulAssign, mul_assign, *=);
        impl_arithmetic_assign_op!($type, DivAssign, div_assign, /=);

        impl ops::Neg for $type {
            type Output = Self;

            fn neg(self) -> Self {
                Self(self.0.neg())
            }
        }
    }
}

#[macro_export]
macro_rules! geometry_arithmetic_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                Value::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    Ok((*$self $op *rhs).into())
                }
                Value::Number(n) => {
                    Ok((*$self $op f64::from(n)).into())
                }
                unexpected => {
                    type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! geometry_arithmetic_assign_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                Value::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    *$self $op *rhs;
                    Ok(())
                }
                Value::Number(n) => {
                    *$self $op f64::from(n);
                    Ok(())
                }
                unexpected => {
                    type_error(&format!("a {} or Number", Self::TYPE), unexpected)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! geometry_comparison_op {
    ($self:ident, $rhs:expr, $op:tt) => {
        {
            match $rhs {
                Value::Object(rhs) if rhs.is_a::<Self>() => {
                    let rhs = rhs.cast::<Self>().unwrap();
                    Ok(*$self $op *rhs)
                }
                unexpected => {
                    type_error(&format!("a {}", Self::TYPE), unexpected)
                }
            }
        }
    }
}
