use crate::{Error, MemoryArena, Result, String};

pub(crate) const PATH_SEP: char = '/';
pub(crate) const MAX_PATH_BYTES: usize = 4096;
pub(crate) const MAX_FILENAME_BYTES: usize = 256;

enum Void {}
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;

extern "C" {
    fn GetStdHandle(handle: u32) -> *mut Void;
    fn WriteFileEx(
        file: *mut Void,
        buffer: *const Void,
        bytes_to_write: usize,
        written: &mut u32,
        overlapped: *mut Void,
    ) -> i32;
}

pub(crate) fn write_stdout(text: &str) {
    /*unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stdout_handle,
            text.as_ptr() as *const _,
            text.as_bytes().len(),
            &mut written,
            core::ptr::null_mut(),
        )
    };*/
}

pub(crate) fn write_stdout_raw(ptr: *const u8, size: usize) {
    /* unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stdout_handle,
            ptr as *const _,
            size,
            &mut written,
            core::ptr::null_mut(),
        )
    };*/
}

const STD_ERROR_HANDLE: u32 = -12i32 as u32;

pub(crate) fn write_stderr(text: &str) {
    /*unsafe {
        let stderr_handle = GetStdHandle(STD_ERROR_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stderr_handle,
            text.as_ptr() as *const _,
            text.as_bytes().len(),
            &mut written,
            core::ptr::null_mut(),
        )
    };*/
}

pub(crate) fn write_stderr_raw(ptr: *const u8, size: usize) {
    /*unsafe {
        let stderr_handle = GetStdHandle(STD_ERROR_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stderr_handle,
            ptr as *const _,
            size,
            &mut written,
            core::ptr::null_mut(),
        )
    };*/
}

extern "C" {
    fn ExitProcess(code: u32);
}

pub(crate) fn exit() {
    unsafe {
        ExitProcess(0);
    }
}

pub(crate) fn write_file(path: &String, content: &String) -> Result<()> {
    // TODO(sen) Implement
    Err(Error {})
}

pub(crate) fn read_file(memory: &mut MemoryArena, path: &String) -> Result<String> {
    // TODO(sen) Implement
    Err(Error {})
}

pub(crate) fn create_dir_if_not_exists(path: &String) -> Result<()> {
    // TODO(sen) Implement
    Err(Error {})
}

pub(crate) fn allocate_and_clear(total_size: usize) -> Result<*mut u8> {
    // TODO(sen) Implement
    Err(Error {})
}

pub(crate) struct TimeSpec {
    seconds: i64,
    nanoseconds: i64,
}

pub(crate) fn get_timespec_now() -> TimeSpec {
    // TODO(sen) Implement
    TimeSpec {
        seconds: 0,
        nanoseconds: 0,
    }
}

pub(crate) fn get_seconds_from(timespec: &TimeSpec) -> f32 {
    // TODO(sen) Implement
    0.0f32
}
