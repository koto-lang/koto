use crate::{
    Error, KotoSend, KotoSync, Ptr, PtrMut, Result, make_ptr, make_ptr_mut, runtime_error,
    vm::{KotoVm, ReturnOrYield},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};

use super::KValue;

struct BlockOnWakeState {
    is_woken: AtomicBool,
    thread: thread::Thread,
}

impl BlockOnWakeState {
    fn new(thread: thread::Thread) -> Self {
        Self {
            is_woken: AtomicBool::new(false),
            thread,
        }
    }

    fn mark_woken(&self) {
        self.is_woken.store(true, Ordering::SeqCst);
        self.thread.unpark();
    }

    fn reset(&self) {
        self.is_woken.store(false, Ordering::SeqCst);
    }

    fn is_woken(&self) -> bool {
        self.is_woken.load(Ordering::SeqCst)
    }
}

impl Wake for BlockOnWakeState {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.mark_woken();
    }
}

/// The result of polling a [KTask].
pub enum KTaskPoll {
    /// The task has finished.
    Ready(KValue),
    /// The task is waiting for an external event.
    Pending,
}

/// A native future that can be wrapped in a [KTask].
pub trait KotoFuture: Future<Output = Result<KValue>> + KotoSend + 'static {}

impl<T> KotoFuture for T where T: Future<Output = Result<KValue>> + KotoSend + 'static {}

type PinnedKotoFuture = Mutex<Pin<Box<dyn KotoFuture>>>;

/// The interface used by the runtime to spawn and poll tasks.
pub trait KotoTaskExecutor: KotoSend + KotoSync + 'static {
    /// Registers a task with the executor, returning the task handle that should be exposed to Koto.
    fn spawn(&self, task: KTask) -> Result<KTask> {
        Ok(task)
    }

    /// Polls a task.
    fn poll(&self, task: &mut KTask, context: &mut Context<'_>) -> Result<KTaskPoll> {
        task.poll_with_context(context)
    }

    /// Returns a task that will complete after the given duration.
    fn sleep(&self, duration: Duration) -> Result<KTask> {
        self.spawn(KTask::with_future(LocalSleep::new(duration)))
    }
}

/// The default task executor.
///
/// This executor polls Koto tasks and native futures in the current thread.
#[derive(Debug, Default)]
pub struct LocalTaskExecutor;

impl KotoTaskExecutor for LocalTaskExecutor {}

/// The active task manager shared by all VMs in a runtime.
pub struct ActiveTasks {
    executor: Ptr<dyn KotoTaskExecutor>,
    tasks: Vec<KTask>,
    is_polling: bool,
}

impl ActiveTasks {
    /// Creates a new active task manager with the given executor.
    pub fn new(executor: Ptr<dyn KotoTaskExecutor>) -> Self {
        Self {
            executor,
            tasks: Vec::new(),
            is_polling: false,
        }
    }

    /// Returns the executor used to spawn and poll tasks.
    pub fn executor(&self) -> &Ptr<dyn KotoTaskExecutor> {
        &self.executor
    }

    /// Registers a task with the executor.
    pub fn spawn(&mut self, task: KTask) -> Result<KTask> {
        let task = self.executor.spawn(task)?;

        self.add_task(task.clone());

        Ok(task)
    }

    /// Returns a task that will complete after the given duration.
    pub fn sleep(&mut self, duration: Duration) -> Result<KTask> {
        let task = self.executor.sleep(duration)?;
        self.add_task(task.clone());

        Ok(task)
    }

    /// Returns the active tasks that have been woken and should be polled.
    pub(crate) fn woken_tasks(&self, excluded_task: Option<&KTask>) -> Vec<KTask> {
        self.tasks
            .iter()
            .filter(|task| {
                excluded_task.is_none_or(|excluded| !task.is_same_instance(excluded))
                    && task.is_ready_to_poll()
            })
            .cloned()
            .collect()
    }

    /// Removes completed and failed tasks from the active task list.
    pub(crate) fn remove_inactive_tasks(&mut self) {
        self.tasks
            .retain(|task| !(task.is_complete() || task.is_failed()));
    }

    /// Returns true if the active task manager is already polling woken tasks.
    pub(crate) fn is_polling(&self) -> bool {
        self.is_polling
    }

    /// Sets whether the active task manager is polling woken tasks.
    pub(crate) fn set_is_polling(&mut self, is_polling: bool) {
        self.is_polling = is_polling;
    }

    fn add_task(&mut self, task: KTask) {
        if !self.tasks.iter().any(|t| t.is_same_instance(&task)) {
            self.tasks.push(task);
        }
    }
}

impl Default for ActiveTasks {
    fn default() -> Self {
        Self::new(make_ptr!(LocalTaskExecutor))
    }
}

/// A Koto task
///
/// Tasks can be backed by a suspended Koto VM, or by a native future.
#[derive(Clone)]
pub struct KTask(PtrMut<TaskData>);

impl KTask {
    /// Creates a new task from a VM that's ready to be resumed.
    pub(crate) fn with_vm(vm: KotoVm) -> Self {
        Self::with_state(TaskState::Running(vm))
    }

    /// Creates a new task from a native future.
    pub fn with_future(future: impl KotoFuture) -> Self {
        Self::with_state(TaskState::Future(Mutex::new(Box::pin(future))))
    }

    /// Creates a task that has already completed with the given value.
    pub fn with_value(value: KValue) -> Self {
        Self::with_state(TaskState::Complete(value))
    }

    /// Polls the task, returning [KTaskPoll::Pending] if the task isn't ready yet.
    pub fn poll(&mut self) -> Result<KTaskPoll> {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        self.poll_with_context(&mut context)
    }

    /// Polls the task with the given context.
    pub fn poll_with_context(&mut self, context: &mut Context<'_>) -> Result<KTaskPoll> {
        let wake_state = self.wake_state();
        wake_state.reset();

        let waker = Waker::from(Arc::new(TaskWaker {
            wake_state,
            parent_waker: context.waker().clone(),
        }));
        let mut task_context = Context::from_waker(&waker);

        let mut data = self.0.borrow_mut();
        let current = std::mem::replace(&mut data.state, TaskState::RunningNow);
        drop(data);

        match current {
            TaskState::Complete(value) => {
                self.set_state(TaskState::Complete(value.clone()));
                Ok(KTaskPoll::Ready(value))
            }
            TaskState::Failed(error) => {
                self.set_state(TaskState::Failed(error.clone()));
                Err(error)
            }
            TaskState::RunningNow => runtime_error!("task is already running"),
            TaskState::Running(mut vm) => match vm.continue_running_with_context(&mut task_context)
            {
                Ok(ReturnOrYield::Return(value)) => {
                    self.set_state(TaskState::Complete(value.clone()));
                    Ok(KTaskPoll::Ready(value))
                }
                Ok(ReturnOrYield::Yield(_)) => {
                    self.set_state(TaskState::Running(vm));
                    runtime_error!("task yielded a value")
                }
                Ok(ReturnOrYield::Pending) => {
                    self.set_state(TaskState::Running(vm));
                    Ok(KTaskPoll::Pending)
                }
                Err(error) => {
                    self.set_state(TaskState::Failed(error.clone()));
                    Err(error)
                }
            },
            TaskState::Future(future) => {
                let poll_result = {
                    let mut future = future.lock().unwrap();
                    future.as_mut().poll(&mut task_context)
                };

                match poll_result {
                    Poll::Ready(Ok(value)) => {
                        self.set_state(TaskState::Complete(value.clone()));
                        Ok(KTaskPoll::Ready(value))
                    }
                    Poll::Ready(Err(error)) => {
                        self.set_state(TaskState::Failed(error.clone()));
                        Err(error)
                    }
                    Poll::Pending => {
                        self.set_state(TaskState::Future(future));
                        Ok(KTaskPoll::Pending)
                    }
                }
            }
        }
    }

    /// Runs the task, returning an error if the task is pending.
    pub fn run_until_complete(&mut self) -> Result<KValue> {
        match self.poll()? {
            KTaskPoll::Ready(value) => Ok(value),
            KTaskPoll::Pending => runtime_error!("task is pending"),
        }
    }

    /// Runs the task to completion, blocking the current thread while it's pending.
    pub fn block_on(&mut self, vm: &KotoVm) -> Result<KValue> {
        let wake_state = Arc::new(BlockOnWakeState::new(thread::current()));
        let waker = Waker::from(wake_state.clone());
        let mut context = Context::from_waker(&waker);

        loop {
            wake_state.reset();

            match vm.poll_task_with_context(self, &mut context)? {
                KTaskPoll::Ready(value) => return Ok(value),
                KTaskPoll::Pending => {}
            }

            if vm.poll_woken_tasks_except(Some(self)) > 0 {
                continue;
            }

            if self.is_woken() || wake_state.is_woken() {
                continue;
            }

            thread::park();
        }
    }

    /// Returns true if the task has completed successfully.
    pub fn is_complete(&self) -> bool {
        matches!(self.0.borrow().state, TaskState::Complete(_))
    }

    /// Returns true if the task has failed.
    pub fn is_failed(&self) -> bool {
        matches!(self.0.borrow().state, TaskState::Failed(_))
    }

    /// Returns true if the task hasn't completed or failed.
    pub fn is_active(&self) -> bool {
        !(self.is_complete() || self.is_failed())
    }

    /// Returns true if the task has been woken since it was last polled.
    pub fn is_woken(&self) -> bool {
        self.wake_state().is_woken()
    }

    /// Returns true if two tasks refer to the same underlying task state.
    pub fn is_same_instance(&self, other: &Self) -> bool {
        Ptr::address(&self.0) == Ptr::address(&other.0)
    }

    fn with_state(state: TaskState) -> Self {
        Self(make_ptr_mut!(TaskData {
            state,
            wake_state: Arc::new(TaskWakeState::new()),
        }))
    }

    fn wake_state(&self) -> Arc<TaskWakeState> {
        self.0.borrow().wake_state.clone()
    }

    fn is_running_now(&self) -> bool {
        matches!(self.0.borrow().state, TaskState::RunningNow)
    }

    fn is_ready_to_poll(&self) -> bool {
        self.is_woken() && !(self.is_complete() || self.is_failed() || self.is_running_now())
    }

    fn set_state(&mut self, state: TaskState) {
        self.0.borrow_mut().state = state;
    }
}

impl Future for KTask {
    type Output = Result<KValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.poll_with_context(context) {
            Ok(KTaskPoll::Ready(value)) => Poll::Ready(Ok(value)),
            Ok(KTaskPoll::Pending) => Poll::Pending,
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}

enum TaskState {
    Running(KotoVm),
    Future(PinnedKotoFuture),
    Complete(KValue),
    Failed(Error),
    RunningNow,
}

struct TaskData {
    state: TaskState,
    wake_state: Arc<TaskWakeState>,
}

struct TaskWakeState {
    is_woken: AtomicBool,
}

impl TaskWakeState {
    fn new() -> Self {
        Self {
            is_woken: AtomicBool::new(true),
        }
    }

    fn wake(&self) {
        self.is_woken.store(true, Ordering::SeqCst);
    }

    fn reset(&self) {
        self.is_woken.store(false, Ordering::SeqCst);
    }

    fn is_woken(&self) -> bool {
        self.is_woken.load(Ordering::SeqCst)
    }
}

struct TaskWaker {
    wake_state: Arc<TaskWakeState>,
    parent_waker: Waker,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_state.wake();
        self.parent_waker.wake_by_ref();
    }
}

struct LocalSleep {
    deadline: Instant,
    state: Option<Arc<LocalSleepState>>,
}

impl LocalSleep {
    fn new(duration: Duration) -> Self {
        Self {
            deadline: Instant::now() + duration,
            state: None,
        }
    }
}

impl Future for LocalSleep {
    type Output = Result<KValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.deadline {
            return Poll::Ready(Ok(KValue::Null));
        }

        if let Some(state) = &self.state {
            if state.is_complete.load(Ordering::SeqCst) {
                Poll::Ready(Ok(KValue::Null))
            } else {
                *state.waker.lock().unwrap() = Some(context.waker().clone());
                Poll::Pending
            }
        } else {
            let state = Arc::new(LocalSleepState {
                is_complete: AtomicBool::new(false),
                waker: Mutex::new(Some(context.waker().clone())),
            });
            let thread_state = state.clone();
            let duration = self.deadline.saturating_duration_since(Instant::now());

            thread::spawn(move || {
                thread::sleep(duration);
                thread_state.is_complete.store(true, Ordering::SeqCst);
                if let Some(waker) = thread_state.waker.lock().unwrap().take() {
                    waker.wake();
                }
            });

            self.state = Some(state);
            Poll::Pending
        }
    }
}

struct LocalSleepState {
    is_complete: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    struct PendingOnce {
        has_returned_pending: bool,
    }

    impl Future for PendingOnce {
        type Output = Result<KValue>;

        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            if self.has_returned_pending {
                Poll::Ready(Ok(42.into()))
            } else {
                self.has_returned_pending = true;
                Poll::Pending
            }
        }
    }

    #[test]
    fn native_future_task() {
        let mut task = KTask::with_future(PendingOnce {
            has_returned_pending: false,
        });

        assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

        match task.poll().unwrap() {
            KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
            _ => panic!("unexpected task output"),
        }

        assert!(task.is_complete());
        assert!(!task.is_failed());
    }
}
