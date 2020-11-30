use std::{
    env,
    io::Write,
    process::{Command, Stdio},
    str,
};

fn run_koto_repl_test(inputs_and_expected_outputs: &[(&str, Option<&str>)]) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_koto"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute child");

    let stdin = process.stdin.as_mut().expect("failed to get stdin");

    for (input, _) in inputs_and_expected_outputs.iter() {
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write to stdin");
        stdin.write_all(b"\n").expect("Failed to write to stdin");
    }

    let output = process.wait_with_output().expect("Failed to get output");
    let stdout = String::from_utf8(output.stdout).expect("Failed to get output");
    let mut output_lines = stdout.lines().skip_while(|line| line != &"» ");

    for (_, expected) in inputs_and_expected_outputs.iter() {
        output_lines.next(); // prompt (empty line in test)
        if let Some(expected) = expected {
            assert_eq!(output_lines.next().expect("Missing output"), *expected);
        }
    }
}

mod repl_tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        run_koto_repl_test(&[("a = 2", Some("2")), ("a + a", Some("4"))]);
    }

    #[test]
    fn for_loop() {
        run_koto_repl_test(&[
            ("for x in 1..=5", None),
            ("  x", None),
            ("", Some("5")),
            ("x * x", Some("25")),
        ]);
    }

    #[test]
    fn tuple_assignment() {
        run_koto_repl_test(&[("x = 1, 2, 3", Some("(1, 2, 3)")), ("x", Some("(1, 2, 3)"))]);
    }

    #[test]
    fn import_assert() {
        run_koto_repl_test(&[
            ("import test.assert", Some("||")),
            ("assert true", Some("()")),
        ]);
    }
}
