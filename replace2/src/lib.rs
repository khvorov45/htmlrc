#![no_std]

mod platform;

struct Error {}

type Result<T> = core::result::Result<T, Error>;

struct Memory {
    permanent: MemoryArena,
    transient: TransientArena,
}

struct MemoryArena {
    size: usize,
    base: *mut u8,
    used: usize,
}

impl MemoryArena {
    fn push_size(&mut self, size: usize) -> *mut u8 {
        debug_assert!(self.size - self.used >= size);
        let result = unsafe { self.base.add(self.used) };
        self.used += size;
        result
    }
    fn push_and_copy(&mut self, ptr: *const u8, size: usize) -> *mut u8 {
        let base = self.push_size(size);
        let mut dest = base;
        let mut source = ptr;
        for _ in 0..size {
            unsafe {
                *dest = *source;
                dest = dest.add(1);
                source = source.add(1);
            }
        }
        base
    }
    fn push_byte(&mut self, byte: u8) -> *mut u8 {
        let base = self.push_size(1);
        unsafe { *base = byte };
        base
    }
}

struct TransientArena {
    arena: MemoryArena,
    used_count: u32,
}

impl TransientArena {
    fn begin_temporary(&mut self) -> TemporaryMemory {
        self.used_count += 1;
        TemporaryMemory {
            arena: &mut self.arena,
            used_before: self.arena.used,
        }
    }
    fn end_temporary(&mut self, temporary_memory: TemporaryMemory) {
        debug_assert!(self.used_count >= 1);
        self.used_count -= 1;
        unsafe { (*temporary_memory.arena).used = temporary_memory.used_before };
    }
}

struct TemporaryMemory {
    arena: *mut MemoryArena,
    used_before: usize,
}

impl TemporaryMemory {
    /// Does not include `two`
    fn copy_between(&mut self, one: *const u8, two: *const u8) {
        let ptr_distance = two as usize - one as usize;
        let arena = unsafe { &mut *self.arena };
        arena.push_and_copy(one, ptr_distance);
    }
}

/// If null-terminated, the terminator is included in `size`
#[derive(Clone, Copy)]
struct String {
    ptr: *mut u8,
    size: usize,
}

impl String {
    fn from_s(memory: &mut MemoryArena, source: &str) -> String {
        debug_assert!(string_literal_is_valid(source));

        let used_before = memory.used;
        let base = memory.push_and_copy(source.as_ptr(), source.as_bytes().len());
        memory.push_byte(b'\0');

        String {
            ptr: base,
            size: memory.used - used_before,
        }
    }

    fn from_scs(memory: &mut MemoryArena, source1: &str, ch: char, source2: &str) -> String {
        debug_assert!(string_literal_is_valid(source1));
        debug_assert!(string_literal_is_valid(source2));
        debug_assert!(char_is_valid(ch));

        let used_before = memory.used;
        let base = memory.push_and_copy(source1.as_ptr(), source1.as_bytes().len());
        memory.push_byte(ch as u8);
        memory.push_and_copy(source2.as_ptr(), source2.as_bytes().len());
        memory.push_byte(b'\0');

        String {
            ptr: base,
            size: memory.used - used_before,
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

    fn set_ptr(&mut self, new_ptr: *mut u8) {
        let ptr_distance = new_ptr as usize - self.ptr as usize;
        debug_assert!(ptr_distance < self.size);
        let new_size = self.size - ptr_distance;
        self.size = new_size;
        self.ptr = new_ptr;
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

    if let Ok(memory_base_ptr) = allocate_and_clear(total_memory_size) {
        // TODO(sen) Come up with a more robust memory model. The output is an
        // HTML file that's meant to be transferred quickly over the network, so
        // ~1 MB in size. If the output is 1MB then the sum of all input can't
        // be more than 1MB. Allocating 5MB for the permanent store (holds final
        // output string and all the input strings) and 5MB for the transient
        // store (holds strings that are in the process of being built up) seems
        // sufficient for now. The memory for the transient store sets the
        // `resolve_components` recursion limit.
        let mut memory = {
            let permanent = MemoryArena {
                size: total_memory_size / 2,
                base: memory_base_ptr,
                used: 0,
            };
            let transient_arena = MemoryArena {
                size: total_memory_size - permanent.size,
                base: unsafe { permanent.base.add(permanent.size) },
                used: 0,
            };
            let transient = TransientArena {
                arena: transient_arena,
                used_count: 0,
            };
            Memory {
                permanent,
                transient,
            }
        };

        let input_file_path =
            String::from_scs(&mut memory.permanent, input_dir, PATH_SEP, input_file_name);

        if let Ok(input_string) = read_file(&mut memory.permanent, &input_file_path) {
            let result = resolve_components(&mut memory, &input_string);
            debug_assert!(memory.transient.used_count == 0);

            let output_dir_path = String::from_s(&mut memory.permanent, output_dir);
            if create_dir_if_not_exists(&output_dir_path).is_ok() {
                let output_file_path =
                    String::from_scs(&mut memory.permanent, output_dir, PATH_SEP, input_file_name);
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
    second_part: Option<String>,
    name: String,
    // TODO(sen) Implement params
}

struct ByteWindow2 {
    last_byte_index: usize,
    base_ptr: *mut u8,
    this: Byte,
    next: Byte,
}

impl ByteWindow2 {
    fn new(string: &String) -> Option<ByteWindow2> {
        if string.size >= 2 {
            let second_ptr = unsafe { string.ptr.add(1) };
            Some(ByteWindow2 {
                last_byte_index: string.size - 1,
                base_ptr: string.ptr,
                this: Byte {
                    ptr: string.ptr,
                    index: 0,
                    value: unsafe { *string.ptr },
                },
                next: Byte {
                    ptr: second_ptr,
                    index: 1,
                    value: unsafe { *second_ptr },
                },
            })
        } else {
            None
        }
    }

    fn advance_one(&mut self) -> bool {
        if self.next.index < self.last_byte_index {
            self.this = self.next;
            let next_index = (self.next.index + 1) as usize;
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

    fn advance_past_whitespace(&mut self) -> bool {
        if self.next.index <= self.last_byte_index {
            // NOTE(sen) Find the next non-whitespace character
            let mut non_whitespace_found = false;
            for index in (self.next.index + 1)..=self.last_byte_index {
                let ptr = unsafe { self.base_ptr.add(index) };
                let value = unsafe { *ptr };
                if !value.is_ascii_whitespace() {
                    self.this = self.next;
                    self.next = Byte { ptr, index, value };
                    non_whitespace_found = true;
                    break;
                }
            }
            non_whitespace_found
        } else {
            false
        }
    }
}

#[derive(Clone, Copy)]
struct Byte {
    ptr: *mut u8,
    index: usize,
    value: u8,
}

fn resolve_components(memory: &mut Memory, string: &String) -> String {
    fn find_first_component(string: &String) -> Option<ComponentUsed> {
        if let Some(mut window) = ByteWindow2::new(string) {
            let component_start = {
                let mut result = None;
                loop {
                    if window.this.value == b'<' && window.next.value.is_ascii_uppercase() {
                        result = Some((window.this.ptr, window.this.index));
                        window.advance_one();
                        window.advance_one();
                        break;
                    }
                    if !window.advance_past_whitespace() {
                        break;
                    }
                }
                result
            };

            if let Some((start_ptr, start_index)) = component_start {
                let component_name = {
                    let name_start_ptr = unsafe { start_ptr.add(1) };
                    let mut name_length = 1;
                    loop {
                        if window.this.value.is_ascii_alphanumeric() {
                            name_length += 1;
                        } else {
                            break;
                        }
                        if !window.advance_one() {
                            break;
                        }
                    }
                    String {
                        ptr: name_start_ptr,
                        size: name_length,
                    }
                };

                // TODO(sen) Parse arguments

                let component_end = {
                    let mut result = None;
                    loop {
                        if window.this.value == b'/' && window.next.value == b'>' {
                            result = Some((window.next.index, false));
                            window.advance_one();
                            window.advance_one();
                            break;
                        } else if window.this.value == b'>' {
                            result = Some((window.this.index, true));
                            window.advance_one();
                            break;
                        }
                        if !window.advance_past_whitespace() {
                            break;
                        }
                    }
                    result
                };

                if let Some((end_index, two_part)) = component_end {
                    if two_part {
                        // TODO(sen) Handle two-parters
                        None
                    } else {
                        let component_size = end_index - start_index + 1;
                        let component_string = String {
                            ptr: start_ptr,
                            size: component_size,
                        };
                        Some(ComponentUsed {
                            first_part: component_string,
                            second_part: None,
                            name: component_name,
                        })
                    }
                } else {
                    // TODO(sen) This should be an error - found start but not end
                    None
                }
            } else {
                // NOTE(sen) Did not find start
                None
            }
        } else {
            // NOTE(sen) Couldn't create window
            None
        }
    }

    let mut output_memory = memory.transient.begin_temporary();

    let mut string_to_parse = *string;
    while let Some(component_used) = find_first_component(&string_to_parse) {
        // TODO(sen) Read in component contents and store them somewhere (don't read if found)

        debug_line(&component_used.first_part);
        debug_line(&component_used.name);

        output_memory.copy_between(string_to_parse.ptr, component_used.first_part.ptr);

        if let Some(second_part) = component_used.second_part {
            // TODO(sen) Handle two-parters
        } else {
            // TODO(sen) Replace the component with its contents

            string_to_parse.set_ptr(unsafe {
                component_used
                    .first_part
                    .ptr
                    .add(component_used.first_part.size)
            });
        }
    }

    let output_string_permanent = {
        let output_arena = unsafe { &*output_memory.arena };
        let source = unsafe { output_arena.base.add(output_memory.used_before) };
        let size = output_arena.used - output_memory.used_before;
        let base = memory.permanent.push_and_copy(source, size);
        String { ptr: base, size }
    };

    memory.transient.end_temporary(output_memory);

    output_string_permanent
}

fn debug_line(string: &String) {
    use platform::write_stdout;
    write_stdout(string._as_str());
    write_stdout("#");
    write_stdout("\n");
}
