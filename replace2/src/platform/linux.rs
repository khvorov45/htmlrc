use crate::{Error, Memory, Result, String};

pub(crate) const PATH_SEP: char = '/';

pub(crate) fn write_stdout(text: &str) {
    use libc::{write, STDOUT_FILENO};
    unsafe { write(STDOUT_FILENO, text.as_ptr() as *const _, text.len()) };
}

pub(crate) fn _write_stdout_raw(buf: *const u8, count: usize) {
    use libc::{write, STDOUT_FILENO};
    unsafe { write(STDOUT_FILENO, buf.cast(), count) };
}

pub(crate) fn write_stderr(text: &str) {
    use libc::{write, STDERR_FILENO};
    unsafe { write(STDERR_FILENO, text.as_ptr() as *const _, text.len()) };
}

pub(crate) fn exit() {
    unsafe {
        libc::exit(0);
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

pub(crate) fn read_file(memory: &mut Memory, path: &String) -> Result<String> {
    use libc::{__errno_location, fstat, open, read, stat};

    let file_handle = unsafe { open(path.ptr.cast(), 0) };

    if file_handle >= 0 {
        let file_size = unsafe {
            let mut file_info: stat = core::mem::MaybeUninit::zeroed().assume_init();
            fstat(file_handle, &mut file_info);
            file_info.st_size as usize
        };
        let dest = memory.push_size(file_size);
        unsafe { read(file_handle, dest.cast(), file_size) };
        Ok(String {
            ptr: dest,
            size: file_size,
        })
    } else {
        let _errno = unsafe { *__errno_location() };
        Err(Error {})
    }
}
