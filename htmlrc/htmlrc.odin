package htmlrc

import "core:os"

main :: proc() {
    cmdline := open("/proc/self/cmdline", O_RDONLY)
    buf := [256]u8{}
    cmdargs_result := read(cmdline, &buf, 100)
    test()
}

foreign import libc "system:c"

O_RDONLY :: i32(0);

foreign libc {
    /// https://man7.org/linux/man-pages/man2/open.2.html
    open :: proc(path: cstring, flags: i32, mode: u32 = 0) -> i32 ---
    /// https://man7.org/linux/man-pages/man2/read.2.html
    read :: proc(file: i32, buf: rawptr, count: uint) -> int ---
}
