use crate::{BinaryOp, KotoBackend, ReadOp, UnaryOp, WriteOp};

/// Shared VM operations available in both `koto_runtime` and `koto_plugin`.
///
/// This trait can't use the plain `KotoVm` name because both backends already expose
/// a concrete `KotoVm` type.
pub trait KotoVmTrait<B: KotoBackend>: Clone {
    /// Returns a VM that shares the same execution context.
    fn spawn_shared_vm(&self) -> Self;

    /// Calls a function using the VM.
    fn call_function(
        &mut self,
        function: B::Value,
        args: &[B::Value],
    ) -> Result<B::Value, B::Error>;

    /// Calls an instance function using the VM.
    fn call_instance_function(
        &mut self,
        instance: B::Value,
        function: B::Value,
        args: &[B::Value],
    ) -> Result<B::Value, B::Error>;

    /// Runs a unary op using the VM.
    fn run_unary_op(&mut self, op: UnaryOp, value: B::Value) -> Result<B::Value, B::Error>;

    /// Runs a binary op using the VM.
    fn run_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: B::Value,
        rhs: B::Value,
    ) -> Result<B::Value, B::Error>;

    /// Runs a read op using the VM.
    fn run_read_op(
        &mut self,
        op: ReadOp,
        container: B::Value,
        read_arg: B::Value,
    ) -> Result<B::Value, B::Error>;

    /// Runs a write op using the VM.
    fn run_write_op(
        &mut self,
        op: WriteOp,
        container: B::Value,
        write_arg: B::Value,
        write_value: B::Value,
    ) -> Result<B::Value, B::Error>;
}
