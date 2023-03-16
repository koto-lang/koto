use std::{
    env,
    io::Write,
    process::{Command, Stdio},
};

fn run_koto_stdin_test(input: &str, expected_output: &str) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_koto"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute child");

    let stdin = process.stdin.as_mut().expect("failed to get stdin");
    stdin
        .write_all(input.as_bytes())
        .expect("Failed to write to stdin");

    let output = process.wait_with_output().expect("Failed to get output");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("Failed to get output");

    assert_eq!(stdout, expected_output);
}

mod stdin_tests {
    use super::*;

    #[test]
    fn empty_output() {
        run_koto_stdin_test("1 + 1", "");
    }

    #[test]
    fn printed_result() {
        run_koto_stdin_test("print 1 + 1", "2\n");
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
        run_koto_stdin_test(script, expected_output);
    }

    #[test]
    fn writing_directly_to_stdout() {
        let script = "
stdout = io.stdout()
stdout.write 'Hello'
stdout.write ', World!'
";
        let expected_output = "Hello, World!";
        run_koto_stdin_test(script, expected_output);
    }
}
