use crate::{Error, MemoryArena, Result, RunArguments, String};

pub(crate) const PATH_SEP: char = '\\';
/// https://docs.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation
pub(crate) const MAX_PATH_BYTES: usize = 260;
/// https://docs.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation
pub(crate) const MAX_FILENAME_BYTES: usize = 256;

pub fn parse_arguments<'a>() -> RunArguments<'a> {
    // TODO(sen) Implement
    let mut result = RunArguments::default();
    result
}

enum Void {}
/// https://docs.microsoft.com/en-us/windows/console/getstdhandle
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;

// NOTE(sen) Equivalent to /MT except the C++ std
// https://docs.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features
#[link(name = "libcmt")]
#[link(name = "libucrt")]
#[link(name = "libvcruntime")]
#[link(name = "kernel32")]
extern "system" {
    /// https://docs.microsoft.com/en-us/windows/console/getstdhandle
    fn GetStdHandle(handle: u32) -> *mut Void;
    /// https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefileex
    fn WriteFileEx(
        file: *mut Void,
        buffer: *const Void,
        bytes_to_write: usize,
        written: &mut u32,
        overlapped: *mut Void,
    ) -> i32;
}

pub(crate) fn write_stdout(text: &str) {
    unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stdout_handle,
            text.as_ptr() as *const _,
            text.as_bytes().len(),
            &mut written,
            core::ptr::null_mut(),
        )
    };
}

pub(crate) fn write_stdout_raw(ptr: *const u8, size: usize) {
    unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stdout_handle,
            ptr as *const _,
            size,
            &mut written,
            core::ptr::null_mut(),
        )
    };
}

/// https://docs.microsoft.com/en-us/windows/console/getstdhandle
const STD_ERROR_HANDLE: u32 = -12i32 as u32;

pub(crate) fn write_stderr_raw(ptr: *const u8, size: usize) {
    unsafe {
        let stderr_handle = GetStdHandle(STD_ERROR_HANDLE);
        let mut written = 0;
        WriteFileEx(
            stderr_handle,
            ptr as *const _,
            size,
            &mut written,
            core::ptr::null_mut(),
        )
    };
}

extern "system" {
    /// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-exitprocess
    fn ExitProcess(code: u32);
}

pub(crate) fn exit() {
    unsafe {
        ExitProcess(0);
    }
}

pub(crate) fn create_empty_file(path: &String) -> Result<()> {
    // TODO(sen) Implement
    Err(Error {})
}

pub(crate) fn append_to_file(path: &String, ptr: *const u8, size: usize) -> Result<()> {
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
