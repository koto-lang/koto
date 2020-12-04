use {
    crate::{
        external_error, get_external_instance, make_external_value, type_as_string, Error,
        ExternalValue, Value, ValueMap,
    },
    std::{fmt, thread, thread::JoinHandle, time::Duration},
};

pub fn make_module() -> ValueMap {
    use Value::{Empty, Function, Number};

    let mut result = ValueMap::new();

    result.add_fn("sleep", |vm, args| match vm.get_args(args) {
        [Number(seconds)] => {
            if *seconds < 0.0 {
                return external_error!("thread.sleep: negative durations aren't supported");
            }

            thread::sleep(Duration::from_millis((*seconds * 1000.0) as u64));

            Ok(Empty)
        }
        _ => external_error!("thread.sleep: Expected number as argument"),
    });

    result.add_fn("create", |vm, args| match vm.get_args(args) {
        [Function(f)] => {
            let f = f.clone();
            let join_handle = thread::spawn({
                let mut thread_vm = vm.spawn_shared_concurrent_vm();
                move || match thread_vm.run_function(&f, &[]) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            });

            Ok(Thread::make_thread_map(join_handle))
        }
        [unexpected] => external_error!(
            "thread.create: Expected function as argument, found '{}'",
            type_as_string(unexpected),
        ),
        _ => external_error!("thread.create: Expected function as argument"),
    });

    result
}

#[derive(Debug)]
struct Thread {
    join_handle: Option<JoinHandle<Result<(), Error>>>,
}

impl Thread {
    fn make_thread_map(join_handle: JoinHandle<Result<(), Error>>) -> Value {
        let mut result = ValueMap::new();

        result.add_instance_fn("join", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "Thread", "join", Thread, thread, {
                let result = thread.join_handle.take().unwrap().join();
                match result {
                    Ok(Ok(_)) => Ok(Value::Empty),
                    Ok(Err(koto_error)) => Err(koto_error),
                    Err(_) => external_error!("thread.join: thread panicked"),
                }
            })
        });

        result.insert(
            Value::ExternalDataId,
            make_external_value(Self {
                join_handle: Some(join_handle),
            }),
        );

        Value::Map(result)
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
