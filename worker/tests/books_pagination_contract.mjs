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
    assert.deepEqual(first.body.items[0].authors, []);
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

    // Linked authors must appear on summary items with IDs so clients can
    // group books and CDs by author ID instead of name text.
    const author = await request("POST", "/api/authors", { name: `page-author-${marker}` });
    assert.equal(author.status, 201);
    try {
      const linked = await request("POST", `/api/books/${ids[0]}/authors/${author.body.id}`);
      assert.ok(linked.status === 200 || linked.status === 204, `link author: ${linked.status}`);
      const page = await request("GET", "/api/books?limit=100");
      const item = page.body.items.find((entry) => entry.id === ids[0]);
      assert.ok(item, "linked book appears on summary page");
      assert.ok(Array.isArray(item.authors), "summary item carries authors array");
      assert.ok(
        item.authors.some((entry) => entry.id === author.body.id && entry.name === `page-author-${marker}`),
        "summary authors carry the linked author ID",
      );
    } finally {
      await request("DELETE", `/api/authors/${author.body.id}`);
    }

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
