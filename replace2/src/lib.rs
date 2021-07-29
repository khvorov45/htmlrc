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

/// If null-terminated, the terminator is included in `size`
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

    if let Ok(mut memory) = allocate_and_clear(total_memory_size) {
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

struct ComponentUsed {
    first_part: String,
    second_part: String,
    name: String,
    // TODO(sen) Implement params
}

struct ByteWindow2 {
    current_index: i32,
    last_byte_index: usize,
    base_ptr: *const u8,
    this: Byte,
    next: Byte,
}

impl ByteWindow2 {
    fn new(string: &String) -> ByteWindow2 {
        debug_assert!(string.size >= 2);
        debug_assert!(string.size <= core::i32::MAX as usize);
        ByteWindow2 {
            current_index: -1,
            last_byte_index: string.size - 1,
            base_ptr: string.ptr,
            this: Byte::default(),
            next: Byte {
                ptr: string.ptr,
                index: 0,
                value: unsafe { *string.ptr },
            },
        }
    }
    fn advance(&mut self) -> bool {
        if self.current_index < self.last_byte_index as i32 {
            self.current_index += 1;
            self.this = self.next.clone();
            let next_index = (self.current_index + 1) as usize;
            let next_ptr = unsafe { self.base_ptr.add(next_index) };
            self.next = Byte {
                ptr: next_ptr,
                index: next_index,
                value: unsafe { *next_ptr },
            };
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct Byte {
    ptr: *const u8,
    index: usize,
    value: u8,
}

impl Default for Byte {
    fn default() -> Self {
        Byte {
            ptr: core::ptr::null(),
            index: 0,
            value: b'\0',
        }
    }
}

fn resolve_components(string: &String) -> String {
    fn find_first_component(string: &String) -> Option<ComponentUsed> {
        if string.size >= 2 {
            let mut window = ByteWindow2::new(&string);
            let mut component_start = None;
            while window.advance() {
                if window.this.value == b'<' {
                    if window.next.value.is_ascii_uppercase() {
                        component_start = Some((window.this.ptr, window.this.index));
                        break;
                    } else {
                        // TODO(sen) Skip whitespaces
                    }
                }
            }

            // TODO(sen) Find the end of the component

            // TODO(sen) Replace with found component
            None
        } else {
            None
        }
    }

    while let Some(component_used) = find_first_component(&string) {
        // TODO(sen) Replace this component and change the string that goes into
        // `find_first_component`
    }

    String::new(string.ptr, string.size)
}
