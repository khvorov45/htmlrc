#![no_std]

mod platform;

struct Error {}

type Result<T> = core::result::Result<T, Error>;

struct Memory {
    /// Filepath for input/output, one at a time
    filepath: MemoryArena,
    /// Contents of input files read as-is. Multiple at a time. Amount depends
    /// on how much components are nested
    input: MemoryArena,
    /// Final resolved string
    output: MemoryArena,
    /// Components table
    components: MemoryArena,
    component_names: MemoryArena,
    component_contents: MemoryArena,
}

struct MemoryArena {
    size: usize,
    base: *mut u8,
    used: usize,
    temporary_count: usize,
}

impl MemoryArena {
    fn new(base: *mut u8, offset: &mut usize, size: usize) -> MemoryArena {
        let result = MemoryArena {
            size,
            base: unsafe { base.add(*offset) },
            used: 0,
            temporary_count: 0,
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
    fn begin_temporary(&mut self) -> TemporaryMemory {
        self.temporary_count += 1;
        TemporaryMemory {
            arena: self,
            used_before: self.used,
        }
    }
}

struct TemporaryMemory {
    arena: *mut MemoryArena,
    used_before: usize,
}

impl TemporaryMemory {
    fn arena(&mut self) -> &mut MemoryArena {
        unsafe { &mut *self.arena }
    }
    fn reset(&mut self) {
        self.arena().used = self.used_before;
    }
    fn end(mut self) {
        let used_before = self.used_before;
        let arena = self.arena();
        debug_assert!(arena.temporary_count >= 1);
        arena.temporary_count -= 1;
        arena.used = used_before;
    }
}

/// If null-terminated, the terminator is included in `size`
#[derive(Clone, Copy)]
struct String {
    ptr: *const u8,
    size: usize,
}

impl String {
    fn from_s(memory: &mut MemoryArena, source: &String) -> String {
        let used_before = memory.used;
        let base = memory.push_and_copy(source.ptr, source.size);
        memory.push_byte(b'\0');
        String {
            ptr: base,
            size: memory.used - used_before,
        }
    }

    fn from_scs(memory: &mut MemoryArena, source1: &String, ch: char, source2: &String) -> String {
        debug_assert!(char_is_valid(ch));
        let used_before = memory.used;
        let base = memory.push_and_copy(source1.ptr, source1.size);
        memory.push_byte(ch as u8);
        memory.push_and_copy(source2.ptr, source2.size);
        memory.push_byte(b'\0');
        String {
            ptr: base,
            size: memory.used - used_before,
        }
    }

    fn from_scss(
        memory: &mut MemoryArena,
        source1: &String,
        ch: char,
        source2: &String,
        source3: &String,
    ) -> String {
        debug_assert!(char_is_valid(ch));
        let used_before = memory.used;
        let base = memory.push_and_copy(source1.ptr, source1.size);
        memory.push_byte(ch as u8);
        memory.push_and_copy(source2.ptr, source2.size);
        memory.push_and_copy(source3.ptr, source3.size);
        memory.push_byte(b'\0');
        String {
            ptr: base,
            size: memory.used - used_before,
        }
    }

    /// Does not modify memory
    fn trim(&self) -> String {
        let mut byte = self.ptr;
        let mut first_non_whitespace = 0;
        while unsafe { *byte }.is_ascii_whitespace() {
            byte = unsafe { byte.add(1) };
            first_non_whitespace += 1;
        }
        byte = unsafe { self.ptr.add(self.size - 1) };
        let mut last_non_whitespace = self.size - 1;
        while unsafe { *byte }.is_ascii_whitespace() {
            byte = unsafe { byte.sub(1) };
            last_non_whitespace -= 1;
        }
        let ptr;
        let size;
        if last_non_whitespace < first_non_whitespace {
            // NOTE(sen) This is a whitespace-only string
            ptr = self.ptr;
            size = 0
        } else {
            ptr = unsafe { self.ptr.add(first_non_whitespace) };
            size = last_non_whitespace - first_non_whitespace + 1;
        }
        String { ptr, size }
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

trait ToString {
    fn to_string(&self) -> String;
}
impl ToString for str {
    fn to_string(&self) -> String {
        debug_assert!(string_literal_is_valid(self));
        String {
            ptr: self.as_ptr(),
            size: self.as_bytes().len(),
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

struct Components {
    // TODO(sen) Hash-based lookups
    first: Option<*const Component>,
}

struct Component {
    name: String,
    /// Leading and trailing whitespaces are removed
    contents: String,
    // TODO(sen) Handle multiple slots
    /// For components with children (two-parters) only
    slot: Option<Slot>,
    next: Option<*const Component>,
}

struct Slot {
    whole_literal: String,
}

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

pub fn run(input_dir: &str, input_file_name: &str, output_dir: &str) {
    use platform::{
        allocate_and_clear, create_dir_if_not_exists, exit, read_file, write_file, write_stderr,
        write_stdout, MAX_FILENAME_BYTES, MAX_PATH_BYTES, PATH_SEP,
    };

    let input_dir = input_dir.to_string();
    let input_file_name = input_file_name.to_string();
    let output_dir = output_dir.to_string();

    let (
        filepath_size,
        components_size,
        component_names_size,
        component_contents_size,
        io_size,
        total_memory_size,
    ) = {
        let filepath = MAX_PATH_BYTES;
        // TODO(sen) How many components do we need?
        let components_count = 512;
        let components = components_count * core::mem::size_of::<Component>();
        let component_names = components_count * MAX_FILENAME_BYTES;
        // TODO(sen) How big do we expect each component to be?
        let component_contents = components_count * 128 * KILOBYTE;
        // TODO(sen) How big should each io arena be?
        let io = 10 * MEGABYTE;
        (
            filepath,
            components,
            component_names,
            component_contents,
            io,
            filepath + components + component_names + component_contents + io * 2,
        )
    };

    if let Ok(memory_base_ptr) = allocate_and_clear(total_memory_size) {
        let mut memory = {
            let mut size_used = 0;
            let filepath = MemoryArena::new(memory_base_ptr, &mut size_used, filepath_size);
            let components = MemoryArena::new(memory_base_ptr, &mut size_used, components_size);
            let component_names =
                MemoryArena::new(memory_base_ptr, &mut size_used, component_names_size);
            let component_contents =
                MemoryArena::new(memory_base_ptr, &mut size_used, component_contents_size);
            let input = MemoryArena::new(memory_base_ptr, &mut size_used, io_size);
            let output = MemoryArena::new(memory_base_ptr, &mut size_used, io_size);
            debug_assert!(size_used == total_memory_size);
            Memory {
                filepath,
                input,
                output,
                components,
                component_names,
                component_contents,
            }
        };

        let mut components = Components { first: None };

        let mut filepath_memory = memory.filepath.begin_temporary();
        let input_file_path = String::from_scs(
            filepath_memory.arena(),
            &input_dir,
            PATH_SEP,
            &input_file_name,
        );

        let mut input_memory = memory.input.begin_temporary();
        if let Ok(input_string) = read_file(input_memory.arena(), &input_file_path) {
            filepath_memory.end();

            let result = resolve_components(
                &mut memory,
                &mut memory.output,
                &input_string,
                &mut components,
                &input_dir,
            );

            debug_assert!(memory.filepath.temporary_count == 0);
            debug_assert!(memory.input.temporary_count == 1);

            let mut filepath_memory = memory.filepath.begin_temporary();
            let output_dir_path = String::from_s(filepath_memory.arena(), &output_dir);
            if create_dir_if_not_exists(&output_dir_path).is_ok() {
                filepath_memory.reset();
                let output_file_path = String::from_scs(
                    filepath_memory.arena(),
                    &output_dir,
                    PATH_SEP,
                    &input_file_name,
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

fn size_between(ptr1: *const u8, ptr2: *const u8) -> usize {
    debug_assert!(ptr2 > ptr1);
    ptr2 as usize - ptr1 as usize + 1
}

struct ComponentUsed {
    first_part: String,
    second_part: Option<String>,
    name: String,
    // TODO(sen) Implement params
}

struct ByteWindow2 {
    last_byte: *const u8,
    this: Byte,
    next: Byte,
}

impl ByteWindow2 {
    fn new(string: &String) -> Option<ByteWindow2> {
        if string.size >= 2 {
            let second_ptr = unsafe { string.ptr.add(1) };
            Some(ByteWindow2 {
                last_byte: unsafe { string.ptr.add(string.size - 1) },
                this: Byte {
                    ptr: string.ptr,
                    value: unsafe { *string.ptr },
                },
                next: Byte {
                    ptr: second_ptr,
                    value: unsafe { *second_ptr },
                },
            })
        } else {
            None
        }
    }

    fn advance_one(&mut self) -> bool {
        if self.next.ptr < self.last_byte {
            self.this = self.next;
            let next_ptr = unsafe { self.next.ptr.add(1) };
            self.next = Byte {
                ptr: next_ptr,
                value: unsafe { *next_ptr },
            };
            true
        } else {
            false
        }
    }

    fn advance_past_whitespace(&mut self) -> bool {
        if self.next.ptr < self.last_byte {
            // NOTE(sen) Find the next non-whitespace character
            let mut non_whitespace_found = false;
            let mut ptr = unsafe { self.next.ptr.add(1) };
            while ptr <= self.last_byte {
                let value = unsafe { *ptr };
                if !value.is_ascii_whitespace() {
                    self.this = self.next;
                    self.next = Byte { ptr, value };
                    non_whitespace_found = true;
                    break;
                }
                ptr = unsafe { ptr.add(1) };
            }
            non_whitespace_found
        } else {
            false
        }
    }

    fn next_name(&mut self) -> Option<String> {
        let start = {
            let mut result = None;
            loop {
                if self.this.value.is_ascii_alphabetic() {
                    result = Some(self.this.ptr);
                    break;
                } else if !self.this.value.is_ascii_whitespace() {
                    break;
                }
                if !self.advance_one() {
                    break;
                }
            }
            result
        };
        if let Some(start) = start {
            let mut size = 0;
            loop {
                if self.this.value.is_ascii_alphanumeric() {
                    size += 1;
                } else {
                    break;
                }
                if !self.advance_one() {
                    break;
                }
            }
            let result = String { ptr: start, size };
            Some(result)
        } else {
            None
        }
    }

    fn remaining(&self) -> usize {
        size_between(self.this.ptr, self.last_byte)
    }
}

#[derive(Clone, Copy)]
struct Byte {
    ptr: *const u8,
    value: u8,
}

fn resolve_components(
    memory: *mut Memory,
    output_memory: *mut MemoryArena,
    string: &String,
    components: &mut Components,
    input_dir: &String,
) -> String {
    fn find_next_component(window: &mut ByteWindow2) -> Option<ComponentUsed> {
        let first_part_start = {
            let mut result = None;
            loop {
                if window.this.value == b'<' && window.next.value.is_ascii_uppercase() {
                    result = Some(window.this.ptr);
                    break;
                }
                if !window.advance_past_whitespace() {
                    break;
                }
            }
            result?
        };

        window.advance_one();
        // TODO(sen) If none - error
        let component_name = window.next_name()?;

        // TODO(sen) Parse arguments

        let (first_part_end, two_part) = {
            let mut result = None;
            loop {
                if window.this.value == b'/' && window.next.value == b'>' {
                    result = Some((window.next.ptr, false));
                    window.advance_one();
                    window.advance_one();
                    break;
                } else if window.this.value == b'>' {
                    result = Some((window.this.ptr, true));
                    window.advance_one();
                    break;
                }
                if !window.advance_past_whitespace() {
                    break;
                }
            }
            // TODO(sen) If none - error
            result?
        };

        let first_part = String {
            ptr: first_part_start,
            size: size_between(first_part_start, first_part_end),
        };

        let second_part = if two_part {
            let second_part_start = {
                let mut result = None;
                loop {
                    let test_start = {
                        let mut result = None;
                        loop {
                            if window.this.value == b'<' && window.next.value == b'/' {
                                result = Some(window.this.ptr);
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
                    let test_name = window.next_name();
                    if test_name == Some(component_name) {
                        result = test_start;
                        break;
                    }
                    if !window.advance_past_whitespace() {
                        break;
                    }
                }
                // TODO(sen) If none - error
                result?
            };
            let second_part_end = {
                let mut result = None;
                loop {
                    if window.this.value == b'>' {
                        result = Some(window.this.ptr);
                        window.advance_one();
                        break;
                    }
                    if !window.advance_past_whitespace() {
                        break;
                    }
                }
                // TODO(sen) If none - error
                result?
            };
            let second_part = String {
                ptr: second_part_start,
                size: size_between(second_part_start, second_part_end),
            };
            Some(second_part)
        } else {
            None
        };

        Some(ComponentUsed {
            first_part,
            second_part,
            name: component_name,
        })
    }

    let memory = unsafe { &mut *memory };
    let output_memory = unsafe { &mut *output_memory };
    let output_used_before = output_memory.used;
    let output_base = unsafe { output_memory.base.add(output_used_before) };
    if let Some(mut window) = ByteWindow2::new(string) {
        let mut search_start = window.this.ptr;
        let mut search_length = window.remaining();
        while let Some(component_used) = find_next_component(&mut window) {
            // NOTE(sen) Find the component in cache or read it anew and store it in cache
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
                    let new_component =
                        unsafe { &mut *memory.components.push_struct::<Component>() };
                    new_component.name = {
                        let size = component_used.name.size;
                        let ptr = memory
                            .component_names
                            .push_and_copy(component_used.name.ptr, size);
                        String { ptr, size }
                    };

                    let mut filepath_memory = memory.filepath.begin_temporary();
                    let new_component_path = String::from_scss(
                        filepath_memory.arena(),
                        input_dir,
                        platform::PATH_SEP,
                        &new_component.name,
                        &".html".to_string(),
                    );
                    let mut new_component_contents_raw_mem = memory.input.begin_temporary();
                    let new_component_contents_raw_result = platform::read_file(
                        new_component_contents_raw_mem.arena(),
                        &new_component_path,
                    );
                    filepath_memory.end();

                    if let Ok(new_component_contents_raw) = new_component_contents_raw_result {
                        new_component.contents = resolve_components(
                            memory,
                            &mut memory.component_contents,
                            // NOTE(sen) We don't want any leading/trailing whitespaces in components
                            &new_component_contents_raw.trim(),
                            components,
                            input_dir,
                        );
                    } else {
                        // TODO(sen) Error - component used but not found
                    }
                    new_component_contents_raw_mem.end();

                    new_component.next = components.first;
                    components.first = Some(new_component);
                    new_component
                }
            };

            // NOTE(sen) Copy the part of the string that's before the component
            output_memory.push_and_copy_between(search_start, component_used.first_part.ptr);

            if let Some(second_part) = component_used.second_part {
                let component_used_contents_raw = {
                    let base = unsafe {
                        component_used
                            .first_part
                            .ptr
                            .add(component_used.first_part.size)
                    };
                    let one_past_end = second_part.ptr;
                    let size = one_past_end as usize - base as usize;
                    String { ptr: base, size }
                };

                let mut component_used_contents_processed_mem = memory.input.begin_temporary();
                let component_used_contents_processed = resolve_components(
                    memory,
                    component_used_contents_processed_mem.arena(),
                    &component_used_contents_raw,
                    components,
                    input_dir,
                );

                component_used_contents_processed_mem.end();

                // TODO(sen) Replace the component with its contents. Those contents
                // will need their slots resolved with the above string
            } else {
                // NOTE(sen) Replace the component with its contents
                output_memory.push_and_copy(
                    component_in_hash.contents.ptr,
                    component_in_hash.contents.size,
                );
            }

            // NOTE(sen) Reset for the next loop
            search_start = window.this.ptr;
            search_length = window.remaining();
        }
        // NOTE(sen) Copy the part of the input string where no component was found
        output_memory.push_and_copy(search_start, search_length);
    } else {
        // NOTE(sen) Couldn't create window - copy the whole input string
        output_memory.push_and_copy(string.ptr, string.size);
    }

    #[allow(clippy::let_and_return)]
    let result = String {
        ptr: output_base,
        size: output_memory.used - output_used_before,
    };

    result
}

fn debug_line(string: &String) {
    use platform::{write_stdout, write_stdout_raw};
    write_stdout_raw(string.ptr, string.size);
    write_stdout("#");
    write_stdout("\n");
}
