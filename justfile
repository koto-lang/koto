checks: test test_rc clippy clippy_rc fmt check_links doc wasm

check_links:
  mlc --offline README.md
  mlc --offline CONTRIBUTING.md
  mlc --offline docs

clippy:
  cargo clippy --workspace -- -D warnings

clippy_rc:
  cargo clippy -p koto_memory --no-default-features --features rc -- -D warnings

doc *args:
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude koto_cli {{args}}

fmt:
  cargo fmt --all -- --check

setup:
  cargo install cargo-watch mlc wasm-pack

temp *args:
  cargo run {{args}} -- --tests -i temp.koto

test *args:
  cargo test {{args}}

test_rc *args:
  cargo test --tests --no-default-features --features rc \
    -p koto_parser \
    -p koto_bytecode \
    -p koto_runtime \
    -p koto \
    {{args}}
  just test_libs --no-default-features --features rc

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

test_libs *args:
  cargo test \
    -p koto_json \
    -p koto_random \
    -p koto_tempfile \
    -p koto_toml \
    -p koto_yaml \
    {{args}}

test_parser *args:
  cargo test -p koto_lexer -p koto_parser {{args}}

test_release *args:
  just test --release {{args}}

test_runtime *args:
  cargo test -p koto_runtime -p koto_bytecode {{args}}

wasm:
  cd crates/koto/examples/wasm && wasm-pack test --node

watch command *args:
  cargo watch -s "just {{command}} {{args}}"
