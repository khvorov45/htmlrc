#![no_std]

mod platform;

struct Error {}

type Result<T> = core::result::Result<T, Error>;

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

pub fn handle_panic(info: &core::panic::PanicInfo) {
    log_error!("{}\n", info);
    platform::exit();
}

pub fn run(input_dir: &str, input_file_name: &str, output_dir: &str) {
    use platform::{
        allocate_and_clear, create_dir_if_not_exists, exit, get_seconds_from, get_timespec_now,
        last_cycle_count, read_file, write_file, MAX_FILENAME_BYTES, MAX_PATH_BYTES, PATH_SEP,
    };

    let program_start_time = get_timespec_now();
    let program_start_cycle = last_cycle_count();

    let input_dir = input_dir.to_string();
    let input_file_name = input_file_name.to_string();
    let output_dir = output_dir.to_string();

    log_debug_title("START");
    log_debug!("Input directory: {}\n", &input_dir);
    log_debug!("Input file: {}\n", &input_file_name);
    log_debug!("output_dir: {}\n", &output_dir);
    log_debug_line_sep();

    // TODO(sen) Actually read the directory here
    let (total_html_file_count, total_html_file_size) = (512, 512 * 128 * KILOBYTE);

    let filepath_size = MAX_PATH_BYTES;
    let components_size = total_html_file_count * core::mem::size_of::<Component>();
    let component_names_size = total_html_file_count * MAX_FILENAME_BYTES;
    let component_contents_size = total_html_file_size;
    let component_arguments_size = 512 * core::mem::size_of::<Argument>(); // TODO(sen) How many?
    let input_size = total_html_file_size;
    let output_size = 10 * MEGABYTE;
    let total_size = filepath_size
        + components_size
        + component_names_size
        + component_contents_size
        + component_arguments_size
        + input_size
        + output_size;

    log_debug_title("MEMORY");
    log_debug!("Filepath: {}B\n", filepath_size);
    log_debug!("Components: {}KB\n", components_size / 1024);
    log_debug!("Component names: {}KB\n", component_names_size / 1024);
    log_debug!(
        "Component contents: {}MB\n",
        component_contents_size / 1024 / 1024
    );
    log_debug!(
        "Component arguments: {}KB\n",
        component_arguments_size / 1024
    );
    log_debug!("Input: {}MB\n", input_size / 1024 / 1024);
    log_debug!("Total: {}MB\n", total_size / 1024 / 1024);
    log_debug_line_sep();

    if let Ok(memory_base_ptr) = allocate_and_clear(total_size) {
        let mut memory = {
            let mut size_used = 0;
            let filepath = MemoryArena::new(memory_base_ptr, &mut size_used, filepath_size);
            let components = MemoryArena::new(memory_base_ptr, &mut size_used, components_size);
            let component_names =
                MemoryArena::new(memory_base_ptr, &mut size_used, component_names_size);
            let component_contents =
                MemoryArena::new(memory_base_ptr, &mut size_used, component_contents_size);
            let component_arguments =
                MemoryArena::new(memory_base_ptr, &mut size_used, component_arguments_size);
            let input = MemoryArena::new(memory_base_ptr, &mut size_used, input_size);
            let output = MemoryArena::new(memory_base_ptr, &mut size_used, output_size);
            debug_assert!(size_used == total_size);
            Memory {
                filepath,
                input,
                output,
                components,
                component_names,
                component_contents,
                component_arguments,
            }
        };

        let mut components = Components {
            first: core::ptr::null(),
            count: 0,
        };

        let mut filepath_memory = memory.filepath.begin_temporary();
        let input_file_path = String::from_scs(
            filepath_memory.arena.as_ref_mut(),
            &input_dir,
            PATH_SEP,
            &input_file_name,
        );

        let mut input_memory = memory.input.begin_temporary();
        if let Ok(input_string) = read_file(input_memory.arena.as_ref_mut(), &input_file_path) {
            log_debug!("started resolution of input at {}\n", input_file_path);
            filepath_memory.end();
            let result = resolve(
                &mut memory,
                &mut memory.output,
                &input_string,
                &mut components,
                &input_dir,
                None,
            );
            log_debug!("input resolution finished\n");

            debug_assert!(memory.filepath.temporary_count == 0);
            debug_assert!(memory.input.temporary_count == 1);

            let mut filepath_memory = memory.filepath.begin_temporary();
            let output_dir_path = String::from_s(filepath_memory.arena.as_ref_mut(), &output_dir);
            if create_dir_if_not_exists(&output_dir_path).is_ok() {
                filepath_memory.reset();

                let output_file_path = String::from_scs(
                    filepath_memory.arena.as_ref_mut(),
                    &output_dir,
                    PATH_SEP,
                    &input_file_name,
                );

                #[allow(clippy::branches_sharing_code)]
                if write_file(&output_file_path, &result).is_ok() {
                    log_info!("Wrote output to {}\n", output_file_path);
                    log_debug!(
                        "Completed in {:.5}s, {}cycles\n",
                        get_seconds_from(&program_start_time),
                        last_cycle_count() - program_start_cycle
                    );
                } else {
                    log_error!("Failed to write to output file {}\n", output_file_path);
                }
            } else {
                log_error!("Failed to create output directory {}\n", output_dir);
            }
        } else {
            log_error!("Failed to read input from {}\n", input_file_path);
        }
    } else {
        log_error!(
            "Memory allocation failed (size requested: {} bytes)\n",
            total_size
        );
    }

    exit();
}

trait PointerDeref<T> {
    fn deref(&self) -> T;
}

impl<T: Copy> PointerDeref<T> for *const T {
    fn deref(&self) -> T {
        unsafe { **self }
    }
}

impl<T: Copy> PointerDeref<T> for *mut T {
    fn deref(&self) -> T {
        unsafe { **self }
    }
}

trait ConstPointer<T> {
    fn plus(&self, offset: usize) -> Self;
    fn minus(&self, offset: usize) -> Self;
    fn get_ref(&self) -> &T;
}

impl<T> ConstPointer<T> for *const T {
    fn plus(&self, offset: usize) -> Self {
        unsafe { self.add(offset) }
    }
    fn minus(&self, offset: usize) -> Self {
        unsafe { self.sub(offset) }
    }
    fn get_ref(&self) -> &T {
        unsafe { &**self }
    }
}

impl<T> ConstPointer<T> for *mut T {
    fn plus(&self, offset: usize) -> Self {
        unsafe { self.add(offset) }
    }
    fn minus(&self, offset: usize) -> Self {
        unsafe { self.sub(offset) }
    }
    fn get_ref(&self) -> &T {
        unsafe { &**self }
    }
}

trait MutPointer<T> {
    fn deref_and_assign(&self, other: T);
    fn as_ref_mut(&mut self) -> &mut T;
}

impl<T> MutPointer<T> for *mut T {
    fn deref_and_assign(&self, other: T) {
        unsafe { **self = other }
    }
    fn as_ref_mut(&mut self) -> &mut T {
        unsafe { &mut **self }
    }
}

struct Memory {
    /// Filepath for input/output, one at a time
    filepath: MemoryArena,
    /// Contents of input files read as-is. Multiple at a time. Amount depends
    /// on how much components are nested
    input: MemoryArena,
    /// Final resolved string
    output: MemoryArena, // TODO(sen) Flush to file when full
    /// Components table
    components: MemoryArena,
    component_names: MemoryArena,
    component_contents: MemoryArena,
    component_arguments: MemoryArena,
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
            base: base.plus(*offset),
            used: 0,
            temporary_count: 0,
        };
        *offset += size;
        result
    }
    fn push_size(&mut self, size: usize) -> *mut u8 {
        debug_assert!(self.size - self.used >= size);
        let result = self.base.plus(self.used);
        self.used += size;
        result
    }
    fn push_and_copy(&mut self, ptr: *const u8, size: usize) -> *mut u8 {
        let base = self.push_size(size);
        let mut dest = base;
        let mut source = ptr;
        for _ in 0..size {
            dest.deref_and_assign(source.deref());
            dest = dest.plus(1);
            source = source.plus(1);
        }
        base
    }
    fn push_byte(&mut self, byte: u8) -> *mut u8 {
        let base = self.push_size(1);
        base.deref_and_assign(byte);
        base
    }
    fn push_struct<T>(&mut self) -> *mut T {
        let base = self.push_size(core::mem::size_of::<T>());
        base.cast()
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
    fn reset(&mut self) {
        self.arena.as_ref_mut().used = self.used_before;
    }
    fn end(mut self) {
        let used_before = self.used_before;
        let arena = self.arena.as_ref_mut();
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
        while byte.deref().is_ascii_whitespace() {
            byte = byte.plus(1);
            first_non_whitespace += 1;
        }
        byte = self.ptr.plus(self.size - 1);
        let mut last_non_whitespace = self.size - 1;
        while byte.deref().is_ascii_whitespace() {
            byte = byte.minus(1);
            last_non_whitespace -= 1;
        }
        let ptr;
        let size;
        if last_non_whitespace < first_non_whitespace {
            // NOTE(sen) This is a whitespace-only string
            ptr = self.ptr;
            size = 0
        } else {
            ptr = self.ptr.plus(first_non_whitespace);
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
                if self_byte.deref() != other_byte.deref() {
                    result = false;
                    break;
                }
                self_byte = self_byte.plus(1);
                other_byte = other_byte.plus(1);
            }
        } else {
            result = false;
        }
        result
    }
}

impl core::cmp::PartialEq<&str> for String {
    fn eq(&self, other: &&str) -> bool {
        let other = String {
            ptr: other.as_ptr(),
            size: other.as_bytes().len(),
        };
        self == &other
    }
}

impl core::fmt::Display for String {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let source = self.ptr;
        for index in 0..self.size {
            f.write_char(source.plus(index).deref() as char)?;
        }
        Ok(())
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
    first: *const Component,
    count: usize,
}

struct Component {
    name: String,
    /// Leading and trailing whitespaces are removed
    contents: String,
}

struct Arguments {
    first: *const Argument,
    count: usize,
}

struct Argument {
    name: String,
    value: String,
}

struct Tokeniser {
    this: *const u8,
    this_index: usize,
    last_index: usize,
}

impl Tokeniser {
    fn new(string: &String) -> Tokeniser {
        debug_assert!(string.size > 0);
        Tokeniser {
            this: string.ptr,
            this_index: 0,
            last_index: string.size - 1,
        }
    }

    fn peek(&self, offset: usize) -> Option<*const u8> {
        if self.this_index + offset <= self.last_index {
            Some(self.this.plus(offset))
        } else {
            None
        }
    }

    fn advance(&mut self, offset: usize) -> bool {
        if let Some(ptr) = self.peek(offset) {
            self.this = ptr;
            self.this_index += offset;
            true
        } else if self.this_index == self.last_index {
            // NOTE(sen) This should make it impossible to peek anything
            // including the current pointer
            self.this = self.this.plus(1);
            self.this_index = self.last_index + 1;
            false
        } else {
            false
        }
    }

    /// Counts positions where the predicate fails starting from the current position
    fn advance_until(&mut self, predicate: fn(&Tokeniser) -> bool) -> usize {
        let mut counter = 0;
        loop {
            if predicate(self) {
                break;
            }
            counter += 1;
            if !self.advance(1) {
                break;
            }
        }
        counter
    }

    fn next_token(&mut self, argument_memory: &mut MemoryArena) -> Option<Token> {
        let token_type = self.current_token()?;
        match token_type {
            TokenType::String => {
                let base = self.this;
                let size = self.advance_until(|tokeniser| {
                    tokeniser.current_token() != Some(TokenType::String)
                });
                Some(Token::String(String { ptr: base, size }))
            }
            TokenType::ComponentTag => {
                self.advance(1);
                let name_base = self.this;
                let name_size =
                    self.advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_alphanumeric());
                let name = String {
                    ptr: name_base,
                    size: name_size,
                };
                let mut tag = ComponentTag {
                    name,
                    args: Arguments {
                        first: core::ptr::null(),
                        count: 0,
                    },
                };
                loop {
                    self.advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_whitespace());
                    if !self.this.deref().is_ascii_alphabetic() {
                        break;
                    }
                    let arg_name_base = self.this;
                    let arg_name_size = self
                        .advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_alphanumeric());
                    let arg_name = String {
                        ptr: arg_name_base,
                        size: arg_name_size,
                    };
                    self.advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_whitespace());
                    if self.this.deref() != b'=' {
                        // TODO(sen) Error - unexpected argument
                        break;
                    }
                    self.advance(1);
                    self.advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_whitespace());
                    if self.this.deref() != b'"' {
                        // TODO(sen) Error - unexpected argument
                        break;
                    }
                    self.advance(1);
                    let arg_value_base = self.this;
                    let arg_value_size =
                        self.advance_until(|tokeniser| tokeniser.this.deref() == b'"');
                    self.advance(1);
                    let arg_value = String {
                        ptr: arg_value_base,
                        size: arg_value_size,
                    };
                    let mut arg_ptr = argument_memory.push_struct::<Argument>();
                    let arg = arg_ptr.as_ref_mut();
                    arg.name = arg_name;
                    arg.value = arg_value;
                    if tag.args.count == 0 {
                        tag.args.first = arg;
                    }
                    tag.args.count += 1;
                }
                if self.this.deref() == b'/' {
                    self.advance(1);
                    if self.this.deref() == b'>' {
                        self.advance(1);
                        Some(Token::ComponentTag(tag))
                    } else {
                        // TODO(sen) Error - unexpected opening
                        None
                    }
                } else {
                    // TODO(sen) Error - unexpected opening
                    None
                }
            }
        }
    }

    fn current_token(&self) -> Option<TokenType> {
        let ptr0 = self.peek(0)?;
        let value0 = ptr0.deref();
        let mut result = TokenType::String;
        if value0 == b'<' {
            if let Some(ptr1) = self.peek(1) {
                let value1 = ptr1.deref();
                if value1.is_ascii_uppercase() {
                    result = TokenType::ComponentTag;
                }
            }
        }
        Some(result)
    }
}

#[derive(PartialEq)]
enum TokenType {
    String,
    ComponentTag,
}

enum Token {
    String(String),
    ComponentTag(ComponentTag),
}

struct ComponentTag {
    name: String,
    args: Arguments,
}

// TODO(sen) Rework this to handle components in a cleaner way by
// reading the input as a token stream where the tokens are plain strings to be
// pasted or components whose (resolved) contents need to be pasted
fn resolve(
    memory: *mut Memory,
    output_memory: *mut MemoryArena,
    string: &String,
    components: &mut Components,
    input_dir: &String,
    args: Option<&Arguments>,
) -> String {
    // TODO(sen) Cleaner way to handle memory here
    let memory = unsafe { &mut *memory };
    let output_memory = unsafe { &mut *output_memory };

    // NOTE(sen) Output preparation, write final resolved string to `output_base`
    let output_used_before = output_memory.used;
    let output_base = output_memory.base.plus(output_used_before);

    if string.size > 0 {
        let mut tokeniser = Tokeniser::new(string);
        let mut argument_memory = memory.component_arguments.begin_temporary();
        while let Some(token) = tokeniser.next_token(argument_memory.arena.as_ref_mut()) {
            match token {
                Token::String(string) => {
                    output_memory.push_and_copy(string.ptr, string.size);
                }
                Token::ComponentTag(component_tag) => {
                    // NOTE(sen) Find the component in cache or read it anew and store it in cache
                    let component_in_hash = {
                        // TODO(sen) Replace with a hash-based lookup
                        let mut lookup_result = None;
                        for component_index in 0..components.count {
                            let component_in_hash = components.first.plus(component_index);
                            let component_in_hash_value = unsafe { &*component_in_hash };
                            if component_in_hash_value.name == component_tag.name {
                                lookup_result = Some(component_in_hash_value);
                                break;
                            }
                        }
                        if let Some(component_looked_up) = lookup_result {
                            log_debug!("found component {} in cache\n", component_tag.name);
                            component_looked_up
                        } else {
                            log_debug!("did not find component {} in cache\n", component_tag.name);
                            // NOTE(sen) This should be zeroed since the arena is never overwritten
                            let new_component =
                                unsafe { &mut *memory.components.push_struct::<Component>() };

                            // NOTE(sen) Name from use
                            new_component.name = {
                                let size = component_tag.name.size;
                                let ptr = memory
                                    .component_names
                                    .push_and_copy(component_tag.name.ptr, size);
                                String { ptr, size }
                            };

                            // NOTE(sen) Read in contents from file
                            let mut filepath_memory = memory.filepath.begin_temporary();
                            let new_component_path = String::from_scss(
                                filepath_memory.arena.as_ref_mut(),
                                input_dir,
                                platform::PATH_SEP,
                                &new_component.name,
                                &".html".to_string(),
                            );
                            log_debug!("reading new component from {}\n", new_component_path);
                            let new_component_contents_raw_result = platform::read_file(
                                &mut memory.component_contents,
                                &new_component_path,
                            );
                            filepath_memory.end();
                            if let Ok(new_component_contents_raw) =
                                new_component_contents_raw_result
                            {
                                new_component.contents = new_component_contents_raw.trim()
                            } else {
                                // TODO(sen) Error - component used but not found
                            }

                            if components.count == 0 {
                                components.first = new_component;
                            }
                            components.count += 1;

                            new_component
                        }
                    };
                    log_debug_line_sep();
                    log_debug!(
                        "Start writing contents of {} to output\n",
                        component_in_hash.name
                    );
                    for arg_index in 0..component_tag.args.count {
                        let arg = component_tag.args.first.plus(arg_index);
                        let arg = arg.get_ref();
                        log_debug!("argument: #{}#=#{}#\n", arg.name, arg.value);
                    }
                    resolve(
                        memory,
                        &mut memory.output,
                        &component_in_hash.contents,
                        components,
                        input_dir,
                        Some(&component_tag.args),
                    );
                    log_debug!(
                        "Finish writing contents of {} to output\n",
                        component_in_hash.name
                    );
                    log_debug_line_sep();
                    argument_memory.reset();
                }
            };
        }
        argument_memory.end();
    }

    // NOTE(sen) All output should be in the output arena at this point
    #[allow(clippy::let_and_return)]
    let result = String {
        ptr: output_base,
        size: output_memory.used - output_used_before,
    };

    result
}

// SECTION Debug logging

// TODO(sen) Make debug logging go away for release builds

use core::fmt::Write;

struct Log<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> Log<'a> {
    fn new(buf: &'a mut [u8]) -> Log {
        Log { buf, offset: 0 }
    }
}

impl<'a> Write for Log<'a> {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        let bytes = string.as_bytes();
        let remainder = &mut self.buf[self.offset..];
        if remainder.len() >= bytes.len() {
            let dest = &mut remainder[..bytes.len()];
            dest.copy_from_slice(bytes);
            self.offset += bytes.len();
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

#[macro_export]
macro_rules! log {
    ($out:expr, $($arg:tt)*) => {
        // TODO(sen) Better buffer handling here
        let mut buf = [0; 100];
        if ::core::write!(Log::new(&mut buf), $($arg)*).is_ok() {
            $out(buf.as_ptr(), buf.len());
        } else {
            $crate::platform::write_stderr("couldn't write to buffer\n");
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => (log!(crate::platform::write_stdout_raw, $($arg)*))
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => (log!(crate::platform::write_stderr_raw, $($arg)*))
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => (log!(crate::platform::write_stdout_raw, $($arg)*))
}

fn debug_line_raw(string: &String) {
    log_debug_line_sep();
    platform::write_stdout("#");
    platform::write_stdout_raw(string.ptr, string.size);
    platform::write_stdout("#\n");
    log_debug_line_sep();
}

fn log_debug_line_sep() {
    log_debug!("--------------\n");
}

fn log_debug_title(string: &str) {
    log_debug!("#### {} ####\n", string);
}
