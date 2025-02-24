use koto_derive::*;
use koto_runtime::{Result, prelude::*};
use koto_test_utils::*;
use std::{error::Error, fs, path::PathBuf, result::Result as StdResult};

#[derive(Clone, Debug, Default, KotoCopy, KotoType)]
struct TestContainer {
    data: Vec<KValue>,
}

#[koto_impl(runtime = koto_runtime)]
impl TestContainer {
    #[koto_method]
    fn push(&mut self, args: &[KValue]) {
        self.data.extend_from_slice(args);
    }

    #[koto_method]
    fn to_tuple(&self) -> KValue {
        KTuple::from(self.data.clone()).into()
    }
}

impl KotoObject for TestContainer {
    fn size(&self) -> Option<usize> {
        Some(self.data.len())
    }

    fn index(&self, index: &KValue) -> Result<KValue> {
        match index {
            KValue::Number(i) => Ok(self.data[usize::from(i)].clone()),
            unexpected => unexpected_type("An index", unexpected),
        }
    }

    fn index_mut(&mut self, index: &KValue, value: &KValue) -> Result<()> {
        match index {
            KValue::Number(i) => {
                self.data[usize::from(i)] = value.clone();
                Ok(())
            }
            unexpected => unexpected_type("An index", unexpected),
        }
    }
}

#[test]
fn shuffle() -> StdResult<(), Box<dyn Error>> {
    let vm = KotoVm::default();
    let prelude = vm.prelude();

    prelude.insert("random", koto_random::make_module());

    prelude.add_fn("new_container", |ctx| {
        let data: Vec<KValue> = ctx.args().to_vec();
        Ok(KObject::from(TestContainer { data }).into())
    });

    let script_path = PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "tests", "shuffle.koto"]);
    let script = fs::read_to_string(&script_path)?;

    run_test_script(vm, &script, Some(script_path.into()), None)?;

    Ok(())
}
