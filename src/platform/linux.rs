use crate::{ConstPointer, Error, MemoryArena, PointerDeref, Result, RunArguments, String};

pub(crate) const PATH_SEP: char = '/';
pub(crate) const MAX_PATH_BYTES: usize = 4096;
pub(crate) const MAX_FILENAME_BYTES: usize = 256;

enum Void {}

pub fn parse_arguments<'a>(argc: isize, argv: *const *const u8) -> RunArguments<'a> {
    let mut result = RunArguments::default();
    for arg_index in 1..platform_args.argc as usize {
        let base = platform_args.argv.plus(arg_index).deref();
        let mut size = 0;
        while base.plus(size).deref() != b'\0' {
            size += 1;
        }
        let arg_slice = unsafe { core::slice::from_raw_parts(base, size) };
        let arg = unsafe { core::str::from_utf8_unchecked(arg_slice) };
        match arg_index {
            1 => result.input_dir = arg,
            2 => result.input_file_name = arg,
            3 => result.output_dir = arg,
            _ => {}
        }
    }
    result
}

#[link(name = "c")]
extern "system" {
    fn write(file: i32, buffer: *const Void, count: usize) -> isize;
    fn __errno_location() -> *mut i32;
    fn open(path: *const i8, flag: i32) -> i32;
    fn close(file: i32) -> i32;
}

const STDOUT_FILENO: i32 = 1;

pub(crate) fn write_stdout(text: &str) {
    unsafe {
        write(
            STDOUT_FILENO,
            text.as_ptr() as *const _,
            text.as_bytes().len(),
        )
    };
}

pub(crate) fn write_stdout_raw(ptr: *const u8, size: usize) {
    unsafe { write(STDOUT_FILENO, ptr as *const _, size) };
}

pub(crate) fn write_stderr_raw(ptr: *const u8, size: usize) {
    const STDERR_FILENO: i32 = 2;
    unsafe { write(STDERR_FILENO, ptr as *const _, size) };
}

pub(crate) fn exit() {
    extern "system" {
        fn exit(code: i32);
    }
    unsafe {
        exit(0);
    }
}

const O_WRONLY: i32 = 1;

pub(crate) fn create_empty_file(path: &String) -> Result<()> {
    extern "system" {
        fn creat(path: *const i8, mode: u32) -> i32;
    }
    const O_TRUNC: i32 = 512;

    let mut file_handle = unsafe { open(path.ptr.cast(), O_WRONLY | O_TRUNC) };

    const S_IRUSR: u32 = 256;
    const S_IWUSR: u32 = 128;
    const S_IRGRP: u32 = 32;
    const S_IWGRP: u32 = 16;
    const S_IROTH: u32 = 4;

    if file_handle == -1 {
        file_handle = unsafe {
            creat(
                path.ptr.cast(),
                S_IRUSR | S_IWUSR | S_IRGRP | S_IWGRP | S_IROTH,
            )
        };
    }

    if file_handle >= 0 {
        Ok(())
    } else {
        let _errno = unsafe { *__errno_location() };
        Err(Error {})
    }
}

pub(crate) fn append_to_file(path: &String, ptr: *const u8, size: usize) -> Result<()> {
    const O_APPEND: i32 = 1024;

    let file_handle = unsafe { open(path.ptr.cast(), O_WRONLY | O_APPEND) };

    if file_handle >= 0 {
        let write_result = unsafe { write(file_handle, ptr.cast(), size) };
        if write_result == -1 {
            let _errno = unsafe { *__errno_location() };
            Err(Error {})
        } else {
            unsafe {
                close(file_handle);
            }
            Ok(())
        }
    } else {
        let _errno = unsafe { *__errno_location() };
        Err(Error {})
    }
}

pub(crate) fn read_file(memory: &mut MemoryArena, path: &String) -> Result<String> {
    extern "system" {
        fn fstat(file: i32, buffer: *mut stat) -> i32;
        fn read(file: i32, buffer: *mut Void, bytes_to_read: usize) -> isize;
    }
    #[repr(C)]
    struct stat {
        _unused_front: [u64; 6],
        st_size: i64,
        _unused_back: [u64; 11],
    }

    let file_handle = unsafe { open(path.ptr.cast(), 0) };

    if file_handle >= 0 {
        let file_size = unsafe {
            let mut file_info: stat = core::mem::MaybeUninit::zeroed().assume_init();
            fstat(file_handle, &mut file_info);
            file_info.st_size as usize
        };
        let dest = memory.push_size(file_size);

        let read_result = unsafe { read(file_handle, dest.cast(), file_size) };
        if read_result == -1 {
            let _errno = unsafe { *__errno_location() };
            Err(Error {})
        } else {
            unsafe { close(file_handle) };
            Ok(String {
                ptr: dest,
                size: file_size,
            })
        }
    } else {
        let _errno = unsafe { *__errno_location() };
        Err(Error {})
    }
}

pub(crate) fn create_dir_if_not_exists(path: &String) -> Result<()> {
    extern "system" {
        fn opendir(dirname: *const i8) -> *mut Void;
        fn mkdir(path: *const i8, mode: u32) -> i32;
        fn closedir(dir: *mut Void) -> i32;
    }
    const ENOENT: i32 = 2;
    const S_IROTH: u32 = 4;
    const S_IXOTH: u32 = 1;
    const S_IRWXU: u32 = 448;
    const S_IRWXG: u32 = 56;

    let open_result = unsafe { opendir(path.ptr.cast()) };

    if open_result.is_null() {
        let errno = unsafe { *__errno_location() };
        if errno == ENOENT {
            let create_result =
                unsafe { mkdir(path.ptr.cast(), S_IROTH | S_IRWXG | S_IRWXU | S_IXOTH) };
            if create_result == -1 {
                let _errno = unsafe { *__errno_location() };
                Err(Error {})
            } else {
                Ok(())
            }
        } else {
            Err(Error {})
        }
    } else {
        unsafe { closedir(open_result) };
        Ok(())
    }
}

pub(crate) fn allocate_and_clear(total_size: usize) -> Result<*mut u8> {
    extern "system" {
        fn mmap(
            addr: *mut Void,
            len: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut Void;
    }
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const MAP_ANONYMOUS: i32 = 0x0020;
    const MAP_PRIVATE: i32 = 0x0002;
    const MAP_FAILED: *mut Void = !0 as *mut Void;

    let ptr = unsafe {
        mmap(
            core::ptr::null_mut(),
            total_size,
            PROT_READ | PROT_WRITE,
            // NOTE(sen) `MAP_ANONYMOUS` should clear the contents to 0
            // https://man7.org/linux/man-pages/man2/mmap.2.html
            MAP_ANONYMOUS | MAP_PRIVATE,
            -1,
            0,
        )
    };
    if ptr == MAP_FAILED {
        let _errno = unsafe { *__errno_location() };
        Err(Error {})
    } else {
        Ok(ptr.cast())
    }
}

pub(crate) struct TimeSpec {
    seconds: i64,
    nanoseconds: i64,
}

pub(crate) fn get_timespec_now() -> TimeSpec {
    extern "system" {
        fn clock_gettime(clk_id: i32, tp: *mut timespec) -> i32;
    }
    #[repr(C)]
    pub struct timespec {
        pub tv_sec: i64,
        pub tv_nsec: i64,
    }
    const CLOCK_MONOTONIC: i32 = 1;

    unsafe {
        let mut timespec: timespec = core::mem::MaybeUninit::zeroed().assume_init();
        clock_gettime(CLOCK_MONOTONIC, &mut timespec);
        TimeSpec {
            seconds: timespec.tv_sec,
            nanoseconds: timespec.tv_nsec,
        }
    }
}

pub(crate) fn get_seconds_from(timespec: &TimeSpec) -> f32 {
    let now = get_timespec_now();
    let seconds = now.seconds - timespec.seconds;
    let nanoseconds = now.nanoseconds - timespec.nanoseconds;
    seconds as f32 + nanoseconds as f32 * 1e-9
}
