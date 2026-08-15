import test from "node:test";
import assert from "node:assert/strict";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN;
const isbn = process.env.AMAZON_TEST_ISBN ?? "9784041164693";

if (!apiToken) {
  throw new Error("WORKER_API_TOKEN is required");
}

const authHeaders = {
  authorization: `Bearer ${apiToken}`,
  "content-type": "application/json",
};

test("ISBN registration stores the Amazon physical-book cover", async () => {
  const response = await fetch(`${baseUrl}/api/books`, {
    method: "POST",
    headers: authHeaders,
    body: JSON.stringify({ isbn }),
    signal: AbortSignal.timeout(90_000),
  });
  const body = await response.json();

  assert.equal(response.status, 201, JSON.stringify(body));
  assert.equal(body.source, "amazon", JSON.stringify(body));
  assert.equal(body.book?.isbn, isbn, JSON.stringify(body));
  assert.match(body.book?.cover_url ?? "", /\.[a-z0-9]+$/i, JSON.stringify(body));

  const bookId = body.book?.id;
  assert.ok(Number.isInteger(bookId) && bookId > 0, JSON.stringify(body));

  try {
    const coverResponse = await fetch(`${baseUrl}/api/books/${bookId}/cover`, {
      headers: { authorization: `Bearer ${apiToken}` },
      signal: AbortSignal.timeout(30_000),
    });
    assert.equal(coverResponse.status, 200);
    assert.match(
      coverResponse.headers.get("content-type") ?? "",
      /^image\//,
    );
    assert.ok(Number(coverResponse.headers.get("content-length") ?? 0) > 0);
  } finally {
    const cleanup = await fetch(`${baseUrl}/api/books/${bookId}`, {
      method: "DELETE",
      headers: { authorization: `Bearer ${apiToken}` },
      signal: AbortSignal.timeout(30_000),
    });
    assert.equal(cleanup.status, 204, await cleanup.text());
  }
});
