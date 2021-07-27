#![no_std]

mod platform;

struct Error {}

type Result<T> = core::result::Result<T, Error>;

struct Memory {
    size: usize,
    base: *mut u8,
    used: usize,
}

impl Memory {
    fn push_size(&mut self, size: usize) -> *mut u8 {
        debug_assert!(self.size - self.used >= size);
        let result = unsafe { self.base.add(self.used) };
        self.used += size;
        result
    }
    fn push_str(&mut self, string: &str) -> *mut u8 {
        debug_assert!({
            let mut string_is_ascii = true;
            for ch in string.chars() {
                if !ch.is_ascii() {
                    string_is_ascii = false;
                    break;
                }
            }
            string_is_ascii
        });
        self.push_bytes(string.as_bytes())
    }
    fn push_char(&mut self, ch: char) -> *mut u8 {
        debug_assert!(ch.is_ascii());
        let int: u32 = unsafe { core::mem::transmute(ch) };
        let byte = int as u8;
        let dest = self.push_size(1);
        unsafe { *dest = byte };
        dest
    }
    fn push_bytes(&mut self, bytes: &[u8]) -> *mut u8 {
        let mut dest = self.push_size(bytes.len());
        let result = dest;
        for byte in bytes {
            unsafe {
                *dest = *byte;
                dest = dest.add(1);
            }
        }
        result
    }
}

struct String {
    ptr: *const u8,
    size: usize,
}

impl String {
    fn as_str(&self) -> &str {
        unsafe {
            core::str::from_utf8_unchecked(
                core::ptr::slice_from_raw_parts(self.ptr, self.size)
                    .as_ref()
                    .unwrap(),
            )
        }
    }
}

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

pub fn run(input_dir: &str, input_file_name: &str) {
    use platform::{allocate_and_clear, exit, read_file, write_stderr, write_stdout};

    let total_memory_size = 10 * MEGABYTE;

    if let Ok(memory) = allocate_and_clear(total_memory_size) {
        let mut memory = Memory {
            size: total_memory_size,
            base: memory,
            used: 0,
        };
        let input_file_path = concat_path(&mut memory, input_dir, input_file_name);
        if let Ok(string) = read_file(&mut memory, &input_file_path) {
            write_stdout(string.as_str());
        } else {
            write_stderr("Failed to read input\n");
        }
    } else {
        write_stderr("Memory allocation failed\n");
    }

    write_stdout("Done\n");

    exit();
}

fn concat_path(memory: &mut Memory, one: &str, two: &str) -> String {
    let used_before = memory.used;
    let path_base = memory.push_str(one);
    memory.push_char(platform::PATH_SEP);
    memory.push_str(two);
    memory.push_char('\0'); // NOTE(sen) Make sure the path is null-terminated
    let path_size = memory.used - used_before;
    String {
        ptr: path_base,
        size: path_size,
    }
}
