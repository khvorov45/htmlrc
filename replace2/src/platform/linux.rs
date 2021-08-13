use crate::{Error, MemoryArena, Result, String};

pub(crate) const PATH_SEP: char = '/';
pub(crate) const MAX_PATH_BYTES: usize = 4096;
pub(crate) const MAX_FILENAME_BYTES: usize = 256;

pub(crate) fn write_stdout(text: &str) {
    use libc::{write, STDOUT_FILENO};
    unsafe {
        write(
            STDOUT_FILENO,
            text.as_ptr() as *const _,
            text.as_bytes().len(),
        )
    };
}

pub(crate) fn write_stdout_raw(ptr: *const u8, size: usize) {
    use libc::{write, STDOUT_FILENO};
    unsafe { write(STDOUT_FILENO, ptr as *const _, size) };
}

pub(crate) fn write_stderr_raw(ptr: *const u8, size: usize) {
    use libc::{write, STDERR_FILENO};
    unsafe { write(STDERR_FILENO, ptr as *const _, size) };
}

pub(crate) fn exit() {
    unsafe {
        libc::exit(0);
    }
}

pub(crate) fn write_file(path: &String, content: &String) -> Result<()> {
    use libc::{
        __errno_location, close, creat, open, write, O_TRUNC, O_WRONLY, S_IRGRP, S_IROTH, S_IRUSR,
        S_IWGRP, S_IWUSR,
    };

    let mut file_handle = unsafe { open(path.ptr.cast(), O_WRONLY | O_TRUNC) };

    if file_handle == -1 {
        file_handle = unsafe {
            creat(
                path.ptr.cast(),
                S_IRUSR | S_IWUSR | S_IRGRP | S_IWGRP | S_IROTH,
            )
        };
    }

    if file_handle >= 0 {
        let write_result = unsafe { write(file_handle, content.ptr.cast(), content.size) };
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
    use libc::{__errno_location, close, fstat, open, read, stat};

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
    use libc::{
        __errno_location, closedir, mkdir, opendir, ENOENT, S_IROTH, S_IRWXG, S_IRWXU, S_IXOTH,
    };

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
    use libc::{
        __errno_location, mmap, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_READ, PROT_WRITE,
    };
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
    use libc::{clock_gettime, timespec, CLOCK_MONOTONIC};
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
