use {indexmap::IndexMap, std::iter::Peekable};

const HELP_RESULT_STR: &str = "# ➝ ";
const HELP_INDENT: usize = 2;

struct HelpEntry {
    name: String,
    help: String,
}

pub struct Help {
    help_map: IndexMap<String, HelpEntry>,
    module_names: Vec<String>,
}

impl Help {
    pub fn new() -> Self {
        macro_rules! include_doc {
            ($doc:expr) => {
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/", $doc))
            };
        }

        let guide_files = [
            include_doc!("language/basics.md"),
            include_doc!("language/conditional_expressions.md"),
            include_doc!("language/core_library.md"),
            include_doc!("language/errors.md"),
            include_doc!("language/functions.md"),
            include_doc!("language/generators.md"),
            include_doc!("language/iterators.md"),
            include_doc!("language/lists.md"),
            include_doc!("language/loops.md"),
            include_doc!("language/maps.md"),
            include_doc!("language/meta_maps.md"),
            include_doc!("language/modules.md"),
            include_doc!("language/packed_numbers.md"),
            include_doc!("language/ranges.md"),
            include_doc!("language/strings.md"),
            include_doc!("language/testing.md"),
            include_doc!("language/tuples.md"),
            include_doc!("language/value_unpacking.md"),
        ];

        let reference_files = [
            include_doc!("core_lib/io.md"),
            include_doc!("core_lib/iterator.md"),
            include_doc!("core_lib/koto.md"),
            include_doc!("core_lib/list.md"),
            include_doc!("core_lib/map.md"),
            include_doc!("core_lib/number.md"),
            include_doc!("core_lib/num2.md"),
            include_doc!("core_lib/num4.md"),
            include_doc!("core_lib/os.md"),
            include_doc!("core_lib/range.md"),
            include_doc!("core_lib/string.md"),
            include_doc!("core_lib/test.md"),
            include_doc!("core_lib/tuple.md"),
        ];

        let mut result = Self {
            help_map: IndexMap::new(),
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
                let search_lower = search.trim().to_lowercase();
                match self.help_map.get(&search_lower) {
                    Some(entry) => {
                        let mut help = format!(
                            "{indent}{name}\n{indent}{underline}{help}",
                            indent = " ".repeat(HELP_INDENT),
                            name = entry.name,
                            underline = "=".repeat(entry.name.len()),
                            help = entry.help
                        );

                        let sub_match = format!("{}.", search_lower);
                        let match_level = sub_match.chars().filter(|&c| c == '.').count();
                        let sub_entries = self
                            .help_map
                            .iter()
                            .filter(|(key, _)| {
                                key.starts_with(&sub_match)
                                    && key.chars().filter(|&c| c == '.').count() <= match_level
                            })
                            .collect::<Vec<_>>();

                        if !sub_entries.is_empty() {
                            let sub_entry_prefix = format!("{}.", entry.name);
                            help += "

  --------

  Help is available for the following sub-topics:\n    ";

                            for (i, (_, sub_entry)) in sub_entries.iter().enumerate() {
                                if i > 0 {
                                    help.push_str(", ");
                                }

                                help.push_str(
                                    sub_entry.name.strip_prefix(&sub_entry_prefix).unwrap(),
                                );
                            }
                        }

                        help
                    }
                    None => {
                        let matches = self
                            .help_map
                            .iter()
                            .filter(|(key, _)| key.contains(&search_lower))
                            .collect::<Vec<_>>();

                        match matches.as_slice() {
                            [] => format!("  No matches for '{search}' found."),
                            [(only_match, _)] => self.get_help(Some(only_match)),
                            _ => {
                                let mut help = String::new();
                                help.push_str("  More than one match found: ");
                                for (_, HelpEntry { name, .. }) in matches {
                                    help.push_str("\n    ");
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

  To look up the core library documentation, run `help <module>.<item>` (e.g. `help map.keys`).

  Help is available for the following topics:
    "
                .to_string();

                let mut topics = self
                    .help_map
                    .keys()
                    // Filter out core library entries
                    .filter(|key| !key.contains('.') && !self.module_names.contains(key))
                    .collect::<Vec<_>>();
                // Sort the topics into alphabetical order
                topics.sort();
                // Bump topics starting with non-alphnumeric characters to the end of the list
                topics.sort_by_key(|name| !name.chars().next().unwrap().is_alphanumeric());

                for (i, topic) in topics.iter().enumerate() {
                    if i > 0 {
                        help.push_str(", ");
                    }

                    help.push_str(topic);
                }

                help.push_str(
                    "

  Help is available for the following core library modules:
    ",
                );

                for (i, module_name) in self.module_names.iter().enumerate() {
                    if i > 0 {
                        help.push_str(", ");
                    }

                    help.push_str(module_name);
                }

                help
            }
        }
    }

    fn add_help_from_guide(&mut self, markdown: &str) {
        let mut parser = pulldown_cmark::Parser::new(markdown).peekable();

        // Consume the module overview section
        let (file_name, help) = consume_help_section(&mut parser, None);
        if !help.trim().is_empty() {
            self.help_map.insert(
                file_name.to_lowercase().replace(' ', "_"),
                HelpEntry {
                    name: file_name,
                    help,
                },
            );
        }

        // Add sub-topics
        while parser.peek().is_some() {
            let (entry_name, help) = consume_help_section(&mut parser, None);
            self.help_map.insert(
                entry_name.to_lowercase().replace(' ', "_"),
                HelpEntry {
                    name: entry_name,
                    help,
                },
            );
        }
    }

    fn add_help_from_reference(&mut self, markdown: &str) {
        let mut parser = pulldown_cmark::Parser::new(markdown).peekable();

        let (module_name, help) = consume_help_section(&mut parser, None);
        if !help.trim().is_empty() {
            self.help_map.insert(
                module_name.clone(),
                HelpEntry {
                    name: module_name.clone(),
                    help,
                },
            );
        }
        self.module_names.push(module_name.clone());

        // Consume each module entry
        while parser.peek().is_some() {
            let (entry_name, help) = consume_help_section(&mut parser, Some(&module_name));
            self.help_map.insert(
                entry_name.to_lowercase(),
                HelpEntry {
                    name: entry_name,
                    help,
                },
            );
        }
    }
}

fn consume_help_section<'a, 'b>(
    parser: &mut Peekable<pulldown_cmark::Parser<'a, 'b>>,
    module_name: Option<&str>,
) -> (String, String) {
    use pulldown_cmark::{CodeBlockKind, Event::*, HeadingLevel, Tag::*};

    let mut section_level = None;
    let mut section_name = String::new();
    let indent = " ".repeat(HELP_INDENT);
    let mut result = indent.clone();

    let mut list_indent = 0;
    let mut in_section_heading = false;
    let mut heading_start = 0;
    let mut first_heading = true;
    let mut in_koto_code = false;
    let mut in_type_declaration = false;

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
                        result.push_str("\n\n");
                    }
                    None => {
                        in_section_heading = true;
                        section_level = Some(*level);
                    }
                }
                heading_start = result.len();
            }
            End(Heading(_, _, _)) => {
                if !first_heading {
                    let heading_length = result.len() - heading_start;
                    result.push('\n');
                    for _ in 0..heading_length {
                        result.push('-');
                    }
                }
                in_section_heading = false;
                first_heading = false;
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
                    Some("koto") => in_koto_code = true,
                    Some("kototype") => in_type_declaration = true,
                    _ => {}
                }
            }
            End(CodeBlock(_)) => {
                in_koto_code = false;
                in_type_declaration = false;
            }
            Start(Emphasis) => result.push('_'),
            End(Emphasis) => result.push('_'),
            Start(Strong) => result.push('*'),
            End(Strong) => result.push('*'),
            Text(text) => {
                if in_section_heading {
                    section_name.push_str(text);
                } else if in_koto_code {
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
                } else if in_type_declaration {
                    result.push('`');
                    result.push_str(text.trim_end());
                    result.push('`');
                } else {
                    result.push_str(text);
                }
            }
            Code(code) => {
                if in_section_heading {
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
    let result = result.replace('\n', &format!("\n{indent}"));

    (section_name, result)
}
