use {
    crate::{
        runtime_error, CallArgs, ExternalData, MetaMap, RuntimeError, RwLock, Value, ValueMap,
    },
    lazy_static::lazy_static,
    std::{fmt, sync::Arc, thread, thread::JoinHandle, time::Duration},
};

pub fn make_module() -> ValueMap {
    use Value::{Empty, Number};

    let mut result = ValueMap::new();

    #[cfg(not(target_arch = "wasm32"))]
    result.add_fn("create", |vm, args| match vm.get_args(args) {
        [f] if f.is_callable() => {
            let f = f.clone();
            let join_handle = thread::spawn({
                let mut thread_vm = vm.spawn_shared_concurrent_vm();
                move || match thread_vm.run_function(f, CallArgs::None) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(e.with_prefix("thread.create")),
                }
            });

            Ok(Thread::make_external_value(join_handle))
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
            let seconds: f64 = seconds.into();

            if seconds < 0.0 {
                return runtime_error!("thread.sleep: the duration must be positive");
            } else if !seconds.is_finite() {
                return runtime_error!("thread.sleep: the duration must be finite");
            }

            thread::sleep(Duration::from_secs_f64(seconds));

            Ok(Empty)
        }
        _ => runtime_error!("thread.sleep: Expected a Number as argument"),
    });

    result
}

lazy_static! {
    static ref THREAD_META: Arc<RwLock<MetaMap>> = {
        let mut meta = MetaMap::with_type_name("Thread");

        meta.add_named_instance_fn_mut("join", |thread: &mut Thread, _, _| {
            let result = thread.join_handle.take().unwrap().join();
            match result {
                Ok(Ok(result)) => Ok(result),
                Ok(Err(koto_error)) => Err(koto_error),
                Err(_) => runtime_error!("thread.join: thread panicked"),
            }
        });

        Arc::new(RwLock::new(meta))
    };
}

#[derive(Debug)]
struct Thread {
    join_handle: Option<JoinHandle<Result<Value, RuntimeError>>>,
}

impl Thread {
    #[cfg(not(target_arch = "wasm32"))]
    fn make_external_value(join_handle: JoinHandle<Result<Value, RuntimeError>>) -> Value {
        let result = crate::ExternalValue::with_shared_meta_map(
            Thread {
                join_handle: Some(join_handle),
            },
            THREAD_META.clone(),
        );

        Value::ExternalValue(result)
    }
}

impl ExternalData for Thread {
    fn value_type(&self) -> String {
        "Thread".to_string()
    }
}

impl fmt::Display for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Thread")
    }
}
