#![no_std]
#![no_main]
#![windows_subsystem = "console"]

use replace2::{handle_panic, parse_arguments, run, PlatformArguments};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    handle_panic(info);
    loop {}
}

#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "C" fn main(argc: isize, argv: *const *const u8) -> isize {
    let args = parse_arguments(PlatformArguments { argc, argv });
    run(args);
    0
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "C" fn mainCRTStartup() -> ! {
    replace2::run("input", "page-plain.html", "build");
    #[allow(clippy::empty_loop)]
    loop {}
}
