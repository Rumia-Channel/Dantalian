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

// 生成物の実体に合わせて再生可否を判定する。
// Opus は Ogg コンテナ、AAC は ADTS の裸ストリームで、MP4 コンテナは別の形式なので
// audio/mp4 の対応結果を raw AAC の再生可否として流用しない。
const AUDIO_VARIANT_MIME_TYPES = {
    opus: ['audio/ogg; codecs="opus"'],
    aac: ["audio/aac"],
};

function audioVariantCanPlay(format) {
    const mimeTypes = AUDIO_VARIANT_MIME_TYPES[format];
    if (!mimeTypes) return false;
    const probe = document.createElement("audio");
    return mimeTypes.some((mimeType) => probe.canPlayType(mimeType) !== "");
}

// 拡張子の解決順はサーバー側 (source_extension) と同じにする。
// ファイル名に拡張子が無ければ、末尾が拡張子を表す旧形式の file_hash を使う。
function audioTrackExtension(track) {
    const fileName = String(track?.file_name || "");
    const fromName = fileName.includes(".") ? fileName.split(".").pop().toLowerCase() : "";
    if (fromName) return fromName;
    const fileHash = String(track?.file_hash || "");
    return fileHash.includes(".") ? fileHash.split(".").pop().toLowerCase() : "";
}

async function fetchAudioPlayability(track, extension) {
    const fileHash = String(track?.file_hash || "");
    if (!fileHash || !extension) return null;
    try {
        const response = await fetch(
            `/api/audio/playability/${encodeURIComponent(fileHash)}?ext=${encodeURIComponent(extension)}`,
            { cache: "no-store", credentials: "same-origin" },
        );
        if (!response.ok) return null;
        const value = await response.json();
        if (!value || typeof value !== "object") return null;
        if (!["original", "opus", "aac"].every((key) => value[key] && typeof value[key] === "object")) {
            return null;
        }
        return value;
    } catch {
        return null;
    }
}

function audioSourceCandidatesFromPlayability(track, playability) {
    const fileHash = String(track?.file_hash || "");
    if (!fileHash) return { urls: [], sizes: [] };
    const original = `/audio/${encodeURIComponent(fileHash)}`;
    const extension = audioTrackExtension(track);
    const compressedAllowed = audioDataSaverPolicy.enabled
        && audioDataSaverPolicy.extensions.has(extension);
    if (!compressedAllowed) {
        return {
            urls: [original],
            sizes: [playability?.original?.size_bytes ?? null],
        };
    }

    const query = `ext=${encodeURIComponent(extension)}`;
    const variants = [
        {
            format: "opus",
            url: `/api/audio/stream/${encodeURIComponent(fileHash)}?${query}&format=opus`,
            playable: audioVariantCanPlay("opus"),
        },
        {
            format: "aac",
            url: `/api/audio/stream/${encodeURIComponent(fileHash)}?${query}&format=aac`,
            playable: audioVariantCanPlay("aac"),
        },
    ];
    const candidates = variants.filter((variant) => {
        if (!variant.playable) return false;
        return !playability || playability[variant.format]?.available !== false;
    });
    if (!playability || playability.original?.available !== false) {
        candidates.push({
            format: "original",
            url: original,
            playable: true,
        });
    }
    return {
        urls: candidates.map((candidate) => candidate.url),
        sizes: candidates.map((candidate) => playability?.[candidate.format]?.size_bytes ?? null),
    };
}

async function audioSourceSelection(track) {
    const playability = await fetchAudioPlayability(track, audioTrackExtension(track));
    return audioSourceCandidatesFromPlayability(track, playability);
}

// キャッシュ処理など、再生可否APIを使わない呼び出し向けの従来候補。
function audioSourceCandidates(track) {
    return audioSourceCandidatesFromPlayability(track, null).urls;
}
