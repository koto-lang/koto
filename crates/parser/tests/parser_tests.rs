use koto_parser::{Ast, Error, Parser};
use std::{borrow::Cow, env, fs, path::Path};

const UPDATE_ENV_VAR: &str = "KOTO_UPDATE_PARSER_SNAPSHOTS";
const SNAPSHOT_DIVIDER: &str = "# ---\n";
const AST_SECTION_DIVIDER: &str = "# ---\nconstants:\n";

macro_rules! ast_snapshot_tests {
    (
        $(#[$attr:meta])*
        $module:ident {
            $($test_name:ident,)*
        }
    ) => {
        $(#[$attr])*
        mod $module {
            use super::*;
            $(
                #[test]
                fn $test_name() {
                    const SNAPSHOT_NAME: &str =
                        concat!(stringify!($module), "/", stringify!($test_name));
                    const SNAPSHOT_PATH: &str = concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/snapshots/",
                        stringify!($module),
                        "/",
                        stringify!($test_name),
                        ".ast",
                    );
                    const EXPECTED_SNAPSHOT: &str = include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/snapshots/",
                        stringify!($module),
                        "/",
                        stringify!($test_name),
                        ".ast",
                    ));
                    check_ast_snapshot(
                        SNAPSHOT_NAME,
                        SNAPSHOT_PATH,
                        EXPECTED_SNAPSHOT,
                    )
                }
            )*
        }
    };
}

pub fn check_ast_snapshot(snapshot_name: &str, snapshot_path: &str, expected_snapshot: &str) {
    let expected_snapshot = normalize_line_endings(expected_snapshot);
    let (expected_ast, sources) = parse_snapshot(snapshot_name, &expected_snapshot);
    let expected_error = expected_error(snapshot_name);
    let snapshot_path = Path::new(snapshot_path);

    if env::var(UPDATE_ENV_VAR).is_ok() {
        update_ast_snapshot(snapshot_name, sources, expected_error, snapshot_path);
    } else {
        check_ast_matches_snapshot(
            snapshot_name,
            expected_ast,
            sources,
            expected_error,
            snapshot_path,
        );
    }
}

fn check_ast_matches_snapshot<'a>(
    snapshot_name: &str,
    expected_ast: &str,
    sources: impl Iterator<Item = &'a str>,
    expected_error: bool,
    snapshot_path: &Path,
) {
    let mut found_source = false;

    for source in sources {
        found_source = true;

        let ast = parse_source(source, expected_error);
        let actual_ast = ast.to_string();

        if expected_ast != actual_ast {
            panic!(
                "\
Parser snapshot mismatch: {snapshot_path:?}

Source:
{source}
Expected:
{expected_ast}
Actual:
{actual_ast}
{}\n\
                 Run `just update_parser_snapshots` to update parser snapshots.",
                first_difference(expected_ast, &actual_ast)
            );
        }
    }

    if !found_source {
        panic!("Parser snapshot {snapshot_name:?} doesn't contain any sources");
    }
}

fn update_ast_snapshot<'a>(
    snapshot_name: &str,
    sources: impl Iterator<Item = &'a str>,
    expected_error: bool,
    snapshot_path: &Path,
) {
    let mut actual_snapshot = String::new();
    let actual_ast = render_ast_for_sources(snapshot_name, sources, expected_error, |source| {
        write_source_section(&mut actual_snapshot, source);
    });

    actual_snapshot.push_str(SNAPSHOT_DIVIDER);
    actual_snapshot.push_str(&actual_ast);

    let actual_snapshot = normalize_line_endings(&actual_snapshot);
    fs::write(snapshot_path, actual_snapshot.as_ref())
        .unwrap_or_else(|error| panic!("Failed to update {snapshot_path:?}: {error}"));
}

fn render_ast_for_sources<'a>(
    snapshot_name: &str,
    mut sources: impl Iterator<Item = &'a str>,
    expected_error: bool,
    mut on_source: impl FnMut(&str),
) -> String {
    let first_source = sources
        .next()
        .unwrap_or_else(|| panic!("Parser snapshot {snapshot_name:?} doesn't contain any sources"));

    on_source(first_source);

    let first_ast = parse_source(first_source, expected_error);
    let actual_ast = first_ast.to_string();

    for source in sources {
        on_source(source);

        let ast = parse_source(source, expected_error);
        let actual = ast.to_string();
        assert_eq!(
            actual_ast, actual,
            "Equivalent source didn't produce the expected AST"
        );
    }

    actual_ast
}

fn expected_error(snapshot_name: &str) -> bool {
    cfg_select! {
        feature = "error_ast" => {
            snapshot_name.starts_with("partial_ast_after_error/")
        }
        _ => {
            let _ = snapshot_name;
            false
        }
    }
}

fn normalize_line_endings(s: &str) -> Cow<'_, str> {
    if s.contains('\r') {
        Cow::Owned(s.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

fn parse_source(source: &str, expected_error: bool) -> Ast {
    match Parser::parse(source) {
        Ok(_) if expected_error => panic!("Expected parsing to fail"),
        Ok(ast) => ast,
        Err(error) if expected_error => cfg_select! {
            feature = "error_ast" => {
                partial_ast(error)
            }
            _ => {
                panic_for_error(error)
            }
        },
        Err(error) => panic_for_error(error),
    }
}

#[cfg(feature = "error_ast")]
fn partial_ast(error: Error) -> Ast {
    match error.ast {
        Some(ast) => *ast,
        None => panic!("Missing AST after error ({error})"),
    }
}

fn parse_snapshot<'a>(
    snapshot_name: &str,
    snapshot: &'a str,
) -> (&'a str, impl Iterator<Item = &'a str> + 'a) {
    let ast_start = snapshot
        .rfind(AST_SECTION_DIVIDER)
        .unwrap_or_else(|| panic!("Parser snapshot {snapshot_name:?} doesn't contain an AST"));
    let sources = snapshot[..ast_start]
        .split(SNAPSHOT_DIVIDER)
        .skip(1)
        .filter(|section| !section.is_empty());
    let expected_ast = &snapshot[ast_start + SNAPSHOT_DIVIDER.len()..];

    (expected_ast, sources)
}

fn write_source_section(result: &mut String, source: &str) {
    result.push_str(SNAPSHOT_DIVIDER);
    result.push_str(source);
    if !source.ends_with('\n') {
        result.push('\n');
    }
}

fn first_difference(expected: &str, actual: &str) -> String {
    let mut expected_lines = expected.split('\n').peekable();
    let mut actual_lines = actual.split('\n').peekable();

    while expected_lines.peek().is_some() || actual_lines.peek().is_some() {
        match (expected_lines.next(), actual_lines.next()) {
            (Some(expected), Some(actual)) if expected == actual => continue,
            (Some(expected), Some(actual)) => {
                return format!(
                    "\
First difference:
  expected: {expected}
  actual:   {actual}"
                );
            }
            (Some(expected), None) => {
                return format!(
                    "\
First difference:
  expected: {expected}
  actual:   <end of file>"
                );
            }
            (None, Some(actual)) => {
                return format!(
                    "\
First difference:
  expected: <end of file>
  actual:   {actual}"
                );
            }
            (None, None) => break,
        }
    }

    unreachable!("Expected to find a difference between the inputs")
}

fn panic_for_error(error: Error) -> ! {
    panic!("{error} - {:?}", error.span.start)
}

mod parser {
    use super::*;

    ast_snapshot_tests! {
        values {
            literals,
            number_notation,
            multiline_strings,
            strings_with_escape_codes,
            strings_with_interpolated_ids,
            string_with_interpolated_expression,
            string_with_formatted_expression,
            raw_strings,
            negatives,
        }
    }

    ast_snapshot_tests! {
        lists {
            basic_lists,
            nested_list,
            list_with_line_breaks,
        }
    }

    ast_snapshot_tests! {
        maps {
            maps_with_braces,
            map_block_first_entry_with_string_key,
            map_block_first_entry_is_nested_map_block,
            map_block_first_entry_is_comma_separated_tuple,
            map_block_second_entry_is_paren_free_call,
            map_block_meta,
        }
    }

    ast_snapshot_tests! {
        ranges {
            ranges_from_literals,
            range_from_expressions,
            range_from_values,
            range_from_chains,
            ranges_in_lists,
            ranges_in_tuple,
        }
    }

    ast_snapshot_tests! {
        tuples {
            tuple,
            tuple_with_missing_value,
            empty_parentheses,
            single_comma,
            two_commas,
            nested_empty_tuple,
            empty_tuple_inside_tuple,
            single_entry_tuple,
            tuple_in_parens,
        }
    }

    ast_snapshot_tests! {
        assignment {
            single,
            tuple,
            tuple_of_tuples,
            unpack_tuple,
            tuple_with_linebreaks,
            multi_1_to_3_with_ignored_ids,
            compound_assignment,
            list_with_chain_as_first_element,
            map_one,
            map_two,
            map_multiple,
            map_with_as,
            map_ignored_key,
        }
    }

    ast_snapshot_tests! {
        let_expression {
            number,
            number_with_type_hint,
            string_with_optional_type_hint,
            multiple_targets,
            ignored_number_with_type_hint,
            number_with_ignored_id_and_type_hint,
            multi_1_to_3_with_ignored_ids_and_type_hint,
            map_one,
            map_two,
            map_multiple,
            map_with_type_hint,
            map_with_as,
            map_with_type,
            map_with_type_optional,
            map_with_as_and_type,
            map_with_as_and_type_optional,
            map_ignored_key,
        }
    }

    ast_snapshot_tests! {
        export {
            export_assignment,
            export_multi_assignment,
            export_map_block,
        }
    }

    ast_snapshot_tests! {
        arithmetic {
            addition_subtraction,
            add_multiply,
            with_parentheses,
            divide_then_remainder_with_power,
            string_and_id,
            function_call_on_rhs,
            arithmetic_assignment_chained,
            arithmetic_assignment_with_nested_expression,
        }
    }

    ast_snapshot_tests! {
        logic {
            and_or,
            chained_comparisons,
        }
    }

    ast_snapshot_tests! {
        control_flow {
            if_inline,
            if_block,
            if_inline_multi_expressions,
        }
    }

    ast_snapshot_tests! {
        loops {
            for_loop,
            while_loop,
            until_loop,
            for_loop_after_array,
            for_with_range_from_chain_call,
            for_with_unpacked_map,
        }
    }

    ast_snapshot_tests! {
        functions {
            two_args_with_type_hints,
            output_type_hint,
            with_body,
            call_without_parentheses,
            call_with_map_block,
            call_with_trailing_map_block,
            call_with_parentheses,
            call_negative_arg,
            call_arithmetic_arg,
            call_packed_arg_without_parentheses,
            call_packed_arg_with_parentheses,
            recursive_call,
            piped_call_chain,
            indented_piped_calls_after_chain,
            generator_function,
            async_function,
            unpack_call_args,
            multiline_comment_before_function_with_default_arg,
            unpacked_map_argument,
        }
    }

    ast_snapshot_tests! {
        chains {
            indexed_assignment,
            index_range_full,
            index_range_to,
            index_range_from_and_sub_index,
            access_with_id,
            access_with_call,
            access_call_arithmetic_arg,
            access_assignment,
            access_space_separated_call,
            access_indentation_separated_call,
            chain_indentation_separated_with_map_arg,
            map_access_in_list,
            chain_on_call_result,
            index_on_call_result,
            call_on_call_result,
            chain_on_number,
            chain_on_string,
            chain_on_tuple,
            chain_on_list,
            chain_on_map,
            chain_on_range_same_line,
            chain_on_range_next_line,
            nested_chain_call,
            multiline_chain,
            chain_followed_by_continued_expression_on_next_line,
            null_checks_after_root,
            null_checks_between_calls,
            null_checks_before_paren_free_call,
        }
    }

    ast_snapshot_tests! {
        keywords {
            flow,
            keywords_with_args,
        }
    }

    ast_snapshot_tests! {
        semicolons {
            separated_expressions_on_same_line,
            separated_expressions_in_block,
        }
    }

    ast_snapshot_tests! {
        import {
            import_single_item,
            import_item_as,
            import_from_module,
            wildcard_import,
            import_item_used_in_assignment,
            import_multiple_items,
            import_items_from,
            import_nested_items,
        }
    }

    ast_snapshot_tests! {
        error_handling {
            try_catch_with_type_hints,
            throw_value,
            throw_string,
            throw_map,
        }
    }

    ast_snapshot_tests! {
        match_and_switch {
            assign_from_match_with_alternative_patterns,
            match_string_literals,
            match_with_type_pattern,
            match_tuple,
            match_tuple_subslice,
            match_tuple_subslice_with_id,
            match_multi_expression,
            match_pattern_is_chain,
            match_map,
            match_map_with_type,
            match_map_multi_line,
            switch_expression,
            switch_arm_is_debug_expression,
        }
    }

    #[cfg(feature = "error_ast")]
    ast_snapshot_tests! {
        partial_ast_after_error {
            after_assign,
            mid_assignment_on_second_line,
            error_in_function,
        }
    }
}
