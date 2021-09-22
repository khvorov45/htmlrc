package htmlrc

import "core:log"
import "core:intrinsics"
import "core:fmt"
import "core:mem"
import "core:os"
import "core:strings"
import "core:path/filepath"
import "core:unicode"
import "core:unicode/utf8"

COMPONENT_PREFIX :: "_"

main :: proc() {
    context.user_ptr = &Context_Data{};
    context.logger.procedure = logger_proc
    when !ODIN_DEBUG do context.logger.lowest_level = log.Level.Info
    begin_timed_section(Timed_Section.Whole_Program)

    if len(os.args) == 1 || os.args[1] == "--help" || os.args[1] == "-help" || os.args[1] == "help" || os.args[1] == "-h" {
        log.info("USAGE: htmlrc <input>\n\ninput: an html file or a directory of html files")
        return
    }

    format_os_error :: proc(err: os.Errno) -> string {
        switch err {
            case ERROR_FILE_NOT_FOUND: return "entry does not exist"
            case: return fmt.tprint("error code", err)
        }
    }

    input_dir: string
    input_pages: [dynamic]os.File_Info
    {
        input := os.args[1]
        log.debugf("input: %s\n", input)

        input_handle, open_err := os.open(input)
        if open_err != os.ERROR_NONE {
            log.errorf("failed to open input '%s': %s", input, format_os_error(open_err))
            return
        }
        defer os.close(input_handle)

        input_stat, stat_err := os.stat(input, context.temp_allocator)
        if stat_err != os.ERROR_NONE {
            log.errorf("failed to read input '%s': %d", input, format_os_error(stat_err))
            return
        }

        if input_stat.is_dir {
            input_dir = input
            read_entries, read_dir_err := os.read_dir(input_handle, -1, context.temp_allocator)
            if read_dir_err != os.ERROR_NONE {
                log.errorf("failed to read input directory '%s': %d", input, format_os_error(read_dir_err))
                return
            }
            for read_entry in read_entries {
                if !read_entry.is_dir && strings.has_suffix(read_entry.name, ".html") {
                    test := read_entry.name[0]
                    if !strings.has_prefix(read_entry.name, COMPONENT_PREFIX) {
                        append(&input_pages, read_entry)
                    }
                }
            }
            if len(input_pages) == 0 {
                log.errorf("no html pages (don't start with '%s') found in input directory '%s'", COMPONENT_PREFIX, input)
                return
            }
        } else if strings.has_suffix(input_stat.name, ".html") {
            input_dir = filepath.dir(input_stat.fullpath)
            // NOTE(sen) Allow any name for single-file mode
            append(&input_pages, input_stat)
        } else {
            log.errorf("input '%s' is not a directory and not an html file", input)
            return
        }

        log.debugf("input dir: %s", input_dir)
        log.debugf("input pages:")
        for input_page in input_pages {
            log.debugf("%s", input_page.name)
        }
        log.debugf("")
    }

    output_dir: string
    {
        output := "build"
        if len(os.args) >= 3 {
            output = os.args[2]
        }
        log.debugf("output dir: %s\n", output)

        output_handle, open_err := os.open(output)
        if open_err == os.ERROR_NONE {
            output_dir = output
        } else if open_err == ERROR_FILE_NOT_FOUND {
            make_dir_err := make_directory(output)
            if make_dir_err != os.ERROR_NONE {
                log.errorf("failed to create output dir '%s': %s", output, format_os_error(make_dir_err))
                return
            }
            output_dir = output
        } else {
            log.errorf("failed to open output '%s': %s", output, format_os_error(open_err))
            return
        }
        os.close(output_handle)
    }

    components : map[string]string
    for input_page in input_pages {
        log.debugf("starting resolution of %s", input_page.name)
        input_page_contents, read_success := os.read_entire_file(input_page.fullpath)
        if !read_success {
            log.errorf("failed to read input '%s'", input_page.name)
            return
        }
        input_resolved, resolve_success := resolve_one_string(string(input_page_contents), &components, input_dir)
        if resolve_success {
            output_path := filepath.join(output_dir, input_page.name)
            defer delete(output_path)
            write_success := os.write_entire_file(output_path, transmute([]byte)input_resolved)
            if !write_success {
                log.errorf("failed to write output '%s'", output_path)
            }
            log.debugf("wrote output: %s", output_path)
        }
    }
    log.debugf("")

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

logger_proc :: proc(data: rawptr, level: log.Level, text: string, options: log.Options, location := #caller_location) {
    fmt.println(text)
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

resolve_one_string :: proc(input: string, components: ^map[string]string, input_dir: string) -> (string, bool) {

    if len(input) < 4 {
        return strings.clone(input), true
    }

    no_inline_components: [dynamic]string

    log.debugf("looking for inline components")
    component_search_string := input
    for {
        inline_component_start := "{{"
        inline_component_end := "}}"
        component_start := strings.index(component_search_string, inline_component_start)
        if component_start == -1 {
            append(&no_inline_components, component_search_string)
            break
        }
        append(&no_inline_components, component_search_string[:component_start])
        component_search_string = component_search_string[component_start + len(inline_component_start):]

        first_whitespace := strings.index_proc(component_search_string, strings.is_space)
        if first_whitespace == -1 {
            log.errorf("inline component is incomplete")
            return "", false
        }

        component_name := component_search_string[:first_whitespace]
        log.debugf("found: `%s`", component_name)

        component_search_string = component_search_string[first_whitespace + 1:]

        component_end := strings.index(component_search_string, inline_component_end)
        if component_end == -1 {
            log.errorf("component %s does not end with '%s'", component_name, inline_component_start)
            return "", false
        }

        component_contents := component_search_string[:component_end]

        components[component_name] = strings.trim_space(component_contents)

        component_search_string = component_search_string[component_end + len(inline_component_end):]
    }
    log.debugf("")

    log.debug("looking for components used")
    used_component_search := strings.concatenate(no_inline_components[:])
    delete(no_inline_components)
    output: [dynamic]string
    for {
        used_component_index := -1
        prev_char: rune = 0
        for ch, index in used_component_search {
            if prev_char == '<' && unicode.is_upper(ch) {
                used_component_index = index
                break
            }
            prev_char = ch
        }

        if used_component_index == -1 {
            append(&output, used_component_search)
            break
        }

        append(&output, used_component_search[:used_component_index - 1])
        used_component_search = used_component_search[used_component_index:]

        is_not_alphanum :: proc(ch: rune) -> bool { return !unicode.is_alpha(ch) && !unicode.is_number(ch) }
        first_non_alphanum := strings.index_proc(used_component_search, is_not_alphanum)
        if first_non_alphanum == -1 {
            log.errorf("used component is incomplete")
            return "", false
        }

        used_component_name := used_component_search[:first_non_alphanum]
        log.debugf("found: `%s`", used_component_name)

        used_component_contents: string
        {
            contents, present := components[used_component_name]
            if present {
                log.debug("found in loaded components")
                used_component_contents = contents
            } else {
                log.debug("not found in loaded components")
                used_component_path := strings.concatenate({input_dir, filepath.SEPARATOR_STRING, COMPONENT_PREFIX, used_component_name, ".html"})
                file_contents, success := os.read_entire_file(used_component_path)
                if success {
                    used_component_contents = strings.trim_space(string(file_contents))
                    components[used_component_name] = used_component_contents
                } else {
                    log.errorf("could not read %s", used_component_path)
                    return "", false
                }
            }
        }


        used_component_search = used_component_search[first_non_alphanum:]

        // TODO(sen) Parse arguments

        // TODO(sen) Write resolved component contents

        used_component_end_mark := "/>"
        used_component_end := strings.index(used_component_search, used_component_end_mark)
        if used_component_end == -1 {
            log.errorf("component %s does not end with '%s'", used_component_name, used_component_end_mark)
            return "", false
        }

        used_component_search = used_component_search[used_component_end + len(used_component_end_mark):]
    }

    output_string := strings.concatenate(output[:])
    delete(output)

    return output_string, true
}
