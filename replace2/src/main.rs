#![no_std]
#![no_main]
#![windows_subsystem = "console"]

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    replace2::handle_panic(info);
    loop {}
}

#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "C" fn main(_argc: isize, _argv: *const *const u8) -> isize {
    replace2::run("input", "page-plain.html", "build");
    0
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "C" fn mainCRTStartup() -> ! {
    replace2::run("input", "page-plain.html", "build");
    #[allow(clippy::empty_loop)]
    loop {}
}
