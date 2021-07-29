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
}

/// Always null-terminated, the null-terminator is included in `size`
struct String {
    ptr: *const u8,
    size: usize,
}

impl String {
    fn new(ptr: *const u8, size: usize) -> String {
        debug_assert!({
            let last_char: char = unsafe { *ptr.add(size).cast() };
            last_char == '\0'
        });
        String { ptr, size }
    }
    fn from_s(memory: &mut Memory, source: &str) -> String {
        debug_assert!(string_literal_is_valid(source));

        let source_bytes = source.as_bytes();
        let source_size = source_bytes.len();
        let total_size = source_size + 1; // NOTE(sen) For the null terminator

        let first_byte = memory.push_size(total_size);
        let mut dest = first_byte;
        for source_byte in source_bytes {
            unsafe {
                *dest = *source_byte;
                dest = dest.add(1);
            };
        }
        unsafe { *dest = b'\0' };

        String {
            ptr: first_byte,
            size: total_size,
        }
    }
    fn from_scs(memory: &mut Memory, source1: &str, ch: char, source2: &str) -> String {
        debug_assert!(string_literal_is_valid(source1));
        debug_assert!(string_literal_is_valid(source2));
        debug_assert!(char_is_valid(ch));

        let source1_bytes = source1.as_bytes();
        let source1_size = source1_bytes.len();

        let source2_bytes = source2.as_bytes();
        let source2_size = source2_bytes.len();

        let total_size = source1_size + source2_size + 1; // NOTE(sen) For the null terminator

        let first_byte = memory.push_size(total_size);
        let mut dest = first_byte;
        for source_byte in source1_bytes {
            unsafe {
                *dest = *source_byte;
                dest = dest.add(1);
            };
        }
        unsafe {
            *dest = ch as u8;
            dest = dest.add(1);
        };
        for source_byte in source2_bytes {
            unsafe {
                *dest = *source_byte;
                dest = dest.add(1);
            };
        }
        unsafe { *dest = b'\0' };

        String {
            ptr: first_byte,
            size: total_size,
        }
    }
    fn _as_str(&self) -> &str {
        unsafe {
            core::str::from_utf8_unchecked(
                core::ptr::slice_from_raw_parts(self.ptr, self.size)
                    .as_ref()
                    .unwrap(),
            )
        }
    }
}

fn string_literal_is_valid(literal: &str) -> bool {
    let mut result = true;
    for ch in literal.chars() {
        if !char_is_valid(ch) {
            result = false;
            break;
        }
    }
    result
}

fn char_is_valid(ch: char) -> bool {
    ch.is_ascii() || ch == '\0'
}

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

pub fn run(input_dir: &str, input_file_name: &str, output_dir: &str) {
    use platform::{
        allocate_and_clear, create_dir_if_not_exists, exit, read_file, write_file, write_stderr,
        write_stdout, PATH_SEP,
    };

    let total_memory_size = 10 * MEGABYTE;

    if let Ok(memory) = allocate_and_clear(total_memory_size) {
        let mut memory = Memory {
            size: total_memory_size,
            base: memory,
            used: 0,
        };
        let input_file_path = String::from_scs(&mut memory, input_dir, PATH_SEP, input_file_name);
        if let Ok(input_string) = read_file(&mut memory, &input_file_path) {
            let result = resolve_components(&input_string);
            let output_dir_path = String::from_s(&mut memory, output_dir);
            if create_dir_if_not_exists(&output_dir_path).is_ok() {
                let output_file_path =
                    String::from_scs(&mut memory, output_dir, PATH_SEP, input_file_name);
                if write_file(&output_file_path, &result).is_ok() {
                    write_stdout("Done\n");
                } else {
                    write_stderr("Failed to write to output file\n");
                }
            } else {
                write_stderr("Failed to create output directory\n");
            }
        } else {
            write_stderr("Failed to read input\n");
        }
    } else {
        write_stderr("Memory allocation failed\n");
    }

    exit();
}

fn resolve_components(string: &String) -> String {
    // TODO(sen) Actually implement this
    String::new(string.ptr, string.size)
}
