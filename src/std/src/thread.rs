use crate::{get_external_instance, single_arg_fn, type_as_string, ExternalValue};
use koto_runtime::{external_error, value, Error, Value, ValueMap};
use std::{fmt, thread, thread::JoinHandle, time::Duration};

pub fn register(global: &mut ValueMap) {
    use Value::{Empty, Function, Number};

    let mut thread = ValueMap::new();

    single_arg_fn!(thread, "sleep", Number, seconds, {
        if *seconds < 0.0 {
            return external_error!("thread.sleep: negative durations aren't supported");
        }

        thread::sleep(Duration::from_secs(*seconds as u64));

        Ok(Empty)
    });

    thread.add_fn("create", |runtime, args| match &args {
        [Function(f)] => {
            let join_handle = thread::spawn({
                let mut thread_vm = runtime.spawn_shared_vm();
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
    });

    global.add_map("thread", thread);
}

#[derive(Debug)]
struct Thread {
    join_handle: Option<JoinHandle<Result<(), Error>>>,
}

impl Thread {
    fn make_thread_map(join_handle: JoinHandle<Result<(), Error>>) -> Value {
        let mut result = ValueMap::new();

        result.add_instance_fn("join", |_, args| {
            get_external_instance!(args, "Thread", "join", Thread, thread, {
                let result = thread.join_handle.take().unwrap().join();
                match result {
                    Ok(Ok(_)) => Ok(Value::Empty),
                    Ok(Err(koto_error)) => Err(koto_error),
                    Err(_) => external_error!("thread.join: thread panicked"),
                }
            })
        });

        result.set_external_value(Self {
            join_handle: Some(join_handle),
        });

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
