use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

fn main() {
    let html_files = [
        "static/index.html",
        "static/register/index.html",
        "static/manage/index.html",
        "static/edit/index.html",
        "static/authors/index.html",
    ];

    let css_files = [
        "static/css/base.css",
        "static/css/form.css",
        "static/css/book-card.css",
        "static/css/series.css",
        "static/css/detail.css",
        "static/css/settings.css",
    ];

    let js_files = [
        "static/js/utils.js",
        "static/js/searchable-select.js",
        "static/js/settings.js",
        "static/js/register.js",
        "static/js/series.js",
        "static/js/book-grid.js",
        "static/js/detail.js",
        "static/js/authors.js",
        "static/js/edit.js",
        "static/js/app.js",
    ];

    let font_files = [
        "static/fonts/MaterialIcons-Regular.ttf",
        "static/fonts/MaterialIconsOutlined-Regular.otf",
    ];

    for file in html_files
        .iter()
        .chain(css_files.iter())
        .chain(js_files.iter())
        .chain(font_files.iter())
    {
        println!("cargo:rerun-if-changed={}", file);
    }

    let mut hasher = DefaultHasher::new();

    for file in html_files
        .iter()
        .chain(css_files.iter())
        .chain(js_files.iter())
        .chain(font_files.iter())
    {
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
