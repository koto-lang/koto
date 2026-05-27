use koto_bytecode::{CompilerSettings, ModuleLoader};
use koto_runtime::{Ptr, PtrMut, prelude::*};
use koto_test_utils::OutputCapture;
use std::{
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    thread,
};

fn compile_script(script: &str) -> Ptr<koto_bytecode::Chunk> {
    ModuleLoader::default()
        .compile_script(script, None, CompilerSettings::default())
        .unwrap()
}

fn compile_script_with_path(script: &str, script_path: PathBuf) -> Ptr<koto_bytecode::Chunk> {
    ModuleLoader::default()
        .compile_script(
            script,
            Some(script_path.into()),
            CompilerSettings::default(),
        )
        .unwrap()
}

static TEMP_SCRIPT_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempScriptDir {
    path: PathBuf,
}

impl Drop for TempScriptDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn compile_script_with_module(
    main_script: &str,
    module_name: &str,
    module_script: &str,
) -> (TempScriptDir, Ptr<koto_bytecode::Chunk>) {
    compile_script_with_modules(main_script, &[(module_name, module_script)])
}

fn compile_script_with_modules(
    main_script: &str,
    modules: &[(&str, &str)],
) -> (TempScriptDir, Ptr<koto_bytecode::Chunk>) {
    let counter = TEMP_SCRIPT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "koto_async_import_{}_{}",
        std::process::id(),
        counter
    ));
    fs::create_dir_all(&path).unwrap();

    let main_path = path.join("main.koto");
    fs::write(&main_path, main_script).unwrap();

    for (module_name, module_script) in modules {
        let module_path = path.join(format!("{module_name}.koto"));
        fs::write(module_path, module_script).unwrap();
    }

    let chunk = compile_script_with_path(main_script, main_path);
    (TempScriptDir { path }, chunk)
}

struct AlwaysPending;

impl Future for AlwaysPending {
    type Output = koto_runtime::Result<KValue>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

struct PendingOnce {
    has_returned_pending: bool,
}

impl Future for PendingOnce {
    type Output = koto_runtime::Result<KValue>;

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if self.has_returned_pending {
            Poll::Ready(Ok(42.into()))
        } else {
            self.has_returned_pending = true;
            Poll::Pending
        }
    }
}

fn wakeable_native_vm_function(
    result: KValue,
    is_ready: PtrMut<bool>,
    waker: PtrMut<Option<Waker>>,
) -> KNativeVmFunction {
    KNativeVmFunction::new(move |ctx| {
        let result = result.clone();
        let is_ready = is_ready.clone();
        let waker = waker.clone();
        ctx.run_with_vm(move |_| async move {
            Wakeable { is_ready, waker }.await?;
            Ok(result)
        })
    })
}

struct Wakeable {
    is_ready: PtrMut<bool>,
    waker: PtrMut<Option<Waker>>,
}

impl Future for Wakeable {
    type Output = koto_runtime::Result<KValue>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if *self.is_ready.borrow() {
            Poll::Ready(Ok(42.into()))
        } else {
            *self.waker.borrow_mut() = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

fn make_wakeable_vm() -> (KotoVm, PtrMut<bool>, PtrMut<Option<Waker>>) {
    let vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    insert_wakeable(&vm, is_ready.clone(), waker.clone());

    (vm, is_ready, waker)
}

fn insert_wakeable(vm: &KotoVm, is_ready: PtrMut<bool>, waker: PtrMut<Option<Waker>>) {
    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );
}

fn insert_pending_once(vm: &KotoVm) {
    vm.prelude().insert(
        "pending_once",
        KNativeFunction::new(|ctx| {
            Ok(ctx
                .spawn_future(PendingOnce {
                    has_returned_pending: false,
                })?
                .into())
        }),
    );
}

fn run_wakeable_script(script: &str) -> KValue {
    let (mut vm, is_ready, waker) = make_wakeable_vm();
    run_wakeable_chunk(&mut vm, compile_script(script), is_ready, waker)
}

fn run_wakeable_chunk(
    vm: &mut KotoVm,
    chunk: Ptr<koto_bytecode::Chunk>,
    is_ready: PtrMut<bool>,
    waker: PtrMut<Option<Waker>>,
) -> KValue {
    let mut task = vm.run_as_task(chunk).unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(result) => result,
        KTaskPoll::Pending => panic!("unexpected pending task"),
    }
}

fn assert_number(value: &KValue, expected: i64) {
    assert!(matches!(value, KValue::Number(n) if i64::from(*n) == expected));
}

fn assert_number_tuple(value: &KValue, expected: &[i64]) {
    let KValue::Tuple(result) = value else {
        panic!("expected tuple, found {value:?}");
    };

    assert_eq!(result.len(), expected.len());
    for (value, expected) in result.iter().zip(expected) {
        assert_number(value, *expected);
    }
}

fn assert_string(value: &KValue, expected: &str) {
    assert!(matches!(value, KValue::Str(s) if s.as_str() == expected));
}

struct ThreadWakeable {
    is_ready: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Future for ThreadWakeable {
    type Output = koto_runtime::Result<KValue>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_ready.load(Ordering::SeqCst) {
            Poll::Ready(Ok(42.into()))
        } else {
            *self.waker.lock().unwrap() = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

fn insert_thread_wakeable(vm: &KotoVm) -> (Arc<AtomicBool>, Arc<Mutex<Option<Waker>>>) {
    let is_ready = Arc::new(AtomicBool::new(false));
    let waker = Arc::new(Mutex::new(None));

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(ThreadWakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    (is_ready, waker)
}

fn spawn_thread_waker(
    is_ready: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            if let Some(waker) = waker.lock().unwrap().take() {
                is_ready.store(true, Ordering::SeqCst);
                waker.wake();
                break;
            }

            thread::yield_now();
        }
    })
}

#[derive(Clone, Default)]
struct CountingExecutor {
    spawns: PtrMut<usize>,
    polls: PtrMut<usize>,
}

impl KotoTaskExecutor for CountingExecutor {
    fn spawn(&self, task: KTask) -> koto_runtime::Result<KTask> {
        *self.spawns.borrow_mut() += 1;
        Ok(task)
    }

    fn poll(&self, task: &mut KTask, context: &mut Context<'_>) -> koto_runtime::Result<KTaskPoll> {
        *self.polls.borrow_mut() += 1;
        LocalTaskExecutor.poll(task, context)
    }
}

#[test]
fn run_chunk_as_task() {
    let mut vm = KotoVm::default();
    let mut task = vm.run_as_task(compile_script("await 42")).unwrap();

    match task.run_until_complete().unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected result: {unexpected:?}"),
    }
}

#[test]
fn spawn_async_vm_runs_chunk() {
    let vm = KotoVm::default();
    let chunk = compile_script("1 + 1");
    let mut async_vm = vm.spawn_async_vm();
    let mut task = KTask::with_future(async move { async_vm.run(chunk).await });

    match task.run_until_complete().unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 2),
        unexpected => panic!("unexpected result: {unexpected:?}"),
    }
}

#[test]
fn block_on_waits_for_wake() {
    let mut vm = KotoVm::default();
    let (is_ready, waker) = insert_thread_wakeable(&vm);

    let mut task = vm.run_as_task(compile_script("await wakeable()")).unwrap();
    let wake_thread = spawn_thread_waker(is_ready, waker);

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }

    wake_thread.join().unwrap();
}

#[test]
fn pending_task_suspends_run() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_task",
        KNativeFunction::new(|ctx| Ok(ctx.spawn_future(AlwaysPending)?.into())),
    );

    let result = vm.run(compile_script("await pending_task()")).unwrap();
    assert!(result.is_pending());
}

#[test]
fn awaited_import_suspends_and_returns_exports() {
    let (_temp_dir, chunk) = compile_script_with_module(
        "
x = await import foo
x.value
",
        "foo",
        "
export value = await wakeable()
",
    );
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    match run_wakeable_chunk(&mut vm, chunk, is_ready, waker) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn awaited_from_import_suspends_and_binds_items() {
    let (_temp_dir, chunk) = compile_script_with_module(
        "
await from foo import value
value
",
        "foo",
        "
export value = await wakeable()
",
    );
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    match run_wakeable_chunk(&mut vm, chunk, is_ready, waker) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn plain_import_errors_when_module_suspends() {
    let (_temp_dir, chunk) = compile_script_with_module(
        "
import foo
",
        "foo",
        "
export value = await wakeable()
",
    );
    let (mut vm, _, _) = make_wakeable_vm();
    let mut task = vm.run_as_task(chunk).unwrap();
    let error = match task.poll() {
        Ok(_) => panic!("expected import to fail"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("requires await"));
}

#[test]
fn awaited_import_suspends_for_module_tests() {
    let (_temp_dir, chunk) = compile_script_with_module(
        "
x = await import foo
x.value
",
        "foo",
        "
@test async_test = || await wakeable()
export value = 99
",
    );
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    match run_wakeable_chunk(&mut vm, chunk, is_ready, waker) {
        KValue::Number(n) => assert_eq!(i64::from(n), 99),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn awaited_import_suspends_for_module_main() {
    let (_temp_dir, chunk) = compile_script_with_module(
        "
x = await import foo
x.value
",
        "foo",
        "
@main = || await wakeable()
export value = 99
",
    );
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    match run_wakeable_chunk(&mut vm, chunk, is_ready, waker) {
        KValue::Number(n) => assert_eq!(i64::from(n), 99),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn concurrent_awaited_imports_share_in_progress_module() {
    let (_temp_dir, chunk) = compile_script_with_module(
        "
tasks = [1, 2]
  .each |_|
    task.spawn ||
      x = await import foo
      x.value
  .to_list()

results = await task.join tasks
results[0] + results[1]
",
        "foo",
        "
export value = await wakeable()
",
    );
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    match run_wakeable_chunk(&mut vm, chunk, is_ready, waker) {
        KValue::Number(n) => assert_eq!(i64::from(n), 84),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn recursive_async_import_errors() {
    let (_temp_dir, chunk) = compile_script_with_modules(
        "
await import a
",
        &[
            (
                "a",
                "
await import b
",
            ),
            (
                "b",
                "
await import a
",
            ),
        ],
    );
    let mut vm = KotoVm::default();
    let mut task = vm.run_as_task(chunk).unwrap();
    let error = match task.poll() {
        Ok(_) => panic!("expected import to fail"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("recursive import"));
}

#[test]
fn pending_task_resumes_from_await() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_once",
        KNativeFunction::new(|ctx| {
            Ok(ctx
                .spawn_future(PendingOnce {
                    has_returned_pending: false,
                })?
                .into())
        }),
    );
    let mut task = vm
        .run_as_task(compile_script("await pending_once()"))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn task_executor_is_used_for_spawning_and_polling() {
    let executor = CountingExecutor::default();
    let mut vm = KotoVm::with_settings(KotoVmSettings {
        task_executor: make_ptr!(executor.clone()),
        ..Default::default()
    });
    vm.prelude().insert(
        "pending_once",
        KNativeFunction::new(|ctx| {
            Ok(ctx
                .spawn_future(PendingOnce {
                    has_returned_pending: false,
                })?
                .into())
        }),
    );
    let mut task = vm
        .run_as_task(compile_script("await pending_once()"))
        .unwrap();

    assert_eq!(*executor.spawns.borrow(), 1);
    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert_eq!(*executor.spawns.borrow(), 2);
    assert_eq!(*executor.polls.borrow(), 2);

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }

    assert_eq!(*executor.polls.borrow(), 4);
}

#[test]
fn task_spawn_returns_a_task() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
f = |x| await x + 1
t = task.spawn f, 41
(koto.type t), (await t)
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert!(matches!(result.get(0), Some(KValue::Str(s)) if s.as_str() == "Task"));
            assert!(matches!(result.get(1), Some(KValue::Number(n)) if i64::from(*n) == 42));
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_spawn_handles_suspendable_function_calls() {
    match run_wakeable_script(
        "
f = ||
  x = [1]
  x.resize_with 2, ||
    await wakeable()
    42
  x

t = task.spawn f
await t
",
    ) {
        KValue::List(list) => {
            let list = list.data();
            assert_eq!(list.len(), 2);
            assert_number(&list[0], 1);
            assert_number(&list[1], 42);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_spawn_handles_suspendable_native_vm_functions() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "f",
        wakeable_native_vm_function(42.into(), is_ready.clone(), waker.clone()),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
from task import spawn
await spawn f
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn call_function_as_task_handles_suspendable_function_calls() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export f = ||
  x = [1]
  x.resize_with 2, ||
    await wakeable()
    42
  x
",
    ))
    .unwrap();

    let f = vm.exports().get("f").unwrap();
    let mut task = vm.call_function_as_task(f, &[]).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::List(list)) => {
            let list = list.data();
            assert_eq!(list.len(), 2);
            assert_number(&list[0], 1);
            assert_number(&list[1], 42);
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn call_async_function_as_task_returns_the_awaited_result() {
    let mut vm = KotoVm::default();

    vm.run(compile_script("export f = || await 42")).unwrap();

    let f = vm.exports().get("f").unwrap();
    let mut task = vm.call_function_as_task(f, &[]).unwrap();

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn run_binary_op_as_task_handles_async_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
foo = |x|
  x: x
  @+: |other|
    await wakeable()
    self.x + other.x

export lhs = foo 20
export rhs = foo 22
",
    ))
    .unwrap();

    let lhs = vm.exports().get("lhs").unwrap();
    let rhs = vm.exports().get("rhs").unwrap();
    let mut task = vm.run_binary_op_as_task(BinaryOp::Add, lhs, rhs).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn run_binary_op_returns_pending_for_async_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
foo = |x|
  x: x
  @+: |other|
    await wakeable()
    self.x + other.x

export lhs = foo 20
export rhs = foo 22
",
    ))
    .unwrap();

    let lhs = vm.exports().get("lhs").unwrap();
    let rhs = vm.exports().get("rhs").unwrap();
    let output = vm.run_binary_op(BinaryOp::Add, lhs, rhs).unwrap();

    assert!(output.is_pending());
    assert!(waker.borrow().is_some());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match output.into_task().block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn run_read_op_as_task_handles_async_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @index: |i|
    await wakeable()
    i + 1
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm
        .run_read_op_as_task(ReadOp::Index, x, KValue::from(41))
        .unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn run_unary_op_as_task_handles_async_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @size: ||
    await wakeable()
    42
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm.run_unary_op_as_task(UnaryOp::Size, x).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn value_to_string_as_task_handles_async_display() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @display: ||
    await wakeable()
    'display x'
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm.value_to_string_as_task(x).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Str(result)) => assert_eq!(result.as_str(), "display x"),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn value_to_debug_string_as_task_handles_async_debug() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @debug: ||
    await wakeable()
    'debug x'
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm.value_to_debug_string_as_task(x).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Str(result)) => assert_eq!(result.as_str(), "debug x"),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn value_to_debug_string_returns_pending_for_async_debug() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @debug: ||
    await wakeable()
    'debug x'
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let output = vm.value_to_debug_string(&x).unwrap();

    assert!(output.is_pending());
    assert!(waker.borrow().is_some());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match output.into_task().block_on(&vm).unwrap() {
        KValue::Str(result) => assert_eq!(result.as_str(), "debug x"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn value_to_string_as_task_handles_nested_async_display() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @display: ||
    await wakeable()
    'display x'
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm
        .value_to_string_as_task(KValue::List(KList::from_slice(&[x])))
        .unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Str(result)) => assert_eq!(result.as_str(), "[display x]"),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn value_to_string_blocks_on_nested_async_display() {
    let mut vm = KotoVm::default();
    let (is_ready, waker) = insert_thread_wakeable(&vm);

    vm.run(compile_script(
        "
export x =
  @display: ||
    await wakeable()
    'display x'
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let wake_thread = spawn_thread_waker(is_ready, waker);

    let result = vm
        .value_to_string(&KValue::List(KList::from_slice(&[x])))
        .unwrap()
        .into_task()
        .block_on(&vm)
        .unwrap();
    let KValue::Str(result) = result else {
        panic!("Expected String from @display, found {result:?}");
    };

    wake_thread.join().unwrap();

    assert_eq!(result.as_str(), "[display x]");
}

#[test]
fn value_to_string_returns_pending_for_async_display() {
    let mut vm = KotoVm::default();
    let (is_ready, waker) = insert_thread_wakeable(&vm);

    vm.run(compile_script(
        "
export x =
  @display: ||
    await wakeable()
    'display x'
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let output = vm
        .value_to_string(&KValue::List(KList::from_slice(&[x])))
        .unwrap();

    assert!(output.is_pending());
    assert!(waker.lock().unwrap().is_some());

    let wake_thread = spawn_thread_waker(is_ready, waker);

    match output.into_task().block_on(&vm).unwrap() {
        KValue::Str(result) => assert_eq!(result.as_str(), "[display x]"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }

    wake_thread.join().unwrap();
}

#[test]
fn call_function_blocks_on_async_function() {
    let mut vm = KotoVm::default();
    let (is_ready, waker) = insert_thread_wakeable(&vm);

    vm.run(compile_script(
        "
export f = ||
  await wakeable()
  99
",
    ))
    .unwrap();

    let f = vm.exports().get("f").unwrap();
    let wake_thread = spawn_thread_waker(is_ready, waker);
    let result = vm
        .call_function(f, &[])
        .unwrap()
        .into_task()
        .block_on(&vm)
        .unwrap();
    wake_thread.join().unwrap();

    assert_number(&result, 99);
}

#[test]
fn call_function_returns_pending_for_async_function() {
    let mut vm = KotoVm::default();
    let (is_ready, waker) = insert_thread_wakeable(&vm);

    vm.run(compile_script(
        "
export f = ||
  await wakeable()
  99
",
    ))
    .unwrap();

    let f = vm.exports().get("f").unwrap();
    let output = vm.call_function(f, &[]).unwrap();
    assert!(output.is_pending());

    let wake_thread = spawn_thread_waker(is_ready, waker);
    let result = output.into_task().block_on(&vm).unwrap();
    wake_thread.join().unwrap();

    assert_number(&result, 99);
}

#[test]
fn run_unary_next_as_task_handles_async_iterators() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
gen = ||
  yield await wakeable()

export iter = gen()
",
    ))
    .unwrap();

    let iter = vm.exports().get("iter").unwrap();
    let mut task = vm.run_unary_op_as_task(UnaryOp::Next, iter).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn overridden_index_assign_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
x =
  data: [0]
  @index_assign: |i, value|
    await wakeable()
    self.data[i] = value

assigned = x[0] = 42
x.data[0] + assigned
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 84),
        _ => panic!("unexpected result"),
    }
}

#[test]
fn overridden_access_assign_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
x =
  @access_assign: |key, value|
    await wakeable()
    map.insert self, key, value * 2

assigned = x.foo = 21
x.foo + assigned
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 63),
        _ => panic!("unexpected result"),
    }
}

#[test]
fn run_write_op_as_task_handles_async_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  data: [0]
  @index_assign: |i, value|
    await wakeable()
    self.data[i] = value
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm
        .run_write_op_as_task(WriteOp::IndexAssign, x.clone(), 0.into(), 42.into())
        .unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }

    let data = vm
        .run_read_op(ReadOp::Access, x, KValue::from("data"))
        .unwrap()
        .into_task()
        .block_on(&vm)
        .unwrap();
    match vm
        .run_read_op(ReadOp::Index, data, 0.into())
        .unwrap()
        .into_task()
        .block_on(&vm)
        .unwrap()
    {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected side effect"),
    }
}

#[test]
fn make_iterator_as_task_handles_async_iterator_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @iterator: ||
    await wakeable()
    [41, 42]
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let mut task = vm.make_iterator_as_task(x).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    let mut iterator = match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Iterator(iterator)) => iterator,
        _ => panic!("unexpected task output"),
    };

    match iterator.next() {
        Some(KIteratorOutput::Value(value)) => assert_number(&value, 41),
        _ => panic!("unexpected iterator output"),
    }
}

#[test]
fn make_iterator_returns_pending_for_async_iterator_overrides() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export x =
  @iterator: ||
    await wakeable()
    [41, 42]
",
    ))
    .unwrap();

    let x = vm.exports().get("x").unwrap();
    let output = vm.make_iterator(x).unwrap();

    assert!(output.is_pending());
    assert!(waker.borrow().is_some());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    let mut iterator = match output.into_task().block_on(&vm).unwrap() {
        KValue::Iterator(iterator) => iterator,
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    };

    match iterator.next() {
        Some(KIteratorOutput::Value(value)) => assert_number(&value, 41),
        _ => panic!("unexpected iterator output"),
    }
}

#[test]
fn iterator_next_blocks_on_async_generator() {
    let mut vm = KotoVm::default();
    insert_pending_once(&vm);

    vm.run(compile_script(
        "
gen = ||
  yield await pending_once()
  yield 99

export x = gen()
",
    ))
    .unwrap();

    let mut iterator = match vm.exports().get("x").unwrap() {
        KValue::Iterator(iterator) => iterator,
        unexpected => panic!("unexpected export: {unexpected:?}"),
    };

    match iterator.next() {
        Some(KIteratorOutput::Value(value)) => assert_number(&value, 42),
        _ => panic!("unexpected iterator output"),
    }

    match iterator.next() {
        Some(KIteratorOutput::Value(value)) => assert_number(&value, 99),
        _ => panic!("unexpected iterator output"),
    }
}

#[test]
fn iterator_next_blocks_on_async_string_split_predicate() {
    let mut vm = KotoVm::default();
    insert_pending_once(&vm);

    vm.run(compile_script(
        "
export x = '1-2'.split |c|
  await pending_once()
  c == '-'
",
    ))
    .unwrap();

    let mut iterator = match vm.exports().get("x").unwrap() {
        KValue::Iterator(iterator) => iterator,
        unexpected => panic!("unexpected export: {unexpected:?}"),
    };

    match iterator.next() {
        Some(KIteratorOutput::Value(KValue::Str(value))) => assert_eq!(value.as_str(), "1"),
        _ => panic!("unexpected iterator output"),
    }

    match iterator.next() {
        Some(KIteratorOutput::Value(KValue::Str(value))) => assert_eq!(value.as_str(), "2"),
        _ => panic!("unexpected iterator output"),
    }
}

#[test]
fn iterator_next_blocks_on_async_adaptor_callback() {
    let mut vm = KotoVm::default();
    insert_pending_once(&vm);

    vm.run(compile_script(
        "
export x = (1..2).each |n|
  await pending_once()
  n + 1
",
    ))
    .unwrap();

    let mut iterator = match vm.exports().get("x").unwrap() {
        KValue::Iterator(iterator) => iterator,
        unexpected => panic!("unexpected export: {unexpected:?}"),
    };

    match iterator.next() {
        Some(KIteratorOutput::Value(value)) => assert_number(&value, 2),
        _ => panic!("unexpected iterator output"),
    }
}

#[test]
fn run_tests_as_task_handles_async_tests() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export tests =
  result: 0
  @test async_test: ||
    await wakeable()
    self.result = 42
",
    ))
    .unwrap();

    let tests = match vm.exports().get("tests").unwrap() {
        KValue::Map(tests) => tests,
        _ => panic!("unexpected tests export"),
    };
    let mut task = vm.run_tests_as_task(tests.clone()).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Ready(KValue::Null)
    ));

    assert_number(&tests.get("result").unwrap(), 42);
}

#[test]
fn run_tests_returns_pending_for_async_tests() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.run(compile_script(
        "
export tests =
  result: 0
  @test async_test: ||
    await wakeable()
    self.result = 42
",
    ))
    .unwrap();

    let tests = match vm.exports().get("tests").unwrap() {
        KValue::Map(tests) => tests,
        _ => panic!("unexpected tests export"),
    };
    let output = vm.run_tests(tests.clone()).unwrap();

    assert!(output.is_pending());
    assert!(waker.borrow().is_some());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(matches!(
        output.into_task().block_on(&vm).unwrap(),
        KValue::Null
    ));
    assert_number(&tests.get("result").unwrap(), 42);
}

#[test]
fn native_vm_meta_op_suspends_instruction() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    let mut meta = MetaMap::default();
    meta.insert(
        BinaryOp::Add.into(),
        wakeable_native_vm_function(42.into(), is_ready.clone(), waker.clone()).into(),
    );

    vm.prelude()
        .insert("x", KMap::with_contents(ValueMap::default(), Some(meta)));

    let mut task = vm.run_as_task(compile_script("x + 1")).unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn native_vm_meta_op_suspends_iterator_next() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    let mut meta = MetaMap::default();
    meta.insert(
        UnaryOp::Next.into(),
        wakeable_native_vm_function(42.into(), is_ready.clone(), waker.clone()).into(),
    );

    vm.prelude()
        .insert("x", KMap::with_contents(ValueMap::default(), Some(meta)));

    let mut task = vm
        .run_as_task(compile_script("iterator.next(x).get()"))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn meta_iterator_next_suspends_for_internal_pending_operation() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()

x =
  done: false
  @next: ||
    if self.done
      null
    else
      self.done = true
      iterator.to_list gen()
      42

iterator.next(x).get()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn meta_iterator_next_back_suspends_for_internal_pending_operation() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()

x =
  done: false
  @next: || null
  @next_back: ||
    if self.done
      null
    else
      self.done = true
      iterator.to_list gen()
      42

iterator.next_back(x).get()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn native_vm_meta_assign_op_suspends_instruction() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    let mut meta = MetaMap::default();
    meta.insert(
        WriteOp::IndexAssign.into(),
        wakeable_native_vm_function(KValue::Null, is_ready.clone(), waker.clone()).into(),
    );

    vm.prelude()
        .insert("x", KMap::with_contents(ValueMap::default(), Some(meta)));

    let mut task = vm
        .run_as_task(compile_script(
            "
x[0] = 99
42
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn native_vm_runner_binary_op_suspends_for_discarded_meta_result() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "run_add_assign",
        KNativeVmFunction::new(|ctx| match ctx.args() {
            [lhs, rhs] => {
                let lhs = lhs.clone();
                let rhs = rhs.clone();
                ctx.run_with_vm(|mut vm| async move {
                    vm.run_binary_op(BinaryOp::AddAssign, lhs, rhs).await
                })
            }
            unexpected => unexpected_args("|Any, Any|", unexpected).map(FunctionOutput::Ready),
        }),
    );

    let mut meta = MetaMap::default();
    meta.insert(
        BinaryOp::AddAssign.into(),
        wakeable_native_vm_function(KValue::Null, is_ready.clone(), waker.clone()).into(),
    );

    vm.prelude()
        .insert("x", KMap::with_contents(ValueMap::default(), Some(meta)));

    let mut task = vm
        .run_as_task(compile_script("run_add_assign x, 1"))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Map(_)) => {}
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn native_vm_runner_read_op_suspends() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "read_index",
        KNativeVmFunction::new(|ctx| match ctx.args() {
            [container, index] => {
                let container = container.clone();
                let index = index.clone();
                ctx.run_with_vm(|mut vm| async move {
                    vm.run_read_op(ReadOp::Index, container, index).await
                })
            }
            unexpected => unexpected_args("|Any, Any|", unexpected).map(FunctionOutput::Ready),
        }),
    );

    let mut meta = MetaMap::default();
    meta.insert(
        ReadOp::Index.into(),
        wakeable_native_vm_function(42.into(), is_ready.clone(), waker.clone()).into(),
    );

    vm.prelude()
        .insert("x", KMap::with_contents(ValueMap::default(), Some(meta)));

    let mut task = vm.run_as_task(compile_script("read_index x, 0")).unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn native_vm_runner_write_op_suspends() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "write_index",
        KNativeVmFunction::new(|ctx| match ctx.args() {
            [container, index, value] => {
                let container = container.clone();
                let index = index.clone();
                let value = value.clone();
                ctx.run_with_vm(|mut vm| async move {
                    vm.run_write_op(WriteOp::IndexAssign, container, index, value)
                        .await
                })
            }
            unexpected => unexpected_args("|Any, Any, Any|", unexpected).map(FunctionOutput::Ready),
        }),
    );

    let mut meta = MetaMap::default();
    meta.insert(
        WriteOp::IndexAssign.into(),
        wakeable_native_vm_function(KValue::Null, is_ready.clone(), waker.clone()).into(),
    );

    vm.prelude()
        .insert("x", KMap::with_contents(ValueMap::default(), Some(meta)));

    let mut task = vm
        .run_as_task(compile_script("write_index x, 0, 42"))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn overridden_arithmetic_suspends_inside_fallback_check() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()

x =
  @+: |_|
    iterator.to_list gen()
    42

x + 1
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn iterator_callbacks_can_return_tasks_as_values() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
tasks = [1, 2]
  .each |n|
    task.spawn || await n
  .to_list()

types = []
for t in tasks
  types.push koto.type(t)

results = await task.join tasks

types[0], types[1], results[0], results[1]
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 4);
            assert_string(&result[0], "Task");
            assert_string(&result[1], "Task");
            assert_number(&result[2], 1);
            assert_number(&result[3], 2);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn poll_woken_tasks_advances_spawned_tasks() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
f = || await 42
task.spawn f
",
        ))
        .unwrap();

    let child_task = match task.block_on(&vm).unwrap() {
        KValue::Task(task) => task,
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    };

    assert!(!child_task.is_complete());
    assert_eq!(vm.poll_woken_tasks(), 1);
    assert!(child_task.is_complete());
}

#[test]
fn spawned_tasks_can_be_awaited_in_any_order() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
f = |x| await x
a = task.spawn f, 1
b = task.spawn f, 2
(await b), (await a), task.is_complete(a), task.is_complete(b)
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 4);
            assert!(matches!(result.get(0), Some(KValue::Number(n)) if i64::from(*n) == 2));
            assert!(matches!(result.get(1), Some(KValue::Number(n)) if i64::from(*n) == 1));
            assert!(matches!(result.get(2), Some(KValue::Bool(true))));
            assert!(matches!(result.get(3), Some(KValue::Bool(true))));
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn nested_spawned_tasks() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
inner = |x| await x + 1
outer = |x|
  t = task.spawn inner, x
  y = await t
  y + 1
await task.spawn outer, 40
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn completed_tasks_can_be_awaited_repeatedly() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
f = || await 21
t = task.spawn f
a = await t
b = await t
a + b
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_sleep_completes_with_default_executor() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
await task.sleep 0.001
42
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_status_reports_active_and_complete_tasks() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
f = || await 42
t = task.spawn f
before = task.status t
value = await t
after = task.status t
before, after, value
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 3);
            assert!(matches!(result.get(0), Some(KValue::Str(s)) if s.as_str() == "active"));
            assert!(matches!(result.get(1), Some(KValue::Str(s)) if s.as_str() == "complete"));
            assert!(matches!(result.get(2), Some(KValue::Number(n)) if i64::from(*n) == 42));
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_status_reports_failed_tasks() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
bad = ||
  await 0
  throw 'boom'
task.spawn bad
",
        ))
        .unwrap();

    let failed_task = match task.block_on(&vm).unwrap() {
        KValue::Task(task) => task,
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    };

    assert_eq!(vm.poll_woken_tasks(), 1);
    assert!(failed_task.is_failed());

    vm.prelude()
        .insert("failed_task", KValue::Task(failed_task));

    let mut task = vm
        .run_as_task(compile_script("task.status failed_task"))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Str(status) => assert_eq!(status.as_str(), "failed"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_join_waits_for_all_tasks() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
a = wakeable()
b = task.spawn || await 21
await task.join [a, b]
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert!(matches!(result.first(), Some(KValue::Number(n)) if i64::from(*n) == 42));
            assert!(matches!(result.get(1), Some(KValue::Number(n)) if i64::from(*n) == 21));
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn task_select_returns_the_first_ready_task() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_task",
        KNativeFunction::new(|ctx| Ok(ctx.spawn_future(AlwaysPending)?.into())),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
slow = pending_task()
fast = task.spawn || await 42
await task.select [slow, fast]
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_timeout_returns_the_task_result() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
t = task.spawn || await 42
await task.timeout t, 1
",
        ))
        .unwrap();

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn task_timeout_errors_when_the_timeout_completes_first() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_task",
        KNativeFunction::new(|ctx| Ok(ctx.spawn_future(AlwaysPending)?.into())),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
t = pending_task()
await task.timeout t, 0
",
        ))
        .unwrap();

    let error = task.block_on(&vm).unwrap_err();
    assert!(error.to_string().contains("task timed out"));
}

#[test]
fn failed_spawned_tasks_rethrow_when_awaited() {
    let mut vm = KotoVm::default();
    let mut task = vm
        .run_as_task(compile_script(
            "
f = ||
  await 0
  throw 'boom'
t = task.spawn f
await t
",
        ))
        .unwrap();

    let error = task.block_on(&vm).unwrap_err();
    assert!(error.to_string().contains("boom"));
}

#[test]
fn pending_future_wakes_task() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm.run_as_task(compile_script("await wakeable()")).unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn spawned_task_wakes_awaiting_task() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
f = || await wakeable()
t = task.spawn f
await t
",
        ))
        .unwrap();

    assert!(matches!(
        vm.poll_task(&mut task).unwrap(),
        KTaskPoll::Pending
    ));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match vm.poll_task(&mut task).unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn async_generator_iteration_suspends_the_calling_task() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_once",
        KNativeFunction::new(|ctx| {
            Ok(ctx
                .spawn_future(PendingOnce {
                    has_returned_pending: false,
                })?
                .into())
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
gen = ||
  yield await pending_once()
  yield await pending_once()
result = 0
for x in gen()
  result += x
result
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 84),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn async_generator_iteration_wakes_the_calling_task() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
gen = ||
  yield await wakeable()
result = 0
for x in gen()
  result = x
result
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn overridden_next_suspends_the_calling_task() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_once",
        KNativeFunction::new(|ctx| {
            Ok(ctx
                .spawn_future(PendingOnce {
                    has_returned_pending: false,
                })?
                .into())
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  n: 0
  @next: ||
    await pending_once()
    self.n += 1
    if self.n <= 2
      self.n
    else
      null

result = 0
for n in x
  result += n
result
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 3),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn overridden_next_wakes_the_calling_task() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  n: 0
  @next: ||
    if self.n == 0
      await wakeable()
      self.n = 1
      42
    else
      null

result = 0
for n in x
  result = n
result
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn overridden_next_back_suspends_the_calling_task() {
    match run_wakeable_script(
        "
x =
  n: 0
  @next: || null
  @next_back: ||
    if self.n == 0
      await wakeable()
      self.n = 1
      42
    else
      null

x.next_back().get()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn reversed_suspends_for_async_next_back() {
    match run_wakeable_script(
        "
x =
  n: 0
  @next: || null
  @next_back: ||
    if self.n == 0
      await wakeable()
      self.n = 1
      42
    else
      null

x.reversed().to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number(&result[0], 42);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn next_back_suspends_for_reversed_each_callback() {
    match run_wakeable_script(
        "
x = [1, 2]
  .each |n|
    await wakeable()
    n + 40
  .reversed()

x.next_back().get()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 41),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn overridden_index_result_can_be_awaited() {
    let mut vm = KotoVm::default();
    vm.prelude().insert(
        "pending_once",
        KNativeFunction::new(|ctx| {
            Ok(ctx
                .spawn_future(PendingOnce {
                    has_returned_pending: false,
                })?
                .into())
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  @index: |i|
    await pending_once()
    i + 1

await x[41]
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn overridden_index_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
x =
  @index: |i|
    await wakeable()
    i + 1

x[41]
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn overridden_access_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
x =
  @access: |key|
    await wakeable()
    key + '_value'

x.foo
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "foo_value"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn overridden_arithmetic_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @+: |other|
    await wakeable()
    self.x + other.x

(foo 20) + (foo 22)
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn overridden_comparison_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @<: |other|
    await wakeable()
    self.x < other.x

(foo 1) < (foo 2)
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_equality_suspends_for_async_element_equality() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @==: |other|
    await wakeable()
    self.x == other.x

[foo 1, foo 2] == [foo 1, foo 2]
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn map_inequality_suspends_for_async_value_equality() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @==: |other|
    await wakeable()
    self.x == other.x

{a: foo 1} != {a: foo 2}
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn comparison_fallbacks_suspend_for_async_ops() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @<: |other|
    await wakeable()
    self.x < other.x
  @==: |other|
    await wakeable()
    self.x == other.x

(foo 1) <= (foo 1) and (foo 2) > (foo 1) and (foo 2) >= (foo 2)
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn overridden_iterator_instruction_suspends_for_async_result() {
    match run_wakeable_script(
        "
x =
  @iterator: ||
    await wakeable()
    [40, 2].iter()

result = 0
for n in x
  result += n
result
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn koto_size_suspends_for_async_size_op() {
    match run_wakeable_script(
        "
x =
  @size: || await wakeable()

koto.size x
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn size_instruction_suspends_for_async_size_op() {
    match run_wakeable_script(
        "
x =
  @size: || await wakeable()

size x
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn argument_unpacking_suspends_for_async_size_op() {
    match run_wakeable_script(
        "
foo = |data|
  data: data
  @index: |index| self.data[index]
  @size: ||
    await wakeable()
    size self.data

f = |(a, b)| a + b
f foo (20, 22)
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn variadic_argument_unpacking_suspends_for_async_size_op() {
    match run_wakeable_script(
        "
foo = |data|
  data: data
  @index: |index| self.data[index]
  @size: ||
    await wakeable()
    size self.data

f = |(a, others...)| a + others.sum()
f foo (20, 2, 20)
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn packed_call_args_suspend_for_async_iterator_op() {
    match run_wakeable_script(
        "
f = |args...| args.sum()

x =
  @iterator: ||
    await wakeable()
    [20, 22].iter()

f x...
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn packed_call_args_suspend_for_async_next_op() {
    let mut vm = KotoVm::default();
    insert_pending_once(&vm);

    let mut task = vm
        .run_as_task(compile_script(
            "
f = |args...| args.sum()

x =
  values: [20, 22]
  i: 0

  @next: ||
    if self.i == size self.values
      null
    else
      await pending_once()
      result = self.values[self.i]
      self.i += 1
      result

f x...
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Number(n)) => assert_eq!(i64::from(n), 42),
        KTaskPoll::Ready(unexpected) => panic!("unexpected task output: {unexpected:?}"),
        KTaskPoll::Pending => panic!("unexpected pending task"),
    }
}

#[test]
fn io_print_suspends_for_async_display() {
    let (mut vm, output) = OutputCapture::make_vm_with_output_capture();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);
    insert_wakeable(&vm, is_ready.clone(), waker.clone());

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  @display: ||
    await wakeable()
    'value {42}'

print x
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(output.captured_output().is_empty());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Null) => {}
        _ => panic!("unexpected task output"),
    }

    assert_eq!(output.captured_output().as_str(), "value 42\n");
}

#[test]
fn io_print_suspends_for_nested_async_display() {
    let (mut vm, output) = OutputCapture::make_vm_with_output_capture();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);
    insert_wakeable(&vm, is_ready.clone(), waker.clone());

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  @display: ||
    await wakeable()
    'x'

print [x]
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(output.captured_output().is_empty());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Null) => {}
        _ => panic!("unexpected task output"),
    }

    assert_eq!(output.captured_output().as_str(), "[x]\n");
}

#[test]
fn io_file_write_suspends_for_nested_async_display() {
    let (mut vm, output) = OutputCapture::make_vm_with_output_capture();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);
    insert_wakeable(&vm, is_ready.clone(), waker.clone());

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  @display: ||
    await wakeable()
    'x'

io.stdout.write [x]
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(output.captured_output().is_empty());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Null) => {}
        _ => panic!("unexpected task output"),
    }

    assert_eq!(output.captured_output().as_str(), "[x]");
}

#[test]
fn io_file_write_line_suspends_for_nested_async_display() {
    let (mut vm, output) = OutputCapture::make_vm_with_output_capture();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);
    insert_wakeable(&vm, is_ready.clone(), waker.clone());

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  @display: ||
    await wakeable()
    'x'

io.stdout.write_line [x]
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(output.captured_output().is_empty());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::Null) => {}
        _ => panic!("unexpected task output"),
    }

    assert_eq!(output.captured_output().as_str(), "[x]\n");
}

#[test]
fn io_extend_path_suspends_for_async_display() {
    match run_wakeable_script(
        "
x =
  @display: ||
    await wakeable()
    'child'

io.extend_path 'root', x
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "root/child"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_for_async_generator() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
gen = ||
  yield await wakeable()
gen().to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert!(matches!(result.first(), Some(KValue::Number(n)) if i64::from(*n) == 42));
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn to_list_suspends_through_take_chain() {
    let mut vm = KotoVm::default();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);

    vm.prelude().insert(
        "wakeable",
        KNativeFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| {
                Ok(ctx
                    .spawn_future(Wakeable {
                        is_ready: is_ready.clone(),
                        waker: waker.clone(),
                    })?
                    .into())
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
gen = ||
  yield await wakeable()
  yield 99
gen().take(1).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert!(matches!(result.first(), Some(KValue::Number(n)) if i64::from(*n) == 42));
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn iterator_count_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 99
gen().count()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 2),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_last_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 99
gen().last()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 99),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_next_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
gen().next().get()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_advance_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
iter = gen()
iter.advance 1
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 0),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_tuple_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 99
gen().to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert!(matches!(&result[0], KValue::Number(n) if i64::from(*n) == 42));
            assert!(matches!(&result[1], KValue::Number(n) if i64::from(*n) == 99));
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_map_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield ('answer', await wakeable())
gen().to_map()
",
    ) {
        KValue::Map(result) => {
            assert!(matches!(result.get("answer"), Some(KValue::Number(n)) if i64::from(n) == 42));
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_string_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield '!'
gen().to_string()
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "42!"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_chain_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
gen().chain([99]).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 42);
            assert_number(&result[1], 99);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_skip_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 99
gen().skip(1).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number(&result[0], 99);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_step_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield 1
  yield await wakeable()
  yield 3
gen().step(2).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 1);
            assert_number(&result[1], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_zip_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
gen().zip([99]).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number_tuple(&result[0], &[42, 99]);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_chunks_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield 1
  yield await wakeable()
  yield 3
gen().chunks(2).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number_tuple(&result[0], &[1, 42]);
            assert_number_tuple(&result[1], &[3]);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_windows_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield 1
  yield await wakeable()
  yield 3
gen().windows(2).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number_tuple(&result[0], &[1, 42]);
            assert_number_tuple(&result[1], &[42, 3]);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_enumerate_adaptor() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 99
gen().enumerate().to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number_tuple(&result[0], &[0, 42]);
            assert_number_tuple(&result[1], &[1, 99]);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_flatten_adaptor_nested_iterator_creation() {
    match run_wakeable_script(
        "
x =
  @iterator: ||
    await wakeable()
    [41, 42].iter()

[x].flatten().to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 41);
            assert_number(&result[1], 42);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_adaptor_construction_suspends_for_async_iterator_op() {
    match run_wakeable_script(
        "
x =
  @iterator: ||
    await wakeable()
    [1, 2, 3].iter()

x.skip(1).to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 2);
            assert_number(&result[1], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_callback_adaptor_construction_suspends_for_async_iterator_op() {
    match run_wakeable_script(
        "
x =
  @iterator: ||
    await wakeable()
    [1, 2].iter()

x
  .each |n| n + 40
  .to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 41);
            assert_number(&result[1], 42);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_all_suspends_for_async_predicate() {
    match run_wakeable_script(
        "
[1, 2].all |n|
  await wakeable()
  n < 10
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_any_suspends_for_async_predicate() {
    match run_wakeable_script(
        "
[1, 42].any |n|
  await wakeable()
  n == 42
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_find_suspends_for_async_predicate() {
    match run_wakeable_script(
        "
[1, 42, 99].find |n|
  await wakeable()
  n == 42
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_position_suspends_for_async_predicate() {
    match run_wakeable_script(
        "
[1, 42, 99].position |n|
  await wakeable()
  n == 42
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 1),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_fold_suspends_for_async_callback() {
    match run_wakeable_script(
        "
[1, 2, 3].fold 0, |total, n|
  await wakeable()
  total + n
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 6),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_consume_suspends_for_async_callback() {
    match run_wakeable_script(
        "
seen = []
[1, 2, 3].consume |n|
  await wakeable()
  seen.push n
seen.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 2);
            assert_number(&result[2], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_sum_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 8
gen().sum()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 50),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_product_suspends_for_async_generator() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 2
gen().product()
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 84),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_min_suspends_for_async_key_function() {
    match run_wakeable_script(
        "
[3, 1, 2].min |n|
  await wakeable()
  n
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 1),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_max_suspends_for_async_key_function() {
    match run_wakeable_script(
        "
[3, 1, 2].max |n|
  await wakeable()
  n
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 3),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_min_max_suspends_for_async_key_function() {
    match run_wakeable_script(
        "
[3, 1, 2].min_max |n|
  await wakeable()
  n
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 1);
            assert_number(&result[1], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_each_adaptor_callback() {
    match run_wakeable_script(
        "
x = [1, 2].each |n|
  await wakeable()
  n * 10
x.to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 10);
            assert_number(&result[1], 20);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_each_adaptor_native_vm_callback() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.prelude().insert(
        "f",
        wakeable_native_vm_function(42.into(), is_ready.clone(), waker.clone()),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
[1].each(f).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number(&result[0], 42);
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn to_list_suspends_through_keep_adaptor_callback() {
    match run_wakeable_script(
        "
x = [1, 2, 3].keep |n|
  await wakeable()
  n > 1
x.to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 2);
            assert_number(&result[1], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_keep_adaptor_native_vm_callback() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.prelude().insert(
        "f",
        wakeable_native_vm_function(true.into(), is_ready.clone(), waker.clone()),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
[42].keep(f).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number(&result[0], 42);
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn to_list_suspends_through_take_while_adaptor_callback() {
    match run_wakeable_script(
        "
x = [1, 2, 3].take |n|
  await wakeable()
  n < 3
x.to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 1);
            assert_number(&result[1], 2);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_take_while_adaptor_native_vm_callback() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.prelude().insert(
        "f",
        wakeable_native_vm_function(true.into(), is_ready.clone(), waker.clone()),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
[42].take(f).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number(&result[0], 42);
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn to_list_suspends_through_intersperse_with_adaptor_callback() {
    match run_wakeable_script(
        "
x = [1, 2].intersperse ||
  await wakeable()
  0
x.to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 0);
            assert_number(&result[2], 2);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn to_list_suspends_through_intersperse_with_adaptor_native_vm_callback() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.prelude().insert(
        "f",
        wakeable_native_vm_function(0.into(), is_ready.clone(), waker.clone()),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
[1, 2].intersperse(f).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 0);
            assert_number(&result[2], 2);
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn iterator_generate_suspends_for_async_callback() {
    match run_wakeable_script(
        "
f = ||
  await wakeable()
iterator.generate(f, 2).to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 42);
            assert_number(&result[1], 42);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn iterator_generate_suspends_for_native_vm_callback() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.prelude().insert(
        "f",
        wakeable_native_vm_function(42.into(), is_ready.clone(), waker.clone()),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
iterator.generate(f, 1).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 1);
            assert_number(&result[0], 42);
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn string_split_suspends_for_async_predicate() {
    match run_wakeable_script(
        "
x = '1-2_3'.split |c|
  await wakeable()
  '-_'.contains c
x.to_list()
",
    ) {
        KValue::List(result) => {
            let result = result.data();
            assert_eq!(result.len(), 3);
            assert!(matches!(&result[0], KValue::Str(s) if s.as_str() == "1"));
            assert!(matches!(&result[1], KValue::Str(s) if s.as_str() == "2"));
            assert!(matches!(&result[2], KValue::Str(s) if s.as_str() == "3"));
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn string_split_suspends_for_native_vm_predicate() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();

    vm.prelude().insert(
        "is_dash",
        KNativeVmFunction::new({
            let is_ready = is_ready.clone();
            let waker = waker.clone();
            move |ctx| match ctx.args() {
                [KValue::Str(s)] => {
                    let result = KValue::Bool(s.as_str() == "-");
                    let is_ready = is_ready.clone();
                    let waker = waker.clone();
                    ctx.run_with_vm(move |_| async move {
                        Wakeable { is_ready, waker }.await?;
                        Ok(result)
                    })
                }
                unexpected => {
                    unexpected_args::<KValue>("|String|", unexpected).map(FunctionOutput::Ready)
                }
            }
        }),
    );

    let mut task = vm
        .run_as_task(compile_script(
            "
'1-2'.split(is_dash).to_list()
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));
    assert!(!task.is_woken());

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    assert!(task.is_woken());

    match task.poll().unwrap() {
        KTaskPoll::Ready(KValue::List(result)) => {
            let result = result.data();
            assert_eq!(result.len(), 2);
            assert!(matches!(&result[0], KValue::Str(s) if s.as_str() == "1"));
            assert!(matches!(&result[1], KValue::Str(s) if s.as_str() == "2"));
        }
        _ => panic!("unexpected task output"),
    }
}

#[test]
fn string_from_bytes_suspends_for_async_iterable() {
    match run_wakeable_script(
        "
gen = ||
  yield 97
  yield (await wakeable()) + 56
  yield 99
string.from_bytes gen()
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "abc"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_extend_suspends_for_async_iterable() {
    match run_wakeable_script(
        "
gen = ||
  yield await wakeable()
  yield 99
x = [1]
x.extend gen()
x.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 42);
            assert_number(&result[2], 99);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_contains_suspends_for_async_equality() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @==: |other|
    await wakeable()
    self.x == other.x

[foo(1), foo(2)].contains foo(2)
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_resize_with_suspends_for_async_callback() {
    match run_wakeable_script(
        "
x = [1]
x.resize_with 3, ||
  await wakeable()
  42
x.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 42);
            assert_number(&result[2], 42);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_retain_suspends_for_async_predicate() {
    match run_wakeable_script(
        "
x = [1, 2, 3]
x.retain |n|
  await wakeable()
  n > 1
x.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 2);
            assert_number(&result[1], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_retain_suspends_for_async_equality() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @==: |other|
    await wakeable()
    self.x == other.x

x = [foo(1), foo(2), foo(1)]
x.retain foo(1)
result = []
for item in x
  result.push item.x
result.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 1);
            assert_number(&result[1], 1);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_transform_suspends_for_async_callback() {
    match run_wakeable_script(
        "
x = [1, 2]
x.transform |n|
  await wakeable()
  n * 10
x.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 10);
            assert_number(&result[1], 20);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_sort_suspends_for_async_key_function() {
    match run_wakeable_script(
        "
x = [3, 1, 2]
x.sort |n|
  await wakeable()
  n
x.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 2);
            assert_number(&result[2], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn list_sort_suspends_for_async_comparison() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @<: |other|
    await wakeable()
    self.x < other.x
  @>: |other|
    await wakeable()
    self.x > other.x

x = [foo(2), foo(1)]
x.sort()
result = []
for item in x
  result.push item.x
result.to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_number(&result[0], 1);
            assert_number(&result[1], 2);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn tuple_contains_suspends_for_async_equality() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @==: |other|
    await wakeable()
    self.x == other.x

(foo(1), foo(2)).contains foo(2)
",
    ) {
        KValue::Bool(result) => assert!(result),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn tuple_sort_copy_suspends_for_async_key_function() {
    match run_wakeable_script(
        "
x = (3, 1, 2).sort_copy |n|
  await wakeable()
  n
x
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 3);
            assert_number(&result[0], 1);
            assert_number(&result[1], 2);
            assert_number(&result[2], 3);
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn map_extend_suspends_for_async_iterable() {
    match run_wakeable_script(
        "
gen = ||
  yield ('a', await wakeable())
  yield ('b', 99)
x = {}
x.extend gen()
x
",
    ) {
        KValue::Map(result) => {
            assert!(
                matches!(result.get("a"), Some(value) if matches!(value, KValue::Number(n) if i64::from(n) == 42))
            );
            assert!(
                matches!(result.get("b"), Some(value) if matches!(value, KValue::Number(n) if i64::from(n) == 99))
            );
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn map_update_suspends_for_async_callback() {
    match run_wakeable_script(
        "
x = {a: 1}
x.update 'a', |n|
  await wakeable()
  n + 41
x.a
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn map_sort_suspends_for_async_key_function() {
    match run_wakeable_script(
        "
x = {a: 2, b: 1}
x.sort |key, value|
  await wakeable()
  value
x.keys().to_tuple()
",
    ) {
        KValue::Tuple(result) => {
            assert_eq!(result.len(), 2);
            assert_string(&result[0], "b");
            assert_string(&result[1], "a");
        }
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn test_assertions_suspend_for_async_comparison() {
    match run_wakeable_script(
        "
foo = |x|
  x: x
  @==: |other|
    await wakeable()
    self.x == other.x
  @!=: |other|
    await wakeable()
    self.x != other.x

assert_eq foo(42), foo(42)
assert_ne foo(1), foo(2)
42
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn test_assertion_messages_suspend_for_async_display() {
    let (mut vm, is_ready, waker) = make_wakeable_vm();
    let mut task = vm
        .run_as_task(compile_script(
            "
a =
  x: 1
  @display: ||
    await wakeable()
    'a'
b =
  x: 2
  @display: ||
    await wakeable()
    'b'

assert_eq a, b
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    let error = task.block_on(&vm).unwrap_err();
    assert!(error.to_string().contains("'a' is not equal to 'b'"));
}

#[test]
fn iterator_to_string_suspends_for_async_display() {
    match run_wakeable_script(
        "
x =
  @display: ||
    await wakeable()
    'x'

[x].to_string()
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "x"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn string_interpolation_suspends_for_async_display() {
    match run_wakeable_script(
        "
x =
  @display: ||
    await wakeable()
    'x'

'value: {x}'
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "value: x"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn string_interpolation_suspends_for_nested_async_display() {
    match run_wakeable_script(
        "
x =
  @display: ||
    await wakeable()
    'x'

'value: {[x]}'
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "value: [x]"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn string_interpolation_suspends_for_async_debug() {
    match run_wakeable_script(
        "
x =
  @debug: ||
    await wakeable()
    'debug x'

'value: {x:?}'
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "value: debug x"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn string_interpolation_suspends_for_nested_async_debug() {
    match run_wakeable_script(
        "
x =
  @debug: ||
    await wakeable()
    'debug x'

'value: {[x]:?}'
",
    ) {
        KValue::Str(result) => assert_eq!(result.as_str(), "value: [debug x]"),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}

#[test]
fn debug_instruction_suspends_for_async_debug() {
    let (mut vm, output) = OutputCapture::make_vm_with_output_capture();
    let is_ready: PtrMut<bool> = make_ptr_mut!(false);
    let waker: PtrMut<Option<Waker>> = make_ptr_mut!(None);
    insert_wakeable(&vm, is_ready.clone(), waker.clone());

    let mut task = vm
        .run_as_task(compile_script(
            "
x =
  @debug: ||
    await wakeable()
    'debug x'

debug x
42
",
        ))
        .unwrap();

    assert!(matches!(task.poll().unwrap(), KTaskPoll::Pending));

    *is_ready.borrow_mut() = true;
    waker.borrow_mut().take().unwrap().wake();

    match task.block_on(&vm).unwrap() {
        KValue::Number(n) => assert_eq!(i64::from(n), 42),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }

    assert_eq!(output.captured_output().as_str(), "[7] x: debug x\n");
}

#[test]
fn test_run_tests_suspends_for_async_tests() {
    match run_wakeable_script(
        "
tests =
  result: 0
  @pre_test: ||
    await wakeable()
    self.result += 1
  @post_test: ||
    await wakeable()
    self.result += 10
  @test async_test: ||
    await wakeable()
    self.result += 100

test.run_tests tests
tests.result
",
    ) {
        KValue::Number(n) => assert_eq!(i64::from(n), 111),
        unexpected => panic!("unexpected task output: {unexpected:?}"),
    }
}
