const WORKER_DIRECT_UPLOAD_MAX_BYTES = 95 * 1024 * 1024;
const UPLOAD_CHUNK_SIZE = 90 * 1024 * 1024;

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

async function uploadFileWithChunks(url, fieldName, file) {
    // Worker routes accept a single multipart request up to their direct-upload
    // limit. The native chunk protocol is a different contract and must not be
    // sent to the Worker endpoint as a fake file upload.
    if (await isWorkerRuntime()) {
    if (file.size > WORKER_DIRECT_UPLOAD_MAX_BYTES) {
        return new Response(
            JSON.stringify({
                error: "file exceeds the Worker direct upload limit",
                code: "presigned_multipart_required",
                max_bytes: WORKER_DIRECT_UPLOAD_MAX_BYTES,
            }),
            {
                status: 413,
                headers: { "Content-Type": "application/json" },
            },
        );
    }
    const form = new FormData();
    form.append(fieldName, file);
    return fetch(url, { method: "POST", body: form });
    }

    if (file.size <= UPLOAD_CHUNK_SIZE) {
        const form = new FormData();
        form.append(fieldName, file);
        return fetch(url, { method: "POST", body: form });
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
        if (!response.ok) return response;
    }

    return response;
}
