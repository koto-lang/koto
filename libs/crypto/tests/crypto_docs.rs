//! The doc examples exercise the full `crypto` API, including the encryption and
//! signing functions, so this test only runs when both features are enabled.

#![cfg(all(feature = "encryption", feature = "signing"))]

use koto_runtime::{Result, prelude::*};
use koto_test_utils::run_koto_examples_in_markdown;

#[test]
fn crypto_docs() -> Result<()> {
    let mut prelude_entries = ValueMap::default();
    prelude_entries.insert("crypto".into(), koto_crypto::make_module().into());
    let markdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/libs/crypto.md"
    ));
    run_koto_examples_in_markdown(markdown, prelude_entries)
}
