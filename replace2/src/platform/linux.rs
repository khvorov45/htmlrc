use crate::{Error, Memory, Result, String};

pub(crate) const PATH_SEP: char = '/';

pub(crate) fn write_stdout(text: &str) {
    use libc::{write, STDOUT_FILENO};
    unsafe { write(STDOUT_FILENO, text.as_ptr() as *const _, text.len()) };
}

pub(crate) fn write_file(path: &String, content: &String) -> Result<()> {
    use libc::{__errno_location, close, open, write, O_WRONLY};

    let file_handle = unsafe { open(path.ptr.cast(), O_WRONLY) };

    if file_handle == -1 {
        // TODO(sen) Create the file with the right permissions
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

pub(crate) fn create_dir_if_not_exists(path: &String) -> Result<()> {
    use libc::{__errno_location, mkdir, opendir, ENOENT, S_IROTH, S_IRWXG, S_IRWXU, S_IXOTH};

    let open_result = unsafe { opendir(path.ptr.cast()) };

    let mut result = Ok(());
    if open_result.is_null() {
        let errno = unsafe { *__errno_location() };
        if errno == ENOENT {
            let create_result =
                unsafe { mkdir(path.ptr.cast(), S_IROTH | S_IRWXG | S_IRWXU | S_IXOTH) };
            if create_result == -1 {
                write_stdout(path.as_str());
                let _errno = unsafe { *__errno_location() };
                result = Err(Error {});
            }
        } else {
            result = Err(Error {});
        }
    }
    result
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
        // TODO(sen) Check for errors and close file
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
