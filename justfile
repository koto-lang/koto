checks: fmt clippy clippy_rc test test_rc check_links doc wasm

check_links:
  mlc --offline README.md
  mlc --offline docs

clippy:
  cargo clippy --workspace -- -D warnings

clippy_rc:
  cargo clippy -p koto_memory --no-default-features --features rc -- -D warnings

doc:
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude koto_cli

fmt:
  cargo fmt --all -- --check

setup:
  cargo install cargo-watch mlc wasm-pack

temp *args:
  cargo run {{args}} -- --tests -i temp.koto

test *args:
  cargo test {{args}}

test_rc *args:
  cargo test -p koto_runtime --no-default-features --features rc {{args}}

test_benches:
  cargo test --benches

test_docs:
  cargo test \
    --test docs_examples \
    --test color_docs \
    --test geometry_docs \
    --test json_docs \
    --test random_docs \
    --test regex_docs \
    --test tempfile_docs \
    --test toml_docs \
    --test yaml_docs

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
  cd crates/koto/examples/wasm && wasm-pack test --node

watch command *args:
  cargo watch -s "just {{command}} {{args}}"
