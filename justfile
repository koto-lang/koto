koto_tests:
  cargo watch -x "test --test koto_tests"

test:
  cargo test --all-targets --benches

test_benches:
  cargo test --benches

temp:
  cargo watch -x "run -- temp.koto"
