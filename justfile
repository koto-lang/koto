koto_tests:
  cargo watch -x "test --test koto_tests"

runtime_tests:
  cargo watch -x "test --package koto_runtime"

test:
  cargo test --all-targets --benches

test_benches:
  cargo test --benches

temp:
  cargo watch -x "run -- -b -S temp.koto"
