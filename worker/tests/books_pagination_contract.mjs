import test from "node:test";
import assert from "node:assert/strict";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";
const authHeaders = {
  authorization: `Bearer ${apiToken}`,
  "content-type": "application/json",
};

async function request(method, path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: authHeaders,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  return {
    status: response.status,
    body: text.length === 0 ? undefined : JSON.parse(text),
  };
}

test("Books API returns summary pages without N+1 detail fields", async () => {
  const ids = [];
  const marker = Date.now().toString();
  try {
    for (let index = 0; index < 3; index += 1) {
      const created = await request("POST", "/api/books/manual", {
        isbn: `books-page-${marker}-${index}`,
        title: `Books page ${marker}-${index}`,
      });
      assert.equal(created.status, 201);
      ids.push(created.body.book.id);
    }

    const first = await request("GET", "/api/books?limit=1");
    assert.equal(first.status, 200);
    assert.ok(Array.isArray(first.body.items));
    assert.equal(first.body.items.length, 1);
    assert.equal(first.body.items[0].authors, undefined);
    assert.equal(typeof first.body.items[0].copies_count, "number");
    assert.equal(typeof first.body.items[0].lent_count, "number");
    assert.ok(first.body.next_cursor);

    const seen = new Set();
    let page = first.body;
    while (page) {
      for (const item of page.items) {
        if (ids.includes(item.id)) seen.add(item.id);
      }
      page = page.next_cursor
        ? (await request("GET", `/api/books?limit=1&cursor=${encodeURIComponent(page.next_cursor)}`)).body
        : null;
    }
    assert.deepEqual([...seen].sort((a, b) => a - b), [...ids].sort((a, b) => a - b));

    const detail = await request("GET", `/api/books/${ids[0]}`);
    assert.equal(detail.status, 200);
    assert.ok(Array.isArray(detail.body.authors));

    const invalid = await request("GET", "/api/books?cursor=invalid");
    assert.equal(invalid.status, 400);
    const invalidLimit = await request("GET", "/api/books?limit=0");
    assert.equal(invalidLimit.status, 400);
  } finally {
    for (const id of ids) {
      await request("DELETE", `/api/books/${id}`);
    }
  }
});
