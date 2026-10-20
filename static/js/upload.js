const UPLOAD_CHUNK_SIZE = 90 * 1024 * 1024;
// 音声はこのサイズ以上で presigned multipart (ブラウザ → Wasabi 直送) に倒す。
// Cloudflare 経由の単発アップロードはプラン上限 (Free/Pro 100MB) に当たるため。
const PRESIGNED_MULTIPART_MIN_BYTES = 8 * 1024 * 1024;
// Worker の直アップロード上限。EPUB はこれを超えた場合だけ presigned multipart
// (汎用セッション) を使い、カバーなどそれ以外は従来どおり直アップロードのみ。
const WORKER_DIRECT_UPLOAD_MAX_BYTES = 95 * 1024 * 1024;
const MULTIPART_PART_CONCURRENCY = 3;

let workerRuntimePromise;

function createUploadId() {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
        return crypto.randomUUID();
    }
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function isWorkerRuntime() {
    if (!workerRuntimePromise) {
        workerRuntimePromise = fetch("/api/health", { cache: "no-store" })
            .then((response) => response.ok ? response.json() : null)
            .then((body) => body?.runtime === "cloudflare-worker")
            .catch(() => false);
    }
    return workerRuntimePromise;
}

function directUploadLimitResponse(maxBytes) {
    return new Response(
        JSON.stringify({
            error: "file exceeds the direct upload limit",
            code: "presigned_multipart_required",
            max_bytes: maxBytes,
        }),
        {
            status: 413,
            headers: { "Content-Type": "application/json" },
        },
    );
}

// 失敗したアップロードは、原因を特定できるだけの情報 (URL・サイズ・応答本文) を
// 必ずコンソールへ残す。原因不明の 5xx を追うための最低限の観測点。
async function logUploadFailure(url, file, response) {
    const diagnostic = typeof readApiDiagnostic === "function"
        ? await readApiDiagnostic(response)
        : { status: response.status, headers: {}, body: "", json: null, cloudflareCode: "", rayId: "", serverMessage: "" };
    console.error(
        `upload failed: ${file.name} (${file.size} bytes, ${file.type || "unknown"}) -> ${url} | ` +
        (typeof describeApiDiagnostic === "function" ? describeApiDiagnostic(diagnostic) : `status=${diagnostic.status}`),
        diagnostic,
    );
    return diagnostic;
}

// ブラウザから Wasabi へ直接 PUT する経路のエラー。CORS 設定不足は
// fetch が TypeError になり応答すら読めないため、専用の印を付けて UI へ渡す。
function presignedUploadError(kind, message, cause) {
    const error = new Error(message);
    error.presigned = true;
    error.presignedKind = kind;
    if (cause) error.cause = cause;
    return error;
}

async function sha256Hex(file) {
    const digest = await crypto.subtle.digest("SHA-256", await file.arrayBuffer());
    return Array.from(new Uint8Array(digest))
        .map((byte) => byte.toString(16).padStart(2, "0"))
        .join("");
}

async function readUploadJson(response) {
    try {
        return await response.json();
    } catch {
        return null;
    }
}

function multipartSessionId(body) {
    const id = body && typeof body.id === "string" ? body.id : "";
    return /^[0-9a-f]{32}$/i.test(id) ? id : "";
}

// アップロード先の URL からオブジェクト種別を決める。Worker はこの種別で
// オブジェクトキーの接頭辞 (epubs/ か audio/) と許可拡張子を検証する。
// 判定は js/upload-target.js の純関数に置き、テストで固定している。
function isAudioUploadTarget(sessionUrl) {
    return uploadTargetIsAudio(sessionUrl);
}

function multipartObjectKind(sessionUrl) {
    return uploadTargetObjectKind(sessionUrl);
}

// 音声のマルチパート経路は finalize でトラックへ結び付けるため、`…/audio` の
// 下に専用のセッション URL を持つ。
function audioMultipartSessionUrl(sessionUrl) {
    return uploadTargetAudioSessionUrl(sessionUrl);
}

// finalize の基底はセッション URL ではなくトラックの音声 URL。
function audioFinalizeUrl(uploadUrl) {
    return uploadTargetAudioFinalizeUrl(uploadUrl);
}

// クライアントと Worker のデプロイ時期がずれて presigned multipart ルートが
// 未実装の場合に、その間だけ従来の直アップロードへ戻す。恒久的に落とすと、
// 一時的な 404 で以降の大容量アップロードが全部 Worker を通ってしまうため、
// クールダウン付きで扱う。
const PRESIGNED_MULTIPART_RETRY_AFTER_MS = 30_000;
let presignedMultipartUnavailableUntil = 0;

function presignedMultipartRouteUnavailable() {
    return Date.now() < presignedMultipartUnavailableUntil;
}

function markPresignedMultipartUnavailable() {
    presignedMultipartUnavailableUntil = Date.now() + PRESIGNED_MULTIPART_RETRY_AFTER_MS;
}

async function startPresignedMultipartUpload(sessionUrl, file, uploadKind) {
    const response = await fetch(sessionUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            expected_size: file.size,
            content_type: uploadTargetContentType(file.name, file.type),
            // セッション URL ではなくファイルから用途を決める。`.m4a` を音声として
            // 送るため、URL 判定だけに頼らない。
            object_kind: uploadKind ?? multipartObjectKind(sessionUrl),
            file_name: file.name,
        }),
    });
    if (response.status === 404) {
        return { routeUnavailable: true };
    }
    if (!response.ok) {
        await logUploadFailure(sessionUrl, file, response.clone());
        return { error: response };
    }
    const body = await readUploadJson(response);
    const sessionId = multipartSessionId(body);
    const partSize = Number(body?.part_size);
    if (!sessionId || !Number.isFinite(partSize) || partSize <= 0) {
        return { error: new Response(
            JSON.stringify({ error: "presigned multipart session response was invalid" }),
            { status: 502, headers: { "Content-Type": "application/json" } },
        ) };
    }
    return { sessionId, partSize };
}

// 中断時の後始末。EPUB の汎用セッションは削除ルートで中断できるが、音声の
// セッション削除ルートは未実装なので 404 は握りつぶす (送信済みパートは
// Wasabi 側のライフサイクルで失効する)。
async function abortPresignedMultipartUpload(upload) {
    try {
        await fetch(`${upload.sessionUrl}/${upload.sessionId}`, { method: "DELETE" });
    } catch {}
}

async function putPresignedPart(sessionUrl, sessionId, partNumber, chunk, file, upload) {
    const signResponse = await fetch(
        `${sessionUrl}/${sessionId}/parts/${partNumber}/sign`,
        { method: "POST" },
    );
    if (!signResponse.ok) {
        await logUploadFailure(`${sessionUrl}/${sessionId}/parts/${partNumber}/sign`, file, signResponse.clone());
        upload.failedResponse = signResponse;
        throw presignedUploadError("sign", `multipart part ${partNumber} could not be signed`);
    }
    const signed = await readUploadJson(signResponse);
    const uploadUrl = signed && typeof signed.upload_url === "string" ? signed.upload_url : "";
    if (!uploadUrl) {
        upload.failedResponse = new Response(
            JSON.stringify({ error: "presigned part response was invalid" }),
            { status: 502, headers: { "Content-Type": "application/json" } },
        );
        throw presignedUploadError("sign", `multipart part ${partNumber} returned no upload URL`);
    }
    let partResponse;
    try {
        // Content-Type は署名対象に含めないので付けない (単純リクエストのままにする)。
        partResponse = await fetch(uploadUrl, { method: "PUT", body: chunk });
    } catch (cause) {
        throw presignedUploadError(
            "put",
            `multipart part ${partNumber} could not reach the object storage`,
            cause,
        );
    }
    if (!partResponse.ok) {
        const diagnostic = await logUploadFailure(`part ${partNumber} -> object storage`, file, partResponse.clone());
        upload.failedResponse = new Response(
            JSON.stringify({
                error: "object storage rejected the uploaded part",
                status: diagnostic.status,
                body: diagnostic.body.slice(0, 300),
            }),
            { status: 502, headers: { "Content-Type": "application/json" } },
        );
        throw presignedUploadError("put", `multipart part ${partNumber} was rejected`);
    }
    const etag = (partResponse.headers.get("etag") || "").trim();
    if (!etag) {
        upload.failedResponse = new Response(
            JSON.stringify({
                error: "object storage response did not expose the part ETag",
                hint: "Expose the ETag header in the Wasabi bucket CORS rules",
            }),
            { status: 502, headers: { "Content-Type": "application/json" } },
        );
        throw presignedUploadError("etag", `multipart part ${partNumber} returned no ETag`);
    }
    return { part_number: partNumber, etag };
}

// 大容量アップロード: init → パートを並列 PUT → complete → finalize。
// 音声はセッションをトラックへ、EPUB はセッションを書籍へ結び付ける。
// Worker は本文を受け取らず、セッション管理と検証だけを行う。
// マルチパート経路が未実装の Worker に当たった場合は null を返し、呼び出し側が
// 従来の直アップロードへ戻せるようにする。
async function uploadWithPresignedMultipart(uploadUrl, file, uploadKind) {
    const sessionUrl = uploadTargetPresignedSessionUrl(uploadUrl);
    const finalizeUrl = uploadTargetPresignedFinalizeUrl(uploadUrl);
    const started = await startPresignedMultipartUpload(sessionUrl, file, uploadKind);
    if (started.routeUnavailable) {
        markPresignedMultipartUnavailable();
        console.warn(
            "presigned multipart endpoint is unavailable on this Worker; " +
            "falling back to the Worker direct upload path",
        );
        return null;
    }
    if (started.error) return started.error;
    const upload = { sessionUrl, uploadUrl, sessionId: started.sessionId, failedResponse: null };
    const partSize = started.partSize;
    const totalParts = Math.ceil(file.size / partSize);
    const parts = new Array(totalParts);
    try {
        for (let index = 0; index < totalParts; index += MULTIPART_PART_CONCURRENCY) {
            const batch = [];
            for (let offset = 0; offset < MULTIPART_PART_CONCURRENCY && index + offset < totalParts; offset++) {
                const partIndex = index + offset;
                const chunk = file.slice(partIndex * partSize, Math.min((partIndex + 1) * partSize, file.size));
                batch.push(putPresignedPart(sessionUrl, upload.sessionId, partIndex + 1, chunk, file, upload));
            }
            const settled = await Promise.all(batch);
            settled.forEach((part, offset) => {
                parts[index + offset] = part;
            });
        }
        const completeResponse = await fetch(`${sessionUrl}/${upload.sessionId}/complete`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ parts }),
        });
        if (!completeResponse.ok) {
            await logUploadFailure(`${sessionUrl}/${upload.sessionId}/complete`, file, completeResponse.clone());
            return completeResponse;
        }
        return await fetch(finalizeUrl, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ session_id: upload.sessionId, name: file.name }),
        });
    } catch (error) {
        await abortPresignedMultipartUpload(upload);
        if (upload.failedResponse) return upload.failedResponse;
        throw error;
    }
}

// `uploadKind` (`audio` / `epub`) を渡すと、Worker の content_type 検証と
// 拡張子判定をファイル基準で行える。省略時は URL から推定する。
async function uploadFileWithChunks(url, fieldName, file, uploadKind) {
    const kind = uploadKind || (isAudioUploadTarget(url) ? "audio" : "epub");
    // presigned multipart は音声 (8MiB 以上) と、Worker の直アップロード上限を
    // 超える EPUB が使う。カバーは常に直アップロードのみ。
    const presignedEligible = kind === "audio" || uploadTargetIsEpubUpload(url);
    // Worker routes accept a single multipart request up to their direct-upload
    // limit. The native chunk protocol is a different contract and must not be
    // sent to the Worker endpoint as a fake file upload.
    if (await isWorkerRuntime()) {
        // 音声は 8MiB から、EPUB は直アップロードでは送れないサイズだけ
        // presigned multipart に切り替える。
        const requiresPresigned = presignedEligible
            && (kind === "audio"
                ? file.size >= PRESIGNED_MULTIPART_MIN_BYTES
                : file.size > WORKER_DIRECT_UPLOAD_MAX_BYTES);
        if (requiresPresigned && !presignedMultipartRouteUnavailable()) {
            // Cloudflare を通さず Wasabi へ直接送る。基底はアップロード先 URL。
            const response = await uploadWithPresignedMultipart(url, file, kind);
            if (response) return response;
        }
        // presigned 経路が使えない旧 Worker では、直アップロード上限を超えた
        // ファイルを送信前に落とす。
        if (file.size > WORKER_DIRECT_UPLOAD_MAX_BYTES) {
            const response = directUploadLimitResponse(WORKER_DIRECT_UPLOAD_MAX_BYTES);
            await logUploadFailure(url, file, response.clone());
            return response;
        }
        const form = new FormData();
        form.append(fieldName, file);
        const response = await fetch(url, { method: "POST", body: form });
        if (!response.ok) await logUploadFailure(url, file, response.clone());
        return response;
    }

    if (file.size <= UPLOAD_CHUNK_SIZE) {
        const form = new FormData();
        form.append(fieldName, file);
        const response = await fetch(url, { method: "POST", body: form });
        if (!response.ok) await logUploadFailure(url, file, response.clone());
        return response;
    }

    const uploadId = createUploadId();
    const totalParts = Math.ceil(file.size / UPLOAD_CHUNK_SIZE);
    let response = null;

    for (let part = 0; part < totalParts; part++) {
        const start = part * UPLOAD_CHUNK_SIZE;
        const end = Math.min(start + UPLOAD_CHUNK_SIZE, file.size);
        const chunk = file.slice(start, end, file.type || "application/octet-stream");
        const query = new URLSearchParams({
            upload_id: uploadId,
            part: String(part),
            total_parts: String(totalParts),
        });
        const form = new FormData();
        form.append(fieldName, chunk, file.name);
        response = await fetch(`${url}?${query}`, { method: "POST", body: form });
        if (!response.ok) {
            // 送信済みのチャンクはサーバー側の一時領域に残る。ネイティブ側の
            // 掃除対象なので、ここでは失敗した部分だけを報告する。
            await logUploadFailure(`${url}?part=${part}/${totalParts}`, chunk, response.clone());
            return response;
        }
    }

    return response;
}

// ブラウザでは読み込むだけ。テスト (worker/tests/upload_integration_unit.mjs)
// からは require して、実際のリクエスト URL 列を検証できるようにする。
if (typeof module !== "undefined" && module.exports) {
    module.exports = { uploadFileWithChunks, audioMultipartSessionUrl, audioFinalizeUrl };
}
