use crate::{builtin_error, single_arg_fn};
use koto_runtime::{
    value,
    value::{deref_value, type_as_string},
    BuiltinValue, Error, RuntimeResult, Value, ValueMap,
};
use std::{
    cell::RefCell,
    fmt, fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

pub fn register(global: &mut ValueMap) {
    use Value::{Bool, Map, Number, Str};

    let mut io = ValueMap::new();

    single_arg_fn!(io, "exists", Str, path, {
        Ok(Bool(Path::new(path.as_ref()).exists()))
    });

    single_arg_fn!(io, "read_to_string", Str, path, {
        {
            match fs::read_to_string(Path::new(path.as_ref())) {
                Ok(result) => Ok(Str(Rc::new(result))),
                Err(e) => {
                    builtin_error!("io.read_to_string: Unable to read file '{}': {}", path, e)
                }
            }
        }
    });

    let file_map = {
        fn file_fn<'a>(
            fn_name: &str,
            args: &[Value<'a>],
            mut file_op: impl FnMut(&mut File) -> RuntimeResult<'a>,
        ) -> RuntimeResult<'a> {
            use Value::Ref;

            if args.len() == 0 {
                return builtin_error!(
                    "File.{}: Expected file instance as first argument",
                    fn_name
                );
            }

            match &args[0] {
                Ref(map_ref) => {
                    match &*map_ref.borrow() {
                        // TODO Find a way to get from ValueMap with &str as key
                        Map(instance) => match instance.borrow().0.get("file") {
                            Some(Value::BuiltinValue(maybe_file)) => {
                                match maybe_file.borrow_mut().downcast_mut::<File>() {
                                    Some(file_handle) => file_op(file_handle),
                                    None => builtin_error!(
                                        "File.{}: Invalid type for file handle, found '{}'",
                                        fn_name,
                                        maybe_file.borrow().value_type()
                                    ),
                                }
                            }
                            Some(unexpected) => builtin_error!(
                                "File.{}: Invalid type for File handle, found '{}'",
                                fn_name,
                                type_as_string(unexpected)
                            ),
                            None => builtin_error!("File.{}: File handle not found", fn_name),
                        },
                        unexpected => builtin_error!(
                            "File.{}: Expected File instance as first argument, found '{}'",
                            fn_name,
                            unexpected
                        ),
                    }
                }
                unexpected => builtin_error!(
                    "File.{}: Expected File instance as first argument, found '{}'",
                    fn_name,
                    unexpected
                ),
            }
        }

        let mut file_map = ValueMap::new();

        file_map.add_instance_fn("path", |_, args| {
            file_fn("path", args, |file_handle| {
                Ok(Str(Rc::new(file_handle.path.to_string_lossy().to_string())))
            })
        });

        file_map.add_instance_fn("write", |_, args| {
            file_fn("write", args, |file_handle| {
                if args.len() < 2 {
                    return builtin_error!("File.write: Expected argument");
                }
                let data = format!("{}", args[1]);

                match file_handle.file.write(data.as_bytes()) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => builtin_error!("File.write: Error while writing to file: {}", e),
                }
            })
        });

        file_map.add_instance_fn("write_line", |_, args| {
            file_fn("write_line", args, |file_handle| {
                let line = if args.len() < 2 {
                    "\n".to_string()
                } else {
                    format!("{}\n", args[1])
                };
                match file_handle.file.write(line.as_bytes()) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => builtin_error!("File.write_line: Error while writing to file: {}", e),
                }
            })
        });

        file_map.add_instance_fn("read_to_string", |_, args| {
            file_fn("read_to_string", args, |file_handle| {
                match file_handle.file.seek(SeekFrom::Start(0)) {
                    Ok(_) => {
                        let mut buffer = String::new();
                        match file_handle.file.read_to_string(&mut buffer) {
                            Ok(_) => Ok(Str(Rc::new(buffer))),
                            Err(e) => builtin_error!(
                                "File.read_to_string: Error while reading data: {}",
                                e
                            ),
                        }
                    }
                    Err(e) => {
                        builtin_error!("File.read_to_string: Error while seeking in file: {}", e)
                    }
                }
            })
        });

        file_map.add_instance_fn("seek", |_, args| {
            file_fn("seek", args, |file_handle| match args.get(1) {
                Some(Number(n)) => {
                    if *n < 0.0 {
                        return builtin_error!("File.seek: Negative seek positions not allowed");
                    }
                    match file_handle.file.seek(SeekFrom::Start(*n as u64)) {
                        Ok(_) => Ok(Value::Empty),
                        Err(e) => builtin_error!("File.seek: Error while seeking in file: {}", e),
                    }
                }
                Some(unexpected) => builtin_error!(
                    "File.seek: Expected Number for seek position, found '{}'",
                    type_as_string(&unexpected)
                ),
                None => builtin_error!("File.seek: Expected seek position as second argument"),
            })
        });

        file_map
    };

    io.add_fn("open", {
        let file_map = file_map.clone();
        move |_, args| {
            if args.len() == 1 {
                match deref_value(&args[0]) {
                    Str(path) => {
                        let path = Path::new(path.as_ref());
                        match fs::File::open(&path) {
                            Ok(file) => {
                                let mut file_map = file_map.clone();

                                file_map.add_value(
                                    "file",
                                    Value::BuiltinValue(Rc::new(RefCell::new(File {
                                        file,
                                        path: path.to_path_buf(),
                                        temporary: false,
                                    }))),
                                );

                                Ok(Map(Rc::new(RefCell::new(file_map))))
                            }
                            Err(e) => {
                                return builtin_error!("io.open: Error while opening path: {}", e);
                            }
                        }
                    }
                    unexpected => builtin_error!(
                        "io.open expects a String as its argument, found '{}'",
                        type_as_string(&unexpected)
                    ),
                }
            } else {
                builtin_error!("io.open expects a single argument, found {}", args.len())
            }
        }
    });

    io.add_fn("create", {
        let file_map = file_map.clone();
        move |_, args| {
            if args.len() == 1 {
                match deref_value(&args[0]) {
                    Str(path) => {
                        let path = Path::new(path.as_ref());
                        match fs::File::create(&path) {
                            Ok(file) => {
                                let mut file_map = file_map.clone();

                                file_map.add_value(
                                    "file",
                                    Value::BuiltinValue(Rc::new(RefCell::new(File {
                                        file,
                                        path: path.to_path_buf(),
                                        temporary: false,
                                    }))),
                                );

                                Ok(Map(Rc::new(RefCell::new(file_map))))
                            }
                            Err(e) => {
                                return builtin_error!(
                                    "io.create: Error while creating file: {}",
                                    e
                                );
                            }
                        }
                    }
                    unexpected => builtin_error!(
                        "io.create expects a String as its argument, found '{}'",
                        type_as_string(&unexpected)
                    ),
                }
            } else {
                builtin_error!("io.create expects a single argument, found {}", args.len())
            }
        }
    });

    io.add_fn("temp_path", {
        |_, _| match tempfile::NamedTempFile::new() {
            Ok(file) => match file.keep() {
                Ok((_temp_file, path)) => Ok(Str(Rc::new(path.to_string_lossy().to_string()))),
                Err(e) => builtin_error!("io.temp_file: Error while making temp path: {}", e),
            },
            Err(e) => builtin_error!("io.temp_file: Error while making temp path: {}", e),
        }
    });

    io.add_fn("temp_file", {
        let file_map = file_map.clone();
        move |_, _| {
            let (temp_file, path) = match tempfile::NamedTempFile::new() {
                Ok(file) => match file.keep() {
                    Ok((temp_file, path)) => (temp_file, path),
                    Err(e) => {
                        return builtin_error!(
                            "io.temp_file: Error while creating temp file: {}",
                            e
                        );
                    }
                },
                Err(e) => {
                    return builtin_error!("io.temp_file: Error while creating temp file: {}", e);
                }
            };

            let mut file_map = file_map.clone();
            file_map.add_value(
                "file",
                Value::BuiltinValue(Rc::new(RefCell::new(File {
                    file: temp_file,
                    path,
                    temporary: true,
                }))),
            );

            Ok(Map(Rc::new(RefCell::new(file_map))))
        }
    });

    io.add_fn("remove_file", {
        |_, args| {
            if args.len() == 1 {
                match deref_value(&args[0]) {
                    Str(path) => {
                        let path = Path::new(path.as_ref());
                        match fs::remove_file(&path) {
                            Ok(_) => Ok(Value::Empty),
                            Err(e) => builtin_error!(
                                "io.remove_file: Error while removing file '{}': {}",
                                path.to_string_lossy(),
                                e
                            ),
                        }
                    }
                    unexpected => builtin_error!(
                        "io.remove_file expects a String as its argument, found '{}'",
                        type_as_string(&unexpected)
                    ),
                }
            } else {
                builtin_error!(
                    "io.remove_file expects a single argument, found {}",
                    args.len()
                )
            }
        }
    });

    global.add_map("io", io);
}

#[derive(Debug)]
struct File {
    file: fs::File,
    path: PathBuf,
    temporary: bool,
}

impl Drop for File {
    fn drop(&mut self) {
        if self.temporary {
            if let Err(_) = fs::remove_file(&self.path) {};
        }
    }
}

impl BuiltinValue for File {
    fn value_type(&self) -> String {
        "File".to_string()
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "File({})", self.path.to_string_lossy())
    }
}
