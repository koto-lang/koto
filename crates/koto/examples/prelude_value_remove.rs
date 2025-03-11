use koto::prelude::*;

fn main() {
    let mut koto = Koto::default();
    let prelude = koto.prelude();

    // Remove the core library's io module from the prelude.
    prelude.remove("io");
    // Remove the os.command function while allowing access to the rest of the os module.
    prelude.remove_path("os.command");

    // These scripts will now throw errors when run.
    assert!(koto.compile_and_run("io.create('temp.txt')").is_err());
    assert!(koto.compile_and_run("os.command('ls')").is_err());

    // os.name is still available so this script will run successfully.
    assert!(koto.compile_and_run("print os.name()").is_ok());
}
