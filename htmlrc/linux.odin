package linux

foreign import libc "system:c"

O_RDONLY :: i32(0)
PROT_READ :: i32(1)
PROT_WRITE :: i32(2)
MAP_ANONYMOUS :: i32(0x0020)
MAP_PRIVATE :: i32(0x0002)
MAP_FAILED :: int(-1)

foreign libc {
    /// https://man7.org/linux/man-pages/man2/open.2.html
    open :: proc(path: cstring, flags: i32, mode: u32 = 0) -> i32 ---
    /// https://man7.org/linux/man-pages/man2/read.2.html
    read :: proc(file: i32, buf: rawptr, count: uint) -> int ---
    /// https://man7.org/linux/man-pages/man2/close.2.html
    close :: proc(file: i32) -> i32 ---
    /// https://man7.org/linux/man-pages/man2/mmap.2.html
    mmap :: proc(addr: rawptr, length: uint, prot: i32, flags: i32, fd: i32, offset: i64) -> rawptr ---
}
