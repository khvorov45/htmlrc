package htmlrc

when ODIN_OS == "linux" { import platform "linux" }

main :: proc() {
    cmd_memory := platform.mmap(nil, 1_000_000,  platform.PROT_READ |  platform.PROT_WRITE,  platform.MAP_ANONYMOUS |  platform.MAP_PRIVATE, -1, 0)
    cmd_line :=  platform.open("/proc/self/cmdline",  platform.O_RDONLY)
    buf := [256]u8{}
    cmd_args_result :=  platform.read(cmd_line, &buf, 100)
    platform.close(cmd_line)
}
