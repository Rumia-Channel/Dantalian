import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "";
const enabled = process.env.WASABI_E2E === "1";
const testPrefix = process.env.WASABI_TEST_PREFIX ?? "";
const authHeaders = {
  authorization: `Bearer ${apiToken}`,
  "content-type": "application/json",
};
const partSize = 8 * 1024 * 1024;

async function jsonRequest(method, path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: authHeaders,
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
    bytes[index] = (index * 29 + seed) % 256;
  }
  return bytes;
}

test("Wasabi EPUB multipart upload completes through direct part URLs", { skip: !enabled }, async () => {
  const parts = [makePart(partSize, 7), makePart(partSize, 19)];
  const expected = Buffer.concat(parts);
  let sessionId;

  const initialized = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: expected.length,
    content_type: "application/epub+zip",
  });
  assert.equal(initialized.response.status, 200);
  sessionId = initialized.body.id;
  assert.match(sessionId, /^[a-f0-9]{32}$/);
  assert.equal(initialized.body.part_size, partSize);
  if (testPrefix) {
    assert.ok(initialized.body.object_key.startsWith(`${testPrefix}/`));
  }
  const fileName = initialized.body.object_key.split("/").at(-1);
  const uploadedParts = [];

  for (const [index, part] of parts.entries()) {
    const partNumber = index + 1;
    const signed = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${sessionId}/parts/${partNumber}/sign`,
    );
    assert.equal(signed.response.status, 200);
    assert.equal(signed.body.part_number, partNumber);
    const uploaded = await fetch(signed.body.upload_url, {
      method: "PUT",
      body: part,
    });
    assert.equal(uploaded.status, 200);
    const etag = uploaded.headers.get("etag");
    assert.ok(etag);
    uploadedParts.push({ part_number: partNumber, etag });
  }

  const wholeFileHash = createHash("sha256").update(expected).digest("hex");
  assert.notEqual(uploadedParts[0].etag.replaceAll("\"", ""), wholeFileHash);
  const completed = await jsonRequest(
    "POST",
    `/api/uploads/multipart/${sessionId}/complete`,
    { parts: uploadedParts },
  );
  assert.equal(completed.response.status, 200);
  assert.equal(completed.body.status, "complete");
  assert.equal(completed.body.expected_size, expected.length);
  assert.equal(completed.body.content_type, "application/epub+zip");

  const full = await fetch(`${baseUrl}/epubs/${fileName}`, {
    headers: { authorization: `Bearer ${apiToken}` },
  });
  assert.equal(full.status, 200);
  assert.equal(full.headers.get("content-type"), "application/epub+zip");
  assert.deepEqual(Buffer.from(await full.arrayBuffer()), expected);

  const range = await fetch(`${baseUrl}/epubs/${fileName}`, {
    headers: {
      authorization: `Bearer ${apiToken}`,
      range: "bytes=1000-1999",
    },
  });
  assert.equal(range.status, 206);
  assert.equal(range.headers.get("content-range"), `bytes 1000-1999/${expected.length}`);
  assert.deepEqual(Buffer.from(await range.arrayBuffer()), expected.subarray(1000, 2000));

  const abortedInit = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: partSize,
    content_type: "application/epub+zip",
  });
  assert.equal(abortedInit.response.status, 200);
  const aborted = await jsonRequest(
    "DELETE",
    `/api/uploads/multipart/${abortedInit.body.id}`,
  );
  assert.equal(aborted.response.status, 204);
  const afterAbort = await jsonRequest(
    "POST",
    `/api/uploads/multipart/${abortedInit.body.id}/parts/1/sign`,
  );
  assert.equal(afterAbort.response.status, 409);
});

test("multipart completion rejects invalid part ordering", { skip: !enabled }, async () => {
  const initialized = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: partSize,
    content_type: "application/epub+zip",
  });
  assert.equal(initialized.response.status, 200);
  const invalid = await jsonRequest(
    "POST",
    `/api/uploads/multipart/${initialized.body.id}/complete`,
    {
      parts: [
        { part_number: 2, etag: "etag-2" },
        { part_number: 2, etag: "etag-2-duplicate" },
      ],
    },
  );
  assert.equal(invalid.response.status, 400);
  const aborted = await jsonRequest(
    "DELETE",
    `/api/uploads/multipart/${initialized.body.id}`,
  );
  assert.equal(aborted.response.status, 204);
});
