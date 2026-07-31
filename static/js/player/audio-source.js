// 省データ再生用の音声ソース選択。設定が無効なら常に原音源へ接続する。

let audioDataSaverPolicy = {
    enabled: false,
    extensions: new Set(),
};

async function loadAudioDataSaverPolicy() {
    try {
        const response = await fetch("/api/settings", { cache: "no-store" });
        if (!response.ok) return;
        const settings = await response.json();
        const enabled = String(settings["audio.data_saver.enabled"] || "false").toLowerCase();
        const extensions = String(settings["audio.data_saver.extensions"] || "wav,flac,aiff,alac")
            .split(",")
            .map((value) => value.trim().replace(/^\./, "").toLowerCase())
            .filter((value) => /^[a-z0-9]{1,10}$/.test(value));
        audioDataSaverPolicy = {
            enabled: enabled === "true" || enabled === "1" || enabled === "on",
            extensions: new Set(extensions),
        };
    } catch {
        audioDataSaverPolicy = { enabled: false, extensions: new Set() };
    }
}

function audioSourceCandidates(track) {
    const fileHash = String(track?.file_hash || "");
    if (!fileHash) return [];
    const original = `/audio/${encodeURIComponent(fileHash)}`;
    const fileName = String(track.file_name || "");
    const extension = fileName.includes(".")
        ? fileName.split(".").pop().toLowerCase()
        : "";
    if (!audioDataSaverPolicy.enabled || !audioDataSaverPolicy.extensions.has(extension)) {
        return [original];
    }

    const query = `ext=${encodeURIComponent(extension)}`;
    const opus = `/api/audio/stream/${encodeURIComponent(fileHash)}?${query}&format=opus`;
    const aac = `/api/audio/stream/${encodeURIComponent(fileHash)}?${query}&format=aac`;
    const probe = document.createElement("audio");
    const canPlayOpus = probe.canPlayType('audio/ogg; codecs="opus"') !== "";
    const canPlayAac = probe.canPlayType("audio/aac") !== "";
    const preferred = [];
    if (canPlayOpus) preferred.push(opus);
    if (canPlayAac) preferred.push(aac);
    return [...preferred, original];
}
