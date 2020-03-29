use koto::Koto;
use std::{fs::read_to_string, path::PathBuf};

fn run_script(script_path: &str) {
    let mut path = PathBuf::new();
    path.push(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push(script_path);
    if !path.exists() {
        panic!(format!("Path doesn't exist: {:?}", path));
    }
    let script = read_to_string(&path).expect(&format!("Unable to load path '{:?}'", &path));

    let mut koto = Koto::new();
    koto.set_script_path(Some(path.to_string_lossy().to_string()));
    if let Err(error) = koto.run_script_with_args(&script, vec![]) {
        eprintln!("{}", error);
        assert!(false);
    }
}

macro_rules! koto_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            run_script(&format!("{}.koto", stringify!($name)));
        }
    };
}

koto_test!(arithmetic);
koto_test!(assignment);
koto_test!(comments);
koto_test!(control_flow);
koto_test!(functions);
koto_test!(functions_in_lookups);
koto_test!(io);
koto_test!(line_breaks);
koto_test!(lists);
koto_test!(list_ops);
koto_test!(logic);
koto_test!(loops);
koto_test!(maps);
koto_test!(maps_and_lists);
koto_test!(math);
koto_test!(ranges);
koto_test!(shares);
koto_test!(strings);
koto_test!(vec4_new);
koto_test!(vec4_ops);
