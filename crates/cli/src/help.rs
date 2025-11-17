use indexmap::IndexMap;
use pulldown_cmark::HeadingLevel;
use std::{
    iter::{self, Peekable},
    rc::Rc,
};

use crate::wrap_string_with_indent;

const HELP_RESULT_STR: &str = "➝ ";
pub const HELP_INDENT: &str = "  ";

macro_rules! include_doc {
    ($doc:expr) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/", $doc))
    };
}

pub struct HelpEntry {
    // The entry's user-displayed name
    pub name: Rc<str>,
    // The entry's contents
    pub help: Rc<str>,
    // Additional keywords that should be checked when searching
    pub keywords: Vec<Rc<str>>,
    // Names of related topics to show in the 'See also' section
    pub see_also: Vec<Rc<str>>,
}

pub struct Help {
    // All help entries, keys are lower_snake_case
    help_map: IndexMap<Rc<str>, HelpEntry>,
    // The list of guide topics
    guide_topics: Vec<Rc<str>>,
    // The list of core library module names
    core_lib_names: Vec<Rc<str>>,
    // The list of extra module names
    extra_lib_names: Vec<Rc<str>>,
}

impl Help {
    pub fn new() -> Self {
        let mut result = Self {
            help_map: IndexMap::new(),
            guide_topics: Vec::new(),
            core_lib_names: Vec::new(),
            extra_lib_names: Vec::new(),
        };

        result.add_help_from_guide();

        let core_lib_files = [
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
        for file_contents in core_lib_files.iter() {
            let module_name = result.add_help_from_reference(file_contents);
            result.core_lib_names.push(module_name);
        }

        let extra_lib_files = [
            include_doc!("libs/color.md"),
            include_doc!("libs/geometry.md"),
            include_doc!("libs/json.md"),
            include_doc!("libs/random.md"),
            include_doc!("libs/regex.md"),
            include_doc!("libs/tempfile.md"),
            include_doc!("libs/toml.md"),
            include_doc!("libs/yaml.md"),
        ];
        for file_contents in extra_lib_files.iter() {
            let module_name = result.add_help_from_reference(file_contents);
            result.extra_lib_names.push(module_name);
        }
        result
    }

    pub fn topics(&self) -> impl Iterator<Item = Rc<str>> {
        self.core_lib_names
            .iter()
            .chain(self.extra_lib_names.iter())
            .chain(self.guide_topics.iter())
            .cloned()
    }

    pub fn all_entries(&self) -> impl Iterator<Item = (&Rc<str>, &HelpEntry)> {
        self.help_map.iter()
    }

    pub fn get_help(&self, search: Option<&str>) -> String {
        match search {
            Some(search) => {
                let search_key = text_to_key(search);
                match self.help_map.get(&search_key) {
                    Some(entry) => {
                        let mut help = format!(
                            "{name}\n{underline}{help}",
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

                            let item_prefix = format!("\n{HELP_INDENT}- ");
                            for see_also_entry in see_also.iter() {
                                help.push_str(&item_prefix);
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
                            [] => format!("No matches for '{search}' found."),
                            [(only_match, _)] => self.get_help(Some(only_match)),
                            _ => {
                                let mut help = String::new();
                                help.push_str("More than one match found: ");
                                let item_prefix = format!("\n{HELP_INDENT}- ");
                                for (_, HelpEntry { name, .. }) in matches {
                                    help.push_str(&item_prefix);
                                    help.push_str(name);
                                }
                                help
                            }
                        }
                    }
                }
            }
            None => {
                let mut help = String::new();

                help.push_str(
                    "
To get help, run 'help <topic>', e.g. 'help strings', or 'help map.keys'.

Tab completion can be used to browse available topics, \
e.g. pressing tab twice after 'help io.' will bring up a list of io module items.

A rendered version of the help docs can also be found here: https://koto.dev/docs

Help is available for the following topics:",
                );

                let topics_indent = HELP_INDENT.repeat(2);
                let mut show_topics = |topic: &str, topics: &[Rc<str>]| {
                    let mut topics_string = String::new();
                    for (i, topic) in topics.iter().enumerate() {
                        if i > 0 {
                            topics_string.push_str(", ");
                        }
                        topics_string.push_str(topic);
                    }

                    help.push_str(&format!(
                        "
{HELP_INDENT}{topic}:
{}
",
                        wrap_string_with_indent(&topics_string, &topics_indent)
                    ));
                };

                show_topics("core library modules", &self.core_lib_names);
                show_topics("additional modules", &self.extra_lib_names);
                show_topics("language guide", &self.guide_topics);

                help
            }
        }
    }

    fn add_help_from_guide(&mut self) {
        let guide_contents = include_doc!("language_guide.md");
        let mut parser = pulldown_cmark::Parser::new(guide_contents).peekable();

        // Skip the guide intro
        consume_help_section(&mut parser, None, HeadingLevel::H1, false);

        while parser.peek().is_some() {
            // Consume the module overview section
            let topic = consume_help_section(&mut parser, None, HeadingLevel::H2, false);
            // We should avoid top-level topics without a body
            debug_assert!(
                !topic.contents.trim().is_empty(),
                "Missing contents for {}",
                topic.name
            );

            // Add sub-topics
            let mut sub_topics = Vec::new();
            loop {
                let sub_topic = consume_help_section(&mut parser, None, HeadingLevel::H3, true);
                if sub_topic.contents.trim().is_empty() {
                    break;
                }
                sub_topics.push(sub_topic);
            }

            let see_also = sub_topics
                .iter()
                .flat_map(|sub_topic| {
                    iter::once(&sub_topic.name).chain(sub_topic.sub_sections.iter())
                })
                .cloned()
                .collect();
            let topic_name = self.add_to_help(topic.name, topic.contents, see_also, vec![]);
            self.guide_topics.push(topic_name.clone());

            for sub_topic in sub_topics {
                let keywords = sub_topic
                    .sub_sections
                    .iter()
                    .map(|sub_section| text_to_key(sub_section))
                    .collect();

                self.add_to_help(
                    sub_topic.name,
                    sub_topic.contents,
                    vec![topic_name.clone()],
                    keywords,
                );
            }
        }
    }

    fn add_help_from_reference(&mut self, markdown: &str) -> Rc<str> {
        let mut parser = pulldown_cmark::Parser::new(markdown).peekable();

        let help_section = consume_help_section(&mut parser, None, HeadingLevel::H1, false);

        // Consume each module entry
        let mut entry_names = Vec::new();
        while parser.peek().is_some() {
            let module_entry = consume_help_section(
                &mut parser,
                Some(&help_section.name),
                HeadingLevel::H2,
                true,
            );
            let module_name =
                self.add_to_help(module_entry.name, module_entry.contents, vec![], vec![]);
            entry_names.push(module_name);
        }

        if !help_section.contents.trim().is_empty() {
            self.add_to_help(
                help_section.name.clone(),
                help_section.contents,
                entry_names,
                vec![],
            );
        }

        help_section.name
    }

    // Adds an entry to the help map, ensuring a unique name and key.
    //
    // The (possibly adjusted) user-facing name is returned.
    fn add_to_help(
        &mut self,
        name: Rc<str>,
        contents: Rc<str>,
        see_also: Vec<Rc<str>>,
        keywords: Vec<Rc<str>>,
    ) -> Rc<str> {
        let key = text_to_key(&name);

        if !self.help_map.contains_key(&key) {
            self.help_map.insert(
                key,
                HelpEntry {
                    name: name.clone(),
                    help: contents,
                    see_also,
                    keywords,
                },
            );
            name.clone()
        } else {
            // An entry with this key already exists, so find a unique key by appending an index.
            let mut attempt = 2;
            loop {
                let key: Rc<str> = format!("{key}_{attempt}").into();

                if !self.help_map.contains_key(&key) {
                    let name: Rc<str> = format!("{name} ({attempt})").into();

                    self.help_map.insert(
                        key,
                        HelpEntry {
                            name: name.clone(),
                            help: contents,
                            see_also,
                            keywords,
                        },
                    );

                    return name;
                }
                attempt += 1;
            }
        }
    }
}

fn text_to_key(text: &str) -> Rc<str> {
    text.chars()
        .filter_map(|c| match c {
            ' ' => Some('_'),
            '(' | ')' => None,
            c if c.is_whitespace() => None,
            c => Some(c),
        })
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .into()
}

struct HelpSection {
    name: Rc<str>,
    contents: Rc<str>,
    sub_sections: Vec<Rc<str>>,
}

#[derive(Debug)]
enum ParsingMode {
    WaitingForSectionStart,
    Any,
    Section,
    SubSection,
    Code,
    TypeDeclaration,
}

// Consumes a section of content between headers
//
// - If the title section is being consumed, then the function will break out at the first
//   sub-header.
// - If a sub-section is being consumed, then
fn consume_help_section(
    parser: &mut Peekable<pulldown_cmark::Parser>,
    module_name: Option<&str>,
    level_to_consume: HeadingLevel,
    include_sub_sections: bool,
) -> HelpSection {
    use pulldown_cmark::{CodeBlockKind, Event::*, Tag, TagEnd};

    let mut section_name = String::new();
    let mut sub_section_name = String::new();
    let mut sub_sections = Vec::new();
    let mut result = HELP_INDENT.to_string();

    let mut list_indent = 0;
    let mut parsing_mode = ParsingMode::WaitingForSectionStart;

    while let Some(peeked) = parser.peek() {
        match peeked {
            Start(Tag::Heading { level, .. }) => {
                use std::cmp::Ordering::*;
                let waiting_for_start = matches!(parsing_mode, ParsingMode::WaitingForSectionStart);
                match level.cmp(&level_to_consume) {
                    Less => {
                        break;
                    }
                    Equal => {
                        if waiting_for_start {
                            parsing_mode = ParsingMode::Section;
                        } else {
                            break;
                        }
                    }
                    Greater => {
                        if waiting_for_start {
                            // Continue consuming until the start of the section is found
                        } else if include_sub_sections {
                            // Start a new subsection
                            parsing_mode = ParsingMode::SubSection;
                            sub_section_name.clear();
                            result.push_str("\n\n");
                        } else {
                            break;
                        }
                    }
                }
            }
            End(TagEnd::Heading(_)) => {
                if matches!(parsing_mode, ParsingMode::SubSection) {
                    sub_sections.push(sub_section_name.as_str().into());
                    result.push('\n');
                    for _ in 0..sub_section_name.len() {
                        result.push('-');
                    }
                }
                parsing_mode = ParsingMode::Any;
            }
            Start(Tag::Link { title, .. }) => result.push_str(title),
            End(TagEnd::Link) => {}
            Start(Tag::List(_)) => {
                if list_indent == 0 {
                    result.push('\n');
                }
                list_indent += 1;
            }
            End(TagEnd::List(_)) => list_indent -= 1,
            Start(Tag::Item) => {
                result.push('\n');
                for _ in 1..list_indent {
                    result.push_str("  ");
                }
                result.push_str("- ");
            }
            End(TagEnd::Item) => {}
            Start(Tag::Paragraph) => result.push_str("\n\n"),
            End(TagEnd::Paragraph) => {}
            Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                result.push_str("\n\n");
                match lang.split(',').next() {
                    Some("koto") => parsing_mode = ParsingMode::Code,
                    Some("kototype") => parsing_mode = ParsingMode::TypeDeclaration,
                    _ => {}
                }
            }
            End(TagEnd::CodeBlock) => parsing_mode = ParsingMode::Any,
            Start(Tag::Emphasis) => result.push('_'),
            End(TagEnd::Emphasis) => result.push('_'),
            Start(Tag::Strong) => result.push('*'),
            End(TagEnd::Strong) => result.push('*'),
            Text(text) => match parsing_mode {
                ParsingMode::WaitingForSectionStart => {}
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
            Code(code) => match parsing_mode {
                ParsingMode::Section => {
                    section_name.push_str(code);
                }
                ParsingMode::SubSection => {
                    sub_section_name.push_str(code);
                    result.push_str(code);
                }
                _ => {
                    result.push('`');
                    result.push_str(code);
                    result.push('`');
                }
            },
            SoftBreak => result.push(' '),
            HardBreak => result.push('\n'),
            _other => {}
        }

        parser.next();
    }

    if let Some(module_name) = module_name {
        section_name = format!("{module_name}.{section_name}");
    }

    HelpSection {
        name: section_name.into(),
        contents: result.into(),
        sub_sections,
    }
}
