use {
    koto::Koto,
    std::{fs::read_to_string, path::PathBuf},
};

#[test]
fn json_lib() {
    let mut path = PathBuf::new();
    path.push(env!("CARGO_MANIFEST_DIR"));
    path.push("../../koto/tests");
    path.push("json.koto");
    let script = read_to_string(&path).expect(&format!("Unable to load path '{:?}'", &path));

    let mut koto = Koto::with_settings(koto::Settings {
        run_tests: true,
        ..Default::default()
    });
    koto.set_script_path(Some(path));

    koto.context_mut()
        .prelude
        .add_map("json", koto_json::make_module());

    match koto.compile(&script) {
        Ok(_) => {
            if let Err(error) = koto.run() {
                panic!(error);
            }
        }
        Err(error) => {
            panic!(error);
        }
    }
}
