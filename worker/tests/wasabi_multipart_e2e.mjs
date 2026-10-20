import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "";
const enabled = process.env.WASABI_E2E === "1";
const testPrefix = process.env.WASABI_TEST_PREFIX ?? "";
const partSize = 8 * 1024 * 1024;

async function jsonRequest(method, path, body, token = apiToken) {
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

// The browser skips the Worker for large EPUBs: it creates a generic EPUB
// multipart session, PUTs the parts straight to Wasabi, and the finalize step
// binds the completed object to the book row.
test("Wasabi EPUB multipart finalize binds the object to a book", { skip: !enabled }, async () => {
  const parts = [makePart(partSize, 5), makePart(1024 * 1024, 13)];
  const expected = Buffer.concat(parts);
  let bookId;

  try {
    const book = await jsonRequest("POST", "/api/books/manual", {
      isbn: `wasabi-epub-${Date.now()}`,
      title: "Wasabi EPUB multipart finalize",
    });
    assert.equal(book.response.status, 201);
    bookId = book.body.book.id;

    const initialized = await jsonRequest("POST", "/api/uploads/multipart/init", {
      expected_size: expected.length,
      content_type: "application/epub+zip",
      object_kind: "epub",
      file_name: "Finalized Book.epub",
    });
    assert.equal(initialized.response.status, 200);
    const sessionId = initialized.body.id;
    const objectKey = initialized.body.object_key;
    const objectName = objectKey.split("/").at(-1);

    // Nothing may be attached before the multipart completion.
    const premature = await jsonRequest("POST", `/api/books/${bookId}/epub/finalize`, {
      session_id: sessionId,
      name: "Finalized Book.epub",
    });
    assert.equal(premature.response.status, 409);

    const uploadedParts = [];
    for (const [index, part] of parts.entries()) {
      const partNumber = index + 1;
      const signed = await jsonRequest(
        "POST",
        `/api/uploads/multipart/${sessionId}/parts/${partNumber}/sign`,
      );
      assert.equal(signed.response.status, 200);
      const uploaded = await fetch(signed.body.upload_url, {
        method: "PUT",
        body: part,
      });
      assert.equal(uploaded.status, 200);
      const etag = uploaded.headers.get("etag");
      assert.ok(etag);
      uploadedParts.push({ part_number: partNumber, etag });
    }

    const completed = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${sessionId}/complete`,
      { parts: uploadedParts },
    );
    assert.equal(completed.response.status, 200);

    const finalized = await jsonRequest("POST", `/api/books/${bookId}/epub/finalize`, {
      session_id: sessionId,
      name: "Finalized Book.epub",
    });
    assert.equal(finalized.response.status, 200);
    assert.equal(finalized.body.object_key, objectKey);
    assert.equal(finalized.body.file_hash, objectName);

    const loaded = await jsonRequest("GET", `/api/books/${bookId}`);
    assert.equal(loaded.response.status, 200);
    assert.equal(loaded.body.epub_file_hash, objectName);
    assert.equal(loaded.body.epub_file_name, "Finalized Book.epub");

    // One object must not be attached twice.
    const repeated = await jsonRequest("POST", `/api/books/${bookId}/epub/finalize`, {
      session_id: sessionId,
      name: "Finalized Book.epub",
    });
    assert.equal(repeated.response.status, 409);

    const full = await fetch(`${baseUrl}/epubs/${encodeURIComponent(objectName)}`, {
      headers: { authorization: `Bearer ${apiToken}` },
    });
    assert.equal(full.status, 200);
    assert.deepEqual(Buffer.from(await full.arrayBuffer()), expected);
  } finally {
    if (bookId !== undefined) {
      await jsonRequest("DELETE", `/api/books/${bookId}`);
    }
  }
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

test("multipart rejects invalid sessions, parts, ownership, and completion metadata", { skip: !enabled }, async () => {
  const invalidSession = await jsonRequest(
    "POST",
    "/api/uploads/multipart/not-a-session/parts/1/sign",
  );
  assert.equal(invalidSession.response.status, 400);

  const invalidPart = await jsonRequest(
    "POST",
    `/api/uploads/multipart/${"0".repeat(32)}/parts/0/sign`,
  );
  assert.equal(invalidPart.response.status, 400);

  const unauthorizedInit = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: partSize,
    content_type: "application/epub+zip",
  });
  assert.equal(unauthorizedInit.response.status, 200);
  const unauthorizedSessionId = unauthorizedInit.body.id;

  try {
    const unauthorized = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${unauthorizedSessionId}/parts/1/sign`,
      undefined,
      "wrong-token",
    );
    assert.equal(unauthorized.response.status, 401);

    const duplicate = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${unauthorizedSessionId}/complete`,
      {
        parts: [
          { part_number: 1, etag: "etag-1" },
          { part_number: 1, etag: "etag-1-duplicate" },
        ],
      },
    );
    assert.equal(duplicate.response.status, 400);
  } finally {
    const aborted = await jsonRequest(
      "DELETE",
      `/api/uploads/multipart/${unauthorizedSessionId}`,
    );
    assert.equal(aborted.response.status, 204);
  }

  const wrongEtagInit = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: partSize,
    content_type: "application/epub+zip",
  });
  assert.equal(wrongEtagInit.response.status, 200);
  const wrongEtagSessionId = wrongEtagInit.body.id;
  try {
    const signed = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${wrongEtagSessionId}/parts/1/sign`,
    );
    assert.equal(signed.response.status, 200);
    const uploaded = await fetch(signed.body.upload_url, {
      method: "PUT",
      body: makePart(partSize, 31),
    });
    assert.equal(uploaded.status, 200);

    const wrongEtag = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${wrongEtagSessionId}/complete`,
      { parts: [{ part_number: 1, etag: "wrong-etag" }] },
    );
    assert.notEqual(wrongEtag.response.status, 200);
  } finally {
    const aborted = await jsonRequest(
      "DELETE",
      `/api/uploads/multipart/${wrongEtagSessionId}`,
    );
    assert.ok([204, 409].includes(aborted.response.status));
  }

  const missingPartInit = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: partSize * 2,
    content_type: "application/epub+zip",
  });
  assert.equal(missingPartInit.response.status, 200);
  const missingPartSessionId = missingPartInit.body.id;
  try {
    const signed = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${missingPartSessionId}/parts/1/sign`,
    );
    assert.equal(signed.response.status, 200);
    const uploaded = await fetch(signed.body.upload_url, {
      method: "PUT",
      body: makePart(partSize, 47),
    });
    assert.equal(uploaded.status, 200);
    const etag = uploaded.headers.get("etag");
    assert.ok(etag);

    const missingPart = await jsonRequest(
      "POST",
      `/api/uploads/multipart/${missingPartSessionId}/complete`,
      { parts: [{ part_number: 1, etag }] },
    );
    assert.equal(missingPart.response.status, 409);
  } finally {
    const aborted = await jsonRequest(
      "DELETE",
      `/api/uploads/multipart/${missingPartSessionId}`,
    );
    assert.ok([204, 409].includes(aborted.response.status));
  }

  const abortInit = await jsonRequest("POST", "/api/uploads/multipart/init", {
    expected_size: partSize,
    content_type: "application/epub+zip",
  });
  assert.equal(abortInit.response.status, 200);
  const abortSessionId = abortInit.body.id;
  const aborted = await jsonRequest(
    "DELETE",
    `/api/uploads/multipart/${abortSessionId}`,
  );
  assert.equal(aborted.response.status, 204);
  const completeAfterAbort = await jsonRequest(
    "POST",
    `/api/uploads/multipart/${abortSessionId}/complete`,
    { parts: [] },
  );
  assert.equal(completeAfterAbort.response.status, 409);
});
