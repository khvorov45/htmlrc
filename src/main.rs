#![no_std]
#![no_main]
#![windows_subsystem = "console"]

use htmlrc::{handle_panic, parse_arguments, run};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    handle_panic(info);
    loop {}
}

#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "C" fn main(argc: isize, argv: *const *const u8) -> isize {
    run(parse_arguments(argc, argv));
    0
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "C" fn mainCRTStartup() -> ! {
    run(parse_arguments());
    #[allow(clippy::empty_loop)]
    loop {}
}
