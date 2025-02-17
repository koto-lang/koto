use crate::{Chunk, Compiler, CompilerError, CompilerSettings};
use dunce::canonicalize;
use koto_memory::Ptr;
use koto_parser::{format_source_excerpt, KString, Span};
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

/// Errors that can be returned from [ModuleLoader] operations
#[derive(Error, Debug)]
#[allow(missing_docs)]
pub enum ModuleLoaderErrorKind {
    #[error("{0}")]
    Compiler(#[from] CompilerError),
    #[error("failed to canonicalize path '{path}' ({error})")]
    FailedToCanonicalizePath { path: PathBuf, error: io::Error },
    #[error("failed to read '{path}' ({error})")]
    FailedToReadScript { path: PathBuf, error: io::Error },
    #[error("failed to get current dir ({0}))")]
    FailedToGetCurrentDir(io::Error),
    #[error("failed to get parent of path ('{0}')")]
    FailedToGetPathParent(PathBuf),
    #[error("unable to find module '{0}'")]
    UnableToFindModule(String),
}

/// The error type used by the [ModuleLoader]
#[derive(Clone, Debug)]
pub struct ModuleLoaderError {
    /// The error
    pub error: Ptr<ModuleLoaderErrorKind>,
    /// The source of the error
    pub source: Option<Ptr<LoaderErrorSource>>,
}

/// The source of a [ModuleLoaderError]
#[derive(Debug)]
pub struct LoaderErrorSource {
    /// The script's contents
    pub contents: String,
    /// The span in the script where the error occurred
    pub span: Span,
    /// The script's path
    pub path: Option<KString>,
}

impl ModuleLoaderError {
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
            error: ModuleLoaderErrorKind::from(error).into(),
            source: Some(source.into()),
        }
    }

    /// Returns true if the error was caused by the expectation of indentation during parsing
    pub fn is_indentation_error(&self) -> bool {
        match self.error.deref() {
            ModuleLoaderErrorKind::Compiler(e) => e.is_indentation_error(),
            _ => false,
        }
    }
}

impl fmt::Display for ModuleLoaderError {
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

impl error::Error for ModuleLoaderError {}

impl From<ModuleLoaderErrorKind> for ModuleLoaderError {
    fn from(error: ModuleLoaderErrorKind) -> Self {
        Self {
            error: error.into(),
            source: None,
        }
    }
}

/// Helper for loading, compiling, and caching Koto modules
#[derive(Clone, Default)]
pub struct ModuleLoader {
    chunks: HashMap<PathBuf, Ptr<Chunk>, BuildHasherDefault<FxHasher>>,
}

impl ModuleLoader {
    /// Compiles a script, deferring to [Compiler::compile]
    pub fn compile_script(
        &mut self,
        script: &str,
        script_path: Option<KString>,
        settings: CompilerSettings,
    ) -> Result<Ptr<Chunk>, ModuleLoaderError> {
        Compiler::compile(script, script_path.clone(), settings)
            .map(Ptr::from)
            .map_err(|e| ModuleLoaderError::from_compiler_error(e, script, script_path))
    }

    /// Finds a module from its name, and then compiles it
    pub fn compile_module(
        &mut self,
        module_name: &str,
        current_script_path: Option<&Path>,
    ) -> Result<CompileModuleResult, ModuleLoaderError> {
        let module_path = find_module(module_name, current_script_path)?;

        match self.chunks.get(&module_path) {
            Some(chunk) => Ok(CompileModuleResult {
                chunk: chunk.clone(),
                path: module_path,
                loaded_from_cache: true,
            }),
            None => {
                let script = std::fs::read_to_string(&module_path).map_err(|error| {
                    ModuleLoaderErrorKind::FailedToReadScript {
                        path: module_path.clone(),
                        error,
                    }
                })?;

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

/// Returned from [ModuleLoader::compile_module]
pub struct CompileModuleResult {
    /// The compiled module
    pub chunk: Ptr<Chunk>,
    // The path of the compiled module
    pub path: PathBuf,
    // True if the module was found in the [ModuleLoader] cache
    pub loaded_from_cache: bool,
}

/// Finds a module that matches the given name
///
/// The `current_script_path` argument gives a location to start searching from,
/// if `None` is provided then `std::env::current_dir` will be used instead.
pub fn find_module(
    module_name: &str,
    current_script_path: Option<&Path>,
) -> Result<PathBuf, ModuleLoaderError> {
    // Get the directory of the provided script path, or the current working directory
    let search_folder = match &current_script_path {
        Some(path) => {
            let canonicalized = canonicalize(path).map_err(|error| {
                ModuleLoaderErrorKind::FailedToCanonicalizePath {
                    path: path.to_path_buf(),
                    error,
                }
            })?;
            if canonicalized.is_file() {
                match canonicalized.parent() {
                    Some(parent_dir) => parent_dir.to_path_buf(),
                    None => {
                        let path = PathBuf::from(path);
                        return Err(ModuleLoaderErrorKind::FailedToGetPathParent(path).into());
                    }
                }
            } else {
                canonicalized
            }
        }
        None => std::env::current_dir().map_err(ModuleLoaderErrorKind::FailedToGetCurrentDir)?,
    };

    // First, check for a neighboring file with a matching name.
    let extension = "koto";
    let result = search_folder.join(module_name).with_extension(extension);
    if result.exists() {
        Ok(result)
    } else {
        // Alternatively, check for a neighboring directory with a matching name,
        // that also contains a main file.
        let result = search_folder
            .join(module_name)
            .join("main")
            .with_extension(extension);
        if result.exists() {
            canonicalize(&result).map_err(|error| {
                ModuleLoaderErrorKind::FailedToCanonicalizePath {
                    path: result,
                    error,
                }
                .into()
            })
        } else {
            Err(ModuleLoaderErrorKind::UnableToFindModule(module_name.into()).into())
        }
    }
}
