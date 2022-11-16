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
macro_rules! koto_arithmetic_op {
    ($type:ident, $op:tt) => {
        |a, b| match b {
            [Value::ExternalValue(b)] if b.has_data::<$type>() =>{
                let b = b.data::<$type>().unwrap();
                Ok((*a $op *b).into())
            }
            [Value::Number(n)] => Ok((*a $op f64::from(n)).into()),
            unexpected => {
                type_error_with_slice(&format!("a {} or Number", stringify!($type)), unexpected)
            }
        }
    }
}

#[macro_export]
macro_rules! koto_arithmetic_assign_op {
    ($type:ident, $op:tt) => {
        |a, b| match b {
            [Value::ExternalValue(b)] if b.has_data::<$type>() =>{
                let b = b.data::<$type>().unwrap();
                *a $op *b;
                Ok(Value::Null)
            }
            [Value::Number(n)] => {
                *a $op f64::from(n);
                Ok(Value::Null)
            }
            unexpected => {
                type_error_with_slice(&format!("a {} or Number", stringify!($type)), unexpected)
            }
        }
    }
}

#[macro_export]
macro_rules! koto_comparison_op {
    ($type:ident, $op:tt) => {
        |a, b| match b {
            [Value::ExternalValue(b)] if b.has_data::<$type>() =>{
                let b = b.data::<$type>().unwrap();
                Ok((*a $op *b).into())
            }
            unexpected => type_error_with_slice(&format!("a {}", stringify!($type)), unexpected),
        }
    }
}

#[macro_export]
macro_rules! add_ops {
    ($type:ident, $builder:expr) => {{
        use {BinaryOp::*, UnaryOp::*};

        $builder
            .data_fn(Display, |x| Ok(x.to_string().into()))
            .data_fn(Negate, |x| Ok($type::from(-(*x)).into()))
            .data_fn_with_args(Add, koto_arithmetic_op!($type, +))
            .data_fn_with_args(Subtract, koto_arithmetic_op!($type, -))
            .data_fn_with_args(Multiply, koto_arithmetic_op!($type, *))
            .data_fn_with_args(Divide, koto_arithmetic_op!($type, /))
            .data_fn_with_args_mut(AddAssign, koto_arithmetic_assign_op!($type, +=))
            .data_fn_with_args_mut(SubtractAssign, koto_arithmetic_assign_op!($type, -=))
            .data_fn_with_args_mut(MultiplyAssign, koto_arithmetic_assign_op!($type, *=))
            .data_fn_with_args_mut(DivideAssign, koto_arithmetic_assign_op!($type, /=))
            .data_fn_with_args(Equal, koto_comparison_op!($type, ==))
            .data_fn_with_args(NotEqual, koto_comparison_op!($type, !=))
    }}
}
