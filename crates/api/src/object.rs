use crate::{KotoBackend, KotoObjectHandle, KotoObjectIterable, KotoType};
use std::fmt;

/// The borrowed `(instance, args)` pair returned by [`KotoCallContext::instance_and_args`].
pub type KotoInstanceAndArgs<'a, B> = (
    &'a <B as KotoBackend>::Value,
    &'a [<B as KotoBackend>::Value],
);

/// Shared named object access available in both backends.
pub trait KotoAccess<B: KotoBackend>: KotoType<B> {
    /// Called for access operations, e.g. `x.foo`.
    fn access(&self, key: &B::String) -> Result<Option<B::Value>, B::Error> {
        let _ = key;
        Ok(None)
    }

    /// Called for assignment operations, e.g. `x.foo = "bar"`.
    fn access_assign(&mut self, key: &B::String, value: &B::Value) -> Result<(), B::Error> {
        let _ = (key, value);
        unimplemented!()
    }
}

/// Shared object behavior implemented by concrete object types and object borrows.
pub trait KotoObjectOps<B: KotoBackend>: KotoObjectHandle<B> {
    /// Displays the object using the backend's display context.
    fn display(&self, ctx: &mut B::DisplayContext<'_>) -> Result<(), B::Error> {
        let _ = fmt::Write::write_str(ctx, self.type_string().as_ref());
        Ok(())
    }

    /// Applies unary negation to the object.
    fn negate(&self) -> Result<B::Value, B::Error> {
        B::unimplemented_object_op("@negate", self.type_string())
    }

    /// Called for indexing operations.
    fn index(&self, index: &B::Value) -> Result<B::Value, B::Error> {
        let _ = index;
        B::unimplemented_object_op("@index", self.type_string())
    }

    /// Called for assignment via indexing.
    fn index_assign(&mut self, index: &B::Value, value: &B::Value) -> Result<(), B::Error> {
        let _ = (index, value);
        B::unimplemented_object_op("@index_assign", self.type_string())
    }

    /// Returns the object's size, if any.
    fn size(&self) -> Result<Option<usize>, B::Error> {
        Ok(None)
    }

    /// Returns whether or not the object is callable.
    fn is_callable(&self) -> Result<bool, B::Error> {
        Ok(false)
    }

    /// Calls the object.
    fn call(&mut self, ctx: &mut B::CallContext<'_>) -> Result<B::Value, B::Error> {
        let _ = ctx;
        B::unimplemented_object_op("@||", self.type_string())
    }

    /// Returns whether the object is iterable.
    fn is_iterable(&self) -> Result<KotoObjectIterable, B::Error> {
        Ok(KotoObjectIterable::NotIterable)
    }

    /// Produces an iterator for iterable objects.
    fn make_iterator(&self, vm: &mut B::Vm) -> Result<B::Iterator, B::Error> {
        let _ = vm;
        B::unimplemented_object_op("@iterator", self.type_string())
    }

    /// Returns the next value if the object is a forward iterator.
    fn iterator_next(&mut self, vm: &mut B::Vm) -> Result<Option<B::IteratorOutput>, B::Error> {
        let _ = vm;
        Ok(None)
    }

    /// Returns the next value from the end if the object is a bidirectional iterator.
    fn iterator_next_back(
        &mut self,
        vm: &mut B::Vm,
    ) -> Result<Option<B::IteratorOutput>, B::Error> {
        let _ = vm;
        Ok(None)
    }

    /// Adds `other` to the object.
    fn add(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@+", self.type_string())
    }

    /// Adds the object to `other` when the object is on the RHS.
    fn add_rhs(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@+", self.type_string())
    }

    /// Subtracts `other` from the object.
    fn subtract(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@-", self.type_string())
    }

    /// Subtracts the object from `other` when the object is on the RHS.
    fn subtract_rhs(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@-", self.type_string())
    }

    /// Multiplies the object by `other`.
    fn multiply(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@*", self.type_string())
    }

    /// Multiplies `other` by the object when the object is on the RHS.
    fn multiply_rhs(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@*", self.type_string())
    }

    /// Divides the object by `other`.
    fn divide(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@/", self.type_string())
    }

    /// Divides `other` by the object when the object is on the RHS.
    fn divide_rhs(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@/", self.type_string())
    }

    /// Computes the object's remainder against `other`.
    fn remainder(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@%", self.type_string())
    }

    /// Computes `other`'s remainder against the object when the object is on the RHS.
    fn remainder_rhs(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@%", self.type_string())
    }

    /// Raises the object to the power of `other`.
    fn power(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@^", self.type_string())
    }

    /// Raises `other` to the power of the object when the object is on the RHS.
    fn power_rhs(&self, other: &B::Value) -> Result<B::Value, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@^", self.type_string())
    }

    /// Performs in-place addition on the object.
    fn add_assign(&mut self, other: &B::Value) -> Result<(), B::Error> {
        let _ = other;
        B::unimplemented_object_op("@+=", self.type_string())
    }

    /// Performs in-place subtraction on the object.
    fn subtract_assign(&mut self, other: &B::Value) -> Result<(), B::Error> {
        let _ = other;
        B::unimplemented_object_op("@-=", self.type_string())
    }

    /// Performs in-place multiplication on the object.
    fn multiply_assign(&mut self, other: &B::Value) -> Result<(), B::Error> {
        let _ = other;
        B::unimplemented_object_op("@*=", self.type_string())
    }

    /// Performs in-place division on the object.
    fn divide_assign(&mut self, other: &B::Value) -> Result<(), B::Error> {
        let _ = other;
        B::unimplemented_object_op("@/=", self.type_string())
    }

    /// Performs in-place remainder on the object.
    fn remainder_assign(&mut self, other: &B::Value) -> Result<(), B::Error> {
        let _ = other;
        B::unimplemented_object_op("@%=", self.type_string())
    }

    /// Performs in-place exponentiation on the object.
    fn power_assign(&mut self, other: &B::Value) -> Result<(), B::Error> {
        let _ = other;
        B::unimplemented_object_op("@^=", self.type_string())
    }

    /// Compares the object for equality.
    fn equal(&self, other: &B::Value) -> Result<bool, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@==", self.type_string())
    }

    /// Compares whether the object is less than `other`.
    fn less(&self, other: &B::Value) -> Result<bool, B::Error> {
        let _ = other;
        B::unimplemented_object_op("@<", self.type_string())
    }

    /// Compares whether the object is less than or equal to `other`.
    fn less_or_equal(&self, other: &B::Value) -> Result<bool, B::Error> {
        match self.less(other) {
            Ok(true) => Ok(true),
            Ok(false) => match self.equal(other) {
                Ok(result) => Ok(result),
                Err(error) if B::is_unimplemented_error(&error) => {
                    B::unimplemented_object_op("@<=", self.type_string())
                }
                error => error,
            },
            Err(error) if B::is_unimplemented_error(&error) => {
                B::unimplemented_object_op("@<=", self.type_string())
            }
            error => error,
        }
    }

    /// Compares whether the object is greater than `other`.
    fn greater(&self, other: &B::Value) -> Result<bool, B::Error> {
        match self.less(other) {
            Ok(true) => Ok(false),
            Ok(false) => match self.equal(other) {
                Ok(result) => Ok(!result),
                Err(error) if B::is_unimplemented_error(&error) => {
                    B::unimplemented_object_op("@>", self.type_string())
                }
                error => error,
            },
            Err(error) if B::is_unimplemented_error(&error) => {
                B::unimplemented_object_op("@>", self.type_string())
            }
            error => error,
        }
    }

    /// Compares whether the object is greater than or equal to `other`.
    fn greater_or_equal(&self, other: &B::Value) -> Result<bool, B::Error> {
        match self.less(other) {
            Ok(result) => Ok(!result),
            Err(error) if B::is_unimplemented_error(&error) => {
                B::unimplemented_object_op("@>=", self.type_string())
            }
            error => error,
        }
    }

    /// Compares the object for inequality.
    fn not_equal(&self, other: &B::Value) -> Result<bool, B::Error> {
        match self.equal(other) {
            Ok(result) => Ok(!result),
            Err(error) if B::is_unimplemented_error(&error) => {
                B::unimplemented_object_op("@!=", self.type_string())
            }
            error => error,
        }
    }

    /// Converts the object into a serializable value.
    fn serialize(&self) -> Result<B::Value, B::Error> {
        B::unimplemented_object_op("serialize", self.type_string())
    }
}

/// Shared function-call context operations available in both backends.
pub trait KotoCallContext<B: KotoBackend> {
    /// Returns the VM associated with the call.
    fn vm(&self) -> &B::Vm;

    /// Returns the VM associated with the call mutably.
    fn vm_mut(&mut self) -> &mut B::Vm;

    /// Returns the `self` instance used for the call.
    fn instance(&self) -> &B::Value;

    /// Returns the call arguments.
    fn args(&self) -> &[B::Value];

    /// Returns the instance and args with which the function was called.
    fn instance_and_args(
        &self,
        instance_check: impl Fn(&B::Value) -> bool,
        expected_args_message: &str,
    ) -> Result<KotoInstanceAndArgs<'_, B>, B::Error>;
}

/// Shared method-call context operations available in both backends.
pub trait KotoMethodContext<B: KotoBackend> {
    /// The borrowed instance type returned by [`KotoMethodContext::instance`].
    type Instance<'a>
    where
        Self: 'a;

    /// The mutably borrowed instance type returned by [`KotoMethodContext::instance_mut`].
    type InstanceMut<'a>
    where
        Self: 'a;

    /// Returns the VM associated with the method call.
    fn vm(&self) -> &B::Vm;

    /// Returns the method call arguments.
    fn args(&self) -> &[B::Value];

    /// Returns an immutable borrow of the object instance.
    fn instance(&self) -> Result<Self::Instance<'_>, B::Error>;

    /// Returns a mutable borrow of the object instance.
    fn instance_mut(&mut self) -> Result<Self::InstanceMut<'_>, B::Error>;

    /// Returns a clone of the instance as a value.
    fn instance_result(&self) -> Result<B::Value, B::Error>;
}

/// Shared named access operations available on object handles in both backends.
pub trait KotoNamedAccess<B: KotoBackend> {
    /// Looks up a named value.
    fn named_value(&self, key: &str) -> Result<Option<B::Value>, B::Error>;

    /// Assigns a named value.
    fn named_value_assign(&mut self, key: &str, value: &B::Value) -> Result<(), B::Error>;
}

/// Shared identity operations for values that can refer to runtime-owned data.
pub trait KotoIdentity {
    /// Returns `true` if both values refer to the same underlying runtime instance.
    fn is_same_instance(&self, other: &Self) -> bool;
}

/// Shared downcasting operations for object handles in both backends.
pub trait KotoObjectCast<B: KotoBackend> {
    /// The borrowed object reference type returned by [`KotoObjectCast::cast`].
    type ObjectRef<'a, T: 'static>
    where
        Self: 'a;

    /// The mutably borrowed object reference type returned by [`KotoObjectCast::cast_mut`].
    type ObjectRefMut<'a, T: 'static>
    where
        Self: 'a;

    /// Returns `true` if the object is of the given Rust type.
    fn is_a<T: KotoType<B> + 'static>(&self) -> bool;

    /// Attempts to borrow and cast the object to the specified Rust type.
    fn cast<T: KotoType<B> + 'static>(&self) -> Result<Self::ObjectRef<'_, T>, B::Error>;

    /// Attempts to mutably borrow and cast the object to the specified Rust type.
    fn cast_mut<T: KotoType<B> + 'static>(&mut self)
    -> Result<Self::ObjectRefMut<'_, T>, B::Error>;
}
