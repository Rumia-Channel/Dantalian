import test from "node:test";
import assert from "node:assert/strict";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "";
const enabled = process.env.WASABI_E2E === "1";
const authHeaders = {
  authorization: `Bearer ${apiToken}`,
  "content-type": "application/json",
};

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

function assertContentRange(response, start, end, total) {
  assert.equal(response.status, 206);
  assert.equal(response.headers.get("content-range"), `bytes ${start}-${end}/${total}`);
}

test("Wasabi basic object lifecycle and range reads", { skip: !enabled }, async () => {
  const bytes = Uint8Array.from({ length: 2_048 }, (_, index) => (index * 37) % 256);
  const contentType = "image/png";
  let bookId;
  let objectKey;
  let downloadUrl;

  try {
    const book = await jsonRequest("POST", "/api/books/manual", {
      isbn: `wasabi-e2e-${Date.now()}`,
      title: "Wasabi E2E object",
    });
    assert.equal(book.response.status, 201);
    bookId = book.body.book.id;

    const initialized = await jsonRequest("POST", "/api/uploads/covers/init", {
      content_type: contentType,
      extension: "png",
      size: bytes.length,
      book_id: bookId,
    });
    assert.equal(initialized.response.status, 200);
    objectKey = initialized.body.object_key;
    assert.match(objectKey, /^[A-Za-z0-9._=/-]+$/);

    const uploaded = await fetch(initialized.body.upload_url, {
      method: "PUT",
      headers: { "content-type": contentType },
      body: bytes,
    });
    assert.equal(uploaded.status, 200);

    const completed = await jsonRequest("POST", "/api/uploads/covers/complete", {
      object_key: objectKey,
    });
    assert.equal(completed.response.status, 200);
    assert.equal(completed.body.object_key, objectKey);
    downloadUrl = completed.body.download_url;

    const head = await fetch(completed.body.head_url, { method: "HEAD" });
    assert.equal(head.status, 200);
    assert.equal(Number(head.headers.get("content-length")), bytes.length);
    assert.equal(head.headers.get("content-type"), contentType);

    const full = await fetch(downloadUrl);
    assert.equal(full.status, 200);
    assert.equal(full.headers.get("content-type"), contentType);
    assert.deepEqual(new Uint8Array(await full.arrayBuffer()), bytes);

    for (const [start, end] of [[0, 99], [100, 199]]) {
      const range = await fetch(downloadUrl, {
        headers: { range: `bytes=${start}-${end}` },
      });
      assertContentRange(range, start, end, bytes.length);
      assert.deepEqual(
        new Uint8Array(await range.arrayBuffer()),
        bytes.slice(start, end + 1),
      );
    }

    const tailStart = 1_000;
    const tail = await fetch(downloadUrl, {
      headers: { range: `bytes=${tailStart}-` },
    });
    assertContentRange(tail, tailStart, bytes.length - 1, bytes.length);
    assert.deepEqual(new Uint8Array(await tail.arrayBuffer()), bytes.slice(tailStart));
    const deleted = await jsonRequest("DELETE", `/api/books/${bookId}/cover`);
    assert.equal(deleted.response.status, 204);


    const afterDelete = await fetch(downloadUrl);
    assert.equal(afterDelete.status, 404);
    const afterDeleteHead = await fetch(completed.body.head_url, { method: "HEAD" });
    assert.equal(afterDeleteHead.status, 404);
  } finally {
    if (bookId !== undefined) {
      await jsonRequest("DELETE", `/api/books/${bookId}`);
    }
  }
});
