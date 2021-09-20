package htmlrc

import "core:log"
import "core:intrinsics"
import "core:fmt"
import "core:mem"

main :: proc() {
    context.user_ptr = &Context_Data{};
    context.logger.procedure = logger_proc

    begin_timed_section(Timed_Section.Whole_Program)
    defer end_timed_section(Timed_Section.Whole_Program)

    main_memory_size := 10 * 1024 * 1024
    scratch_memory_size := 1024
    total_memory_size := main_memory_size + scratch_memory_size

    program_memory := mem.alloc(total_memory_size)

    arena := mem.Arena{}
    mem.init_arena(&arena, mem.byte_slice(program_memory, total_memory_size))
    context.allocator = mem.arena_allocator(&arena)

    scratch := mem.Scratch_Allocator{}
    mem.scratch_allocator_init(&scratch, scratch_memory_size)
    context.temp_allocator = mem.scratch_allocator(&scratch)
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
    log.debugf("%s cycles: %d\n", section, cycles_elapsed)
}

Timed_Section :: enum {
    Whole_Program,
    Count,
}

logger_proc :: proc(data: rawptr, level: log.Level, text: string, options: log.Options, location := #caller_location) {
    fmt.print(text)
}
