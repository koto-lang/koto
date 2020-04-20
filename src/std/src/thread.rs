use crate::{builtin_error, single_arg_fn};
use koto_runtime::{value, Error, Value, ValueMap};
use std::{thread, time::Duration};

pub fn register(global: &mut ValueMap) {
    use Value::{Empty, Number};

    let mut thread = ValueMap::new();

    single_arg_fn!(thread, "sleep", Number, seconds, {
        if *seconds < 0.0 {
            return builtin_error!("thread.sleep: negative durations aren't supported");
        }

        thread::sleep(Duration::from_secs(*seconds as u64));

        Ok(Empty)
    });

    global.add_map("thread", thread);
}
