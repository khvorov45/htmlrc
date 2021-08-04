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
    /// Always one byte away from `this`
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
        if self.can_advance() {
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

    fn skip_whitespace(&mut self) -> usize {
        let mut counter = 0;
        while self.this.value.is_ascii_whitespace() && self.can_advance() {
            self.advance_one();
            counter += 1;
        }
        counter
    }

    fn advance(&mut self, count: usize) -> usize {
        let mut counter = 0;
        for _ in 0..count {
            if !self.advance_one() {
                break;
            }
            counter += 1;
        }
        counter
    }

    /// Will advance just past the name
    fn next_name(&mut self) -> Option<String> {
        let mut result = None;
        self.skip_whitespace();
        if self.this.value.is_ascii_alphabetic() {
            let base = self.this.ptr;
            let mut size = 0;
            while self.this.value.is_ascii_alphanumeric() && self.can_advance() {
                size += 1;
                self.advance_one();
            }
            result = Some(String { ptr: base, size })
        }
        result
    }

    fn size_from_this(&self) -> usize {
        size_between(self.this.ptr, self.last_byte)
    }

    fn can_advance(&self) -> bool {
        self.next.ptr < self.last_byte
    }

    fn find(&mut self, target: &[u8], skip: Skip, stop: Stop) -> Option<*const u8> {
        let mut result = None;
        if !target.is_empty() {
            'search: loop {
                let test_start_ptr = self.this.ptr;
                let mut all_equal = true;
                for (target_index, target_value) in target.iter().enumerate() {
                    let test_ptr = unsafe { test_start_ptr.add(target_index) };
                    let test_value = unsafe { *test_ptr };
                    if test_value != *target_value {
                        all_equal = false;
                        break;
                    }
                }
                if all_equal {
                    result = Some(test_start_ptr);
                    match stop {
                        Stop::First => {}
                        Stop::OnePast => {
                            self.advance(target.len());
                        }
                    }
                    break 'search;
                }
                match skip {
                    Skip::Everything => {
                        if !self.advance_one() {
                            break 'search;
                        }
                    }
                    Skip::Whitespace => {
                        if !self.this.value.is_ascii_whitespace() || !self.advance_one() {
                            break 'search;
                        }
                    }
                }
            }
        }
        result
    }
}

enum Skip {
    Whitespace,
    Everything,
}

enum Stop {
    First,
    OnePast,
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
    // TODO(sen) Cleaner way to handle memory here
    let memory = unsafe { &mut *memory };
    let output_memory = unsafe { &mut *output_memory };

    // NOTE(sen) Output preparation, write final resolved string to `output_base`
    let output_used_before = output_memory.used;
    let output_base = unsafe { output_memory.base.add(output_used_before) };

    // NOTE(sen) Component resolution
    if let Some(mut window) = ByteWindow2::new(string) {
        // NOTE(sen) If there is no custom component, use this to copy input to output
        let mut search_start = window.this.ptr;
        let mut search_length = window.size_from_this();

        // NOTE(sen) Search through input and replace components with their contents
        'component_search: loop {
            // NOTE(sen) Find a custom component
            let component_used = {
                // NOTE(sen) Components start with < followed by an uppercase
                // letter, no spaces in-between
                let first_part_start = {
                    let mut result = None;
                    loop {
                        if window.this.value == b'<' && window.next.value.is_ascii_uppercase() {
                            result = Some(window.this.ptr);
                            break;
                        }
                        if !window.advance_one() {
                            break;
                        }
                    }
                    match result {
                        Some(start) => start,
                        // NOTE(sen) Component not present in window
                        None => break 'component_search,
                    }
                };

                // NOTE(sen) Advance up to the name
                window.advance_one();
                let component_name = match window.next_name() {
                    Some(name) => name,
                    // TODO(sen) Error - found start but not name
                    None => break 'component_search,
                };

                // NOTE(sen) Should be just past the name at this point

                // TODO(sen) Parse arguments

                // NOTE(sen) The opening tag should end with > for two-part
                // components and /> for one-part components
                let (first_part_end, two_part) = {
                    let mut result = None;
                    // TODO(sen) This should not be a loop
                    loop {
                        if window.this.value == b'/' && window.next.value == b'>' {
                            result = Some((window.next.ptr, false));
                            window.advance(2);
                            break;
                        } else if window.this.value == b'>' {
                            result = Some((window.this.ptr, true));
                            window.advance_one();
                            break;
                        }
                        if !window.advance_one() {
                            break;
                        }
                    }
                    match result {
                        Some(result) => result,
                        // TODO(sen) Error - found start and name but not end of that tag
                        None => break 'component_search,
                    }
                };

                let first_part = String {
                    ptr: first_part_start,
                    size: size_between(first_part_start, first_part_end),
                };

                // NOTE(sen) Second part (if present) is just </[spaces]NAME[spaces]>
                let second_part = if two_part {
                    let mut result = None;
                    loop {
                        // NOTE(sen) There shouldn't be any spaces between these two
                        if window.this.value == b'<' && window.next.value == b'/' {
                            let test_start = window.this.ptr;
                            window.advance(2);
                            // NOTE(sen) This will skip whitespaces
                            let test_name = window.next_name();
                            if test_name == Some(component_name) {
                                // NOTE(sen) Only whitespaces are allowed before closing
                                window.skip_whitespace();
                                if window.this.value == b'>' {
                                    result = Some(String {
                                        ptr: test_start,
                                        size: size_between(test_start, window.this.ptr),
                                    });
                                    window.advance_one();
                                    break;
                                } else {
                                    // TODO(sen) Error - found opening but not closing
                                }
                                break;
                            }
                        }
                        if !window.advance_one() {
                            break;
                        }
                    }
                    // TODO(sen) If none - error
                    result
                } else {
                    None
                };

                ComponentUsed {
                    first_part,
                    second_part,
                    name: component_name,
                }
            };

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

                    // NOTE(sen) Name from use
                    new_component.name = {
                        let size = component_used.name.size;
                        let ptr = memory
                            .component_names
                            .push_and_copy(component_used.name.ptr, size);
                        String { ptr, size }
                    };

                    // NOTE(sen) Read in contents from file
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
                        // NOTE(sen) Resolve other components (but not slots) in the component string
                        new_component.contents = resolve_components(
                            memory,
                            &mut memory.component_contents,
                            // NOTE(sen) We don't want any leading/trailing whitespaces in components
                            &new_component_contents_raw.trim(),
                            components,
                            input_dir,
                        );

                        // NOTE(sen) Find the slot (if present)
                        new_component.slot = None;
                        if let Some(mut component_contents_window) =
                            ByteWindow2::new(&new_component.contents)
                        {
                            let target = b"<slot";
                            if let Some(slot_start_ptr) = component_contents_window.find(
                                target,
                                Skip::Everything,
                                Stop::OnePast,
                            ) {
                                // TODO(sen) Just use .skip_whitepace()
                                if let Some(slot_end_ptr) = component_contents_window.find(
                                    b"/>",
                                    Skip::Whitespace,
                                    Stop::First,
                                ) {
                                    let whole_literal = String {
                                        ptr: slot_start_ptr,
                                        size: size_between(slot_start_ptr, slot_end_ptr) + 1,
                                    };
                                    new_component.slot = Some(Slot { whole_literal });
                                } else {
                                    // TODO(sen) Error - found start but not end
                                }
                            }
                        };
                    } else {
                        // TODO(sen) Error - component used but not found
                    }
                    new_component_contents_raw_mem.end();

                    // NOTE(sen) Append to the list
                    new_component.next = components.first;
                    components.first = Some(new_component);
                    new_component
                }
            };

            // NOTE(sen) Copy the part of the string that's before the component
            output_memory.push_and_copy_between(search_start, component_used.first_part.ptr);

            // NOTE(sen) Resolve the component appropriately
            if let Some(second_part) = component_used.second_part {
                // NOTE(sen) This is still raw input
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

                // NOTE(sen) Resolve components (but not slots) the string
                // that's in-between the component parts
                let mut component_used_contents_processed_mem = memory.input.begin_temporary();
                let component_used_contents_processed = resolve_components(
                    memory,
                    component_used_contents_processed_mem.arena(),
                    &component_used_contents_raw,
                    components,
                    input_dir,
                );
                component_used_contents_processed_mem.end();

                // TODO(sen) Resolve component slots and write the resulting string to output
            } else {
                // NOTE(sen) Replace the component with its contents
                output_memory.push_and_copy(
                    component_in_hash.contents.ptr,
                    component_in_hash.contents.size,
                );
            }

            // NOTE(sen) Reset for the next loop
            search_start = window.this.ptr;
            search_length = window.size_from_this();
        }

        // NOTE(sen) Copy the part of the input string where no component was found
        output_memory.push_and_copy(search_start, search_length);
    } else {
        // NOTE(sen) Couldn't create window - copy the whole input string
        output_memory.push_and_copy(string.ptr, string.size);
    }

    // NOTE(sen) All output should be in the output arena at this point
    #[allow(clippy::let_and_return)]
    let result = String {
        ptr: output_base,
        size: output_memory.used - output_used_before,
    };

    result
}

// TODO(sen) Debug logging
fn debug_line(string: &String) {
    use platform::{write_stdout, write_stdout_raw};
    write_stdout_raw(string.ptr, string.size);
    write_stdout("#");
    write_stdout("\n");
}
