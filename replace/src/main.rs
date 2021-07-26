use std::{collections::HashMap, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = std::path::PathBuf::from(".");
    let input_file_name = "index.html";

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

fn resolve_components(
    input_contents: &str,
    input_dir: &Path,
    components: &mut HashMap<String, String>,
) -> String {
    let mut result = String::new();
    let mut current_offset = 0;

    fn find_first_component(string: &str, offset: usize) -> Option<ComponentUsed> {
        let string_with_offset = &string[offset..];

        let mut result = None;
        let mut component = ComponentUsed::default();

        let mut string_iter = string_with_offset.chars().enumerate().peekable();
        let mut component_found = false;
        while !component_found {
            if let Some((this_index, this_char)) = string_iter.next() {
                if let Some((_, next_char)) = string_iter.peek() {
                    if this_char == '<' && next_char.is_uppercase() {
                        component.first_part[0] = this_index + offset;
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
                        component.name = string_with_offset[name_start..next_index].to_string();
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
                            &string_with_offset[name_start..=name_end]
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

                            &string_with_offset[value_start..=value_end]
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
                    component.first_part[1] = this_index + offset;
                    break;
                }
                if let Some((next_index, next_char)) = string_iter.peek().copied() {
                    if this_char == '/' && next_char == '>' {
                        component.first_part[1] = next_index + offset;
                        break;
                    }
                }
            }

            if second_part_present {
                'second_part_search: while let Some((this_index, this_char)) = string_iter.next() {
                    if let Some((next_index, next_char)) = string_iter.peek().copied() {
                        if this_char == '<' && next_char == '/' {
                            let test_name = &string_with_offset
                                [(next_index + 1)..(next_index + 1 + component.name.len())];
                            if component.name == test_name {
                                let mut second_part = [this_index + offset, 0];
                                for _ in 0..component.name.len() {
                                    string_iter.next();
                                }
                                loop {
                                    if let Some((this_index, this_char)) = string_iter.next() {
                                        if this_char == '>' {
                                            second_part[1] = this_index + offset;
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

    while let Some(component_used) = find_first_component(input_contents, current_offset) {
        result.push_str(&input_contents[current_offset..component_used.first_part[0]]);

        if components.get(&component_used.name).is_none() {
            let component_contents = {
                let component_file_path =
                    input_dir.join(format!("{}.html", component_used.name.as_str()));
                let input_contents =
                    std::fs::read_to_string(component_file_path.as_path()).unwrap();
                resolve_components(input_contents.trim_end(), input_dir, components)
            };
            components.insert(component_used.name.clone(), component_contents);
        }

        let component_contents = components.get(&component_used.name).unwrap().clone();

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
            let children = resolve_components(
                &input_contents[(component_used.first_part[1] + 1)..second_part[0]],
                input_dir,
                components,
            );

            if component_contents_to_write.contains("<slot />") {
                contents_slots_resolved =
                    component_contents_to_write.replace("<slot />", children.as_str());
            } else {
                let mut children_with_offset = children.as_str();
                let mut contents_slots_resolved_partially = component_contents_to_write.to_string();
                while let Some(slot_start) = children_with_offset.find("<slot") {
                    children_with_offset = &children_with_offset[(slot_start + 5)..];
                    let name_start = children_with_offset
                        .find(|c: char| c.is_alphabetic())
                        .unwrap();

                    children_with_offset = &children_with_offset[name_start..];
                    let open_tag_end = children_with_offset.find('>').unwrap();

                    let child_name = &children_with_offset[..open_tag_end];

                    children_with_offset = &children_with_offset[(open_tag_end + 1)..];

                    let slot_end = children_with_offset.find("</slot>").unwrap();

                    let child_content = &children_with_offset[..slot_end];

                    // TODO(sen) Reduce string copying
                    contents_slots_resolved_partially = contents_slots_resolved_partially
                        .replace(format!("<slot {} />", child_name).as_str(), child_content);

                    children_with_offset = &children_with_offset[(slot_end + 7)..];
                }
                contents_slots_resolved = contents_slots_resolved_partially;
            }

            component_contents_to_write = contents_slots_resolved.as_str();
            current_offset = second_part[1] + 1;
        }
        result.push_str(component_contents_to_write);
    }

    result.push_str(&input_contents[current_offset..]);

    result
}
