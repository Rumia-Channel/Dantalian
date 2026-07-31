// 音声のオフラインキャッシュ。
// 大きな音声本体は IndexedDB、一覧・容量確認用の軽い管理情報は localStorage に保存する。

const AUDIO_CACHE_DB_NAME = "dantalian-audio-cache-v1";
const AUDIO_CACHE_DB_VERSION = 1;
const AUDIO_CACHE_STORE = "tracks";
const AUDIO_CACHE_MANIFEST_KEY = "dantalian_audio_cache_manifest_v1";

let audioCacheDbPromise = null;

function openAudioCacheDb() {
    if (typeof indexedDB === "undefined") {
        return Promise.reject(new Error("このブラウザは音声キャッシュに対応していません"));
    }
    if (audioCacheDbPromise) return audioCacheDbPromise;

    audioCacheDbPromise = new Promise((resolve, reject) => {
        const request = indexedDB.open(AUDIO_CACHE_DB_NAME, AUDIO_CACHE_DB_VERSION);
        request.onupgradeneeded = () => {
            const db = request.result;
            if (!db.objectStoreNames.contains(AUDIO_CACHE_STORE)) {
                db.createObjectStore(AUDIO_CACHE_STORE, { keyPath: "fileHash" });
            }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error || new Error("音声キャッシュDBを開けません"));
    });
    return audioCacheDbPromise;
}

function readAudioCacheManifest() {
    try {
        const value = JSON.parse(localStorage.getItem(AUDIO_CACHE_MANIFEST_KEY) || "{}");
        return value && typeof value === "object" && !Array.isArray(value) ? value : {};
    } catch {
        return {};
    }
}

function writeAudioCacheManifest(manifest) {
    try {
        localStorage.setItem(AUDIO_CACHE_MANIFEST_KEY, JSON.stringify(manifest));
    } catch {
        // IndexedDB が本体の正であり、localStorage は補助的な管理情報なので保存失敗を許容する。
    }
}

async function readAudioCacheRecord(fileHash) {
    const db = await openAudioCacheDb();
    return new Promise((resolve, reject) => {
        const transaction = db.transaction(AUDIO_CACHE_STORE, "readonly");
        const request = transaction.objectStore(AUDIO_CACHE_STORE).get(fileHash);
        request.onsuccess = () => resolve(request.result || null);
        request.onerror = () => reject(request.error || new Error("音声キャッシュを読み込めません"));
    });
}

async function writeAudioCacheRecord(record) {
    const db = await openAudioCacheDb();
    await new Promise((resolve, reject) => {
        const transaction = db.transaction(AUDIO_CACHE_STORE, "readwrite");
        transaction.objectStore(AUDIO_CACHE_STORE).put(record);
        transaction.oncomplete = resolve;
        transaction.onerror = () => reject(transaction.error || new Error("音声キャッシュを保存できません"));
        transaction.onabort = () => reject(transaction.error || new Error("音声キャッシュの保存が中断されました"));
    });

    const manifest = readAudioCacheManifest();
    manifest[record.fileHash] = {
        format: record.format,
        size: record.size,
        cachedAt: record.cachedAt,
        trackId: record.trackId ?? null,
        cdId: record.cdId ?? null,
        title: record.title || "",
    };
    writeAudioCacheManifest(manifest);
}

async function deleteAudioCacheRecord(fileHash) {
    const db = await openAudioCacheDb();
    await new Promise((resolve, reject) => {
        const transaction = db.transaction(AUDIO_CACHE_STORE, "readwrite");
        transaction.objectStore(AUDIO_CACHE_STORE).delete(fileHash);
        transaction.oncomplete = resolve;
        transaction.onerror = () => reject(transaction.error || new Error("音声キャッシュを削除できません"));
        transaction.onabort = () => reject(transaction.error || new Error("音声キャッシュの削除が中断されました"));
    });

    const manifest = readAudioCacheManifest();
    delete manifest[fileHash];
    writeAudioCacheManifest(manifest);
}

function audioCacheExtension(track) {
    const fileName = String(track?.file_name || "");
    const dot = fileName.lastIndexOf(".");
    return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : "";
}

function audioCacheOriginalUrl(fileHash) {
    return `/audio/${encodeURIComponent(fileHash)}`;
}

function audioCacheEncodedUrl(fileHash, extension, format) {
    const query = new URLSearchParams({
        ext: extension,
        format,
        cache: "true",
    });
    return `/api/audio/stream/${encodeURIComponent(fileHash)}?${query.toString()}`;
}

function audioCacheHashStem(fileHash) {
    const dot = fileHash.lastIndexOf(".");
    return dot > 0 ? fileHash.slice(0, dot) : fileHash;
}

function audioCacheCanPlayFormat(format) {
    const probe = document.createElement("audio");
    if (format === "opus") {
        return probe.canPlayType('audio/ogg; codecs="opus"') !== "";
    }
    if (format === "aac") {
        return [
            "audio/aac",
            'audio/mp4; codecs="mp4a.40.2"',
        ].some((mime) => probe.canPlayType(mime) !== "");
    }
    return true;
}

function audioCacheIsEncodedResponse(response, fileHash, format) {
    try {
        const path = new URL(response.url, window.location.href).pathname;
        const expected = `/audio/encoded/${format}/${audioCacheHashStem(fileHash)}.${format}`;
        return path === expected;
    } catch {
        return false;
    }
}

function audioCacheCandidates(track) {
    const fileHash = String(track?.file_hash || "");
    if (!fileHash) return [];
    const extension = audioCacheExtension(track);
    return [
        {
            format: "opus",
            mime: "audio/ogg; codecs=opus",
            url: audioCacheEncodedUrl(fileHash, extension, "opus"),
        },
        {
            format: "aac",
            mime: "audio/aac",
            url: audioCacheEncodedUrl(fileHash, extension, "aac"),
        },
        {
            format: "original",
            mime: "application/octet-stream",
            url: audioCacheOriginalUrl(fileHash),
        },
    ];
}

async function cacheAudioTrack(track, context = {}) {
    const fileHash = String(track?.file_hash || "");
    if (!fileHash) return { ok: false, error: "音声ファイルがありません" };

    const existing = await readAudioCacheRecord(fileHash).catch(() => null);
    if (existing?.blob instanceof Blob && existing.blob.size > 0) {
        return { ok: true, format: existing.format, size: existing.size, reused: true };
    }

    let lastError = null;
    for (const candidate of audioCacheCandidates(track)) {
        if (candidate.format !== "original" && !audioCacheCanPlayFormat(candidate.format)) {
            continue;
        }

        try {
            const response = await fetch(candidate.url, {
                cache: "no-store",
                credentials: "same-origin",
            });
            if (!response.ok) {
                lastError = new Error(`HTTP ${response.status}`);
                continue;
            }
            // エンコード失敗時のAPIは原音へリダイレクトするため、opus/aacとして保存しない。
            if (candidate.format !== "original" && !audioCacheIsEncodedResponse(response, fileHash, candidate.format)) {
                lastError = new Error(`${candidate.format} が生成されませんでした`);
                continue;
            }

            const blob = await response.blob();
            if (!blob.size) {
                lastError = new Error("空の音声データです");
                continue;
            }
            const record = {
                fileHash,
                blob,
                format: candidate.format,
                mime: blob.type || candidate.mime,
                size: blob.size,
                cachedAt: new Date().toISOString(),
                trackId: track.id ?? null,
                cdId: context.cdId ?? null,
                title: track.title || "",
            };
            await writeAudioCacheRecord(record);
            return { ok: true, format: record.format, size: record.size, reused: false };
        } catch (error) {
            lastError = error instanceof Error ? error : new Error(String(error));
        }
    }

    return { ok: false, error: lastError?.message || "再生可能な音声を取得できませんでした" };
}

async function cacheAudioAlbum(cd, onProgress) {
    const tracks = [...new Map(
        (cd?.tracks || [])
            .filter((track) => track && track.file_hash)
            .map((track) => [String(track.file_hash), track])
    ).values()];
    const results = [];
    for (let index = 0; index < tracks.length; index += 1) {
        const track = tracks[index];
        const result = await cacheAudioTrack(track, { cdId: cd?.id });
        results.push({ track, ...result });
        if (typeof onProgress === "function") {
            onProgress({ index: index + 1, total: tracks.length, track, result });
        }
    }
    return {
        total: tracks.length,
        results,
        succeeded: results.filter((item) => item.ok),
        failed: results.filter((item) => !item.ok),
    };
}

async function getAudioCacheStatus(tracks) {
    const uniqueTracks = [...new Map(
        (tracks || [])
            .filter((track) => track && track.file_hash)
            .map((track) => [String(track.file_hash), track])
    ).values()];
    const records = await Promise.all(uniqueTracks.map((track) =>
        readAudioCacheRecord(String(track.file_hash)).catch(() => null)
    ));
    const cached = records.filter((record) => record?.blob instanceof Blob && record.blob.size > 0);
    return {
        total: uniqueTracks.length,
        cached: cached.length,
        allCached: uniqueTracks.length > 0 && cached.length === uniqueTracks.length,
        bytes: cached.reduce((sum, record) => sum + (Number(record.size) || record.blob.size || 0), 0),
        formats: cached.reduce((map, record) => {
            map[record.format] = (map[record.format] || 0) + 1;
            return map;
        }, {}),
    };
}

async function deleteAudioCacheAlbum(cd) {
    const hashes = [...new Set(
        (cd?.tracks || [])
            .filter((track) => track && track.file_hash)
            .map((track) => String(track.file_hash))
    )];
    for (const hash of hashes) {
        await deleteAudioCacheRecord(hash);
    }
    return hashes.length;
}

async function getCachedAudioSource(track) {
    const fileHash = String(track?.file_hash || "");
    if (!fileHash || typeof URL === "undefined" || typeof URL.createObjectURL !== "function") return null;
    const record = await readAudioCacheRecord(fileHash).catch(() => null);
    if (!record?.blob || record.blob.size === 0) return null;
    return {
        url: URL.createObjectURL(record.blob),
        format: record.format,
    };
}

function releaseCachedAudioSource(sourceUrl) {
    if (!sourceUrl || typeof URL === "undefined" || typeof URL.revokeObjectURL !== "function") return;
    URL.revokeObjectURL(sourceUrl);
}
