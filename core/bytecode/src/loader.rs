use crate::{Chunk, Compiler, CompilerError, CompilerSettings};
use dunce::canonicalize;
use koto_memory::Ptr;
use koto_parser::{format_source_excerpt, Parser, ParserError, Span};
use rustc_hash::FxHasher;
use std::{collections::HashMap, error, fmt, hash::BuildHasherDefault, io, path::PathBuf};
use thiserror::Error;

/// Errors that can be returned from [Loader] operations
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum LoaderErrorKind {
    #[error(transparent)]
    Parser(#[from] ParserError),
    #[error(transparent)]
    Compiler(#[from] CompilerError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Failed to get parent of path ('{0}')")]
    FailedToGetPathParent(PathBuf),
    #[error("Unable to find module '{0}'")]
    UnableToFindModule(String),
}

/// The error type used by the [Loader]
#[derive(Debug)]
pub struct LoaderError {
    error: Box<LoaderErrorKind>,
    source: Option<Box<LoaderErrorSource>>,
}

#[derive(Debug)]
struct LoaderErrorSource {
    contents: String,
    span: Span,
    path: Option<PathBuf>,
}

impl LoaderError {
    pub(crate) fn from_parser_error(
        error: ParserError,
        source: &str,
        source_path: Option<PathBuf>,
    ) -> Self {
        let source = LoaderErrorSource {
            contents: source.into(),
            span: error.span,
            path: source_path,
        };
        Self {
            error: Box::new(error.into()),
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn from_compiler_error(
        error: CompilerError,
        source: &str,
        source_path: Option<PathBuf>,
    ) -> Self {
        let source = LoaderErrorSource {
            contents: source.into(),
            span: error.span,
            path: source_path,
        };
        Self {
            error: Box::new(error.into()),
            source: Some(Box::new(source)),
        }
    }

    /// Returns true if the error was caused by the expectation of indentation during parsing
    pub fn is_indentation_error(&self) -> bool {
        match self.error.as_ref() {
            LoaderErrorKind::Parser(e) => e.is_indentation_error(),
            _ => false,
        }
    }
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(source) = &self.source {
            write!(
                f,
                "{}",
                format_source_excerpt(&source.contents, &source.span, &source.path)
            )?;
        }
        Ok(())
    }
}

impl error::Error for LoaderError {}

impl From<io::Error> for LoaderError {
    fn from(error: io::Error) -> Self {
        Self {
            error: Box::new(error.into()),
            source: None,
        }
    }
}

impl From<LoaderErrorKind> for LoaderError {
    fn from(error: LoaderErrorKind) -> Self {
        Self {
            error: Box::new(error),
            source: None,
        }
    }
}

/// Helper for loading, compiling, and caching Koto modules
#[derive(Clone, Default)]
pub struct Loader {
    chunks: HashMap<PathBuf, Ptr<Chunk>, BuildHasherDefault<FxHasher>>,
}

impl Loader {
    fn compile(
        &mut self,
        script: &str,
        script_path: Option<PathBuf>,
        compiler_settings: CompilerSettings,
    ) -> Result<Ptr<Chunk>, LoaderError> {
        match Parser::parse(script) {
            Ok(ast) => {
                let (bytes, mut debug_info) = match Compiler::compile(&ast, compiler_settings) {
                    Ok((bytes, debug_info)) => (bytes, debug_info),
                    Err(e) => return Err(LoaderError::from_compiler_error(e, script, script_path)),
                };

                debug_info.source = script.to_string();

                Ok(Chunk::new(bytes, ast.consume_constants(), script_path, debug_info).into())
            }
            Err(e) => Err(LoaderError::from_parser_error(e, script, script_path)),
        }
    }

    /// Compiles a script
    pub fn compile_script(
        &mut self,
        script: &str,
        script_path: &Option<PathBuf>,
        settings: CompilerSettings,
    ) -> Result<Ptr<Chunk>, LoaderError> {
        self.compile(script, script_path.clone(), settings)
    }

    /// Finds a module from its name, and then compiles it
    pub fn compile_module(
        &mut self,
        name: &str,
        load_from_path: Option<PathBuf>,
    ) -> Result<CompileModuleResult, LoaderError> {
        // Get either the directory of the provided path, or the current working directory
        let search_folder = match &load_from_path {
            Some(path) => match canonicalize(path)? {
                canonicalized if canonicalized.is_file() => match canonicalized.parent() {
                    Some(parent_dir) => parent_dir.to_path_buf(),
                    None => return Err(LoaderErrorKind::FailedToGetPathParent(path.clone()).into()),
                },
                canonicalized => canonicalized,
            },
            None => std::env::current_dir()?,
        };

        let mut load_module_from_path = |module_path: PathBuf| {
            let module_path = module_path.canonicalize()?;

            match self.chunks.get(&module_path) {
                Some(chunk) => Ok(CompileModuleResult {
                    chunk: chunk.clone(),
                    path: module_path,
                    loaded_from_cache: true,
                }),
                None => {
                    let script = std::fs::read_to_string(&module_path)?;

                    let chunk = self.compile(
                        &script,
                        Some(module_path.clone()),
                        CompilerSettings::default(),
                    )?;

                    self.chunks.insert(module_path.clone(), chunk.clone());

                    Ok(CompileModuleResult {
                        chunk,
                        path: module_path,
                        loaded_from_cache: false,
                    })
                }
            }
        };

        let extension = "koto";
        let named_path = search_folder.join(name);

        // First, check for a neighbouring file with a matching name.
        let module_path = named_path.with_extension(extension);
        if module_path.exists() {
            load_module_from_path(module_path)
        } else {
            // Alternatively, check for a neighbouring directory with a matching name,
            // that also contains a main file.
            let module_path = named_path.join("main").with_extension(extension);
            if module_path.exists() {
                load_module_from_path(module_path)
            } else {
                Err(LoaderErrorKind::UnableToFindModule(name.into()).into())
            }
        }
    }

    /// Clears the compiled module cache
    pub fn clear_cache(&mut self) {
        self.chunks.clear();
    }
}

pub struct CompileModuleResult {
    pub chunk: Ptr<Chunk>,
    pub path: PathBuf,
    pub loaded_from_cache: bool,
}
