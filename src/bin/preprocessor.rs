use dantalian::audio_preprocessor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn preprocess_audio(source: &[u8], extension: &str) -> Result<String, JsValue> {
    let metadata = audio_preprocessor::inspect(source, extension)
        .map_err(|error| JsValue::from_str(&error))?;
    serde_json::to_string(&metadata).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn main() {}
