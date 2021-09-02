#![no_std]

mod platform;

struct Error {}

type Result<T> = core::result::Result<T, Error>;

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

pub fn handle_panic(info: &core::panic::PanicInfo) {
    log_error!("{}\n", info);
    platform::os::exit();
}

pub struct RunArguments<'a> {
    input_dir: &'a str,
    input_file_name: &'a str,
    output_dir: &'a str,
}

impl<'a> Default for RunArguments<'a> {
    fn default() -> Self {
        Self {
            input_dir: "src",
            input_file_name: "index.html",
            output_dir: "build",
        }
    }
}

pub use platform::os::parse_arguments;

pub fn run(args: RunArguments) {
    use platform::{
        arch::last_cycle_count,
        os::{
            allocate_and_clear, append_to_file, create_dir_if_not_exists, create_empty_file, exit,
            get_max_and_total_html_size, get_seconds_from, get_timespec_now, read_file,
            MAX_FILENAME_BYTES, MAX_PATH_BYTES,
        },
    };

    let program_start_time = get_timespec_now();
    let program_start_cycle = last_cycle_count();

    let input_dir = args.input_dir.to_string();
    let input_file_name = args.input_file_name.to_string();
    let output_dir = args.output_dir.to_string();

    log_debug_title("START");
    log_debug!("Input directory: {}\n", &input_dir);
    log_debug!("Input file: {}\n", &input_file_name);
    log_debug!("output_dir: {}\n", &output_dir);
    log_debug_line_sep();

    let (max_html_file_size, total_html_file_size) =
        match get_max_and_total_html_size(args.input_dir) {
            Ok((a, b)) => (a, b),
            Err(_) => {
                log_error!("Failed to scan input directory {}\n", input_dir);
                exit();
                return;
            }
        };

    if max_html_file_size == 0 || total_html_file_size == 0 {
        log_info!("No html input found in {}", input_dir);
        exit();
        return;
    }

    let mut memory = {
        // NOTE(sen) Both of these should be more than anybody will ever need per page
        let total_supported_components = 131072;
        let total_supported_simultaneous_arguments = 131072;

        let input_path_size = MAX_PATH_BYTES;
        let output_path_size = MAX_PATH_BYTES;
        let components_size = total_supported_components * core::mem::size_of::<NameValue>();
        let component_names_size = total_supported_components * MAX_FILENAME_BYTES;
        let component_contents_size = total_html_file_size;
        let component_arguments_size =
            total_supported_simultaneous_arguments * core::mem::size_of::<NameValue>();
        let input_size = total_html_file_size;
        let output_size = max_html_file_size.max(10 * MEGABYTE);

        let total_size = input_path_size
            + output_path_size
            + components_size
            + component_names_size
            + component_contents_size
            + component_arguments_size
            + input_size
            + output_size;

        log_debug_title("MEMORY");
        log_debug!("Input path: {}B\n", input_path_size);
        log_debug!("Output path: {}B\n", output_path_size);
        log_debug!("Components: {}MB\n", components_size / 1024 / 1024);
        log_debug!(
            "Component names: {}MB\n",
            component_names_size / 1024 / 1024
        );
        log_debug!(
            "Component contents: {}MB\n",
            component_contents_size / 1024 / 1024
        );
        log_debug!(
            "Component arguments: {}MB\n",
            component_arguments_size / 1024 / 1024
        );
        log_debug!("Input: {}MB\n", input_size / 1024 / 1024);
        log_debug!("Output: {}MB\n", output_size / 1024 / 1024);
        log_debug!("Total: {}MB\n", total_size / 1024 / 1024);
        log_debug_line_sep();

        let memory_base_ptr = match allocate_and_clear(total_size) {
            Ok(ptr) => ptr,
            Err(_) => {
                log_error!(
                    "Memory allocation failed (size requested: {} bytes)\n",
                    total_size
                );
                exit();
                return;
            }
        };
        let mut size_used = 0;
        let input_path = MemoryArena::new(memory_base_ptr, &mut size_used, input_path_size);
        let mut output_path = MemoryArena::new(memory_base_ptr, &mut size_used, output_path_size);
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

        let output_file_path = {
            let mut filepath = Filepath::new(&mut output_path);

            let output_dir_path = filepath.new_path(output_dir).get_string();
            if create_dir_if_not_exists(&output_dir_path).is_err() {
                log_error!("Failed to create output directory {}\n", output_dir);
                exit();
                return;
            }

            let output_file_path = filepath
                .new_path(output_dir)
                .add_entry(input_file_name)
                .get_string();

            if create_empty_file(&output_file_path).is_err() {
                log_error!("Failed to create output file {}\n", output_file_path);
                exit();
                return;
            }

            output_file_path
        };

        let output = FlushableArena {
            arena: output,
            flush_file: output_file_path,
        };

        Memory {
            input_path,
            input,
            output,
            components,
            component_names,
            component_contents,
            component_arguments,
        }
    };

    let mut components = NameValueArray::new(&mut memory.components);

    let mut filepath = Filepath::new(&mut memory.input_path);

    let input_file_path = filepath
        .new_path(input_dir)
        .add_entry(input_file_name)
        .get_string();

    // NOTE(sen) This will never be "freed" anyway, so no need for temporary
    let input_string = match read_file(&mut memory.input, &input_file_path) {
        Ok(string) => string,
        Err(_) => {
            log_error!("Failed to read input from {}\n", input_file_path);
            exit();
            return;
        }
    };

    log_debug!("started resolution of input at {}\n", input_file_path);

    if resolve(
        &mut memory,
        &input_string,
        &mut components,
        input_dir,
        None,
        None,
        &mut filepath,
    )
    .is_err()
    {
        // NOTE(sen) The message should have been generated before this
        exit();
        return;
    };

    log_debug!("input resolution finished\n");

    debug_assert!(memory.input.temporary_count == 0);
    debug_assert!(memory.component_arguments.temporary_count == 0);

    if append_to_file(
        &memory.output.flush_file,
        memory.output.arena.base,
        memory.output.arena.used,
    )
    .is_err()
    {
        log_error!(
            "Failed to write to output file {}\n",
            memory.output.flush_file
        );
        exit();
        return;
    }

    log_info!("Wrote output to {}\n", memory.output.flush_file);
    log_debug!(
        "Completed in {:.5}s, {}cycles\n",
        get_seconds_from(&program_start_time),
        last_cycle_count() - program_start_cycle
    );

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
    fn get_ref_mut(&mut self) -> &mut T;
}

impl<T> MutPointer<T> for *mut T {
    fn deref_and_assign(&self, other: T) {
        unsafe { **self = other }
    }
    fn get_ref_mut(&mut self) -> &mut T {
        unsafe { &mut **self }
    }
}

struct Memory {
    /// Filepath for input, one at a time
    input_path: MemoryArena,
    /// Contents of input files read as-is. Multiple at a time. Amount depends
    /// on how much components are nested
    input: MemoryArena,
    /// Final resolved string
    output: FlushableArena,
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
    fn push_byte(&mut self, byte: u8) -> *mut u8 {
        let result = self.push_size(1);
        result.deref_and_assign(byte);
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

struct FlushableArena {
    arena: MemoryArena,
    flush_file: String,
}

impl FlushableArena {
    fn push_and_copy(&mut self, ptr: *const u8, size: usize) {
        if self.arena.size - self.arena.used < size {
            log_debug!("flushing memory to {}\n", self.flush_file);
            if platform::os::append_to_file(&self.flush_file, self.arena.base, self.arena.used)
                .is_err()
            {
                panic!("failed flushing output to {}\n", self.flush_file);
            }
            self.arena.used = 0;
        }
        self.arena.push_and_copy(ptr, size);
    }
}

struct TemporaryMemory {
    arena: *mut MemoryArena,
    used_before: usize,
}

impl TemporaryMemory {
    fn reset(&mut self) {
        self.arena.get_ref_mut().used = self.used_before;
    }
    fn end(mut self) {
        let used_before = self.used_before;
        let arena = self.arena.get_ref_mut();
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
    fn copy(arena: &mut MemoryArena, source: String) -> String {
        String {
            ptr: arena.push_and_copy(source.ptr, source.size),
            size: source.size,
        }
    }
    /// Does not modify memory
    fn trim(&self) -> String {
        let mut result = String {
            ptr: self.ptr,
            size: self.size,
        };
        if self.size > 0 {
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
            if last_non_whitespace < first_non_whitespace {
                // NOTE(sen) This is a whitespace-only string
                result.ptr = self.ptr;
                result.size = 0
            } else {
                result.ptr = self.ptr.plus(first_non_whitespace);
                result.size = last_non_whitespace - first_non_whitespace + 1;
            }
        }
        result
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
        use core::fmt::Write;
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
    for ch in literal.chars() {
        if !char_is_valid(ch) {
            return false;
        }
    }
    true
}

fn char_is_valid(ch: char) -> bool {
    ch.is_ascii() && ch != '\0'
}

struct Filepath {
    arena: *mut MemoryArena,
    complete: bool,
}

impl Filepath {
    fn new(arena: *mut MemoryArena) -> Self {
        Self {
            arena,
            complete: false,
        }
    }
    fn new_path(&mut self, entry: String) -> &mut Self {
        self.complete = false;
        let arena = self.arena.get_ref_mut();
        arena.used = 0;
        arena.push_and_copy(entry.ptr, entry.size);
        self
    }
    fn add_entry(&mut self, entry: String) -> &mut Self {
        debug_assert!(!self.complete);
        let arena = self.arena.get_ref_mut();
        arena.push_byte(platform::os::PATH_SEP as u8);
        arena.push_and_copy(entry.ptr, entry.size);
        self
    }
    fn add_ext(&mut self, ext: String) -> &mut Self {
        debug_assert!(!self.complete);
        let arena = self.arena.get_ref_mut();
        arena.push_byte(b'.');
        arena.push_and_copy(ext.ptr, ext.size);
        self
    }
    fn get_string(&mut self) -> String {
        debug_assert!(!self.complete);
        let arena = self.arena.get_ref_mut();
        arena.push_byte(b'\0');
        self.complete = true;
        String {
            ptr: arena.base,
            size: arena.used,
        }
    }
}

struct NameValueArray {
    first: *const NameValue,
    count: usize,
    arena: *mut MemoryArena,
}

impl NameValueArray {
    fn new(arena: *mut MemoryArena) -> Self {
        Self {
            first: core::ptr::null(),
            count: 0,
            arena,
        }
    }
    fn find_by_name(&self, name: String) -> Option<*const NameValue> {
        for index in 0..self.count {
            let entry = self.first.plus(index);
            if entry.get_ref().name == name {
                return Some(entry);
            }
        }
        None
    }
    fn new_empty_entry(&mut self) -> *mut NameValue {
        let entry = self.arena.get_ref_mut().push_struct::<NameValue>();
        if self.count == 0 {
            self.first = entry;
        }
        self.count += 1;
        entry
    }
}

struct NameValue {
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

    fn advance(&mut self) -> bool {
        if let Some(ptr) = self.peek(1) {
            self.this = ptr;
            self.this_index += 1;
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
            if !self.advance() {
                break;
            }
        }
        counter
    }

    fn advance_until_not_alphanumeric(&mut self) -> usize {
        self.advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_alphanumeric())
    }

    fn advance_until_not_whitespace(&mut self) -> usize {
        self.advance_until(|tokeniser| !tokeniser.this.deref().is_ascii_whitespace())
    }

    fn next_token(&mut self, argument_memory: &mut MemoryArena) -> Option<Result<Token>> {
        let token_type = self.current_token()?;
        match token_type {
            TokenType::String => {
                let base = self.this;
                let size = self.advance_until(|tokeniser| {
                    tokeniser.current_token() != Some(TokenType::String)
                });
                Some(Ok(Token::String(String { ptr: base, size })))
            }

            TokenType::ComponentTag => {
                self.advance();
                let name_base = self.this;
                let name_size = self.advance_until_not_alphanumeric();
                let name = String {
                    ptr: name_base,
                    size: name_size,
                };
                let mut tag = ComponentTag {
                    name,
                    args: NameValueArray::new(argument_memory),
                };
                loop {
                    self.advance_until_not_whitespace();
                    if !self.this.deref().is_ascii_alphabetic() {
                        break;
                    }
                    let arg_name_base = self.this;
                    let arg_name_size = self.advance_until_not_alphanumeric();
                    let arg_name = String {
                        ptr: arg_name_base,
                        size: arg_name_size,
                    };
                    self.advance_until_not_whitespace();
                    if self.this.deref() != b'=' {
                        log_error!("unexpected argument: expected '='\n");
                        return Some(Err(Error {}));
                    }
                    self.advance();
                    self.advance_until_not_whitespace();
                    if self.this.deref() != b'"' {
                        log_error!("unexpected argument: expected '\"'\n");
                        return Some(Err(Error {}));
                    }
                    self.advance();
                    let arg_value_base = self.this;
                    let arg_value_size =
                        self.advance_until(|tokeniser| tokeniser.this.deref() == b'"');
                    self.advance();
                    let arg_value = String {
                        ptr: arg_value_base,
                        size: arg_value_size,
                    };
                    let mut arg_ptr = tag.args.new_empty_entry();
                    let arg = arg_ptr.get_ref_mut();
                    arg.name = arg_name;
                    arg.value = arg_value;
                }
                if self.this.deref() == b'/' {
                    self.advance();
                    if self.this.deref() == b'>' {
                        self.advance();
                        Some(Ok(Token::ComponentTag(tag)))
                    } else {
                        log_error!("unexpected component closing: expected '>'\n");
                        Some(Err(Error {}))
                    }
                } else {
                    log_error!("unexpected component closing: expected '/'\n");
                    Some(Err(Error {}))
                }
            }

            TokenType::Argument => {
                self.advance();
                let arg_name_base = self.this;
                let arg_name_size = self.advance_until_not_alphanumeric();
                let arg_name = String {
                    ptr: arg_name_base,
                    size: arg_name_size,
                };
                Some(Ok(Token::Argument(arg_name)))
            }

            TokenType::InlineComponent => {
                self.advance();
                self.advance();
                let name_base = self.this;
                let name_size = self.advance_until_not_alphanumeric();
                let name = String {
                    ptr: name_base,
                    size: name_size,
                };
                self.advance_until(|tokeniser| tokeniser.this.deref() == b'\n');
                self.advance();
                let value_base = self.this;
                let value_size_plus_one = self.advance_until(|tokeniser| {
                    let this_value = tokeniser.this.deref();
                    let prev_value = tokeniser.this.minus(1).deref();
                    this_value == b'}' && prev_value == b'}'
                });
                let value = String {
                    ptr: value_base,
                    size: value_size_plus_one - 1,
                };
                let value = value.trim();
                self.advance();
                Some(Ok(Token::InlineComponent(NameValue { name, value })))
            }
        }
    }

    fn current_token(&self) -> Option<TokenType> {
        let ptr0 = self.peek(0)?;
        let value0 = ptr0.deref();
        let mut result = TokenType::String;
        if let Some(ptr1) = self.peek(1) {
            let value1 = ptr1.deref();
            if value0 == b'<' && value1.is_ascii_uppercase() {
                result = TokenType::ComponentTag;
            } else if value0 == b'$' && value1.is_ascii_alphabetic() {
                result = TokenType::Argument;
            } else if value0 == b'{' && value1 == b'{' {
                result = TokenType::InlineComponent;
            }
        }

        Some(result)
    }
}

#[derive(PartialEq)]
enum TokenType {
    String,
    ComponentTag,
    Argument,
    InlineComponent,
}

enum Token {
    String(String),
    ComponentTag(ComponentTag),
    Argument(String),
    InlineComponent(NameValue),
}

struct ComponentTag {
    name: String,
    args: NameValueArray,
}

/// Writes to the output arena
fn resolve(
    memory: &mut Memory,
    string: &String,
    components: &mut NameValueArray,
    input_dir: String,
    args: Option<&NameValueArray>,
    parent_args: Option<&NameValueArray>,
    filepath: &mut Filepath,
) -> Result<()> {
    if string.size > 0 {
        let mut tokeniser = Tokeniser::new(string);
        let mut argument_memory = memory.component_arguments.begin_temporary();
        while let Some(token) = tokeniser.next_token(argument_memory.arena.get_ref_mut()) {
            match token? {
                Token::String(string) => {
                    memory.output.push_and_copy(string.ptr, string.size);
                }
                Token::ComponentTag(component_tag) => {
                    // NOTE(sen) Find the component in cache or read it anew and store it in cache
                    let component_in_cache = {
                        log_debug!("looking for component {}\n", component_tag.name);
                        if let Some(component_looked_up) =
                            components.find_by_name(component_tag.name)
                        {
                            log_debug!("found component {} in cache\n", component_tag.name);
                            component_looked_up
                        } else {
                            log_debug!("did not find component {} in cache\n", component_tag.name);
                            let new_component = components.new_empty_entry();
                            let new_component = unsafe { &mut *new_component };

                            // NOTE(sen) Name from use
                            new_component.name =
                                String::copy(&mut memory.component_names, component_tag.name);

                            // NOTE(sen) Read in contents from file
                            let new_component_path = filepath
                                .new_path(input_dir)
                                .add_entry(new_component.name)
                                .add_ext("html".to_string())
                                .get_string();
                            log_debug!("reading new component from {}\n", new_component_path);
                            let new_component_contents_raw_result = platform::os::read_file(
                                &mut memory.component_contents,
                                &new_component_path,
                            );
                            if let Ok(new_component_contents_raw) =
                                new_component_contents_raw_result
                            {
                                new_component.value = new_component_contents_raw.trim()
                            } else {
                                log_error!(
                                    "Component {} used but not found in {}\n",
                                    new_component.name,
                                    input_dir
                                );
                                return Err(Error {});
                            }

                            new_component
                        }
                    };
                    let component_in_cache = component_in_cache.get_ref();

                    log_debug_line_sep();
                    log_debug!(
                        "Start writing contents of {} to output\n",
                        component_in_cache.name
                    );
                    for arg_index in 0..component_tag.args.count {
                        let arg = component_tag.args.first.plus(arg_index);
                        let arg = arg.get_ref();
                        log_debug!("argument: #{}#=#{}#\n", arg.name, arg.value);
                    }
                    resolve(
                        memory,
                        &component_in_cache.value,
                        components,
                        input_dir,
                        Some(&component_tag.args),
                        args,
                        filepath,
                    )?;
                    log_debug!(
                        "Finish writing contents of {} to output\n",
                        component_in_cache.name
                    );
                    log_debug_line_sep();
                    argument_memory.reset();
                }
                Token::Argument(arg_name) => {
                    log_debug!("Found argument #{}#\n", arg_name);
                    let arg = {
                        let mut result = None;
                        if let Some(args_table) = args {
                            result = args_table.find_by_name(arg_name);
                        }
                        result
                    };
                    if let Some(arg) = arg {
                        log_debug_line_sep();
                        log_debug!("Start writing argument {} to output\n", arg_name);
                        resolve(
                            memory,
                            &arg.get_ref().value,
                            components,
                            input_dir,
                            parent_args, // NOTE(sen) The argument value was written in parent
                            None,
                            filepath,
                        )?;
                        log_debug!("Finish writing argument {} to output\n", arg_name);
                        log_debug_line_sep();
                    } else {
                        log_error!("Argument {} used but not passed\n", arg_name);
                        return Err(Error {});
                    }
                }

                Token::InlineComponent(inline_component) => {
                    log_debug!("Adding inline component {}\n", inline_component.name);
                    if components.find_by_name(inline_component.name).is_some() {
                        log_error!(
                            "Component {} defined inline but already present\n",
                            inline_component.name
                        );
                        return Err(Error {});
                    }
                    let mut dest = components.new_empty_entry();
                    let dest = dest.get_ref_mut();
                    dest.name = String::copy(&mut memory.component_names, inline_component.name);
                    dest.value =
                        String::copy(&mut memory.component_contents, inline_component.value);
                }
            };
        }
        argument_memory.end();
    }
    Ok(())
}

// SECTION Debug logging

struct Log<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> Log<'a> {
    fn new(buf: &'a mut [u8]) -> Log {
        Log { buf, offset: 0 }
    }
}

impl<'a> core::fmt::Write for Log<'a> {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        let source_full = string.as_bytes();
        let dest_full = &mut self.buf[self.offset..];
        let bytes_to_write = source_full.len().min(dest_full.len());
        let source = &source_full[..bytes_to_write];
        let dest = &mut dest_full[..bytes_to_write];
        dest.copy_from_slice(source);
        self.offset += bytes_to_write;
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($out:expr, $($arg:tt)*) => {{
        use core::fmt::Write;
        let mut buf = [0; 100];
        let _ = ::core::write!(crate::Log::new(&mut buf), $($arg)*);
        $out(buf.as_ptr(), buf.len());
    }};
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        crate::log!(crate::platform::os::write_stdout_raw, $($arg)*)
    }
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => (crate::log!(crate::platform::os::write_stderr_raw, $($arg)*))
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => (crate::log!(crate::platform::os::write_stdout_raw, $($arg)*))
}

#[allow(dead_code)]
fn debug_line_raw(string: &String) {
    #[cfg(debug_assertions)]
    {
        log_debug_line_sep();
        platform::os::write_stdout("#");
        platform::os::write_stdout_raw(string.ptr, string.size);
        platform::os::write_stdout("#\n");
        log_debug_line_sep();
    }
}

fn log_debug_line_sep() {
    log_debug!("--------------\n");
}

fn log_debug_title(string: &str) {
    log_debug!("#### {} ####\n", string);
}
