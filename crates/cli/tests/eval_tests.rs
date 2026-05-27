use std::{
    env,
    io::{Read, Write},
    process::{Command, Stdio},
};
#[cfg(feature = "tokio")]
use std::{
    fs,
    net::TcpListener,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn run_koto_eval_test(script: &str, piped_input: &str, expected_output: &str) {
    run_koto_eval_test_with_args(script, &[], piped_input, expected_output);
}

fn run_koto_eval_test_with_args(
    script: &str,
    args: &[&str],
    piped_input: &str,
    expected_output: &str,
) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_koto"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("--eval")
        .arg(script)
        .args(args)
        .spawn()
        .expect("failed to execute child");

    let stdin = process.stdin.as_mut().expect("failed to get stdin");
    stdin
        .write_all(piped_input.as_bytes())
        .expect("Failed to write to stdin");

    let output = process.wait_with_output().expect("Failed to get output");
    let stdout = String::from_utf8(output.stdout).expect("Failed to get output");
    let stderr = String::from_utf8(output.stderr).expect("Failed to get stderr");

    assert!(
        output.status.success(),
        "Process exited with error code {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code(),
    );
    assert_eq!(stdout, expected_output);
}

#[cfg(feature = "tokio")]
fn temp_path(name: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_nanos();
    let path = env::temp_dir().join(format!("koto_{name}_{}_{}", std::process::id(), now));

    path.to_string_lossy().into()
}

#[cfg(feature = "tokio")]
fn spawn_http_server(body: &str, content_type: &str) -> (String, thread::JoinHandle<()>) {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind test http listener");
    let address = listener
        .local_addr()
        .expect("failed to get test http listener address");
    listener
        .set_nonblocking(true)
        .expect("failed to set test http listener to non-blocking");

    let body = body.to_string();
    let content_type = content_type.to_string();

    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .expect("failed to set test http stream read timeout");

                    let mut request = [0; 1024];
                    let _ = stream.read(&mut request);

                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        content_type,
                        body
                    );

                    stream
                        .write_all(response.as_bytes())
                        .expect("failed to write test http response");
                    stream.flush().expect("failed to flush test http response");

                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "timed out waiting for the test http request"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept test http request: {error}"),
            }
        }
    });

    (format!("http://{address}"), server)
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
stdin = io.stdin
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

    #[test]
    fn top_level_await_sleep() {
        let script = "
from task import sleep

await sleep 0.001
print 'awake'
";
        run_koto_eval_test(script, "", "awake\n");
    }

    #[test]
    fn spawned_sleep_tasks() {
        let script = "
from task import sleep, spawn

tasks = (0..3)
  .each |n|
    spawn ||
      await sleep n * 0.001
  .to_list()

while not tasks.is_empty()
  tasks.retain task.is_active
  await sleep 0.001

print 'done'
";
        run_koto_eval_test(script, "", "done\n");
    }

    #[test]
    #[cfg(feature = "tokio")]
    fn http_client_available() {
        run_koto_eval_test("print koto.type http.client()", "", "Client\n");
    }

    #[test]
    #[cfg(feature = "tokio")]
    fn async_http_get() {
        let (url, server) = spawn_http_server(r#"{"message":"hello async http"}"#, "application/json");
        let script = "
response = await http.get os.args[0]
print response.status()
print response.ok()
print response.header 'content-type'
print response.text()
";

        run_koto_eval_test_with_args(
            script,
            &[&url],
            "",
            "200\ntrue\napplication/json\n{\"message\":\"hello async http\"}\n",
        );

        server
            .join()
            .expect("test http server thread should complete");
    }

    #[test]
    #[cfg(feature = "tokio")]
    fn async_io_read_write() {
        let path = temp_path("io_async");
        let script = "
from io_async import read_to_string, write

path = os.args[0]
await write path, 'hello async io'
print await read_to_string path
";

        run_koto_eval_test_with_args(script, &[&path], "", "hello async io\n");

        if Path::new(&path).exists() {
            fs::remove_file(path).expect("failed to remove temp file");
        }
    }
}
