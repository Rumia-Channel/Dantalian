// Shared helpers for the Wasabi audio E2E contracts. The audio path uploads the
// object parts straight to Wasabi with presigned URLs, so the tests exercise the
// same init/sign/complete/finalize sequence the browser uses.
export const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
export const apiToken = process.env.WORKER_API_TOKEN ?? "";
export const enabled = process.env.WASABI_E2E === "1";
export const testPrefix = process.env.WASABI_TEST_PREFIX ?? "";

const MULTIPART_PATH = "/api/uploads/multipart";
const partSize = 8 * 1024 * 1024;

export async function jsonRequest(method, path, body, token = apiToken) {
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  return {
    response,
    body: text.length === 0 ? undefined : JSON.parse(text),
  };
}

function makePart(length, seed) {
  const bytes = Buffer.alloc(length);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = (index * 31 + seed) % 256;
  }
  return bytes;
}

/// Uploads `bytes` through the presigned multipart contract and returns the
/// session plus the completed object metadata.
///
/// `options.sessionUrl` selects the route shape. The browser posts to the track
/// scoped `<track audio url>/multipart`, so tests that model the client must
/// pass it; the default keeps the generic `/api/uploads/multipart` contract.
export async function uploadAudioMultipart(bytes, options = {}) {
  const fileName = options.fileName ?? "track.flac";
  const contentType = options.contentType ?? "audio/flac";
  const chunkSize = options.partSize ?? partSize;
  const sessionUrl = options.sessionUrl ?? null;
  const initPath = sessionUrl ?? `${MULTIPART_PATH}/init`;
  const initialized = await jsonRequest("POST", initPath, {
    expected_size: bytes.length,
    content_type: contentType,
    object_kind: "audio",
    file_name: fileName,
  });
  if (initialized.response.status !== 200) {
    throw new Error(
      `audio multipart init failed: ${initialized.response.status} ${JSON.stringify(initialized.body)}`,
    );
  }
  const sessionId = initialized.body.id;
  const sessionBase = sessionUrl ? `${sessionUrl}/${sessionId}` : `${MULTIPART_PATH}/${sessionId}`;
  const parts = [];
  let partNumber = 1;
  for (let offset = 0; offset < bytes.length; offset += chunkSize, partNumber += 1) {
    const signed = await jsonRequest("POST", `${sessionBase}/parts/${partNumber}/sign`);
    if (signed.response.status !== 200) {
      throw new Error(`audio multipart sign failed: ${signed.response.status}`);
    }
    const uploaded = await fetch(signed.body.upload_url, {
      method: "PUT",
      body: bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length)),
    });
    if (!uploaded.ok) {
      throw new Error(`presigned part ${partNumber} failed: ${uploaded.status}`);
    }
    const etag = uploaded.headers.get("etag");
    if (!etag) throw new Error(`presigned part ${partNumber} exposed no ETag`);
    parts.push({ part_number: partNumber, etag });
  }
  const completed = await jsonRequest("POST", `${sessionBase}/complete`, { parts });
  return {
    sessionId,
    objectKey: initialized.body.object_key,
    partSize: initialized.body.part_size,
    completed,
    parts,
  };
}

/// Two 6 MiB parts: Wasabi requires every part except the last to be at least
/// 5 MiB, so the contract cannot be exercised with a tiny fixture.
export function audioFixture() {
  const parts = [makePart(6 * 1024 * 1024, 3), makePart(6 * 1024 * 1024, 11)];
  return { parts, bytes: Buffer.concat(parts) };
}
