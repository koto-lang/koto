use {
    crate::{runtime_error, ExternalValue, RuntimeError, Value, ValueMap},
    std::{fmt, thread, thread::JoinHandle, time::Duration},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::get_external_instance;

pub fn make_module() -> ValueMap {
    use Value::{Empty, Number};

    let mut result = ValueMap::new();

    #[cfg(not(target_arch = "wasm32"))]
    result.add_fn("create", |vm, args| match vm.get_args(args) {
        [f] if f.is_callable() => {
            let f = f.clone();
            let join_handle = thread::spawn({
                let mut thread_vm = vm.spawn_shared_concurrent_vm();
                move || match thread_vm.run_function(f, &[]) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(e.with_prefix("thread.create")),
                }
            });

            Ok(Thread::make_thread_map(join_handle))
        }
        [unexpected] => runtime_error!(
            "thread.create: Expected callable value as argument, found '{}'",
            unexpected.type_as_string(),
        ),
        _ => runtime_error!("thread.create: Expected callable value as argument"),
    });

    #[cfg(target_arch = "wasm32")]
    result.add_fn("create", |_, _| {
        runtime_error!("thread.create: Not supported on this platform")
    });

    result.add_fn("sleep", |vm, args| match vm.get_args(args) {
        [Number(seconds)] => {
            if *seconds < 0.0 {
                return runtime_error!("thread.sleep: negative durations aren't supported");
            }

            thread::sleep(Duration::from_millis((f64::from(seconds) * 1000.0) as u64));

            Ok(Empty)
        }
        _ => runtime_error!("thread.sleep: Expected number as argument"),
    });

    result
}

#[derive(Debug)]
struct Thread {
    join_handle: Option<JoinHandle<Result<Value, RuntimeError>>>,
}

impl Thread {
    #[cfg(not(target_arch = "wasm32"))]
    fn make_thread_map(join_handle: JoinHandle<Result<Value, RuntimeError>>) -> Value {
        //TODO: use once_cell::sync::Lazy to make this a one-time cost
        let mut vtable = ValueMap::new();

        vtable.add_instance_fn("join", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "Thread", "join", Thread, thread, {
                let result = thread.join_handle.take().unwrap().join();
                match result {
                    Ok(Ok(result)) => Ok(result),
                    Ok(Err(koto_error)) => Err(koto_error),
                    Err(_) => runtime_error!("thread.join: thread panicked"),
                }
            })
        });

        Value::make_external_value(Self { join_handle: Some(join_handle) }, vtable)
    }
}

impl ExternalValue for Thread {
    fn value_type(&self) -> String {
        "Thread".to_string()
    }
}

impl fmt::Display for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Thread")
    }
}
