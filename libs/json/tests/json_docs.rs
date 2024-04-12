use koto_runtime::{prelude::*, Result};
use koto_test_utils::run_koto_examples_in_markdown;

#[test]
fn json_docs() -> Result<()> {
    let mut prelude_entries = ValueMap::default();
    prelude_entries.insert("json".into(), koto_json::make_module().into());
    let markdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/libs/json.md"
    ));
    run_koto_examples_in_markdown(markdown, prelude_entries)
}
