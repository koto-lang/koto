use koto::{prelude::*, Result};
use koto_test_utils::run_koto_examples_in_markdown;
use std::{fs, path::PathBuf};

fn run_doc_examples(subfolder: Option<&[&str]>, name: &str) -> Result<()> {
    let mut path = PathBuf::new();
    path.extend([env!("CARGO_MANIFEST_DIR"), "..", "..", "docs"]);
    if let Some(subfolder) = subfolder {
        path.extend(subfolder);
    }
    path.push(format!("{name}.md"));
    path = path.canonicalize().unwrap();
    let markdown = fs::read_to_string(&path).unwrap();
    run_koto_examples_in_markdown(&markdown, ValueMap::default())
}

macro_rules! test_doc_examples {
    ($path: expr, $name:ident) => {
        #[test]
        #[allow(non_snake_case)]
        fn $name() -> Result<()> {
            run_doc_examples($path, stringify!($name))
        }
    };
}

macro_rules! test_top_level_examples {
    ($name:ident) => {
        test_doc_examples!(None, $name);
    };
}

test_top_level_examples!(about);
test_top_level_examples!(language_guide);

mod core_lib {
    use super::*;

    macro_rules! test_core_lib_examples {
        ($name:ident) => {
            test_doc_examples!(Some(&["core_lib"]), $name);
        };
    }

    test_core_lib_examples!(iterator);
    test_core_lib_examples!(koto);
    test_core_lib_examples!(list);
    test_core_lib_examples!(map);
    test_core_lib_examples!(number);
    test_core_lib_examples!(os);
    test_core_lib_examples!(range);
    test_core_lib_examples!(string);
    test_core_lib_examples!(test);
    test_core_lib_examples!(tuple);
}
