use std::{
    io::Write,
    iter,
    path::PathBuf,
    process::{Output, Stdio},
    str,
};

fn check_output(output: Output, expected_stdout: &str, expected_stderr: &str) {
    let stdout = str::from_utf8(&output.stdout).expect("Failed to read stdout");
    let stderr = str::from_utf8(&output.stderr).expect("Failed to read stderr");

    if !output.status.success() {
        panic!("CLI test failed:\n\n>> stdout:\n{stdout}\n\n>> stderr:\n{stderr}",)
    }

    assert_eq!(stdout, expected_stdout);
    assert_eq!(stderr, expected_stderr);
}

fn check_cli_run_file(
    script_path: &[&str],
    args: &[&str],
    expected_stdout: &str,
    expected_stderr: &str,
) {
    let script_path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."]
        .iter()
        .chain(script_path.iter())
        .collect();

    check_output(
        test_bin::get_test_bin("koto")
            .args(iter::once(&script_path.to_string_lossy().as_ref()).chain(args))
            .output()
            .expect("Failed to run CLI"),
        expected_stdout,
        expected_stderr,
    )
}

fn check_cli_piped_input(input: &'static str, expected_stdout: &str, expected_stderr: &str) {
    let mut cli = test_bin::get_test_bin("koto")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to run CLI");

    let mut stdin = cli.stdin.take().expect("Failed to open stdin");

    std::thread::spawn(move || {
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write to stdin");
    });

    check_output(
        cli.wait_with_output().expect("Failed to run CLI"),
        expected_stdout,
        expected_stderr,
    )
}

mod cli {
    use super::*;

    mod run_file {
        use super::*;

        #[test]
        fn spectral_norm() {
            check_cli_run_file(
                &["koto", "benches", "spectral_norm.koto"],
                &["2"],
                "1.1833501765516568\n",
                "",
            );
        }

        #[test]
        fn string_formatting() {
            check_cli_run_file(
                &["koto", "benches", "string_formatting.koto"],
                &["1"],
                "('minus one', 'zero', 'one')\n",
                "",
            );
        }
    }

    mod piped_input {
        use super::*;

        #[test]
        fn square() {
            let input = "
square = |x| x * x
print square 9
";
            check_cli_piped_input(input, "81\n", "");
        }
    }
}
