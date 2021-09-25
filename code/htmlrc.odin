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
    context.logger.procedure = logger_proc // TODO(sen) Better logging
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
        log.info("USAGE: htmlrc <input> <output>\n\ninput: an html file\noutput: a directory (default: %s)", args.output_dir)
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

    input_no_macros, macros, collection_success := collect_macros(string(input_contents))
    if !collection_success do return
    delete(input_contents)

    // NOTE(sen) Expand nested macros
    for mac in &macros {
        contents_expanded, success := expand_macros(mac.contents, macros)
        if !success do return
        mac.contents = contents_expanded
    }

    input_expanded, expansion_success := expand_macros(input_no_macros, macros)
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

Macro :: struct {
    name: string,
    contents: string,
}

collect_macros :: proc(input: string) -> (string, []Macro, bool) {
    input_no_macros: string
    macros: []Macro
    success := false

    return input_no_macros, macros, success
}

expand_macros :: proc(input: string, macros: []Macro) -> (string, bool) {
    output: string
    success := false

    return output, success
}
