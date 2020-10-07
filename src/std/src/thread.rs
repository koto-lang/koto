use {
    crate::{get_external_instance, single_arg_fn, type_as_string, ExternalValue},
    koto_runtime::{external_error, make_external_value, value, Error, Value, ValueMap},
    std::{fmt, thread, thread::JoinHandle, time::Duration},
};

pub fn register(prelude: &mut ValueMap) {
    use Value::{Empty, Function, Number};

    let mut thread = ValueMap::new();

    single_arg_fn!(thread, "sleep", Number, seconds, {
        if *seconds < 0.0 {
            return external_error!("thread.sleep: negative durations aren't supported");
        }

        thread::sleep(Duration::from_secs(*seconds as u64));

        Ok(Empty)
    });

    thread.add_fn("create", |vm, args| {
        let args = vm.get_args_as_vec(args);
        match args.as_slice() {
            [Function(f)] => {
                let join_handle = thread::spawn({
                    let mut thread_vm = vm.spawn_shared_vm();
                    let f = f.clone();
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
        }
    });

    prelude.add_map("thread", thread);
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
