clippy:
  cargo watch -x "clippy --all-targets --all-features"

doc:
  cargo watch -x "doc --workspace --exclude koto_cli"

fmt:
  cargo fmt --all -- --check

temp:
  cargo watch -x "run -- --tests -i temp.koto"

test:
  cargo watch -x "test --tests"

test_all:
  cargo watch -x "test --all-targets"

test_benches:
  cargo watch -x "test --benches"

test_docs:
  cargo watch -x "test --test docs_examples"

test_koto:
  cargo watch -x "test --test koto_tests"

test_libs:
  cargo watch -x "test --test lib_tests"

test_parser:
  cargo watch -x "test --package koto_lexer --package koto_parser"

test_runtime:
  cargo watch -x "test --package koto_runtime"

wasm:
  cd examples/wasm && wasm-pack test --node
