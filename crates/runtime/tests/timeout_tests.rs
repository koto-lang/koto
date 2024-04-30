mod timeout {
    use koto_bytecode::{CompilerSettings, Loader};
    use koto_runtime::{prelude::*, Error, ErrorKind};
    use std::time::Duration;

    fn test_script_with_timeout(script: &str, should_timeout: bool) {
        let mut vm = KotoVm::with_settings(KotoVmSettings {
            execution_limit: Some(Duration::from_millis(1)),
            ..Default::default()
        });

        let mut loader = Loader::default();
        let chunk = match loader.compile_script(script, None, CompilerSettings::default()) {
            Ok(chunk) => chunk,
            Err(error) => {
                panic!("Error while compiling script: {error}");
            }
        };

        let result = vm.run(chunk);

        if should_timeout {
            match result {
                Err(Error {
                    error: ErrorKind::Timeout(_),
                    ..
                }) => {}
                _ => {
                    panic!("Script didn't time out as expected");
                }
            }
        } else {
            match result {
                Ok(_) => {}
                Err(e) => {
                    panic!("Unexpected error: {e}");
                }
            }
        }
    }

    #[test]
    fn no_timeout() {
        let script = "
n = 0
while n < 100
  n += 1
";

        test_script_with_timeout(script, false);
    }

    #[test]
    fn infinite_loop() {
        let script = "
while true
  ()
";

        test_script_with_timeout(script, true);
    }
}
