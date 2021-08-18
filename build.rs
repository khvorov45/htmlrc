#[cfg(target_os = "linux")]
fn main() {
    println!("cargo:rustc-flags=-lc");
}
