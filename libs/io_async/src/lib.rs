//! A Koto language module for async IO operations

use koto_runtime::prelude::*;

pub fn make_module() -> KMap {
    let result = KMap::with_type("io_async");

    result.add_fn("read_to_string", |ctx| match ctx.args() {
        [KValue::Str(path)] => {
            let path = path.to_string();
            Ok(ctx
                .spawn_future(async move {
                    match tokio::fs::read_to_string(&path).await {
                        Ok(contents) => Ok(contents.into()),
                        Err(error) => runtime_error!(
                            "io_async.read_to_string: Unable to read file '{path}': {error}"
                        ),
                    }
                })?
                .into())
        }
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("write", |ctx| match ctx.args() {
        [KValue::Str(path), KValue::Str(contents)] => {
            let path = path.to_string();
            let contents = contents.to_string();
            Ok(ctx
                .spawn_future(async move {
                    match tokio::fs::write(&path, contents).await {
                        Ok(()) => Ok(KValue::Null),
                        Err(error) => {
                            runtime_error!("io_async.write: Unable to write file '{path}': {error}")
                        }
                    }
                })?
                .into())
        }
        unexpected => unexpected_args("|String, String|", unexpected),
    });

    result
}
