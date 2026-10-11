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

if (typeof module !== "undefined" && module.exports) {
    module.exports = {
        uploadTargetIsAudio,
        uploadTargetObjectKind,
        uploadTargetAudioSessionUrl,
        uploadTargetAudioFinalizeUrl,
    };
}
