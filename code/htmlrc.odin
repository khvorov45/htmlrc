package htmlrc

import "core:log"
import "core:intrinsics"
import "core:fmt"
import "core:mem"
import "core:os"
import "core:strings"

main :: proc() {
    context.user_ptr = &Context_Data{};
    context.logger.procedure = logger_proc
    begin_timed_section(Timed_Section.Whole_Program)

    input_dir := os.args[1]
    log.debugf("input dir: %s", input_dir)

    max_html_file_size : i64 = 0
    total_html_file_count := 0
    total_html_file_size : i64 = 0
    {
        input_dir_handle, open_error := os.open(input_dir)
        if open_error != os.ERROR_NONE {
            log.errorf("failed to open input dir %s: %d", input_dir, open_error)
            return
        }
        // NOTE(sen) The `n` parameter seems to just be a guess for how many
        // entries there are. Set to -1 to use default
        input_dir_entries, err := os.read_dir(input_dir_handle, -1)
        if err != os.ERROR_NONE {
            log.errorf("failed to read input dir %s: %d", input_dir, err)
            return
        }
        for entry in input_dir_entries {
            if !entry.is_dir && strings.has_suffix(entry.name, ".html") {
                total_html_file_count += 1
                total_html_file_size += entry.size
                if entry.size > max_html_file_size do max_html_file_size = entry.size
            }
        }
        os.close(input_dir_handle)
        delete(input_dir_entries)
    }

    if total_html_file_count == 0 {
        log.info("no html files found in %s", input_dir)
        return
    }

    // NOTE(sen) Both of these should be more than anybody will ever need per page
    total_supported_components : i64 = 131072
    total_supported_simultaneous_arguments : i64 = 131072

    max_supported_component_name_length : i64 = 260

    components_size : i64 = total_supported_components * size_of(Name_Value);
    component_names_size : i64 = total_supported_components * max_supported_component_name_length;
    component_contents_size : i64 = total_html_file_size;
    component_arguments_size : i64 = total_supported_simultaneous_arguments * size_of(Name_Value);
    input_size : i64 = total_html_file_size;
    output_size : i64 = max(max_html_file_size, 10 * 1024 * 1024);

    total_memory_size := components_size + component_names_size + component_contents_size + component_arguments_size + input_size + output_size

    program_memory := mem.alloc(int(total_memory_size))

    arena := mem.Arena{}
    mem.init_arena(&arena, mem.byte_slice(program_memory, int(total_memory_size)))
    context.allocator = mem.arena_allocator(&arena)

    end_timed_section(Timed_Section.Whole_Program)
}

Name_Value :: struct {
    name: string,
    value: string,
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
