koto_tests:
  cargo watch -x "test --test koto_tests"

test_benches:
  cargo test --benches

temp:
  cargo watch -x "run -- temp.koto"
