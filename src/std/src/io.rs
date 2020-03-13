use crate::{builtin_error, single_arg_fn};
use koto_runtime::{
    value,
    value::{deref_value, type_as_string},
    BuiltinValue, Error, Value, ValueMap,
};
use std::{
    cell::RefCell,
    fmt, fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

pub fn register(global: &mut ValueMap) {
    use Value::{Bool, Map, Ref, Str};

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
        macro_rules! file_fn {
           ($fn_name:expr, $file_map:expr, $args_name:ident, $match_name:ident, $body:block) => {
                $file_map.add_instance_fn($fn_name, |_, $args_name| {
                    match &$args_name[0] {
                        Ref(map_ref) => {
                            match &*map_ref.borrow() {
                                // TODO Find a way to get from ValueMap with &str as key
                                Map(instance) =>
                                    match instance.as_ref().0.get(&"file".to_string()) {
                                    Some(Value::BuiltinValue(maybe_file)) => {
                                        match maybe_file.borrow_mut().downcast_mut::<File>() {
                                            Some($match_name) => $body,
                                            None => builtin_error!(
                                                "File.{}: Invalid type for file handle, found '{}'",
                                                $fn_name,
                                                maybe_file.borrow().value_type()
                                            ),
                                        }
                                    }
                                    Some(unexpected) => builtin_error!(
                                        "File.{}: Invalid type for File handle, found '{}'",
                                        $fn_name,
                                        type_as_string(unexpected)
                                        ),
                                    None =>
                                        builtin_error!("File.{}: File handle not found", $fn_name),
                                },
                                unexpected => builtin_error!(
                                    "File.{}: Expected File instance as first argument, found '{}'",
                                    $fn_name,
                                    unexpected
                                ),
                            }
                        }
                        unexpected => builtin_error!(
                            "File.{}: Expected File instance as first argument, found '{}'",
                            $fn_name,
                            unexpected
                        ),
                    }
                });
           }
        };

        let mut file_map = ValueMap::new();

        file_fn!("path", file_map, args, file_handle, {
            Ok(Str(Rc::new(file_handle.path.to_string_lossy().to_string())))
        });

        file_fn!("write_line", file_map, args, file_handle, {
            let line = if args.len() < 2 {
                "\n".to_string()
            } else {
                format!("{}\n", args[1])
            };
            match file_handle.file.write(line.as_bytes()) {
                Ok(_) => Ok(Value::Empty),
                Err(e) => builtin_error!("File.write_line: Error while writing to file: {}", e),
            }
        });

        file_fn!("read_to_string", file_map, args, file_handle, {
            match file_handle.file.seek(SeekFrom::Start(0)) {
                Ok(_) => {
                    let mut buffer = String::new();
                    match file_handle.file.read_to_string(&mut buffer) {
                        Ok(_) => Ok(Str(Rc::new(buffer))),
                        Err(e) => {
                            builtin_error!("File.read_to_string: Error while reading data: {}", e)
                        }
                    }
                }
                Err(e) => builtin_error!("File.read_to_string: Error while seeking in file: {}", e),
            }
        });

        file_map
    };

    io.add_fn("open", {
        let file_map = file_map.clone();
        move |_, args| {
            if args.len() == 1 {
                match deref_value(&args[0]) {
                    Str(s) => {
                        let path = match Path::new(s.as_ref()).canonicalize() {
                            Ok(path) => path,
                            Err(e) => {
                                return builtin_error!("io.open: Error while opening path: {}", e);
                            }
                        };
                        match fs::File::open(&path) {
                            Ok(file) => {
                                let mut file_map = file_map.clone();

                                file_map.add_value(
                                    "file",
                                    Value::BuiltinValue(Rc::new(RefCell::new(File {
                                        file,
                                        path,
                                        temporary: false,
                                    }))),
                                );

                                Ok(Map(Rc::new(file_map)))
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
                builtin_error!(
                    "io.open expects a single argument argument, found {}",
                    args.len()
                )
            }
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

            Ok(Map(Rc::new(file_map)))
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
