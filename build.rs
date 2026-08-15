use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let html_files = [
        "static/index.html",
        "static/register/index.html",
        "static/manage/index.html",
        "static/edit/index.html",
        "static/authors/index.html",
        "static/music/index.html",
        "static/licenses/index.html",
    ];

    let css_files = [
        "static/css/base.css",
        "static/css/auth.css",
        "static/css/form.css",
        "static/css/book-card.css",
        "static/css/series.css",
        "static/css/detail.css",
        "static/css/settings.css",
        "static/css/responsive.css",
        "static/css/player.css",
        "static/css/player-queue.css",
        "static/css/music.css",
        "static/css/playlist.css",
        "static/css/licenses.css",
    ];

    let js_files = [
        "static/js/header.js",
        "static/js/utils.js",
        "static/js/searchable-select.js",
        "static/js/settings.js",
        "static/js/upload.js",
        "static/js/audio-preprocessor.js",
        "static/js/register/tabs.js",
        "static/js/register/isbn.js",
        "static/js/register/isdn.js",
        "static/js/register/cd-candidates.js",
        "static/js/register/cd.js",
        "static/js/register/audiobook.js",
        "static/js/register/manual.js",
        "static/js/series.js",
        "static/js/storage-locations.js",
        "static/js/labels.js",
        "static/js/book-grid.js",
        "static/js/detail.js",
        "static/js/authors.js",
        "static/js/edit/copies.js",
        "static/js/edit/tracks.js",
        "static/js/edit/main.js",
        "static/js/borrowers.js",
        "static/js/settings-manage.js",
        "static/js/player/audio-source.js",
        "static/js/player/audio-cache.js",
        "static/js/player/engine.js",
        "static/js/player/ui.js",
        "static/js/music/playlists.js",
        "static/js/music/playlist-editor.js",
        "static/js/music/main.js",
        "static/js/licenses.js",
        "static/js/app.js",
    ];

    let wasm_files = [
        "static/wasm/audio_preprocessor.js",
        "static/wasm/audio_preprocessor_bg.wasm",
    ];

    let font_files = [
        "static/fonts/MaterialIcons-Regular.ttf",
        "static/fonts/MaterialIconsOutlined-Regular.otf",
    ];

    let image_files = ["static/favicon.svg"];

    println!("cargo:rerun-if-changed=about.toml");
    println!("cargo:rerun-if-changed=about.hbs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=LICENSE");
    println!("cargo:rerun-if-changed=NOTICE");
    println!("cargo:rerun-if-changed=MODULE_LICENSE_FRAUNHOFER");
    println!("cargo:rerun-if-env-changed=DANTALIAN_GENERATE_LICENSES");

    for file in html_files
        .iter()
        .chain(css_files.iter())
        .chain(js_files.iter())
        .chain(wasm_files.iter())
        .chain(font_files.iter())
        .chain(image_files.iter())
    {
        println!("cargo:rerun-if-changed={}", file);
    }

    let mut hasher = DefaultHasher::new();

    for file in html_files
        .iter()
        .chain(css_files.iter())
        .chain(wasm_files.iter())
        .chain(font_files.iter())
        .chain(image_files.iter())
    {
        let path = Path::new(file);
        if path.exists() {
            let meta = std::fs::metadata(path).unwrap();
            meta.modified().unwrap().hash(&mut hasher);
        }
    }
    let hash = hasher.finish();

    let version = format!("{hash:x}");
    if std::env::var_os("CARGO_FEATURE_NATIVE").is_some() {
        if std::env::var_os("DANTALIAN_GENERATE_LICENSES").is_some() {
            generate_license_page(&version);
        } else {
            generate_basic_license_page(&version);
        }
    }
    println!("cargo:rustc-env=ASSET_VERSION={version}");
}

fn generate_license_page(version: &str) {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let output = out_dir.join("licenses.html");
    let cargo = std::env::var_os("CARGO_ABOUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let status = Command::new(&cargo)
        .current_dir(manifest_dir)
        .args([
            "about",
            "generate",
            "--locked",
            "--offline",
            "--all-features",
            "--output-file",
        ])
        .arg(&output)
        .arg("about.hbs")
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run cargo-about; install cargo-about 0.8.4 or set CARGO_ABOUT: {error}"
            )
        });

    if !status.success() {
        panic!("cargo-about failed with status {status}");
    }

    let html = std::fs::read_to_string(&output)
        .unwrap_or_else(|error| panic!("failed to read generated license page: {error}"));
    let license = std::fs::read_to_string("LICENSE")
        .unwrap_or_else(|error| panic!("failed to read project license: {error}"));
    let html = html
        .replace("ASSET_VERSION", version)
        .replace("__DANTALIAN_LICENSE__", &license);
    std::fs::write(&output, html)
        .unwrap_or_else(|error| panic!("failed to write generated license page: {error}"));
}
fn generate_basic_license_page(version: &str) {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let output = out_dir.join("licenses.html");
    let license = std::fs::read_to_string("LICENSE")
        .unwrap_or_else(|error| panic!("failed to read project license: {error}"));
    let notice = std::fs::read_to_string("NOTICE").unwrap_or_default();
    let escape = |value: &str| {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Dantalian licenses</title></head><body><h1>Dantalian licenses</h1><h2>Project license</h2><pre>{}</pre><h2>Third-party notices</h2><pre>{}</pre><p>Asset version: {version}</p></body></html>",
        escape(&license),
        escape(&notice),
    );
    std::fs::write(output, html).unwrap_or_else(|error| {
        panic!("failed to write basic license page: {error}");
    });
}
