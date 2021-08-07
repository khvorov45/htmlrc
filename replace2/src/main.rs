#![no_std]
#![no_main]
#![windows_subsystem = "console"]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
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
    //replace2::run("input", "page-plain.html", "build");
    //0
}

#[no_mangle]
pub static _fltused: i32 = 0x9875;
