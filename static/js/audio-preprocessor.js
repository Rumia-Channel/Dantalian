let audioPreprocessorModulePromise;

function audioPreprocessorModuleUrl() {
    const version = typeof window.DANTALIAN_ASSET_CACHE_KEY === "string"
        ? window.DANTALIAN_ASSET_CACHE_KEY
        : "dev";
    return `/wasm/audio_preprocessor.js?v=${encodeURIComponent(version)}`;
}

async function loadAudioPreprocessorModule() {
    if (!audioPreprocessorModulePromise) {
        audioPreprocessorModulePromise = import(audioPreprocessorModuleUrl()).then(async (module) => {
            await module.default();
            return module;
        });
    }
    return audioPreprocessorModulePromise;
}

async function preprocessAudioInBrowser(file) {
    if (!file || typeof file.arrayBuffer !== "function") {
        throw new Error("音声ファイルが指定されていません");
    }
    const extension = String(file.name || "").split(".").pop() || "";
    const source = new Uint8Array(await file.arrayBuffer());
    const module = await loadAudioPreprocessorModule();
    return JSON.parse(module.preprocess_audio(source, extension));
}

window.preprocessAudioInBrowser = preprocessAudioInBrowser;
