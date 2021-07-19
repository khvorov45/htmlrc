fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = std::path::PathBuf::from(".");
    let input_file_name = "index.html";

    let input_file = input_dir.join(input_file_name);
    let input_contents = std::fs::read_to_string(input_file).unwrap();

    let mut output_contents = input_contents.clone();

    let mut input_to_parse = input_contents.as_str();
    while let Some(substitution) = Substitution::find_first(&mut input_to_parse) {
        output_contents =
            output_contents.replace(substitution.from.as_str(), substitution.to.as_str());
    }

    let output_dir = std::path::PathBuf::from("build");
    if output_dir.is_dir() {
        std::fs::remove_dir_all(output_dir.as_path())?;
    }
    std::fs::create_dir(output_dir.as_path())?;

    let output_file = output_dir.join(input_file_name);

    std::fs::write(output_file, output_contents.as_bytes())?;

    println!("Done");

    Ok(())
}

#[derive(Debug)]
struct Substitution {
    /// Literal string to be substituted in the output
    from: String,
    /// Literal string to replace `from` with in the output
    to: String,
}

impl Substitution {
    fn find_first(string: &mut &str) -> Option<Self> {
        /// Search for the opening tag `<` that's followed by an uppercase letter
        fn find_component<'a>(string: &mut &'a str) -> (&'a str, Option<&'a str>) {
            // NOTE(sen) Does not handle nodes inside comments
            let mut start_index = string.len();
            if !string.is_empty() {
                for string_index in 0..(string.len() - 1) {
                    let next_char = string.chars().nth(string_index + 1);
                    if string.chars().nth(string_index) == Some('<') {
                        if let Some(next_char) = next_char {
                            if next_char.is_uppercase() {
                                start_index = string_index;
                                break;
                            }
                        }
                    }
                }
            }
            let mut end_index = string.len();
            let mut two_parts = false;
            for string_index in start_index..string.len() {
                let this_char = string.chars().nth(string_index);
                let next_char = string.chars().nth(string_index + 1);
                if this_char == Some('/') && next_char == Some('>') {
                    end_index = string_index + 1;
                    break;
                } else if this_char == Some('>') {
                    two_parts = true;
                    end_index = string_index;
                    break;
                }
            }
            if two_parts {
                // TODO(sen) Handle two-parters somehow
            }

            let component = (&string[start_index..=end_index], None);
            *string = &string[(end_index + 1)..];
            component
        }

        let mut substitution = None;
        let component = find_component(string);
        println!("{:#?}", component);

        if !string.is_empty() {
            fn find_name(string: &mut &str) -> String {
                debug_assert!(string.starts_with('<'));
                let string_to_parse = string[1..].trim_start();
                let name_break = string_to_parse
                    .find(|x: char| !x.is_alphanumeric())
                    .unwrap();
                let (name, left) = string_to_parse.split_at(name_break);
                *string = left;
                name.to_string()
            }

            let name = find_name(string);

            fn find_params(string: &mut &str) -> std::collections::HashMap<String, String> {
                let string_to_parse = string.trim_start();
                let params_break = string_to_parse
                    .find(|x: char| x == '/' || x == '>')
                    .unwrap();
                let (params_string, left) = string_to_parse.split_at(params_break);

                let mut params = std::collections::HashMap::new();

                let params_string_to_parse = params_string.trim_end();
                if !params_string_to_parse.is_empty() {}

                params
            }

            let params = find_params(string);

            fn find_children(string: &mut &str) -> Vec<Substitution> {
                let children = Vec::new();
                let string_to_parse = string.trim_start();
                if let Some(stripped) = string_to_parse.strip_prefix("/>") {
                    *string = stripped;
                } else if let Some(stripped) = string_to_parse.strip_prefix(">") {
                    *string = stripped;
                } else {
                    panic!("unexpected string");
                }

                children
            }

            let children = find_children(string);
        }

        substitution
    }
}
