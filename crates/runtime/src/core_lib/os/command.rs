//!

use crate::{derive::*, prelude::*, Result};
use koto_memory::PtrMut;
use std::process;

/// A wrapper for [std::process::Command], used by `os.command`
#[derive(Clone, Debug, KotoCopy, KotoType)]
pub struct Command(PtrMut<process::Command>);

#[koto_impl(runtime = crate)]
impl Command {
    pub fn new(command: &str) -> KValue {
        let command = make_ptr_mut!(process::Command::new(command));
        KObject::from(Self(command)).into()
    }

    #[koto_method]
    fn args(ctx: MethodContext<Self>) -> Result<KValue> {
        let this = ctx.instance_mut()?;
        let mut command = this.0.borrow_mut();
        for arg in ctx.args {
            match arg {
                KValue::Str(arg) => command.arg(arg.as_str()),
                unexpected => return unexpected_type("String as arg", unexpected),
            };
        }

        ctx.instance_result()
    }

    #[koto_method]
    fn current_dir(ctx: MethodContext<Self>) -> Result<KValue> {
        let this = ctx.instance_mut()?;
        let mut command = this.0.borrow_mut();

        match ctx.args {
            [KValue::Str(path)] => {
                command.current_dir(path.as_str());
            }
            unexpected => return unexpected_args("|String|", unexpected),
        }

        ctx.instance_result()
    }

    #[koto_method]
    fn env(ctx: MethodContext<Self>) -> Result<KValue> {
        let this = ctx.instance_mut()?;
        let mut command = this.0.borrow_mut();

        match ctx.args {
            [KValue::Str(key), KValue::Str(value)] => {
                command.env(key.as_str(), value.as_str());
            }
            unexpected => return unexpected_args("|String, String|", unexpected),
        }

        ctx.instance_result()
    }

    #[koto_method]
    fn env_clear(ctx: MethodContext<Self>) -> Result<KValue> {
        ctx.instance_mut()?.0.borrow_mut().env_clear();
        ctx.instance_result()
    }

    #[koto_method]
    fn env_remove(ctx: MethodContext<Self>) -> Result<KValue> {
        let this = ctx.instance_mut()?;
        let mut command = this.0.borrow_mut();

        match ctx.args {
            [KValue::Str(key)] => {
                command.env_remove(key.as_str());
            }
            unexpected => return unexpected_args("|String|", unexpected),
        }

        ctx.instance_result()
    }

    #[koto_method]
    fn wait_for_output(&mut self) -> Result<KValue> {
        match self.0.borrow_mut().output() {
            Ok(output) => Ok(CommandOutput::new(output)),
            Err(error) => runtime_error!("{error}"),
        }
    }

    #[koto_method]
    fn wait_for_status(&mut self) -> Result<KValue> {
        match self.0.borrow_mut().status() {
            Ok(status) => match status.code() {
                Some(code) => Ok(code.into()),
                None => Ok(KValue::Null),
            },
            Err(error) => runtime_error!("{error}"),
        }
    }
}

impl KotoObject for Command {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!(
            "Command('{}')",
            self.0.borrow_mut().get_program().to_string_lossy()
        ));
        Ok(())
    }
}

/// A wrapper for [std::process::Output], used by `os.command`
#[derive(Clone, Debug, KotoCopy, KotoType)]
struct CommandOutput(process::Output);

#[koto_impl(runtime = crate)]
impl CommandOutput {
    fn new(output: process::Output) -> KValue {
        KObject::from(Self(output)).into()
    }

    #[koto_method]
    fn success(&self) -> KValue {
        self.0.status.success().into()
    }

    #[koto_method]
    fn stdout(&self) -> KValue {
        let bytes = self.0.stdout.clone();
        if cfg!(windows) {
            bytes_to_kvalue_utf16(bytes)
        } else {
            String::from_utf8(bytes).map_or(KValue::Null, KValue::from)
        }
    }

    #[koto_method]
    fn stderr(&self) -> KValue {
        let bytes = self.0.stderr.clone();
        if cfg!(windows) {
            bytes_to_kvalue_utf16(bytes)
        } else {
            String::from_utf8(bytes).map_or(KValue::Null, KValue::from)
        }
    }

    #[koto_method]
    fn stdout_bytes(&self) -> Result<KValue> {
        let bytes = self.0.stdout.clone().into();
        let iterator = KIterator::with_bytes(bytes)?;
        Ok(iterator.into())
    }

    #[koto_method]
    fn stderr_bytes(&self) -> Result<KValue> {
        let bytes = self.0.stderr.clone().into();
        let iterator = KIterator::with_bytes(bytes)?;
        Ok(iterator.into())
    }
}

impl KotoObject for CommandOutput {}

fn bytes_to_kvalue_utf16(bytes: Vec<u8>) -> KValue {
    use KValue::Null;

    if bytes.len() % 2 != 0 {
        return Null;
    }

    // SAFETY: We checked that the length of `bytes` is a multiple of 2
    let bytes_u16: &[u16] =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u16, bytes.len() / 2) };

    String::from_utf16(bytes_u16)
        .ok()
        .map_or(Null, KValue::from)
}
