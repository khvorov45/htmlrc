use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = std::path::PathBuf::from(".");
    let input_file_name = "component-child.html";

    let input_file = input_dir.join(input_file_name);
    let input_contents = std::fs::read_to_string(input_file).unwrap();

    let output_contents = input_contents.clone();

    let mut input_to_parse = input_contents.as_str();
    while let Some(component_used) = ComponentUsed::find_first(input_to_parse) {
        println!("{:#?}", component_used);
        input_to_parse = &input_to_parse[component_used.first_part[1]..];
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

#[derive(Default, Debug)]
struct ComponentUsed {
    first_part: [usize; 2],
    second_part: Option<[usize; 2]>,
    name: String,
    params: HashMap<String, String>,
}

impl ComponentUsed {
    fn find_first(string: &str) -> Option<ComponentUsed> {
        let mut result = None;
        let mut component = ComponentUsed::default();

        let mut string_iter = string.chars().peekable();
        // TODO(sen) Use enumerate instead?
        let mut counter = 0;
        let mut component_found = false;
        while !component_found {
            if let Some(this_char) = string_iter.next() {
                counter += 1;
                if let Some(next_char) = string_iter.peek() {
                    if this_char == '<' && next_char.is_uppercase() {
                        component.first_part[0] = counter - 1;
                        component_found = true;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if component_found {
            let name_start = counter;
            loop {
                string_iter.next();
                counter += 1;
                if let Some(next_char) = string_iter.peek().copied() {
                    if next_char.is_whitespace() || next_char == '/' || next_char == '>' {
                        component.name = string[name_start..counter].to_string();
                        break;
                    }
                } else {
                    break;
                }
            }

            // TODO(sen) Parse arguments here

            let mut second_part_present = false;
            while let Some(this_char) = string_iter.next() {
                counter += 1;
                let next_char = string_iter.peek().copied();
                if this_char == '/' && next_char == Some('>') {
                    component.first_part[1] = counter;
                    break;
                }
                if this_char == '>' {
                    second_part_present = true;
                    component.first_part[1] = counter - 1;
                    break;
                }
            }

            if second_part_present {
                'second_part_search: while let Some(this_char) = string_iter.next() {
                    counter += 1;
                    if let Some(next_char) = string_iter.peek().copied() {
                        if this_char == '<' && next_char == '/' {
                            let test_name =
                                &string[(counter + 1)..(counter + 1 + component.name.len())];
                            if component.name == test_name {
                                let mut second_part = [0, 0];
                                second_part[0] = counter - 2;
                                for _ in 0..component.name.len() {
                                    string_iter.next();
                                    counter += 1;
                                }

                                loop {
                                    if let Some(this_char) = string_iter.next() {
                                        counter += 1;
                                        if this_char == '>' {
                                            second_part[1] = counter - 1;
                                            component.second_part = Some(second_part);
                                            break 'second_part_search;
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
            }

            result = Some(component);
        }
        result
    }
}
