use {
    koto_bytecode::{Chunk, Compiler, CompilerError},
    koto_parser::{Parser, ParserError},
    std::{collections::HashMap, fmt, path::PathBuf, sync::Arc},
};

#[derive(Clone, Debug)]
pub enum LoaderError {
    ParserError(ParserError),
    CompilerError(CompilerError),
    IoError(String),
}

impl From<ParserError> for LoaderError {
    fn from(e: ParserError) -> Self {
        Self::ParserError(e)
    }
}

impl From<CompilerError> for LoaderError {
    fn from(e: CompilerError) -> Self {
        Self::CompilerError(e)
    }
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LoaderError::*;

        match self {
            ParserError(e) => f.write_str(&e.to_string()),
            CompilerError(e) => f.write_str(&e.message),
            IoError(e) => f.write_str(&e),
        }
    }
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
        compiler_options: koto_bytecode::Options,
    ) -> Result<Arc<Chunk>, LoaderError> {
        match Parser::parse(&script) {
            Ok((ast, constants)) => {
                let (bytes, mut debug_info) = match Compiler::compile(&ast, compiler_options) {
                    Ok((bytes, debug_info)) => (bytes, debug_info),
                    Err(e) => return Err(e.into()),
                };

                debug_info.source = script.to_string();

                Ok(Arc::new(Chunk::new(
                    bytes,
                    constants,
                    script_path,
                    debug_info,
                )))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn compile_repl(&mut self, script: &str) -> Result<Arc<Chunk>, LoaderError> {
        self.compile(script, None, koto_bytecode::Options { repl_mode: true })
    }

    pub fn compile_script(
        &mut self,
        script: &str,
        script_path: &Option<PathBuf>,
    ) -> Result<Arc<Chunk>, LoaderError> {
        self.compile(
            script,
            script_path.clone(),
            koto_bytecode::Options::default(),
        )
    }

    pub fn compile_module(
        &mut self,
        name: &str,
        load_from_path: Option<PathBuf>,
    ) -> Result<(Arc<Chunk>, PathBuf), LoaderError> {
        // Get either the directory of the provided path, or the current working directory
        let path = match load_from_path {
            Some(path) => match path.canonicalize() {
                Ok(canonicalized) if canonicalized.is_file() => match canonicalized.parent() {
                    Some(parent_dir) => parent_dir.to_path_buf(),
                    None => {
                        return Err(LoaderError::IoError(
                            "Failed to get parent of provided path".to_string(),
                        ))
                    }
                },
                Ok(canonicalized) => canonicalized,
                Err(e) => return Err(LoaderError::IoError(e.to_string())),
            },
            None => match std::env::current_dir() {
                Ok(path) => path,
                Err(e) => return Err(LoaderError::IoError(e.to_string())),
            },
        };

        let mut load_module_from_path = |module_path: PathBuf| match self.chunks.get(&module_path) {
            Some(chunk) => Ok((chunk.clone(), module_path.clone())),
            None => match std::fs::read_to_string(&module_path) {
                Ok(script) => {
                    let chunk = self.compile(
                        &script,
                        Some(module_path.clone()),
                        koto_bytecode::Options::default(),
                    )?;

                    self.chunks.insert(module_path.clone(), chunk.clone());
                    Ok((chunk, module_path))
                }
                Err(_) => Err(LoaderError::IoError(format!(
                    "File not found: {}",
                    module_path.to_string_lossy()
                ))),
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
            Err(LoaderError::IoError(format!(
                "Unable to find module '{}'",
                name
            )))
        }
    }
}
