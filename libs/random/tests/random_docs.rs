use koto_runtime::{Result, prelude::*};
use koto_test_utils::run_koto_examples_in_markdown;

#[test]
fn random_docs() -> Result<()> {
    let mut prelude_entries = ValueMap::default();
    prelude_entries.insert("random".into(), koto_random::make_module().into());
    let markdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/libs/random.md"
    ));
    run_koto_examples_in_markdown(markdown, prelude_entries)
}
