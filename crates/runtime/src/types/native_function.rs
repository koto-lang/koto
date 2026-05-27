use crate::types::value::RegisterSlice;
use crate::{Ptr, Result, error::unexpected_args_after_instance, prelude::*};
use koto_bytecode::Chunk;
use koto_memory::Address;
use std::{
    fmt,
    future::{Future, poll_fn},
    hash::{Hash, Hasher},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

/// A trait for native functions used by the Koto runtime
pub trait KotoFunction:
    Fn(&mut CallContext) -> Result<KValue> + KotoSend + KotoSync + 'static
{
}

impl<T> KotoFunction for T where
    T: Fn(&mut CallContext) -> Result<KValue> + KotoSend + KotoSync + 'static
{
}

/// A trait for native functions that can execute VM operations and suspend the calling task.
pub trait KotoVmFunction:
    Fn(&mut VmCallContext) -> Result<VmOutput> + KotoSend + KotoSync + 'static
{
}

impl<T> KotoVmFunction for T where
    T: Fn(&mut VmCallContext) -> Result<VmOutput> + KotoSend + KotoSync + 'static
{
}

/// The output of a VM-aware native function.
pub type FunctionOutput = VmOutput;

/// An function that's defined outside of the Koto runtime
///
/// See [`KValue::NativeFunction`]
pub struct KNativeFunction {
    /// The function implementation that should be called when calling the external function
    //
    // Disable a clippy false positive, see https://github.com/rust-lang/rust-clippy/issues/9299
    // The type signature can't be simplified without stabilized trait aliases,
    // see https://github.com/rust-lang/rust/issues/55628
    #[allow(clippy::type_complexity)]
    pub function: Ptr<dyn KotoFunction>,
}

impl KNativeFunction {
    /// Creates a new external function
    pub fn new(function: impl KotoFunction) -> Self {
        Self {
            function: make_ptr!(function),
        }
    }

    /// Returns an address that can be used when rendering the function for debug output.
    pub fn address(&self) -> String {
        Ptr::address(&self.function).to_string()
    }
}

/// A function that's defined outside of the Koto runtime, and can execute VM operations.
///
/// See [`KValue::NativeVmFunction`]
pub struct KNativeVmFunction {
    /// The function implementation that should be called when calling the external function
    //
    // Disable a clippy false positive, see https://github.com/rust-lang/rust-clippy/issues/9299
    // The type signature can't be simplified without stabilized trait aliases,
    // see https://github.com/rust-lang/rust/issues/55628
    #[allow(clippy::type_complexity)]
    pub function: Ptr<dyn KotoVmFunction>,
}

impl KNativeVmFunction {
    /// Creates a new external function that can execute VM operations and suspend.
    pub fn new(function: impl KotoVmFunction) -> Self {
        Self {
            function: make_ptr!(function),
        }
    }

    /// Returns an address that can be used when rendering the function for debug output.
    pub fn address(&self) -> String {
        Ptr::address(&self.function).to_string()
    }
}

impl Clone for KNativeFunction {
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
        }
    }
}

impl Clone for KNativeVmFunction {
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
        }
    }
}

impl fmt::Debug for KNativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "external function: {:?}", Ptr::address(&self.function))
    }
}

impl fmt::Debug for KNativeVmFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "external vm function: {:?}",
            Ptr::address(&self.function)
        )
    }
}

impl Hash for KNativeFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Ptr::address(&self.function).hash(state)
    }
}

impl Hash for KNativeVmFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Ptr::address(&self.function).hash(state)
    }
}

/// The context provided to [native functions](KNativeFunction) when called
///
/// See also: [crate::MethodContext].
#[allow(missing_docs)]
pub struct CallContext<'a> {
    /// The VM making the call
    ///
    /// The VM can be used for operations like [KotoVm::call_function], although the
    /// [CallContext::args] and [CallContext::instance] functions return references, so the values
    /// need to be cloned before mutable operations can be called.
    ///
    /// If a VM needs to be retained after the call, then see [KotoVm::spawn_shared_vm].
    pub vm: &'a mut KotoVm,
    frame_base: u8,
    arg_count: u8,
}

impl<'a> CallContext<'a> {
    /// Returns a new context for calling external functions
    pub fn new(vm: &'a mut KotoVm, frame_base: u8, arg_count: u8) -> Self {
        Self {
            vm,
            frame_base,
            arg_count,
        }
    }

    /// Returns the `self` instance with which the function was called
    pub fn instance(&self) -> &KValue {
        self.vm.get_register(self.frame_base)
    }

    /// Returns the function call's arguments
    pub fn args(&self) -> &[KValue] {
        self.vm.register_slice(self.frame_base + 1, self.arg_count)
    }

    /// Returns the instance and args with which the function was called
    ///
    /// `instance_check` should check the provided value and return true if it is acceptable as an
    /// instance value for the function. If the function was called without an instance (e.g. it's
    /// being called as a standalone function), then the first argument will be checked and returned
    /// as the instance. If no instance is available that passes the check, then an 'expected
    /// arguments' error will be returned with the `expected_args_message`.
    ///
    /// This is used in the core library to allow operations like `list.size()` to be used in method
    /// contexts like `[1, 2, 3].to_tuple()`, or as standalone functions like `to_tuple [1, 2, 3]`.
    pub fn instance_and_args(
        &self,
        instance_check: impl Fn(&KValue) -> bool,
        expected_args_message: &str,
    ) -> Result<(&KValue, &[KValue])> {
        match (self.instance(), self.args()) {
            (instance, args) if instance_check(instance) => Ok((instance, args)),
            (_, [first, rest @ ..]) => {
                if instance_check(first) {
                    Ok((first, rest))
                } else {
                    unexpected_args_after_instance(expected_args_message, first, rest)
                }
            }
            (_, []) => unexpected_args(expected_args_message, &[]),
        }
    }

    /// Spawns a task in the runtime's task executor.
    pub fn spawn_task(&self, task: KTask) -> Result<KTask> {
        self.vm.spawn_task(task)
    }

    /// Spawns a native future in the runtime's task executor.
    pub fn spawn_future(&self, future: impl KotoFuture) -> Result<KTask> {
        self.vm.spawn_future(future)
    }

    /// Returns a task that will complete after the given duration.
    pub fn sleep(&self, duration: Duration) -> Result<KTask> {
        self.vm.sleep(duration)
    }
}

/// The context provided to VM-aware native functions.
#[allow(missing_docs)]
pub struct VmCallContext<'a> {
    ctx: CallContext<'a>,
}

impl<'a> VmCallContext<'a> {
    /// Returns a new context for calling VM-aware external functions.
    pub fn new(vm: &'a mut KotoVm, frame_base: u8, arg_count: u8) -> Self {
        Self {
            ctx: CallContext::new(vm, frame_base, arg_count),
        }
    }

    /// Returns the `self` instance with which the function was called.
    pub fn instance(&self) -> &KValue {
        self.ctx.instance()
    }

    /// Returns the function call's arguments.
    pub fn args(&self) -> &[KValue] {
        self.ctx.args()
    }

    /// Returns the instance and args with which the function was called.
    pub fn instance_and_args(
        &self,
        instance_check: impl Fn(&KValue) -> bool,
        expected_args_message: &str,
    ) -> Result<(&KValue, &[KValue])> {
        self.ctx
            .instance_and_args(instance_check, expected_args_message)
    }

    /// Returns the VM's stdout handle.
    pub fn stdout(&self) -> Ptr<dyn KotoFile> {
        self.ctx.vm.stdout().clone()
    }

    /// Spawns a VM that shares this call's runtime context.
    pub fn spawn_shared_vm(&self) -> KotoVm {
        self.ctx.vm.spawn_shared_vm_with_current_instruction()
    }

    /// Spawns an await-compatible VM that shares this call's runtime context.
    pub fn spawn_async_vm(&self) -> AsyncKotoVm {
        AsyncKotoVm::new(self.ctx.vm.spawn_shared_vm_with_current_instruction())
    }

    /// Runs VM operations, returning immediately when possible and suspending otherwise.
    pub fn run_with_vm<F, Fut>(&mut self, f: F) -> Result<VmOutput>
    where
        F: FnOnce(AsyncKotoVm) -> Fut,
        Fut: Future<Output = Result<KValue>> + KotoSend + 'static,
    {
        let future = f(self.spawn_async_vm());
        let mut task = KTask::with_future(future);

        let poll_result = if let Some(waker) = self.ctx.vm.current_task_waker() {
            let mut context = Context::from_waker(&waker);
            task.poll_with_context(&mut context)?
        } else {
            task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(value) => Ok(VmOutput::Ready(value)),
            KTaskPoll::Pending => Ok(VmOutput::Pending(task)),
        }
    }
}

/// An await-compatible facade for running VM operations.
pub struct AsyncKotoVm {
    vm: KotoVm,
}

struct AsyncDisplayContext {
    result: String,
    parent_containers: Vec<Address>,
    debug: bool,
}

trait AsyncDisplayFuture: Future<Output = Result<()>> + KotoSend {}

impl<T> AsyncDisplayFuture for T where T: Future<Output = Result<()>> + KotoSend {}

impl AsyncDisplayContext {
    fn new(debug: bool) -> Self {
        Self {
            result: String::new(),
            parent_containers: Vec::new(),
            debug,
        }
    }

    fn append(&mut self, s: impl AsRef<str>) {
        self.result.push_str(s.as_ref());
    }

    fn append_char(&mut self, c: char) {
        self.result.push(c);
    }

    fn result(self) -> String {
        self.result
    }

    fn is_contained(&self) -> bool {
        !self.parent_containers.is_empty()
    }

    fn is_in_parents(&self, id: Address) -> bool {
        self.parent_containers.contains(&id)
    }

    fn push_container(&mut self, id: Address) {
        self.parent_containers.push(id);
    }

    fn pop_container(&mut self) {
        self.parent_containers.pop();
    }
}

impl AsyncKotoVm {
    /// Creates an async VM facade from the given VM.
    #[must_use]
    pub fn new(vm: KotoVm) -> Self {
        Self { vm }
    }

    /// Returns the wrapped VM.
    #[must_use]
    pub fn into_vm(self) -> KotoVm {
        self.vm
    }

    /// Returns a reference to the VM's active exports map.
    #[must_use]
    pub fn exports(&self) -> &KMap {
        self.vm.exports()
    }

    /// Runs a chunk, waiting if execution suspends.
    pub async fn run(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        let output = self.vm.run(chunk)?;
        self.poll_output(output).await
    }

    /// Makes an iterator from the given value.
    pub async fn make_iterator(&mut self, mut value: KValue) -> Result<KIterator> {
        loop {
            if matches!(&value, KValue::Map(m) if m.contains_meta_key(&UnaryOp::Iterator.into())) {
                value = self.run_unary_op(UnaryOp::Iterator, value).await?;
            } else {
                return self.vm.make_iterator_from_ready_value(value);
            }
        }
    }

    /// Calls a function with a single argument, waiting if the call suspends.
    pub async fn call_function_with_arg(
        &mut self,
        function: KValue,
        arg: KValue,
    ) -> Result<KValue> {
        let output = self.vm.call_function(function, arg)?;
        self.poll_output(output).await
    }

    /// Calls a function with separate arguments, waiting if the call suspends.
    pub async fn call_function_with_args(
        &mut self,
        function: KValue,
        args: Vec<KValue>,
    ) -> Result<KValue> {
        let output = self.vm.call_function(function, args.as_slice())?;
        self.poll_output(output).await
    }

    /// Calls an instance function with separate arguments, waiting if the call suspends.
    pub async fn call_instance_function_with_args(
        &mut self,
        instance: KValue,
        function: KValue,
        args: Vec<KValue>,
    ) -> Result<KValue> {
        let output = self
            .vm
            .call_instance_function(instance, function, args.as_slice())?;
        self.poll_output(output).await
    }

    /// Calls a function with arguments bundled as a tuple, waiting if the call suspends.
    pub async fn call_function_with_tuple(
        &mut self,
        function: KValue,
        args: Vec<KValue>,
    ) -> Result<KValue> {
        let output = self.vm.call_function(function, CallArgs::AsTuple(&args))?;
        self.poll_output(output).await
    }

    /// Waits for the given value if it's a task.
    pub async fn resolve_task(&mut self, value: KValue) -> Result<KValue> {
        let KValue::Task(mut task) = value else {
            return Ok(value);
        };

        self.poll_task(&mut task).await
    }

    async fn poll_task(&mut self, task: &mut KTask) -> Result<KValue> {
        poll_fn(|context| match task.poll_with_context(context) {
            Ok(KTaskPoll::Ready(result)) => Poll::Ready(Ok(result)),
            Ok(KTaskPoll::Pending) => Poll::Pending,
            Err(error) => Poll::Ready(Err(error)),
        })
        .await
    }

    async fn poll_output(&mut self, output: VmOutput) -> Result<KValue> {
        match output {
            VmOutput::Ready(value) => Ok(value),
            VmOutput::Pending(mut task) => self.poll_task(&mut task).await,
        }
    }

    /// Runs any function tagged with `@test` in the provided map.
    pub async fn run_tests(&mut self, test_map: KMap) -> Result<KValue> {
        use KValue::{Map, Null};

        let (pre_test, post_test, meta_entry_count) = match test_map.meta_map() {
            Some(meta) => {
                let meta = meta.borrow();
                (
                    meta.get(&MetaKey::PreTest).cloned(),
                    meta.get(&MetaKey::PostTest).cloned(),
                    meta.len(),
                )
            }
            None => (None, None, 0),
        };

        let self_arg = Map(test_map.clone());

        for i in 0..meta_entry_count {
            let meta_entry = test_map.meta_map().and_then(|meta| {
                meta.borrow()
                    .get_index(i)
                    .map(|(key, value)| (key.clone(), value.clone()))
            });

            let Some((MetaKey::Test(test_name), test)) = meta_entry else {
                continue;
            };

            if !test.is_callable() {
                return unexpected_type(&format!("Callable for '{test_name}'"), &test);
            }

            if let Some(pre_test) = &pre_test
                && pre_test.is_callable()
            {
                self.call_instance_function_with_args(self_arg.clone(), pre_test.clone(), vec![])
                    .await
                    .map_err(|error| {
                        error.with_context(format!("while preparing to run test '{test_name}'"))
                    })?;
            }

            self.call_instance_function_with_args(self_arg.clone(), test, vec![])
                .await
                .map_err(|error| error.with_context(format!("while running test '{test_name}'")))?;

            if let Some(post_test) = &post_test
                && post_test.is_callable()
            {
                self.call_instance_function_with_args(self_arg.clone(), post_test.clone(), vec![])
                    .await
                    .map_err(|error| {
                        error.with_context(format!("after running test '{test_name}'"))
                    })?;
            }
        }

        Ok(Null)
    }

    /// Runs a binary operation, waiting if a task is returned.
    pub async fn run_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: KValue,
        rhs: KValue,
    ) -> Result<KValue> {
        let output = self.vm.run_binary_op(op, lhs, rhs)?;
        self.poll_output(output).await
    }

    /// Runs a read operation, waiting if a task is returned.
    pub async fn run_read_op(
        &mut self,
        op: ReadOp,
        container: KValue,
        read_arg: KValue,
    ) -> Result<KValue> {
        let output = self.vm.run_read_op(op, container, read_arg)?;
        self.poll_output(output).await
    }

    /// Runs a write operation, waiting if a task is returned.
    pub async fn run_write_op(
        &mut self,
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
    ) -> Result<KValue> {
        let output = self
            .vm
            .run_write_op(op, container, write_arg, write_value)?;
        self.poll_output(output).await
    }

    /// Runs a unary operation, waiting if a task is returned.
    pub async fn run_unary_op(&mut self, op: UnaryOp, value: KValue) -> Result<KValue> {
        let output = self.vm.run_unary_op(op, value)?;
        self.poll_output(output).await
    }

    fn display_value<'a>(
        &'a mut self,
        value: KValue,
        ctx: &'a mut AsyncDisplayContext,
    ) -> Pin<Box<dyn AsyncDisplayFuture + 'a>> {
        Box::pin(async move {
            match value {
                KValue::Null => ctx.append("null"),
                KValue::Bool(b) => ctx.append(b.to_string()),
                KValue::Number(n) => ctx.append(n.to_string()),
                KValue::Range(r) => ctx.append(r.to_string()),
                KValue::Function(f) => {
                    if ctx.debug {
                        ctx.append(format!(
                            "|| (chunk: {}, ip: {})",
                            Ptr::address(&f.chunk),
                            f.ip
                        ));
                    } else {
                        ctx.append("||");
                    }
                }
                KValue::NativeFunction(f) => {
                    if ctx.debug {
                        ctx.append(format!("|| ({})", f.address()));
                    } else {
                        ctx.append("||");
                    }
                }
                KValue::NativeVmFunction(f) => {
                    if ctx.debug {
                        ctx.append(format!("|| ({})", f.address()));
                    } else {
                        ctx.append("||");
                    }
                }
                KValue::Iterator(_) => ctx.append("Iterator"),
                KValue::Task(_) => ctx.append("Task"),
                KValue::TemporaryTuple(RegisterSlice { start, count }) => {
                    ctx.append(format!("TemporaryTuple [{start}..{}]", start + count));
                }
                KValue::Str(s) => {
                    if ctx.is_contained() || ctx.debug {
                        ctx.append_char('\'');
                        ctx.append(s.as_str());
                        ctx.append_char('\'');
                    } else {
                        ctx.append(s.as_str());
                    }
                }
                KValue::List(list) => self.display_list(list, ctx).await?,
                KValue::Tuple(tuple) => self.display_tuple(tuple, ctx).await?,
                KValue::Map(map) => self.display_map(map, ctx).await?,
                KValue::Object(object) => {
                    let mut display_context = DisplayContext::with_vm(&self.vm);
                    if ctx.debug {
                        display_context = display_context.enable_debug();
                    }
                    object.try_borrow()?.display(&mut display_context)?;
                    ctx.append(display_context.result());
                }
            }

            Ok(())
        })
    }

    async fn display_list(&mut self, list: KList, ctx: &mut AsyncDisplayContext) -> Result<()> {
        ctx.append_char('[');

        let id = list.address();
        if ctx.is_in_parents(id) {
            ctx.append("...");
        } else {
            ctx.push_container(id);

            let values = list.data().iter().cloned().collect::<Vec<_>>();
            for (i, value) in values.into_iter().enumerate() {
                if i > 0 {
                    ctx.append(", ");
                }
                self.display_value(value, ctx).await?;
            }

            ctx.pop_container();
        }

        ctx.append_char(']');
        Ok(())
    }

    async fn display_tuple(&mut self, tuple: KTuple, ctx: &mut AsyncDisplayContext) -> Result<()> {
        ctx.append_char('(');

        let id = tuple.address();
        if ctx.is_in_parents(id) {
            ctx.append("...");
        } else {
            ctx.push_container(id);

            let values = tuple.iter().cloned().collect::<Vec<_>>();
            for (i, value) in values.into_iter().enumerate() {
                if i > 0 {
                    ctx.append(", ");
                }
                self.display_value(value, ctx).await?;
            }

            ctx.pop_container();
        }

        ctx.append_char(')');
        Ok(())
    }

    async fn display_map(&mut self, map: KMap, ctx: &mut AsyncDisplayContext) -> Result<()> {
        if ctx.debug && map.contains_meta_key(&UnaryOp::Debug.into()) {
            match self.run_unary_op(UnaryOp::Debug, map.into()).await? {
                KValue::Str(result) => {
                    ctx.append(result.as_str());
                    return Ok(());
                }
                unexpected => return unexpected_type("String from @debug", &unexpected),
            }
        }

        if map.contains_meta_key(&UnaryOp::Display.into()) {
            match self.run_unary_op(UnaryOp::Display, map.into()).await? {
                KValue::Str(result) => {
                    ctx.append(result.as_str());
                    return Ok(());
                }
                unexpected => return unexpected_type("String from @display", &unexpected),
            }
        }

        if let Some(meta_type) = map.meta_type() {
            ctx.append(meta_type.as_str());
            ctx.append_char(' ');
        }

        ctx.append_char('{');

        let id = map.address();
        if ctx.is_in_parents(id) {
            ctx.append("...");
        } else {
            ctx.push_container(id);

            let entries = map
                .data()
                .iter()
                .map(|(key, value)| (key.value().clone(), value.clone()))
                .collect::<Vec<_>>();
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    ctx.append(", ");
                }

                let mut key_ctx = DisplayContext::default();
                key.display(&mut key_ctx)?;
                ctx.append(key_ctx.result());
                ctx.append(": ");

                self.display_value(value, ctx).await?;
            }

            ctx.pop_container();
        }

        ctx.append_char('}');
        Ok(())
    }

    /// Returns a displayable string for the given value, waiting if `@display` suspends.
    pub async fn value_to_string(&mut self, value: KValue) -> Result<String> {
        let mut ctx = AsyncDisplayContext::new(false);
        self.display_value(value, &mut ctx).await?;
        Ok(ctx.result())
    }

    /// Returns a debug string for the given value, waiting if `@debug` suspends.
    pub async fn value_to_debug_string(&mut self, value: KValue) -> Result<String> {
        let mut ctx = AsyncDisplayContext::new(true);
        self.display_value(value, &mut ctx).await?;
        Ok(ctx.result())
    }

    /// Returns the next iterator output, waiting if the iterator is pending.
    pub async fn next(&mut self, iterator: &mut KIterator) -> Result<Option<KIteratorOutput>> {
        poll_fn(|context| match iterator.next_output_with_context(context) {
            KIteratorNext::Output(output) => Poll::Ready(Ok(Some(output))),
            KIteratorNext::Done => Poll::Ready(Ok(None)),
            KIteratorNext::Pending => Poll::Pending,
        })
        .await
    }

    /// Returns the next iterator output from the back, waiting if the iterator is pending.
    pub async fn next_back(&mut self, iterator: &mut KIterator) -> Result<Option<KIteratorOutput>> {
        poll_fn(
            |context| match iterator.next_back_output_with_context(context) {
                KIteratorNext::Output(output) => Poll::Ready(Ok(Some(output))),
                KIteratorNext::Done => Poll::Ready(Ok(None)),
                KIteratorNext::Pending => Poll::Pending,
            },
        )
        .await
    }
}
