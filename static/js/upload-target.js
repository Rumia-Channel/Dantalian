// アップロード先の種別と派生 URL。
//
// Worker は `object_kind` でオブジェクトキーの接頭辞 (audio/ か epubs/) と
// 許可拡張子を検証するため、クライアントの判定が 1 文字でもずれると
// 「音声なのに epub として検証される」ような 400 になる。URL の組み立ても
// 同様に 404 の原因になるため、すべて純関数としてここに集約し
// Node のテスト (worker/tests/upload_target_unit.mjs) で固定する。
//
// 基底はトラックの音声 URL (`…/audio`)。派生先は 3 つある。
//   - セッション: `…/audio/multipart` (init / parts / complete)
//   - finalize : `…/audio/finalize`   (セッションをトラックへ結び付ける)
// 後者はセッション URL を基底にしてはいけない (`…/multipart/finalize` は 404)。
const UPLOAD_AUDIO_PATH_PATTERN = /\/audio(?:\/multipart)?(?:\?|$)/;

function uploadTargetIsAudio(uploadUrl) {
    return typeof uploadUrl === "string" && UPLOAD_AUDIO_PATH_PATTERN.test(uploadUrl);
}

function uploadTargetObjectKind(uploadUrl) {
    return uploadTargetIsAudio(uploadUrl) ? "audio" : "epub";
}

function uploadTargetStripSessionSuffix(uploadUrl) {
    return String(uploadUrl).replace(/\/multipart$/, "");
}

// マルチパートのセッション URL。finalize でトラックへ結び付ける音声だけが
// `…/audio/multipart` を持つ。`/api/uploads/multipart/init` のような汎用 EPUB
// ルートには付けない。既にセッション URL が渡された場合は二重に付けない。
function uploadTargetAudioSessionUrl(uploadUrl) {
    const base = uploadTargetStripSessionSuffix(uploadUrl);
    if (/\/uploads\/multipart$/.test(base)) return base;
    return `${base}/multipart`;
}

// finalize の URL。セッション URL ではなくトラックの音声 URL を基底にする。
function uploadTargetAudioFinalizeUrl(uploadUrl) {
    return `${uploadTargetStripSessionSuffix(uploadUrl)}/finalize`;
}

// 大容量アップロードの presigned multipart セッション URL。
// 音声はトラック URL 配下の専用セッション (`…/audio/multipart`) を使うが、
// EPUB はアップロード先 URL に対応するセッションを持たないため、汎用の
// `/api/uploads/multipart` を使う (Worker 側の object_kind は epub)。
const UPLOAD_GENERIC_MULTIPART_PATH = "/api/uploads/multipart";
// EPUB (書籍ファイル) のアップロード先か。カバー (`…/cover`) や音声は含めない。
const UPLOAD_EPUB_PATH_PATTERN = /\/api\/books\/\d+\/epub(?:\?|$)/;

function uploadTargetIsEpubUpload(uploadUrl) {
    return typeof uploadUrl === "string" && UPLOAD_EPUB_PATH_PATTERN.test(uploadUrl);
}

function uploadTargetPresignedSessionUrl(uploadUrl) {
    if (uploadTargetIsAudio(uploadUrl)) return uploadTargetAudioSessionUrl(uploadUrl);
    return UPLOAD_GENERIC_MULTIPART_PATH;
}

// セッション開始 (init) の URL。音声のセッション URL 自体が init ルートだが、
// 汎用 EPUB セッションの init は `/api/uploads/multipart/init` にあり、
// セッション URL (`…/multipart`) に POST しても 404 になる。
// parts / complete / abort はセッション URL の下 (`…/multipart/:id/...`) を使う。
function uploadTargetPresignedInitUrl(uploadUrl) {
    if (uploadTargetIsAudio(uploadUrl)) return uploadTargetAudioSessionUrl(uploadUrl);
    return `${UPLOAD_GENERIC_MULTIPART_PATH}/init`;
}

// finalize は音声も EPUB もアップロード先 URL の直下 (`…/finalize`) に置く。
function uploadTargetPresignedFinalizeUrl(uploadUrl) {
    if (uploadTargetIsAudio(uploadUrl)) return uploadTargetAudioFinalizeUrl(uploadUrl);
    return `${uploadTargetStripSessionSuffix(uploadUrl)}/finalize`;
}

// ブラウザが返す MIME は環境依存で、.m4a が `video/mp4` や空文字になることが
// ある。Worker は audio/* か application/octet-stream しか受け付けないため、
// 拡張子から既知の MIME を引き直す。未知の拡張子は申告値に任せる。
const UPLOAD_EXTENSION_CONTENT_TYPES = {
    mp3: "audio/mpeg",
    wav: "audio/wav",
    wma: "audio/x-ms-wma",
    flac: "audio/flac",
    ogg: "audio/ogg",
    m4a: "audio/mp4",
    mp4: "audio/mp4",
    aac: "audio/aac",
    opus: "audio/ogg",
    webm: "audio/webm",
    epub: "application/epub+zip",
};

// 送信する content_type。ブラウザの申告値は環境依存なので拡張子を優先する。
function uploadTargetContentType(fileName, reportedType) {
    const name = String(fileName ?? "");
    const dot = name.lastIndexOf(".");
    const extension = dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
    return UPLOAD_EXTENSION_CONTENT_TYPES[extension] ?? (reportedType || "application/octet-stream");
}

// Worker が許可する用途。音声は `audio`、EPUB は `epub`。拡張子だけでは
// 決まらない (`.mp4` は動画にも音声にもなる) ため、`audioOnly` の文脈
// (音声トラックのアップロード) では再生できる拡張子を音声として扱う。
const UPLOAD_AUDIO_EXTENSIONS = ["mp3", "wav", "wma", "flac", "ogg", "m4a", "mp4", "aac", "opus", "webm"];

function uploadTargetPickUploadKind(fileName, reportedType, options = {}) {
    const name = String(fileName ?? "");
    const dot = name.lastIndexOf(".");
    const extension = dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
    const type = String(reportedType ?? "").toLowerCase();
    const isAudio = type.startsWith("audio/") || UPLOAD_AUDIO_EXTENSIONS.includes(extension);
    // `.mp4`/`.m4a` は動画にもなる。音声専用の入力では音声として送る。
    if (options.audioOnly && UPLOAD_AUDIO_EXTENSIONS.includes(extension)) return "audio";
    return isAudio ? "audio" : "epub";
}

if (typeof module !== "undefined" && module.exports) {
    module.exports = {
        uploadTargetIsAudio,
        uploadTargetObjectKind,
        uploadTargetAudioSessionUrl,
        uploadTargetAudioFinalizeUrl,
        uploadTargetIsEpubUpload,
        uploadTargetPresignedSessionUrl,
        uploadTargetPresignedInitUrl,
        uploadTargetPresignedFinalizeUrl,
        uploadTargetContentType,
        uploadTargetPickUploadKind,
    };
}
