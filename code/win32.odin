package htmlrc_win32

import "core:log"
import "core:intrinsics"

import "resolve"

main :: proc() {
    count := intrinsics.read_cycle_counter();
    context.logger.procedure = logger_proc
    context.logger.data = &Log_Data{GetStdHandle(STD_OUTPUT_HANDLE)}
    log.debug("START\n")
    resolve.resolve_one_string()
    cycles_elapsed := intrinsics.read_cycle_counter() - count;
    log.debugf("cycles: %d\n", cycles_elapsed)
}

Log_Data :: struct {
    handle: rawptr,
}

logger_proc :: proc(data: rawptr, level: log.Level, text: string, options: log.Options, location := #caller_location) {
    written : u32 = 0
    text_transmuted := transmute([]byte)text
    WriteFile((cast(^Log_Data)data).handle, &text_transmuted[0], u32(len(text_transmuted)), &written)
}

foreign import kernel32 "system:Kernel32.lib"

STD_OUTPUT_HANDLE : u32 : ~u32(0) - 11 + 1

@(default_calling_convention="stdcall")
foreign kernel32 {
    /// https://docs.microsoft.com/en-us/windows/console/getstdhandle
    GetStdHandle :: proc(handle: u32) -> rawptr ---
    /// https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefile
    WriteFile :: proc(
        file: rawptr,
        buffer: rawptr,
        bytes_to_write: u32,
        written: ^u32,
        overlapped: rawptr = nil,
    ) -> b32 ---
}
