#![cfg(feature = "native")]

use std::{env, time::Instant};

fn main() {
    let mut args = env::args().skip(1);
    let audio_dir = args.next().expect("audio directory");
    let file_hash = args.next().expect("file name");
    let source_extension = args.next().unwrap_or_else(|| "flac".to_string());
    let started = Instant::now();
    let result = dantalian::audio_encoding::ensure_encoded_variants(
        &audio_dir,
        &file_hash,
        &source_extension,
    );
    println!("elapsed_s={:.3}", started.elapsed().as_secs_f64());
    println!("result={result:?}");
}
