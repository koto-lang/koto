use std::{
    env,
    io::Write,
    process::{Command, Stdio},
};

fn run_piped_stdio_test(input: &str, expected_output: &str) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_koto"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute child");

    let stdin = process.stdin.as_mut().expect("failed to get stdin");
    stdin
        .write_all(input.as_bytes())
        .expect("Failed to write to stdin");

    let output = process
        .wait_with_output()
        .expect("Failed to wait for output");
    let stdout = String::from_utf8(output.stdout).expect("Invalid output in stdout");
    let stderr = String::from_utf8(output.stderr).expect("Invalid output in stderr");

    assert!(
        output.status.success(),
        "Process exited with error code {:?}
stdout:
{stdout}

stderr:
{stderr}",
        output.status.code().unwrap(),
    );
    assert_eq!(stdout, expected_output);
}

mod stdin_tests {
    use super::*;

    #[test]
    fn empty_output() {
        run_piped_stdio_test("1 + 1", "");
    }

    #[test]
    fn printed_result() {
        run_piped_stdio_test("print 1 + 1", "2\n");
    }

    #[test]
    fn multiline_output() {
        let script = "
print 'Hello'
print 'World!'
";
        let expected_output = "\
Hello
World!
";
        run_piped_stdio_test(script, expected_output);
    }

    #[test]
    fn writing_directly_to_stdout() {
        let script = "
stdout = io.stdout()
stdout.write 'Hello'
stdout.write ', World!'
";
        let expected_output = "Hello, World!";
        run_piped_stdio_test(script, expected_output);
    }

    #[test]
    fn is_terminal_stdin() {
        let script = "
assert not io.stdin().is_terminal()
1 + 1
";
        run_piped_stdio_test(script, "");
    }

    #[test]
    fn is_terminal_stdout() {
        let script = "
assert not io.stdout().is_terminal()
";
        run_piped_stdio_test(script, "");
    }

    #[test]
    fn is_terminal_stderr() {
        let script = "
assert not io.stderr().is_terminal()
";
        run_piped_stdio_test(script, "");
    }
}
