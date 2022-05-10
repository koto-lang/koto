use {indexmap::IndexMap, std::iter::Peekable, std::ops::Deref};

struct HelpEntry {
    name: String,
    help: String,
}

pub struct Help {
    map: IndexMap<String, HelpEntry>,
}

impl Help {
    pub fn new() -> Self {
        let mut result = Self {
            map: IndexMap::new(),
        };

        let help_modules = [
            include_str!("docs/reference/core_lib/io.md"),
            include_str!("docs/reference/core_lib/iterator.md"),
            include_str!("docs/reference/core_lib/koto.md"),
            include_str!("docs/reference/core_lib/list.md"),
            include_str!("docs/reference/core_lib/map.md"),
            include_str!("docs/reference/core_lib/number.md"),
            include_str!("docs/reference/core_lib/num2.md"),
            include_str!("docs/reference/core_lib/num4.md"),
            include_str!("docs/reference/core_lib/os.md"),
            include_str!("docs/reference/core_lib/range.md"),
            include_str!("docs/reference/core_lib/string.md"),
            include_str!("docs/reference/core_lib/test.md"),
            include_str!("docs/reference/core_lib/tuple.md"),
        ];

        for module in help_modules.iter() {
            result.add_help_from_markdown(module);
        }

        result
    }

    pub fn get_help(&self, search: Option<&str>) -> String {
        match search {
            Some(search) => {
                let search_lower = search.trim().to_lowercase();
                match self.map.get(&search_lower) {
                    Some(entry) => {
                        let mut help = entry.help.clone();

                        let sub_match = format!("{}.", search_lower);
                        let match_level = sub_match.chars().filter(|&c| c == '.').count();
                        let sub_entries = self
                            .map
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
                            .map
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
  To get help for a module, run `help <module>` (e.g. `help map`),
  or for a module item, run `help <module>.<item>` (e.g. `help map.keys`).

  Help is available for the following modules:
    "
                .to_string();

                for (i, entry) in self
                    .map
                    .values()
                    .filter(|HelpEntry { name, .. }| !name.contains('.'))
                    .enumerate()
                {
                    if i > 0 {
                        help.push_str(", ");
                    }

                    help.push_str(&entry.name);
                }

                help
            }
        }
    }

    fn add_help_from_markdown(&mut self, markdown: &str) {
        use pulldown_cmark::{Event, Parser, Tag};

        let mut parser = Parser::new(markdown).peekable();

        // Consume the module overview section
        let (module_name, help) = consume_help_section(&mut parser, None);
        self.map.insert(
            module_name.to_lowercase(),
            HelpEntry {
                name: module_name.clone(),
                help,
            },
        );

        // Skip ahead until the first reference subsection is found
        while let Some(peeked) = parser.peek() {
            if matches!(peeked, Event::Start(Tag::Heading(2))) {
                break;
            }
            parser.next();
        }

        // Consume each module entry
        while parser.peek().is_some() {
            let (entry_name, help) = consume_help_section(&mut parser, Some(&module_name));
            self.map.insert(
                entry_name.to_lowercase(),
                HelpEntry {
                    name: entry_name,
                    help,
                },
            );
        }
    }
}

fn consume_help_section<'a>(
    parser: &mut Peekable<pulldown_cmark::Parser<'a>>,
    module_name: Option<&str>,
) -> (String, String) {
    use pulldown_cmark::{CodeBlockKind, Event::*, Tag::*};

    let mut section_level = None;
    let mut section_name = String::new();
    let indent = " ".repeat(2);
    let mut result = indent.clone();

    let mut list_indent = 0;
    let mut heading_start = 0;
    let mut first_heading = true;
    let mut in_koto_code = false;
    let mut in_type_declaration = false;

    while let Some(peeked) = parser.peek() {
        match peeked {
            Start(Heading(level)) => {
                match section_level {
                    Some(section_level) if section_level >= *level => {
                        // We've reached the end of the section, so break out
                        break;
                    }
                    Some(_) => {
                        // Start a new subsection
                        result.push_str("\n\n");
                    }
                    None => section_level = Some(*level),
                }
                heading_start = result.len();
            }
            End(Heading(_)) => {
                let heading_length = result.len() - heading_start;
                result.push('\n');
                let heading_underline = if first_heading { "=" } else { "-" };
                for _ in 0..heading_length {
                    result.push_str(heading_underline)
                }
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
                match lang.deref() {
                    "koto" => in_koto_code = true,
                    "kototype" => in_type_declaration = true,
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
                if section_name.is_empty() {
                    if let Some(module_name) = module_name {
                        section_name = format!("{module_name}.{text}");
                    } else {
                        section_name = text.to_string();
                    }
                    result.push_str(&section_name);
                } else if in_koto_code {
                    for (i, line) in text.split('\n').enumerate() {
                        if i == 0 {
                            result.push('|');
                        }
                        result.push_str("\n|  ");
                        if !line.starts_with("skip_check!") {
                            let processed_line = line
                                .trim_start_matches("print! ")
                                .replacen("check! ", "# ", 1);
                            result.push_str(&processed_line);
                        }
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
                result.push('`');
                if section_name.is_empty() {
                    if let Some(module_name) = module_name {
                        section_name = format!("{module_name}.{code}");
                    } else {
                        section_name = code.to_string();
                    }
                    result.push_str(&section_name);
                } else {
                    result.push_str(code);
                }
                result.push('`');
            }
            SoftBreak => result.push(' '),
            HardBreak => result.push('\n'),
            _other => {}
        }

        parser.next();
    }

    let result = result.replace('\n', &format!("\n{indent}"));
    (section_name, result)
}
