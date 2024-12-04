use crate::{Chunk, Compiler, CompilerError, CompilerSettings};
use dunce::canonicalize;
use koto_memory::Ptr;
use koto_parser::{format_source_excerpt, KString, Parser, Span};
use rustc_hash::FxHasher;
use std::{
    collections::HashMap,
    error, fmt,
    hash::BuildHasherDefault,
    io,
    ops::Deref,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Errors that can be returned from [Loader] operations
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum LoaderErrorKind {
    #[error("{0}")]
    Parser(#[from] koto_parser::Error),
    #[error("{0}")]
    Compiler(#[from] CompilerError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Failed to get parent of path ('{0}')")]
    FailedToGetPathParent(PathBuf),
    #[error("Unable to find module '{0}'")]
    UnableToFindModule(String),
}

/// The error type used by the [Loader]
#[derive(Clone, Debug)]
pub struct LoaderError {
    /// The error
    pub error: Ptr<LoaderErrorKind>,
    /// The source of the error
    pub source: Option<Ptr<LoaderErrorSource>>,
}

/// The source of a [LoaderError]
#[derive(Debug)]
pub struct LoaderErrorSource {
    /// The script's contents
    pub contents: String,
    /// The span in the script where the error occurred
    pub span: Span,
    /// The script's path
    pub path: Option<KString>,
}

impl LoaderError {
    pub(crate) fn from_parser_error(
        error: koto_parser::Error,
        source: &str,
        source_path: Option<KString>,
    ) -> Self {
        let source = LoaderErrorSource {
            contents: source.into(),
            span: error.span,
            path: source_path,
        };
        Self {
            error: LoaderErrorKind::from(error).into(),
            source: Some(source.into()),
        }
    }

    pub(crate) fn from_compiler_error(
        error: CompilerError,
        source: &str,
        source_path: Option<KString>,
    ) -> Self {
        let source = LoaderErrorSource {
            contents: source.into(),
            span: error.span,
            path: source_path,
        };
        Self {
            error: LoaderErrorKind::from(error).into(),
            source: Some(source.into()),
        }
    }

    /// Returns true if the error was caused by the expectation of indentation during parsing
    pub fn is_indentation_error(&self) -> bool {
        match self.error.deref() {
            LoaderErrorKind::Parser(e) => e.is_indentation_error(),
            _ => false,
        }
    }
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}.", self.error)?;
        if let Some(source) = &self.source {
            write!(
                f,
                "{}",
                format_source_excerpt(&source.contents, &source.span, source.path.as_deref())
            )?;
        }
        Ok(())
    }
}

impl error::Error for LoaderError {}

impl From<io::Error> for LoaderError {
    fn from(error: io::Error) -> Self {
        Self {
            error: Ptr::new(error.into()),
            source: None,
        }
    }
}

impl From<LoaderErrorKind> for LoaderError {
    fn from(error: LoaderErrorKind) -> Self {
        Self {
            error: error.into(),
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
    /// Compiles a script
    pub fn compile_script(
        &mut self,
        script: &str,
        script_path: Option<KString>,
        settings: CompilerSettings,
    ) -> Result<Ptr<Chunk>, LoaderError> {
        match Parser::parse(script) {
            Ok(ast) => {
                let (bytes, mut debug_info) = match Compiler::compile(&ast, settings) {
                    Ok((bytes, debug_info)) => (bytes, debug_info),
                    Err(e) => return Err(LoaderError::from_compiler_error(e, script, script_path)),
                };

                debug_info.source = script.to_string();

                Ok(Chunk {
                    bytes,
                    constants: ast.consume_constants(),
                    source_path: script_path,
                    debug_info,
                }
                .into())
            }
            Err(e) => Err(LoaderError::from_parser_error(e, script, script_path)),
        }
    }

    /// Finds a module from its name, and then compiles it
    pub fn compile_module(
        &mut self,
        module_name: &str,
        current_script_path: Option<impl AsRef<Path>>,
    ) -> Result<CompileModuleResult, LoaderError> {
        let module_path = find_module(module_name, current_script_path)?;

        match self.chunks.get(&module_path) {
            Some(chunk) => Ok(CompileModuleResult {
                chunk: chunk.clone(),
                path: module_path,
                loaded_from_cache: true,
            }),
            None => {
                let script = std::fs::read_to_string(&module_path)?;

                let chunk = self.compile_script(
                    &script,
                    Some(module_path.clone().into()),
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

/// Finds a module that matches the given name
///
/// The current_script_path gives the function a location to start searching from, if None is
/// provided then std::env::current_dir will be used.
pub fn find_module(
    module_name: &str,
    current_script_path: Option<impl AsRef<Path>>,
) -> Result<PathBuf, LoaderError> {
    // Get the directory of the provided script path, or the current working directory
    let search_folder = match &current_script_path {
        Some(path) => {
            let canonicalized = canonicalize(path)?;
            if canonicalized.is_file() {
                match canonicalized.parent() {
                    Some(parent_dir) => parent_dir.to_path_buf(),
                    None => {
                        let path = PathBuf::from(path.as_ref());
                        return Err(LoaderErrorKind::FailedToGetPathParent(path).into());
                    }
                }
            } else {
                canonicalized
            }
        }
        None => std::env::current_dir()?,
    };

    // First, check for a neighbouring file with a matching name.
    let extension = "koto";
    let result = search_folder.join(module_name).with_extension(extension);
    if result.exists() {
        Ok(result)
    } else {
        // Alternatively, check for a neighbouring directory with a matching name,
        // that also contains a main file.
        let result = search_folder
            .join(module_name)
            .join("main")
            .with_extension(extension);
        if result.exists() {
            let result = canonicalize(result)?;
            Ok(result)
        } else {
            Err(LoaderErrorKind::UnableToFindModule(module_name.into()).into())
        }
    }
}
