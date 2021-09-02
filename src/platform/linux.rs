use crate::{
    ConstPointer, Error, Filepath, MemoryArena, MutPointer, PointerDeref, Result, RunArguments,
    String, ToString, MEGABYTE,
};

pub(crate) const PATH_SEP: char = '/';
pub(crate) const MAX_PATH_BYTES: usize = 4096;
pub(crate) const MAX_FILENAME_BYTES: usize = 256;

enum Void {}

pub fn parse_arguments<'a>(argc: isize, argv: *const *const u8) -> RunArguments<'a> {
    let mut result = RunArguments::default();
    for arg_index in 1..argc as usize {
        let base = argv.plus(arg_index).deref();
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

fn get_file_size(handle: i32) -> usize {
    extern "system" {
        fn fstat(file: i32, buffer: *mut stat) -> i32;
    }
    #[repr(C)]
    struct stat {
        _unused_front: [u64; 6],
        st_size: i64,
        _unused_back: [u64; 11],
    }
    let mut file_info: stat = unsafe { core::mem::MaybeUninit::zeroed().assume_init() };
    unsafe { fstat(handle, &mut file_info) };
    file_info.st_size as usize
}

pub(crate) fn read_file(memory: &mut MemoryArena, path: &String) -> Result<String> {
    extern "system" {
        fn read(file: i32, buffer: *mut Void, bytes_to_read: usize) -> isize;
    }

    let file_handle = unsafe { open(path.ptr.cast(), 0) };
    if file_handle >= 0 {
        let file_size = get_file_size(file_handle);
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

pub(crate) fn get_max_and_total_html_size(dir_path: &str) -> Result<(usize, usize)> {
    extern "system" {
        fn munmap(ptr: *mut Void, size: usize) -> i32;
        fn getdents64(handle: i32, buffer: *mut Void, buffer_size: usize) -> isize;
    }

    let (mut filepath, mut memory) = {
        let filepath_size = MAX_PATH_BYTES;
        let dir_read_memory_size = 10 * MEGABYTE;
        let total_size = filepath_size + dir_read_memory_size;
        let memory_base_ptr = match allocate_and_clear(total_size) {
            Ok(ptr) => ptr,
            Err(_) => return Err(Error {}),
        };
        let mut offset = 0;
        let mut filepath_memory = MemoryArena::new(memory_base_ptr, &mut offset, filepath_size);
        let dir_read_memory = MemoryArena::new(memory_base_ptr, &mut offset, dir_read_memory_size);
        debug_assert!(offset == total_size);
        let filepath = Filepath::new(&mut filepath_memory);
        (filepath, dir_read_memory)
    };

    let dir_path_string = filepath.new_path(dir_path.to_string()).get_string();

    const O_RDONLY: i32 = 0;
    const O_DIRECTORY: i32 = 0x10000;
    let dir_handle = unsafe { open(dir_path_string.ptr.cast(), O_RDONLY | O_DIRECTORY) };
    if dir_handle == -1 {
        return Err(Error {});
    }

    let mut largest_html_file_size = 0;
    let mut total_html_file_size = 0;

    #[repr(C)]
    struct linux_dirent64 {
        ignore: [i64; 2],
        dirent_size: u16,
        ignore2: u8,
        name_start: i8,
    }
    loop {
        let bytes_read = unsafe { getdents64(dir_handle, memory.base.cast(), memory.size) };
        if bytes_read == -1 {
            return Err(Error {});
        }
        if bytes_read == 0 {
            break;
        }

        let mut current_dirent_ptr = memory.base;
        let mut bytes_processed = 0;
        while bytes_processed < bytes_read {
            let entry = current_dirent_ptr.cast::<linux_dirent64>();
            let entry = entry.get_ref();

            current_dirent_ptr = current_dirent_ptr.plus(entry.dirent_size as usize);
            bytes_processed += entry.dirent_size as isize;

            // NOTE(sen) Read name backwards
            let name_end_ptr = {
                let mut result = current_dirent_ptr.minus(1);
                while result.deref() == b'\0' {
                    result = result.minus(1);
                }
                result
            };
            let target = b"lmth";
            let mut is_target = true;
            #[allow(clippy::needless_range_loop)]
            for offset in 0..target.len() {
                let name_char = name_end_ptr.minus(offset);
                if name_char.deref() != target[offset] {
                    is_target = false;
                    break;
                }
            }

            // NOTE(sen) Read size
            if is_target {
                let filename_base = &entry.name_start as *const i8;
                let filename_string = String {
                    ptr: filename_base.cast(),
                    size: name_end_ptr as usize - filename_base as usize + 1,
                };
                let full_filepath = filepath
                    .new_path(dir_path.to_string())
                    .add_entry(filename_string)
                    .get_string();
                let file_handle = unsafe { open(full_filepath.ptr.cast(), O_RDONLY) };
                if file_handle >= 0 {
                    let file_size = get_file_size(file_handle);
                    total_html_file_size += file_size;
                    largest_html_file_size = largest_html_file_size.max(file_size);
                    unsafe { close(file_handle) };
                }
            }
        }

        // NOTE(sen) Clear for the next round
        for offset in 0..memory.used {
            memory.base.plus(offset).deref_and_assign(0);
        }
        memory.used = 0;
    }

    let _free_result = unsafe { munmap(memory.base.cast(), memory.size) };
    let _close_result = unsafe { close(dir_handle) };
    Ok((largest_html_file_size, total_html_file_size))
}
