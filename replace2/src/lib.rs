#![no_std]

mod platform;

struct Error {}

type Result<T> = core::result::Result<T, Error>;

struct Memory {
    /// Filepath for input/output, one at a time
    filepath: TransientArena,
    /// Contents of input files read as-is. Multiple at a time. Amount depends
    /// on how much components are nested
    input: TransientArena,
    /// Resolved string being built up. Multiple at a time. Amount depends on
    /// how much components are nested.
    output_processing: TransientArena,
    /// Final resolved strings. Amount is the amount of components used plus the
    /// output string
    output_final: MemoryArena,
    /// Components table. Pointers go to `output_final`
    components: MemoryArena,
}

struct MemoryArena {
    size: usize,
    base: *mut u8,
    used: usize,
}

impl MemoryArena {
    fn new(base: *mut u8, offset: &mut usize, size: usize) -> MemoryArena {
        let result = MemoryArena {
            size,
            base: unsafe { base.add(*offset) },
            used: 0,
        };
        *offset += size;
        result
    }
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
    fn push_struct<T>(&mut self) -> *mut T {
        let base = self.push_size(core::mem::size_of::<T>());
        base.cast()
    }
    /// Does not include `two`
    fn push_and_copy_between(&mut self, one: *const u8, two: *const u8) {
        let ptr_distance = two as usize - one as usize;
        self.push_and_copy(one, ptr_distance);
    }
}

struct TransientArena {
    arena: MemoryArena,
    used_count: u32,
}

impl TransientArena {
    fn new(base: *mut u8, offset: &mut usize, size: usize) -> TransientArena {
        TransientArena {
            arena: MemoryArena::new(base, offset, size),
            used_count: 0,
        }
    }
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
    fn get_arena(&mut self) -> &mut MemoryArena {
        unsafe { &mut *self.arena }
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

    // TODO(sen) Clean up &str/String nonsense
    fn from_scss(
        memory: &mut MemoryArena,
        source1: &str,
        ch: char,
        source2: &String,
        source3: &str,
    ) -> String {
        debug_assert!(string_literal_is_valid(source1));
        debug_assert!(string_literal_is_valid(source3));
        debug_assert!(char_is_valid(ch));

        let used_before = memory.used;
        let base = memory.push_and_copy(source1.as_ptr(), source1.as_bytes().len());
        memory.push_byte(ch as u8);
        memory.push_and_copy(source2.ptr, source2.size);
        memory.push_and_copy(source3.as_ptr(), source3.as_bytes().len());
        memory.push_byte(b'\0');

        String {
            ptr: base,
            size: memory.used - used_before,
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

impl core::cmp::PartialEq for String {
    fn eq(&self, other: &Self) -> bool {
        let mut result = true;
        if self.size == other.size {
            let mut self_byte = self.ptr;
            let mut other_byte = other.ptr;
            for _ in 0..self.size {
                unsafe {
                    if *self_byte != *other_byte {
                        result = false;
                        break;
                    }
                    self_byte = self_byte.add(1);
                    other_byte = other_byte.add(1);
                }
            }
        } else {
            result = false;
        }
        result
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

struct Components {
    // TODO(sen) Hash-based lookups
    first: Option<*const Component>,
}

struct Component {
    name: String,
    contents: String,
    next: Option<*const Component>,
}

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

pub fn run(input_dir: &str, input_file_name: &str, output_dir: &str) {
    use platform::{
        allocate_and_clear, create_dir_if_not_exists, exit, read_file, write_file, write_stderr,
        write_stdout, MAX_PATH_BYTES, PATH_SEP,
    };

    let (filepath_size, components_size, io_size, total_memory_size) = {
        let filepath = MAX_PATH_BYTES;
        // TODO(sen) How many components do we need?
        let components = 4096 * core::mem::size_of::<Component>();
        // TODO(sen) How big should each io arena be?
        let io = 10 * MEGABYTE;
        (filepath, components, io, filepath + components + io * 3)
    };

    if let Ok(memory_base_ptr) = allocate_and_clear(total_memory_size) {
        let mut memory = {
            let mut size_used = 0;
            let filepath = TransientArena::new(memory_base_ptr, &mut size_used, filepath_size);
            let components = MemoryArena::new(memory_base_ptr, &mut size_used, components_size);
            let input = TransientArena::new(memory_base_ptr, &mut size_used, io_size);
            let output_processing = TransientArena::new(memory_base_ptr, &mut size_used, io_size);
            let output_final = MemoryArena::new(memory_base_ptr, &mut size_used, io_size);
            debug_assert!(size_used == total_memory_size);
            Memory {
                filepath,
                input,
                output_processing,
                output_final,
                components,
            }
        };

        let mut components = Components { first: None };

        let mut filepath_memory = memory.filepath.begin_temporary();
        let input_file_path = String::from_scs(
            filepath_memory.get_arena(),
            input_dir,
            PATH_SEP,
            input_file_name,
        );

        let mut input_memory = memory.input.begin_temporary();
        if let Ok(input_string) = read_file(input_memory.get_arena(), &input_file_path) {
            memory.filepath.end_temporary(filepath_memory);

            let result = resolve_components(&mut memory, &input_string, &mut components, input_dir);

            debug_assert!(memory.filepath.used_count == 0);
            debug_assert!(memory.input.used_count == 1);
            debug_assert!(memory.output_processing.used_count == 0);

            let mut filepath_memory = memory.filepath.begin_temporary();
            let output_dir_path = String::from_s(filepath_memory.get_arena(), output_dir);
            if create_dir_if_not_exists(&output_dir_path).is_ok() {
                memory.filepath.end_temporary(filepath_memory);

                let mut filepath_memory = memory.filepath.begin_temporary();
                let output_file_path = String::from_scs(
                    filepath_memory.get_arena(),
                    output_dir,
                    PATH_SEP,
                    input_file_name,
                );
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

fn resolve_components(
    memory: &mut Memory,
    string: &String,
    components: &mut Components,
    input_dir: &str,
) -> String {
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

    let mut output_memory = memory.output_processing.begin_temporary();

    let mut string_to_parse = *string;
    while let Some(component_used) = find_first_component(&string_to_parse) {
        let component_in_hash = {
            // TODO(sen) Replace with a hash-based lookup
            let mut lookup_result = None;
            let mut component_in_hash = components.first;
            while let Some(component_in_hash_ptr) = component_in_hash {
                let component_in_hash_value = unsafe { &*component_in_hash_ptr };
                if component_in_hash_value.name == component_used.name {
                    lookup_result = Some(component_in_hash_value);
                    break;
                } else {
                    component_in_hash = component_in_hash_value.next;
                }
            }

            if let Some(component_looked_up) = lookup_result {
                component_looked_up
            } else {
                // NOTE(sen) This should be zeroed since the areana is never overwritten
                let new_component = unsafe { &mut *memory.components.push_struct::<Component>() };
                new_component.name = {
                    let size = component_used.name.size;
                    let ptr = memory
                        .output_final
                        .push_and_copy(component_used.name.ptr, size);
                    String { ptr, size }
                };

                let mut filepath_memory = memory.filepath.begin_temporary();
                let new_component_path = String::from_scss(
                    filepath_memory.get_arena(),
                    input_dir,
                    platform::PATH_SEP,
                    &new_component.name,
                    ".html",
                );
                let mut new_component_contents_raw_mem = memory.output_processing.begin_temporary();
                let new_component_contents_raw_result = platform::read_file(
                    new_component_contents_raw_mem.get_arena(),
                    &new_component_path,
                );
                memory.filepath.end_temporary(filepath_memory);

                if let Ok(new_component_contents_raw) = new_component_contents_raw_result {
                    // TODO(sen) Remove the trailing whitespaces
                    new_component.contents = resolve_components(
                        memory,
                        &new_component_contents_raw,
                        components,
                        input_dir,
                    );
                } else {
                    // TODO(sen) Error - component used but not found
                }
                memory
                    .output_processing
                    .end_temporary(new_component_contents_raw_mem);

                new_component.next = components.first;
                components.first = Some(new_component);
                new_component
            }
        };

        output_memory
            .get_arena()
            .push_and_copy_between(string_to_parse.ptr, component_used.first_part.ptr);

        if let Some(second_part) = component_used.second_part {
            // TODO(sen) Handle two-parters
        } else {
            // NOTE(sen) Replace the component with its contents
            output_memory.get_arena().push_and_copy(
                component_in_hash.contents.ptr,
                component_in_hash.contents.size,
            );

            // NOTE(sen) Shrink the input string we are parsing
            string_to_parse.set_ptr(unsafe {
                component_used
                    .first_part
                    .ptr
                    .add(component_used.first_part.size)
            });
        }
    }

    // NOTE(sen) Copy the remaining input string
    output_memory
        .get_arena()
        .push_and_copy(string_to_parse.ptr, string_to_parse.size);

    let output_string_permanent = {
        let output_arena = unsafe { &*output_memory.arena };
        let source = unsafe { output_arena.base.add(output_memory.used_before) };
        let size = output_arena.used - output_memory.used_before;
        let base = memory.output_final.push_and_copy(source, size);
        String { ptr: base, size }
    };

    memory.output_processing.end_temporary(output_memory);

    output_string_permanent
}

fn debug_line(string: &String) {
    use platform::{write_stdout, write_stdout_raw};
    write_stdout_raw(string.ptr, string.size);
    write_stdout("#");
    write_stdout("\n");
}
