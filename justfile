checks: fmt clippy clippy_rc test test_rc doc wasm

clippy:
  cargo clippy --workspace -- -D warnings

clippy_rc:
  cargo clippy -p koto_memory --no-default-features --features rc -- -D warnings

doc:
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude koto_cli

fmt:
  cargo fmt --all -- --check

temp:
  cargo run -- --tests -i temp.koto

test *args:
  cargo test {{args}}

test_rc *args:
  cargo test -p koto_runtime --no-default-features --features rc {{args}}

test_benches:
  cargo test --benches

test_docs:
  cargo test --test docs_examples

test_koto:
  cargo test --test koto_tests

test_libs:
  cargo test --test lib_tests

test_parser:
  cargo test --package koto_lexer --package koto_parser

test_release *args:
  just test --release {{args}}
  just test_rc --release {{args}}

test_runtime:
  cargo test --package koto_runtime

wasm:
  cd examples/wasm && wasm-pack test --node

watch command:
  cargo watch -s "just {{command}}"
