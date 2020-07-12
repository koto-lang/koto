use {
    koto_bytecode::{Chunk, Compiler},
    koto_parser::{Parser, Span},
    std::{collections::HashMap, path::PathBuf, sync::Arc},
};

#[derive(Clone, Debug)]
pub struct LoaderError {
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Clone, Default)]
pub struct Loader {
    chunks: HashMap<PathBuf, Arc<Chunk>>,
}

impl Loader {
    fn compile(
        &mut self,
        script: &str,
        script_path: Option<PathBuf>,
        parser_options: koto_parser::Options,
    ) -> Result<Arc<Chunk>, LoaderError> {
        match Parser::parse(&script, parser_options) {
            Ok((ast, constants)) => {
                let (bytes, mut debug_info) = match Compiler::compile(&ast) {
                    Ok((bytes, debug_info)) => (bytes, debug_info),
                    Err(e) => {
                        return Err(LoaderError {
                            message: e.message,
                            span: Some(e.span),
                        })
                    }
                };

                debug_info.source = script.to_string();

                Ok(Arc::new(Chunk::new(
                    bytes,
                    constants,
                    script_path,
                    debug_info,
                )))
            }
            Err(e) => Err(LoaderError {
                message: e.to_string(),
                span: Some(e.span),
            }),
        }
    }

    pub fn compile_repl(&mut self, script: &str) -> Result<Arc<Chunk>, LoaderError> {
        self.compile(
            script,
            None,
            koto_parser::Options {
                export_all_top_level: true,
            },
        )
    }

    pub fn compile_script(
        &mut self,
        script: &str,
        script_path: &Option<PathBuf>,
    ) -> Result<Arc<Chunk>, LoaderError> {
        if let Some(script_path) = script_path {
            if let Some(chunk) = self.chunks.get(script_path) {
                return Ok(chunk.clone());
            }
        }

        let chunk = self.compile(
            script,
            script_path.clone(),
            koto_parser::Options {
                export_all_top_level: false,
            },
        )?;

        if let Some(script_path) = script_path {
            self.chunks.insert(script_path.clone(), chunk.clone());
        }

        Ok(chunk)
    }

    pub fn compile_module(
        &mut self,
        name: &str,
        load_from_path: Option<PathBuf>,
    ) -> Result<(Arc<Chunk>, PathBuf), LoaderError> {
        // Get either the directory of the provided path, or the current working directory
        let path = match load_from_path {
            Some(path) => {
                if path.is_file() {
                    match path.parent() {
                        Some(parent_dir) => parent_dir.to_path_buf(),
                        None => path,
                    }
                } else {
                    path
                }
            }
            None => match std::env::current_dir() {
                Ok(path) => path,
                Err(e) => {
                    return Err(LoaderError {
                        message: e.to_string(),
                        span: None,
                    })
                }
            },
        };

        let path = match path.canonicalize() {
            Ok(canonicalized) => canonicalized,
            Err(e) => {
                return Err(LoaderError {
                    message: e.to_string(),
                    span: None,
                })
            }
        };

        let mut load_module_from_path = |module_path: PathBuf| match self.chunks.get(&module_path) {
            Some(chunk) => Ok((chunk.clone(), module_path.clone())),
            None => match std::fs::read_to_string(&module_path) {
                Ok(script) => {
                    let chunk = self.compile(
                        &script,
                        Some(module_path.clone()),
                        koto_parser::Options {
                            export_all_top_level: false,
                        },
                    )?;

                    self.chunks.insert(module_path.clone(), chunk.clone());
                    Ok((chunk, module_path))
                }
                Err(_) => Err(LoaderError {
                    message: format!("File not found: {}", module_path.to_string_lossy()),
                    span: None,
                }),
            },
        };

        let extension = "koto";
        let named_path = path.join(name);

        // first, check for a neighbouring file with a matching name
        let module_path = named_path.with_extension(extension);
        if module_path.exists() {
            return load_module_from_path(module_path);
        }

        // alternatively, check for a neighbouring directory with a matching name,
        // containing a main file
        let module_path = named_path.join("main").with_extension(extension);
        if module_path.exists() {
            load_module_from_path(module_path)
        } else {
            Err(LoaderError {
                message: format!("Unable to find module '{}'", name),
                span: None,
            })
        }
    }
}
