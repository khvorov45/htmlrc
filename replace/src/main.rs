use std::{collections::HashMap, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = std::path::PathBuf::from(".");
    let input_file_name = "component-param.html";

    let input_file = input_dir.join(input_file_name);
    let input_contents = std::fs::read_to_string(input_file).unwrap();

    let mut components = HashMap::<String, String>::new();

    let output_contents = resolve_components(
        input_contents.as_str(),
        input_dir.as_path(),
        &mut components,
    );

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

        let mut string_iter = string.chars().enumerate().peekable();
        let mut component_found = false;
        while !component_found {
            if let Some((this_index, this_char)) = string_iter.next() {
                if let Some((_, next_char)) = string_iter.peek() {
                    if this_char == '<' && next_char.is_uppercase() {
                        component.first_part[0] = this_index;
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
            // NOTE(sen) Parse name
            let (name_start, _) = string_iter.next().unwrap();
            loop {
                if let Some((next_index, next_char)) = string_iter.peek().copied() {
                    if next_char.is_whitespace() || next_char == '/' || next_char == '>' {
                        component.name = string[name_start..next_index].to_string();
                        break;
                    }
                    string_iter.next();
                }
            }

            // NOTE(sen) Parse arguments
            loop {
                if let Some((next_index, next_char)) = string_iter.peek().copied() {
                    if next_char == '/' || next_char == '>' {
                        break;
                    }
                    if next_char.is_alphabetic() {
                        let name = {
                            let name_start = next_index;
                            let mut name_end = name_start + 1;
                            while let Some((this_index, this_char)) = string_iter.next() {
                                if !this_char.is_alphanumeric() {
                                    name_end = this_index - 1;
                                    break;
                                }
                            }
                            &string[name_start..=name_end]
                        };

                        let value = {
                            let mut value_start = 0;
                            while let Some((this_index, this_char)) = string_iter.next() {
                                if this_char == '"' {
                                    value_start = this_index + 1;
                                    break;
                                }
                            }
                            let mut value_end = value_start + 1;
                            while let Some((this_index, this_char)) = string_iter.next() {
                                if this_char == '"' {
                                    value_end = this_index - 1;
                                    break;
                                }
                            }

                            &string[value_start..=value_end]
                        };
                        component.params.insert(name.to_string(), value.to_string());
                    } else {
                        string_iter.next();
                    }
                }
            }

            let mut second_part_present = false;
            while let Some((this_index, this_char)) = string_iter.next() {
                if this_char == '>' {
                    second_part_present = true;
                    component.first_part[1] = this_index;
                    break;
                }
                if let Some((next_index, next_char)) = string_iter.peek().copied() {
                    if this_char == '/' && next_char == '>' {
                        component.first_part[1] = next_index;
                        break;
                    }
                }
            }

            if second_part_present {
                'second_part_search: while let Some((this_index, this_char)) = string_iter.next() {
                    if let Some((next_index, next_char)) = string_iter.peek().copied() {
                        if this_char == '<' && next_char == '/' {
                            let test_name =
                                &string[(next_index + 1)..(next_index + 1 + component.name.len())];
                            if component.name == test_name {
                                let mut second_part = [this_index, 0];
                                for _ in 0..component.name.len() {
                                    string_iter.next();
                                }
                                loop {
                                    if let Some((this_index, this_char)) = string_iter.next() {
                                        if this_char == '>' {
                                            second_part[1] = this_index;
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
    fn find_with_offset(string: &str, offset: usize) -> Option<ComponentUsed> {
        let mut result = ComponentUsed::find_first(&string[offset..]);
        if let Some(component) = result.as_mut() {
            component.first_part[0] += offset;
            component.first_part[1] += offset;
            if let Some(part) = component.second_part.as_mut() {
                part[0] += offset;
                part[1] += offset;
            }
        }
        result
    }
}

fn resolve_components(
    input_contents: &str,
    input_dir: &Path,
    components: &mut HashMap<String, String>,
) -> String {
    let mut result = String::new();
    let mut current_offset = 0;
    while let Some(component_used) = ComponentUsed::find_with_offset(input_contents, current_offset)
    {
        result.push_str(&input_contents[current_offset..component_used.first_part[0]]);

        if components.get(&component_used.name).is_none() {
            let component_contents = {
                let component_file_path =
                    input_dir.join(format!("{}.html", component_used.name.as_str()));
                let input_contents =
                    std::fs::read_to_string(component_file_path.as_path()).unwrap();
                resolve_components(input_contents.as_str(), input_dir, components)
            };
            components.insert(component_used.name.clone(), component_contents);
        }

        let component_contents = components.get(&component_used.name).unwrap();

        let mut component_contents_to_write = component_contents.as_str();

        let mut contents_params_resolved: String;
        if !component_used.params.is_empty() {
            contents_params_resolved = component_contents_to_write.to_string();
            for (param_name, param_value) in &component_used.params {
                // TODO(sen) Reduce string copying here
                contents_params_resolved = contents_params_resolved
                    .replace(format!("${}", param_name).as_str(), param_value);
            }
            component_contents_to_write = contents_params_resolved.as_str();
        }

        let contents_slots_resolved: String;
        current_offset = component_used.first_part[1] + 1;
        if let Some(second_part) = component_used.second_part {
            let children = &input_contents[(component_used.first_part[1] + 1)..second_part[0]];
            contents_slots_resolved = resolve_children(component_contents_to_write, children);
            component_contents_to_write = contents_slots_resolved.as_str();
            current_offset = second_part[1] + 1;
        }
        result.push_str(component_contents_to_write);
    }

    result.push_str(&input_contents[current_offset..]);

    result
}

fn resolve_children(input: &str, children: &str) -> String {
    // TODO(sen) Handle multiple slots
    let result = input.to_string();
    result.replace("<slot />", children)
}
