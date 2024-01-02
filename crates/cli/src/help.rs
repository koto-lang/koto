use indexmap::IndexMap;
use std::{
    iter::{self, Peekable},
    rc::Rc,
};

const HELP_RESULT_STR: &str = "➝ ";
const HELP_INDENT: usize = 2;

struct HelpEntry {
    // The entry's user-displayed name
    name: Rc<str>,
    // The entry's contents
    help: Rc<str>,
    // Additional keywords that should be checked when searching
    keywords: Vec<Rc<str>>,
    // Names of related topics to show in the 'See also' section
    see_also: Vec<Rc<str>>,
}

pub struct Help {
    // All help entries, keys are lower_snake_case
    help_map: IndexMap<Rc<str>, HelpEntry>,
    // The list of guide topics
    guide_topics: Vec<Rc<str>>,
    // The list of core library module names
    module_names: Vec<Rc<str>>,
}

impl Help {
    pub fn new() -> Self {
        macro_rules! include_doc {
            ($doc:expr) => {
                // Including via a symlink to the top-level docs folder to ensure cargo-package
                // can find it during packaging.
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/", $doc))
            };
        }

        let guide_files = [
            include_doc!("language/basics.md"),
            include_doc!("language/strings.md"),
            include_doc!("language/functions.md"),
            include_doc!("language/lists.md"),
            include_doc!("language/tuples.md"),
            include_doc!("language/maps.md"),
            include_doc!("language/core_library.md"),
            include_doc!("language/iterators.md"),
            include_doc!("language/value_unpacking.md"),
            include_doc!("language/conditional_expressions.md"),
            include_doc!("language/loops.md"),
            include_doc!("language/ranges.md"),
            include_doc!("language/functions_advanced.md"),
            include_doc!("language/generators.md"),
            include_doc!("language/meta_maps.md"),
            include_doc!("language/errors.md"),
            include_doc!("language/testing.md"),
            include_doc!("language/modules.md"),
            include_doc!("language/prelude.md"),
        ];

        let reference_files = [
            include_doc!("core_lib/io.md"),
            include_doc!("core_lib/iterator.md"),
            include_doc!("core_lib/koto.md"),
            include_doc!("core_lib/list.md"),
            include_doc!("core_lib/map.md"),
            include_doc!("core_lib/number.md"),
            include_doc!("core_lib/os.md"),
            include_doc!("core_lib/range.md"),
            include_doc!("core_lib/string.md"),
            include_doc!("core_lib/test.md"),
            include_doc!("core_lib/tuple.md"),
        ];

        let mut result = Self {
            help_map: IndexMap::new(),
            guide_topics: Vec::new(),
            module_names: Vec::new(),
        };

        for file_contents in guide_files.iter() {
            result.add_help_from_guide(file_contents);
        }

        for file_contents in reference_files.iter() {
            result.add_help_from_reference(file_contents);
        }

        result
    }

    pub fn get_help(&self, search: Option<&str>) -> String {
        match search {
            Some(search) => {
                let search_key = text_to_key(search);
                match self.help_map.get(&search_key) {
                    Some(entry) => {
                        let mut help = format!(
                            "{indent}{name}\n{indent}{underline}{help}",
                            indent = " ".repeat(HELP_INDENT),
                            name = entry.name,
                            underline = "=".repeat(entry.name.len()),
                            help = entry.help
                        );

                        let see_also: Vec<_> = entry
                            .see_also
                            .iter()
                            .chain(self.help_map.iter().filter_map(|(key, search_entry)| {
                                if key.contains(search_key.as_ref())
                                    && !entry.see_also.contains(&search_entry.name)
                                    && search_entry.name != entry.name
                                {
                                    Some(&search_entry.name)
                                } else {
                                    None
                                }
                            }))
                            .collect();

                        if !see_also.is_empty() {
                            help += "

  --------

  See also:";

                            for see_also_entry in see_also.iter() {
                                help.push_str("\n    - ");
                                help.push_str(see_also_entry);
                            }
                        }

                        help
                    }
                    None => {
                        let matches = self
                            .help_map
                            .iter()
                            .filter(|(key, value)| {
                                key.contains(search_key.as_ref())
                                    || value
                                        .keywords
                                        .iter()
                                        .any(|keyword| keyword.contains(search_key.as_ref()))
                            })
                            .collect::<Vec<_>>();

                        match matches.as_slice() {
                            [] => format!("  No matches for '{search}' found."),
                            [(only_match, _)] => self.get_help(Some(only_match)),
                            _ => {
                                let mut help = String::new();
                                help.push_str("  More than one match found: ");
                                for (_, HelpEntry { name, .. }) in matches {
                                    help.push_str("\n    - ");
                                    help.push_str(name);
                                }
                                help
                            }
                        }
                    }
                }
            }
            None => {
                let mut help = "
  To get help for a topic, run `help <topic>` (e.g. `help strings`).

  To look up the core library documentation, run `help <module>.<item>` (e.g. `help map.keys`)."
                    .to_string();

                help.push_str(
                    "

  Help is available for the following language guide topics:",
                );

                for guide_topic in self.guide_topics.iter() {
                    help.push_str("\n    - ");
                    help.push_str(guide_topic);
                }

                help.push_str(
                    "

  Help is available for the following core library modules:",
                );

                for module_name in self.module_names.iter() {
                    help.push_str("\n    - ");
                    help.push_str(module_name);
                }

                help
            }
        }
    }

    fn add_help_from_guide(&mut self, markdown: &str) {
        let mut parser = pulldown_cmark::Parser::new(markdown).peekable();

        // Consume the module overview section
        let topic = consume_help_section(&mut parser, None);
        // We should avoid top-level topics without a body
        debug_assert!(
            !topic.contents.trim().is_empty(),
            "Missing contents for {}",
            topic.name
        );

        // Add sub-topics
        let mut sub_topics = Vec::new();
        while parser.peek().is_some() {
            sub_topics.push(consume_help_section(&mut parser, None));
        }

        let see_also = sub_topics
            .iter()
            .flat_map(|sub_topic| iter::once(&sub_topic.name).chain(sub_topic.sub_sections.iter()))
            .cloned()
            .collect();
        self.help_map.insert(
            text_to_key(&topic.name),
            HelpEntry {
                name: topic.name.clone(),
                help: topic.contents,
                see_also,
                keywords: vec![],
            },
        );
        self.guide_topics.push(topic.name.clone());

        for sub_topic in sub_topics {
            self.help_map.insert(
                text_to_key(&sub_topic.name),
                HelpEntry {
                    name: sub_topic.name,
                    help: sub_topic.contents,
                    keywords: sub_topic
                        .sub_sections
                        .iter()
                        .map(|sub_section| text_to_key(sub_section))
                        .collect(),
                    see_also: vec![topic.name.clone()],
                },
            );
        }
    }

    fn add_help_from_reference(&mut self, markdown: &str) {
        let mut parser = pulldown_cmark::Parser::new(markdown).peekable();

        let help_section = consume_help_section(&mut parser, None);

        // Consume each module entry
        let mut entry_names = Vec::new();
        while parser.peek().is_some() {
            let module_entry = consume_help_section(&mut parser, Some(&help_section.name));
            self.help_map.insert(
                text_to_key(&module_entry.name),
                HelpEntry {
                    name: module_entry.name.clone(),
                    help: module_entry.contents,
                    see_also: Vec::new(),
                    keywords: vec![],
                },
            );
            entry_names.push(module_entry.name);
        }

        if !help_section.contents.trim().is_empty() {
            self.help_map.insert(
                text_to_key(&help_section.name),
                HelpEntry {
                    name: help_section.name.clone(),
                    help: help_section.contents,
                    see_also: entry_names,
                    keywords: vec![],
                },
            );
        }
        self.module_names.push(help_section.name.clone());
    }
}

fn text_to_key(text: &str) -> Rc<str> {
    text.trim().to_lowercase().replace(' ', "_").into()
}

struct HelpSection {
    name: Rc<str>,
    contents: Rc<str>,
    sub_sections: Vec<Rc<str>>,
}

enum ParsingMode {
    Any,
    Section,
    SubSection,
    Code,
    TypeDeclaration,
}

fn consume_help_section(
    parser: &mut Peekable<pulldown_cmark::Parser>,
    module_name: Option<&str>,
) -> HelpSection {
    use pulldown_cmark::{CodeBlockKind, Event::*, HeadingLevel, Tag::*};

    let mut section_level = None;
    let mut section_name = String::new();
    let mut sub_section_name = String::new();
    let mut sub_sections = Vec::new();
    let indent = " ".repeat(HELP_INDENT);
    let mut result = indent.clone();

    let mut list_indent = 0;
    let mut parsing_mode = ParsingMode::Any;

    while let Some(peeked) = parser.peek() {
        match peeked {
            Start(Heading(level, _, _)) => {
                match section_level {
                    Some(HeadingLevel::H1) => {
                        // We've reached the end of the title section, so break out
                        break;
                    }
                    Some(section_level) if section_level >= *level => {
                        // We've reached the end of the section, so break out
                        break;
                    }
                    Some(_) => {
                        // Start a new subsection
                        parsing_mode = ParsingMode::SubSection;
                        sub_section_name.clear();
                        result.push_str("\n\n");
                    }
                    None => {
                        parsing_mode = ParsingMode::Section;
                        section_level = Some(*level);
                    }
                }
            }
            End(Heading(_, _, _)) => {
                if matches!(parsing_mode, ParsingMode::SubSection) {
                    sub_sections.push(sub_section_name.as_str().into());
                    result.push('\n');
                    for _ in 0..sub_section_name.len() {
                        result.push('-');
                    }
                }
                parsing_mode = ParsingMode::Any;
            }
            Start(Link(_type, _url, title)) => result.push_str(title),
            End(Link(_, _, _)) => {}
            Start(List(_)) => {
                if list_indent == 0 {
                    result.push('\n');
                }
                list_indent += 1;
            }
            End(List(_)) => list_indent -= 1,
            Start(Item) => {
                result.push('\n');
                for _ in 1..list_indent {
                    result.push_str("  ");
                }
                result.push_str("- ");
            }
            End(Item) => {}
            Start(Paragraph) => result.push_str("\n\n"),
            End(Paragraph) => {}
            Start(CodeBlock(CodeBlockKind::Fenced(lang))) => {
                result.push_str("\n\n");
                match lang.split(',').next() {
                    Some("koto") => parsing_mode = ParsingMode::Code,
                    Some("kototype") => parsing_mode = ParsingMode::TypeDeclaration,
                    _ => {}
                }
            }
            End(CodeBlock(_)) => parsing_mode = ParsingMode::Any,
            Start(Emphasis) => result.push('_'),
            End(Emphasis) => result.push('_'),
            Start(Strong) => result.push('*'),
            End(Strong) => result.push('*'),
            Text(text) => match parsing_mode {
                ParsingMode::Any => result.push_str(text),
                ParsingMode::Section => section_name.push_str(text),
                ParsingMode::SubSection => {
                    sub_section_name.push_str(text);
                    result.push_str(text);
                }
                ParsingMode::Code => {
                    for (i, line) in text.split('\n').enumerate() {
                        if i == 0 {
                            result.push('|');
                        }
                        result.push_str("\n|  ");
                        let processed_line = line.trim_start_matches("print! ").replacen(
                            "check! ",
                            HELP_RESULT_STR,
                            1,
                        );
                        result.push_str(&processed_line);
                    }
                }
                ParsingMode::TypeDeclaration => {
                    result.push('`');
                    result.push_str(text.trim_end());
                    result.push('`');
                }
            },
            Code(code) => {
                if matches!(parsing_mode, ParsingMode::Section) {
                    section_name.push_str(code);
                } else {
                    result.push('`');
                    result.push_str(code);
                    result.push('`');
                }
            }
            SoftBreak => result.push(' '),
            HardBreak => result.push('\n'),
            _other => {}
        }

        parser.next();
    }

    if let Some(module_name) = module_name {
        section_name = format!("{module_name}.{section_name}");
    }
    let contents = result.replace('\n', &format!("\n{indent}"));

    HelpSection {
        name: section_name.into(),
        contents: contents.into(),
        sub_sections,
    }
}
