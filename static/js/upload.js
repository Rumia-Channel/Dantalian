const UPLOAD_CHUNK_SIZE = 90 * 1024 * 1024;

function createUploadId() {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
        return crypto.randomUUID();
    }
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function uploadFileWithChunks(url, fieldName, file) {
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
