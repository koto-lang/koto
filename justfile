koto_tests:
  cargo watch -x "test --test koto_tests"

runtime_tests:
  cargo watch -x "test --package koto_runtime"

parser_tests:
  cargo watch -x "test --package koto_lexer --package koto_parser"

test:
  cargo watch -x "test --tests"

test_all:
  cargo watch -x "test --all-targets"

test_benches:
  cargo watch -x "test --benches"

temp:
  cargo watch -x "run -- --tests -i temp.koto"
