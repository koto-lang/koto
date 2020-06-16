use {
    koto_bytecode::{Chunk, Compiler},
    koto_parser::Parser,
    std::{collections::HashMap, sync::Arc},
};

#[derive(Clone, Default)]
pub struct Loader {
    chunks: HashMap<String, Arc<Chunk>>,
}

impl Loader {
    fn compile(
        &mut self,
        script: &str,
        script_path: &Option<String>,
        parser_options: koto_parser::Options,
    ) -> Result<Arc<Chunk>, String> {
        match Parser::parse(&script, parser_options) {
            Ok((ast, constants)) => {
                let (bytes, mut debug_info) = Compiler::compile(&ast)?;
                debug_info.script_path = script_path.clone();
                Ok(Arc::new(Chunk::new(bytes, constants, debug_info)))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn compile_repl(&mut self, script: &str) -> Result<Arc<Chunk>, String> {
        self.compile(
            script,
            &None,
            koto_parser::Options {
                export_all_top_level: true,
            },
        )
    }

    pub fn compile_script(
        &mut self,
        script: &str,
        script_path: &Option<String>,
    ) -> Result<Arc<Chunk>, String> {
        if let Some(script_path) = script_path {
            if let Some(chunk) = self.chunks.get(script_path) {
                return Ok(chunk.clone());
            }
        }

        let chunk = self.compile(
            script,
            script_path,
            koto_parser::Options {
                export_all_top_level: false,
            },
        )?;

        if let Some(script_path) = script_path {
            self.chunks.insert(script_path.to_string(), chunk.clone());
        }

        Ok(chunk)
    }
}
