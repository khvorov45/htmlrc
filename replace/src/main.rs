fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = ".";
    let (mut pages, components) = {
        let mut pages = Vec::new();
        let mut components = Vec::new();
        for entry in std::fs::read_dir(input_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = entry.file_name();
                let name = name.to_str().unwrap();
                let contents = std::fs::read_to_string(&path)?;
                if name.ends_with(".html") {
                    let name = name.replace(".html", "");
                    if name.starts_with(|c: char| c.is_uppercase()) {
                        components.push(Html {
                            name,
                            path,
                            contents: contents.trim_end().to_string(),
                        });
                    } else {
                        pages.push(Html {
                            name,
                            path,
                            contents,
                        })
                    }
                }
            }
        }
        (pages, components)
    };

    let output_dir = std::path::PathBuf::from("build");
    if output_dir.is_dir() {
        std::fs::remove_dir_all(output_dir.as_path())?;
    }
    std::fs::create_dir(output_dir.as_path())?;

    for page in &mut pages {
        for component in &components {
            let pattern = format!("<{} />", component.name);
            if page.contents.contains(pattern.as_str()) {
                page.contents = page
                    .contents
                    .replace(pattern.as_str(), component.contents.as_str());
            }
        }
        let old_tree =
            page.path
                .strip_prefix(format!("{}{}", input_dir, std::path::MAIN_SEPARATOR))?;
        let new_path = output_dir.join(old_tree);
        std::fs::write(new_path, page.contents.as_bytes())?;
    }

    Ok(())
}

#[derive(Debug)]
struct Html {
    name: String,
    path: std::path::PathBuf,
    contents: String,
}
