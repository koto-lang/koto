//! Support for `os.command`

use crate::{
    core_lib::io::{map_io_err, File},
    derive::*,
    prelude::*,
    Result,
};
use koto_memory::{Ptr, PtrMut};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process;

macro_rules! stdio_setter {
    ($stream:ident, $ctx:expr) => {{
        let this = $ctx.instance_mut()?;
        let mut command = this.0.borrow_mut();

        match $ctx.args {
            [KValue::Str(s)] => match s.as_str() {
                "inherit" => {
                    command.$stream(process::Stdio::inherit());
                }
                "null" => {
                    command.$stream(process::Stdio::null());
                }
                "piped" => {
                    command.$stream(process::Stdio::piped());
                }
                unexpected => {
                    return runtime_error!(
                        "Expected 'inherit', 'null', or 'piped', found '{unexpected}'"
                    )
                }
            },
            unexpected => return unexpected_args("|String|", unexpected),
        }

        $ctx.instance_result()
    }};
}

/// A wrapper for [std::process::Command], used by `os.command`
#[derive(Clone, Debug, KotoCopy, KotoType)]
pub struct Command(PtrMut<process::Command>);

#[koto_impl(runtime = crate)]
impl Command {
    pub fn make_value(command: &str) -> KValue {
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
    fn stdin(ctx: MethodContext<Self>) -> Result<KValue> {
        stdio_setter!(stdin, ctx)
    }

    #[koto_method]
    fn stdout(ctx: MethodContext<Self>) -> Result<KValue> {
        stdio_setter!(stdout, ctx)
    }

    #[koto_method]
    fn stderr(ctx: MethodContext<Self>) -> Result<KValue> {
        stdio_setter!(stderr, ctx)
    }

    #[koto_method]
    fn spawn(&mut self) -> Result<KValue> {
        match self.0.borrow_mut().spawn() {
            Ok(child) => Ok(Child::make_value(child)),
            Err(error) => runtime_error!("{error}"),
        }
    }

    #[koto_method]
    fn wait_for_output(&mut self) -> Result<KValue> {
        match self.0.borrow_mut().output() {
            Ok(output) => Ok(CommandOutput::make_value(output)),
            Err(error) => runtime_error!("{error}"),
        }
    }

    #[koto_method]
    fn wait_for_exit(&mut self) -> Result<KValue> {
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
    fn make_value(output: process::Output) -> KValue {
        KObject::from(Self(output)).into()
    }

    #[koto_method]
    fn exit_code(&self) -> KValue {
        self.0.status.code().into()
    }

    #[koto_method]
    fn success(&self) -> KValue {
        self.0.status.success().into()
    }

    #[koto_method]
    fn stdout(&self) -> KValue {
        let bytes = self.0.stdout.clone();
        String::from_utf8(bytes).ok().into()
    }

    #[koto_method]
    fn stderr(&self) -> KValue {
        let bytes = self.0.stderr.clone();
        String::from_utf8(bytes).ok().into()
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

/// A wrapper for [std::process::Child], used by `os.command.spawn`
#[derive(Clone, KotoCopy, KotoType)]
struct Child {
    handle: PtrMut<Option<process::Child>>,
    // Keep track of stream handles that have been accessed by the user. This allows the streams to
    // be closed without user intervention when waiting for the command to exit. This also allows us
    // to provide a more helpful error message if the stream is reused after the command has exited.
    stdin: Option<(ChildStdin, KValue)>,
    stdout: Option<(ChildStdout, KValue)>,
    stderr: Option<(ChildStderr, KValue)>,
}

macro_rules! child_stream_fn {
    ($self:expr, $stream:ident, $buffer_wrapper:ident, $child_stream:ident) => {{
        let mut this = $self.handle.borrow_mut();
        let Some(child) = this.as_mut() else {
            return runtime_error!("the process has already finished");
        };

        match child.$stream.take() {
            Some(stream) => {
                let stream = $child_stream(make_ptr_mut!(Some($buffer_wrapper::new(stream))));
                let result = KValue::from(File::new(make_ptr!(stream.clone())));
                $self.$stream = Some((stream, result.clone()));
                Ok(result)
            }
            None => match $self.$stream.as_ref() {
                Some((_, stream_value)) => Ok(stream_value.clone()),
                None => runtime_error!("{} has already been captured", stringify!($stream)),
            },
        }
    }};
}

#[koto_impl(runtime = crate)]
impl Child {
    pub fn make_value(child: process::Child) -> KValue {
        KObject::from(Self {
            handle: make_ptr_mut!(Some(child)),
            stdin: None,
            stdout: None,
            stderr: None,
        })
        .into()
    }

    #[koto_method]
    fn id(&self) -> Result<KValue> {
        let mut this = self.handle.borrow_mut();
        let Some(child) = this.as_mut() else {
            return runtime_error!("the process has already finished");
        };
        Ok(child.id().into())
    }

    #[koto_method]
    fn stdin(&mut self) -> Result<KValue> {
        child_stream_fn!(self, stdin, BufWriter, ChildStdin)
    }

    #[koto_method]
    fn stdout(&mut self) -> Result<KValue> {
        child_stream_fn!(self, stdout, BufReader, ChildStdout)
    }

    #[koto_method]
    fn stderr(&mut self) -> Result<KValue> {
        child_stream_fn!(self, stderr, BufReader, ChildStderr)
    }

    #[koto_method]
    fn has_exited(&mut self) -> Result<KValue> {
        let mut this = self.handle.borrow_mut();
        let Some(child) = this.as_mut() else {
            return Ok(true.into());
        };

        match child.try_wait() {
            Ok(Some(_)) => Ok(true.into()),
            Ok(None) => Ok(false.into()),
            Err(error) => runtime_error!("{error}"),
        }
    }

    #[koto_method]
    fn kill(&mut self) -> Result<KValue> {
        let mut this = self.handle.borrow_mut();

        let Some(child) = this.as_mut() else {
            return runtime_error!("the process has already finished");
        };

        match child.kill() {
            Ok(_) => {
                *this = None;
                Ok(true.into())
            }
            Err(_) => Ok(false.into()),
        }
    }

    #[koto_method]
    fn wait_for_output(&mut self) -> Result<KValue> {
        self.close_streams();

        let Some(child) = self.handle.borrow_mut().take() else {
            return runtime_error!("the process has already finished");
        };

        match child.wait_with_output() {
            Ok(output) => Ok(CommandOutput::make_value(output)),
            Err(error) => runtime_error!("{error}"),
        }
    }

    #[koto_method]
    fn wait_for_exit(&mut self) -> Result<KValue> {
        self.close_streams();

        let mut this = self.handle.borrow_mut();
        let Some(child) = this.as_mut() else {
            return runtime_error!("the process has already finished");
        };

        match child.wait() {
            Ok(status) => match status.code() {
                Some(code) => Ok(code.into()),
                None => Ok(KValue::Null),
            },
            Err(error) => runtime_error!("{error}"),
        }
    }

    fn close_streams(&mut self) {
        if let Some((stream, _)) = self.stdin.take() {
            *stream.0.borrow_mut() = None;
        }
        if let Some((stream, _)) = self.stdout.take() {
            *stream.0.borrow_mut() = None;
        }
        if let Some((stream, _)) = self.stderr.take() {
            *stream.0.borrow_mut() = None;
        }
    }
}

impl KotoObject for Child {}

#[derive(Clone)]
struct ChildStdin(PtrMut<Option<BufWriter<process::ChildStdin>>>);

impl KotoFile for ChildStdin {
    fn id(&self) -> KString {
        "_child_stdin_".into()
    }
}

impl KotoRead for ChildStdin {}
impl KotoWrite for ChildStdin {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.write_all(bytes).map_err(map_io_err),
            None => runtime_error!("the stream has been closed"),
        }
    }

    fn write_line(&self, output: &str) -> Result<()> {
        self.write(output.as_bytes())?;
        match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.write_all("\n".as_bytes()).map_err(map_io_err),
            None => runtime_error!("the stream has been closed"),
        }
    }

    fn flush(&self) -> Result<()> {
        match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.flush().map_err(map_io_err),
            None => runtime_error!("the stream has been closed"),
        }
    }
}

#[derive(Clone)]
struct ChildStdout(PtrMut<Option<BufReader<process::ChildStdout>>>);

impl KotoFile for ChildStdout {
    fn id(&self) -> KString {
        "_child_stdout_".into()
    }
}

impl KotoRead for ChildStdout {
    fn read_line(&self) -> Result<Option<String>> {
        let mut result = String::new();
        let bytes_read = match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.read_line(&mut result).map_err(map_io_err)?,
            None => return runtime_error!("the stream has been closed"),
        };
        if bytes_read > 0 {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn read_to_string(&self) -> Result<String> {
        let mut result = String::new();
        match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.read_to_string(&mut result).map_err(map_io_err)?,
            None => return runtime_error!("the stream has been closed"),
        };
        Ok(result)
    }
}
impl KotoWrite for ChildStdout {}

#[derive(Clone)]
struct ChildStderr(PtrMut<Option<BufReader<process::ChildStderr>>>);

impl KotoFile for ChildStderr {
    fn id(&self) -> KString {
        "_child_stderr_".into()
    }
}

impl KotoRead for ChildStderr {
    fn read_line(&self) -> Result<Option<String>> {
        let mut result = String::new();
        let bytes_read = match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.read_line(&mut result).map_err(map_io_err)?,
            None => return runtime_error!("the stream has been closed"),
        };
        if bytes_read > 0 {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn read_to_string(&self) -> Result<String> {
        let mut result = String::new();
        match self.0.borrow_mut().as_mut() {
            Some(stream) => stream.read_to_string(&mut result).map_err(map_io_err)?,
            None => return runtime_error!("the stream has been closed"),
        };
        Ok(result)
    }
}
impl KotoWrite for ChildStderr {}
