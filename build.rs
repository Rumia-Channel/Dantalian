use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=static/style.css");
    println!("cargo:rerun-if-changed=static/script.js");

    let mut hasher = DefaultHasher::new();

    for file in &["static/style.css", "static/script.js"] {
        let path = Path::new(file);
        if path.exists() {
            let meta = std::fs::metadata(path).unwrap();
            meta.modified().unwrap().hash(&mut hasher);
        }
    }

    let hash = hasher.finish();
    let version = format!("{:x}", hash);
    println!("cargo:rustc-env=ASSET_VERSION={}", version);
}
