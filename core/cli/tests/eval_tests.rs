use std::{
    env,
    io::Write,
    process::{Command, Stdio},
};

fn run_koto_eval_test(script: &str, piped_input: &str, expected_output: &str) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_koto"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("--eval")
        .arg(script)
        .spawn()
        .expect("failed to execute child");

    let stdin = process.stdin.as_mut().expect("failed to get stdin");
    stdin
        .write_all(piped_input.as_bytes())
        .expect("Failed to write to stdin");

    let output = process.wait_with_output().expect("Failed to get output");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("Failed to get output");

    assert_eq!(stdout, expected_output);
}

mod eval_tests {
    use super::*;

    #[test]
    fn empty_output() {
        run_koto_eval_test("1 + 1", "", "");
    }

    #[test]
    fn printed_result() {
        run_koto_eval_test("print 1 + 1", "", "2\n");
    }

    #[test]
    fn stdin_read_line() {
        let script = "
stdin = io.stdin()
print stdin.read_line()
print 'xyz'
print stdin.read_line()
";
        let stdin = "\
123
456
789
";
        let expected_output = "\
123
xyz
456
";

        run_koto_eval_test(script, stdin, expected_output);
    }
}
