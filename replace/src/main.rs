fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = std::path::PathBuf::from(".");
    let input_file_name = "index.html";

    let input_file = input_dir.join(input_file_name);
    let input_contents = std::fs::read_to_string(input_file).unwrap();

    let output_contents = input_contents.clone();

    let mut input_to_parse = input_contents.as_str();
    while let Some(substitution) = Substitution::find_first(&mut input_to_parse) {}

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
    literal: String,
    component_name: String,
}

impl Substitution {
    fn find_first(string: &mut &str) -> Option<Self> {
        fn find_start(string: &str) -> &str {
            // NOTE(sen) Does not handle nodes inside comments
            let mut start_index = string.len();
            if !string.is_empty() {
                for string_index in 0..(string.len() - 1) {
                    if string.chars().nth(string_index) == Some('<')
                        && string.chars().nth(string_index + 1) != Some('!')
                    {
                        start_index = string_index;
                        break;
                    }
                }
            }
            &string[start_index..]
        }

        let mut node = None;
        let mut string = find_start(string);

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

            let name = find_name(&mut string);

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

            let params = find_params(&mut string);

            fn find_children(string: &mut &str) -> Vec<Substitution> {
                let children = Vec::new();

                if let Some(stripped) = string.strip_prefix("/>") {
                    *string = stripped;
                } else if let Some(stripped) = string.strip_prefix(">") {
                    *string = stripped;
                } else {
                    panic!("unexpected string");
                }

                children
            }

            let children = find_children(&mut string);

            string = &string[string.len()..];
        }

        node
    }
}
