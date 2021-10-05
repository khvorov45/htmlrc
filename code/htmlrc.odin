package htmlrc

import "core:fmt"
import "core:log"
import "core:os"
import "core:intrinsics"
import "core:path/filepath"
import "core:strings"
import "core:unicode"
import "core:unicode/utf8"

main :: proc() {
    context.user_ptr = &Context_Data{};
    context.logger.procedure = logger_proc
    context.logger.data = &Logger_Data{};
    when !ODIN_DEBUG do context.logger.lowest_level = log.Level.Info
    begin_timed_section(Timed_Section.Whole_Program)

    //
    // SECTION Parse arguments
    //

    // TODO(sen) Better argument parsing
    Args :: struct {
        input_file: string,
        output_dir: string,
    }
    default_args :: proc() -> Args {
        return Args{input_file="", output_dir="out"}
    }
    args := default_args()
    if len(os.args) == 1 || os.args[1] == "--help" || os.args[1] == "-help" || os.args[1] == "help" || os.args[1] == "-h" {
        log.infof("USAGE: htmlrc <input> <output>\n\ninput: an html file\noutput: a directory (default: '%s')", args.output_dir)
        return
    }
    args.input_file = os.args[1]
    if len(os.args) == 3 do args.output_dir = os.args[2]

    //
    // SECTION Validate arguments
    //

    format_os_error :: proc(err: os.Errno) -> string {
        switch err {
            case ERROR_FILE_NOT_FOUND: return "entry does not exist"
            case: return fmt.tprint("error code", err)
        }
    }

    // NOTE(sen) Make sure input exists
    input_file_handle, input_open_error := os.open(args.input_file)
    if input_open_error != os.ERROR_NONE {
        log.errorf("failed to open input file '%s': %s", args.input_file, format_os_error(input_open_error))
        return
    }
    os.close(input_file_handle)

    // NOTE(sen) Make sure input is a file
    input_file_stat, input_stat_error := os.stat(args.input_file, context.temp_allocator)
    if input_stat_error != os.ERROR_NONE {
        log.errorf("failed to scan input file '%s': %s", args.input_file, format_os_error(input_stat_error))
        return
    }
    if input_file_stat.is_dir {
        log.errorf("input '%s' should be a file, not a directory", args.output_dir)
        return
    }

    // NOTE(sen) Make sure output exists
    output_dir_handle, output_open_error := os.open(args.output_dir)
    if output_open_error == ERROR_FILE_NOT_FOUND {
        make_dir_err := make_directory(args.output_dir)
        if make_dir_err != os.ERROR_NONE {
            log.errorf("failed to create output dir '%s': %s", args.output_dir, format_os_error(make_dir_err))
            return
        }
        output_dir_handle, output_open_error = os.open(args.output_dir)
    }
    if output_open_error != os.ERROR_NONE {
        log.errorf("failed to open output dir '%s': %s", args.output_dir, format_os_error(output_open_error))
        return
    }
    os.close(output_dir_handle)

    // NOTE(sen) Make sure output is a directory
    output_dir_stat, output_stat_error := os.stat(args.output_dir, context.temp_allocator)
    if output_stat_error != os.ERROR_NONE {
        log.errorf("failed to scan output dir '%s': %s", args.output_dir, format_os_error(output_stat_error))
        return
    }
    if !output_dir_stat.is_dir {
        log.errorf("output '%s' is not a directory", args.output_dir)
        return
    }

    //
    // SECTION Read input, collect and expand macros, write output
    //

    input_contents, input_read_success := os.read_entire_file(args.input_file)
    if !input_read_success {
        log.errorf("failed to read input file '%s'", args.input_file)
        return
    }
    log.debugf("read input '%s'", args.input_file)

    log.debugf("collecting macros from '%s'", args.input_file)
    input_no_macros, macros, collection_success := collect_macros(string(input_contents))
    if !collection_success do return
    delete(input_contents)
    log.debugf("collected %d macros from '%s'", len(macros), args.input_file);

    log.debugf("expanding macros in input")
    input_expanded, expansion_success := expand_macros(input_no_macros, &macros)
    if !expansion_success do return
    delete(input_no_macros)

    output_file_path := filepath.join(args.output_dir, filepath.base(args.input_file))
    output_write_success := os.write_entire_file(output_file_path, transmute([]byte)input_expanded)
    if !output_write_success {
        log.errorf("failed to write output file '%s'", output_file_path)
    }
    log.infof("wrote output to '%s'", output_file_path)

    end_timed_section(Timed_Section.Whole_Program)
}

Context_Data :: struct {
    cycle_counts: [Timed_Section.Count]i64,
}

begin_timed_section :: proc(section: Timed_Section) {
    context_data := cast(^Context_Data)context.user_ptr
    context_data.cycle_counts[section] = intrinsics.read_cycle_counter()
}

end_timed_section :: proc(section: Timed_Section) {
    context_data := cast(^Context_Data)context.user_ptr
    cycles_elapsed := intrinsics.read_cycle_counter() - context_data.cycle_counts[section]
    log.debugf("%s cycles: %d", section, cycles_elapsed)
}

Timed_Section :: enum {
    Whole_Program,
    Count,
}

Logger_Data :: struct {
    indent_level: int,
}

logger_proc :: proc(data: rawptr, level: log.Level, text: string, options: log.Options, location := #caller_location) {
    logger_data := cast(^Logger_Data)data
    for n_indents := logger_data.indent_level; n_indents > 0; n_indents -= 1 do fmt.print("    ")
    fmt.println(text)
}

inc_indent_level :: proc() {
    logger_data := cast(^Logger_Data)context.logger.data
    logger_data.indent_level += 1
}

dec_indent_level :: proc() {
    logger_data := cast(^Logger_Data)context.logger.data
    logger_data.indent_level -= 1
}

when ODIN_OS == "windows" {

ERROR_FILE_NOT_FOUND :: os.ERROR_FILE_NOT_FOUND

make_directory :: proc(path: string) -> os.Errno {
    result := os.make_directory(path, 0)
    if result == os.ERROR_PATH_NOT_FOUND {
        return os.Errno(ERROR_FILE_NOT_FOUND)
    }
    return os.ERROR_NONE
}

} else {

ERROR_FILE_NOT_FOUND :: os.ENOENT

make_directory :: proc(path: string) -> os.Errno {
    mode := os.S_IXUSR | os.S_IRUSR | os.S_IWUSR | os.S_IXGRP | os.S_IRGRP | os.S_IWGRP | os.S_IXOTH | os.S_IROTH
  	cstr := strings.clone_to_cstring(path)
    result := _unix_mkdir(cstr, i32(mode))
    delete(cstr)
    if result == -1 {
        return os.Errno(os.get_last_error())
    }
    return os.ERROR_NONE
}

when ODIN_OS == "darwin" {
  foreign import libc "System.framework"
} else {
  foreign import libc "system:c"
}

@(default_calling_convention="c")
foreign libc {
	@(link_name="mkdir") _unix_mkdir :: proc(path: cstring, mode: i32) -> i32 ---
}

}

Macro :: struct {
    name: string,
    args: []string,
    contents: string,
    expanded: bool,
}

collect_macros :: proc(input: string) -> (string, map[string]Macro, bool) {
    inc_indent_level()
    defer dec_indent_level()

    input := input
    input_no_macros : [dynamic]string
    macros : map[string]Macro

    macro_mark := "#define"
    for len(input) > 0 {
        before_macro_mark: string
        before_macro_mark, input = split_at(input, index_or_end(input, macro_mark))

        if len(before_macro_mark) > 0 do append(&input_no_macros, before_macro_mark)

        if len(input) == 0 do break // NOTE(sen) No macro found

        input = input[len(macro_mark):]
        input = skip_spaces(input)

        if len(input) == 0 {
            log.error("missing macro definition")
            return "", {}, false
        }

        if !unicode.is_alpha(first_rune(input)) {
            log.error("macro name should start with a letter")
            return "", {}, false
        }

        mac_name: string
        mac_name, input = split_at(input, index_proc_or_end(input, is_not_alphanum))
        assert(len(mac_name) > 0)
        log.debugf("found macro '%s'", mac_name)

        mac := Macro{}
        mac.name = strings.clone(mac_name)

        input = skip_spaces(input)
        if first_rune(input) != '(' {
            log.errorf("macro name '%s' should be followed by '('", mac.name)
            return "", {}, false
        }
        input = skip_first_rune(input)
        input = skip_spaces(input)

        // NOTE(sen) Collect macro arguments
        mac_args: [dynamic]string
        for first_rune(input) != ')' {
            inc_indent_level()
            defer dec_indent_level()

            if !unicode.is_alpha(first_rune(input)) {
                log.error("Contents of () after macro name should have comma-separated parameter names or nothing")
                return "", {}, false
            }

            arg_name: string
            arg_name, input = split_at(input, index_proc_or_end(input, is_not_alphanum))
            assert(len(arg_name) > 0)
            log.debugf("found argument '%s'", arg_name)
            append(&mac_args, strings.clone(arg_name))

            input = skip_spaces(input)
            if first_rune(input) == ',' do input = skip_first_rune(input)
            input = skip_spaces(input)
        }
        mac.args = mac_args[:]
        assert(first_rune(input) == ')')
        input = skip_first_rune(input)
        input = skip_spaces(input)

        if first_rune(input) != '{' {
            log.error("Macro body should start with '{'")
            return "", {}, false
        }
        input = skip_first_rune(input)
        input = skip_spaces(input)

        mac_contents: string;
        mac_contents, input = split_at(input, index_rune_or_end(input, '}'))
        if len(input) == 0 {
            log.error("Contents of () after macro name should have comma-separated parameter names or nothing")
            return "", {}, false
        }
        assert(first_rune(input) == '}')
        input = skip_first_rune(input)
        input = skip_spaces(input)

        mac.contents = strings.clone(strings.trim_right_space(mac_contents))
        mac.expanded = false

        macros[mac.name] = mac
    }

    input_no_macros_string := strings.concatenate(input_no_macros[:])
    delete(input_no_macros)

    return input_no_macros_string, macros, true
}

expand_macros :: proc(input: string, macros: ^map[string]Macro) -> (string, bool) {
    inc_indent_level()
    defer dec_indent_level()

    input := input

    input_expanded: [dynamic]string
    for len(input) > 0 {
        before_macro_use: string
        used_macro_name: string
        before_macro_use, used_macro_name, input = split_placeholder(input, '@')
        if len(before_macro_use) > 0 do append(&input_expanded, before_macro_use)
        if len(used_macro_name) == 0 && len(input) == 0 do break // NOTE(sen) No used macros found

        assert(len(used_macro_name) > 0)
        log.debugf("found used macro '%s'", used_macro_name)

        inc_indent_level()
        defer dec_indent_level()

        mac, mac_found := &macros[used_macro_name]
        if !mac_found {
            log.errorf("macro %s used but not found", used_macro_name)
            return "", false
        }

        // TODO(sen) Catch circular dependencies
        if !mac.expanded {
            old_contents := mac.contents
            log.debugf("expanding contents")
            expanded_contents, success := expand_macros(old_contents, macros)
            if !success do return "", false
            delete(old_contents)
            mac.contents = expanded_contents
            mac.expanded = true
        }

        input = skip_spaces(input)
        if (first_rune(input) != '(') {
            log.errorf("macro name %s should be followed by '('", used_macro_name)
            return "", false
        }
        input = skip_first_rune(input)
        input = skip_spaces(input)

        passed_args: [dynamic]string
        for first_rune(input) != ')' {
            if (first_rune(input) != '"') {
                log.errorf("macro %s arguments should be wrapped in '\"'", used_macro_name)
                return "", false
            }
            input = skip_first_rune(input)
            arg_used: string
            arg_used, input = split_at(input, index_rune_or_end(input, '"'))
            if len(input) == 0 {
                log.errorf("unmatched '\"' in macro %s", used_macro_name)
                return "", false
            }
            log.debugf("found passed argument '%s' (pos %d)", arg_used, len(passed_args))
            append(&passed_args, arg_used)
            input = skip_first_rune(input)
            input = skip_spaces(input)
            if (first_rune(input) == ',') do input = skip_first_rune(input)
            input = skip_spaces(input)
        }
        assert(first_rune(input) == ')')
        input = skip_first_rune(input)

        if len(passed_args) != len(mac.args) {
            log.errorf("macro %s has %d arguments but %d were passed", mac.name, len(mac.args), len(passed_args))
            return "", false
        }

        mac_contents := mac.contents
        for len(mac_contents) > 0 {
            before_arg_use: string
            used_arg_name: string
            before_arg_use, used_arg_name, mac_contents = split_placeholder(mac_contents, '$')
            if len(before_arg_use) > 0 do append(&input_expanded, before_arg_use)
            if len(used_arg_name) == 0 && len(mac_contents) == 0 do break // NOTE(sen) No used argument found

            assert(len(used_arg_name) > 0)

            used_arg_position := index_elem(mac.args, used_arg_name)

            if used_arg_position == -1 {
                log.errorf("macro %s uses argument undeclared argument %s", mac.name, used_arg_name)
                return "", false
            }

            passed_arg_content := passed_args[used_arg_position]

            log.debugf("replaced used argument '%s' with passed '%s'", used_arg_name, passed_arg_content)

            append(&input_expanded, passed_arg_content)
        }
    }

    output := strings.concatenate(input_expanded[:])
    delete(input_expanded)

    return output, true
}

index_or_end :: proc(input: string, search: string) -> int {
    result := strings.index(input, search)
    if result == -1 do result = len(input)
    return result
}

index_proc_or_end :: proc(input: string, pr: proc(rune) -> bool, truth := true) -> int {
    result := strings.index_proc(input, pr, truth)
    if result == -1 do result = len(input)
    return result
}

index_rune_or_end :: proc(input: string, rn: rune) -> int {
    result := strings.index_rune(input, rn)
    if result == -1 do result = len(input)
    return result
}

index_rune_proc_or_end :: proc(input: string, rn: rune, pr: proc(rune) -> bool, truth := true) -> int {
    result := len(input)
    rune_index := strings.index_rune(input, rn)
    if rune_index != -1 &&
        pr(utf8.rune_at_pos(input[rune_index:], 1)) == truth {
        result = rune_index
    }
    return result
}

index_proc_rune_proc_or_end :: proc(input: string, pr1: proc(rune) -> bool, rn: rune, pr2: proc(rune) -> bool) -> int {
    result := len(input)
    rune_index := strings.index_rune(input, rn)
    if rune_index != -1 && pr2(utf8.rune_at_pos(input[rune_index:], 1)) {
        if rune_index > 0 {
            if pr1(utf8.rune_at_pos(input, rune_index - 1)) {
                result = rune_index
            }
        } else {
            result = rune_index
        }
    }
    return result
}

skip_spaces :: proc(input: string) -> string {
    first_nonspace := index_proc_or_end(input, strings.is_space, false)
    return input[first_nonspace:]
}

is_not_alphanum :: proc(ch: rune) -> bool {
    return !unicode.is_alpha(ch) && !unicode.is_number(ch)
}

is_not_alpha :: proc(ch: rune) -> bool {
    return !unicode.is_alpha(ch)
}

split_at :: proc(input: string, index: int) -> (string, string) {
    return input[:index], input[index:]
}

first_rune :: proc(input: string) -> rune {
    return utf8.rune_at_pos(input, 0)
}

skip_first_rune :: proc(input: string) -> string {
    return input[utf8.rune_size(first_rune(input)):]
}

/// Find placeholders like @name
split_placeholder :: proc(input: string, prefix: rune) -> (before: string, placeholder_name: string, after: string) {
    before, after = split_at(input, index_proc_rune_proc_or_end(input, is_not_alpha, prefix, unicode.is_alpha))
    if len(after) == 0 do return // NOTE(sen) No placeholders found

    assert(first_rune(after) == prefix)
    after = skip_first_rune(after)
    assert(len(after) > 0)
    assert(unicode.is_alpha(first_rune(after)))

    placeholder_name, after = split_at(after, index_proc_or_end(after, is_not_alphanum))

    return before, placeholder_name, after
}

index_elem :: proc(array: []$T, elem: T) -> int {
    result := -1
    for value, index in array {
        if value == elem {
            result = index
            break
        }
    }
    return result
}
