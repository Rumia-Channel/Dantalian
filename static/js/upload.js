const UPLOAD_CHUNK_SIZE = 90 * 1024 * 1024;
// Worker 経由の直列アップロードは Cloudflare のリクエスト/メモリ上限に当たるため、
// これを超えるメディアは presigned multipart でブラウザから Wasabi へ直接送る。
const PRESIGNED_MULTIPART_MIN_BYTES = 8 * 1024 * 1024;
// ネイティブ側のチャンク上限。これ以上の本文はどちらの経路でも受け取れない。
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
function isAudioUploadTarget(sessionUrl) {
    return /\/audio(\?|$)/.test(sessionUrl);
}

function multipartObjectKind(sessionUrl) {
    return isAudioUploadTarget(sessionUrl) ? "audio" : "epub";
}

// 音声のマルチパート経路は finalize でトラックへ結び付けるため、`…/audio` の
// 下に専用のセッション URL を持つ。
function audioMultipartSessionUrl(sessionUrl) {
    return `${sessionUrl}/multipart`;
}

// クライアントと Worker のデプロイ時期がずれて `…/audio/multipart` が未実装の
// 場合に true へ。以降は従来の直アップロードへ戻す。本文を送る前の 404 で
// 判明するので、曲全体を無駄に送らずに済む。
let audioMultipartRouteUnavailable = false;

async function startPresignedMultipartUpload(sessionUrl, file) {
    const response = await fetch(sessionUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            expected_size: file.size,
            content_type: file.type || "application/octet-stream",
            object_kind: multipartObjectKind(sessionUrl),
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

async function abortPresignedMultipartUpload(upload, sessionUrl) {
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

// 音声の大容量アップロード: init → パートを並列 PUT → complete → finalize。
// Worker は本文を受け取らず、セッション管理と検証だけを行う。
// マルチパート経路が未実装の Worker に当たった場合は null を返し、呼び出し側が
// 従来の直アップロードへ戻せるようにする。
async function uploadWithPresignedMultipart(sessionUrl, file) {
    const started = await startPresignedMultipartUpload(sessionUrl, file);
    if (started.routeUnavailable) {
        audioMultipartRouteUnavailable = true;
        console.warn(
            "presigned multipart endpoint is unavailable on this Worker; " +
            "falling back to the Worker direct upload path",
        );
        return null;
    }
    if (started.error) return started.error;
    const upload = { sessionUrl, sessionId: started.sessionId, failedResponse: null };
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
        return await fetch(`${sessionUrl}/finalize`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ session_id: upload.sessionId, name: file.name }),
        });
    } catch (error) {
        await abortPresignedMultipartUpload(upload, sessionUrl);
        if (upload.failedResponse) return upload.failedResponse;
        throw error;
    }
}

async function uploadFileWithChunks(url, fieldName, file) {
    // Worker routes accept a single multipart request up to their direct-upload
    // limit. The native chunk protocol is a different contract and must not be
    // sent to the Worker endpoint as a fake file upload.
    if (await isWorkerRuntime()) {
        // 音声だけが presigned multipart の受け入れ先 (finalize) を持つ。
        // カバーや EPUB は Worker の直アップロードのみで、完了後の結び付けが
        // まだ無いため、上限超過は送信前に明示的に落とす。
        if (
            isAudioUploadTarget(url) &&
            file.size >= PRESIGNED_MULTIPART_MIN_BYTES &&
            !audioMultipartRouteUnavailable
        ) {
            // 大きい音声は Cloudflare を通さず Wasabi へ直接送る。
            const response = await uploadWithPresignedMultipart(audioMultipartSessionUrl(url), file);
            if (response) return response;
        }
        if (!isAudioUploadTarget(url) && file.size > WORKER_DIRECT_UPLOAD_MAX_BYTES) {
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

    if (file.size > WORKER_DIRECT_UPLOAD_MAX_BYTES) {
        // ネイティブ用チャンクは 90MB 刻みなので通常ここには来ない。Worker の
        // 直アップロード上限を超える本文を送って 5xx になる前に明示的に落とす。
        const response = directUploadLimitResponse(WORKER_DIRECT_UPLOAD_MAX_BYTES);
        await logUploadFailure(url, file, response.clone());
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
