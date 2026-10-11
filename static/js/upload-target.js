// アップロード先 URL の種別判定。
//
// Worker は `object_kind` でオブジェクトキーの接頭辞 (audio/ か epubs/) と
// 許可拡張子を検証するため、クライアントの判定が 1 文字でもずれると
// 「音声なのに epub として検証される」ような 400 になる。純関数として
// 切り出し、Node のテスト (worker/tests/upload_target_unit.mjs) で固定する。
//
// 重要: 音声のマルチパート経路はセッション URL が `…/audio/multipart` に
// なるため、「`/audio` で終わる」ではなく「`/audio` または `/audio/multipart`
// で終わる」で判定する。`/api/audio/stream/...` のような読み取り専用ルートは
// アップロード先ではないので含めない。
const UPLOAD_AUDIO_PATH_PATTERN = /\/audio(?:\/multipart)?(?:\?|$)/;

function uploadTargetIsAudio(uploadUrl) {
    return typeof uploadUrl === "string" && UPLOAD_AUDIO_PATH_PATTERN.test(uploadUrl);
}

function uploadTargetObjectKind(uploadUrl) {
    return uploadTargetIsAudio(uploadUrl) ? "audio" : "epub";
}

// マルチパートのセッション URL。finalize でトラックへ結び付ける音声だけが
// `…/audio/multipart` を持つ。既にセッション URL が渡された場合は二重に
// 付けない (判定がずれたときに `…/multipart/multipart` を作らないため)。
function uploadTargetAudioSessionUrl(uploadUrl) {
    const base = String(uploadUrl).replace(/\/multipart$/, "");
    return `${base}/multipart`;
}

if (typeof module !== "undefined" && module.exports) {
    module.exports = {
        uploadTargetIsAudio,
        uploadTargetObjectKind,
        uploadTargetAudioSessionUrl,
    };
}
