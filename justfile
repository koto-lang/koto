clippy:
  cargo watch -x "clippy --all-targets --all-features"

doc:
  cargo watch -x "doc --workspace --exclude koto_cli"

fmt:
  cargo fmt --all -- --check

doc_tests:
  cargo watch -x "test --test docs_examples"

koto_tests:
  cargo watch -x "test --test koto_tests"

lib_tests:
  cargo watch -x "test --test lib_tests"

parser_tests:
  cargo watch -x "test --package koto_lexer --package koto_parser"

runtime_tests:
  cargo watch -x "test --package koto_runtime"

temp:
  cargo watch -x "run -- --tests -i temp.koto"

test:
  cargo watch -x "test --tests"

test_all:
  cargo watch -x "test --all-targets"

test_benches:
  cargo watch -x "test --benches"

wasm:
  cd examples/wasm && wasm-pack build
