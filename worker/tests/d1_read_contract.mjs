import assert from "node:assert/strict";
import test from "node:test";
import { randomInt } from "node:crypto";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const token = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";

async function request(method, path, body) {
  const response = await fetch(baseUrl + path, {
    method,
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  return { status: response.status, body: text ? JSON.parse(text) : null };
}

async function create(path, body, cleanup) {
  const result = await request("POST", path, body);
  assert.equal(result.status, 201, JSON.stringify(result));
  const item = result.body.book ?? result.body.cd ?? result.body;
  cleanup.unshift(() => request("DELETE", `${path.replace(/\/manual$/, "")}/${item.id}`));
  return item;
}
async function clean(cleanup) { for (const remove of cleanup) await remove(); }
function isbn() { return "978" + String(randomInt(10_000_000_000)).padStart(10, "0"); }

// Native exposes album-owned fields differently. This preserves the existing
// Worker contract while consolidating its metadata reads into one D1 statement.
test("Worker metadata retains per-track overrides and technical tags, including a track without a metadata row", async () => {
  const created = await request("POST", "/api/cds", {
    title: "Worker inherited album", publisher: "Inherited publisher", label: "Inherited label",
  });
  assert.equal(created.status, 201);
  const cd = created.body.cd;
  try {
    assert.equal((await request("PUT", `/api/cds/${cd.id}/metadata`, { year: 2025, genre: "Inherited genre" })).status, 204);
    const added = await request("POST", `/api/cds/${cd.id}/tracks`, { track_number: 1, title: "Fresh track" });
    assert.equal(added.status, 201);
    const path = `/api/cds/${cd.id}/tracks/${added.body.id}/metadata`;
    const initial = await request("GET", path);
    assert.equal(initial.status, 200);
    assert.equal(initial.body.album, "Worker inherited album");
    assert.equal(initial.body.year, 2025);
    assert.deepEqual(initial.body.artists, []);
    const saved = await request("PUT", path, {
      title: "Per-track title", album: "Track album", year: 2001, label: null,
      sample_rate: 48000, channels: 2, tags: { REVIEW_NOTE: "Custom metadata" },
    });
    assert.equal(saved.status, 204);
    const fetched = await request("GET", path);
    assert.equal(fetched.status, 200);
    assert.equal(fetched.body.title, "Per-track title");
    assert.equal(fetched.body.album, "Track album");
    assert.equal(fetched.body.year, 2001);
    assert.equal(fetched.body.label, "Inherited label");
    assert.equal(fetched.body.genre, "Inherited genre");
    assert.equal(fetched.body.sample_rate, 48000);
    assert.equal(fetched.body.channels, 2);
    assert.deepEqual(fetched.body.tags, { REVIEW_NOTE: "Custom metadata" });
    assert.equal((await request("PUT", path, { album: null, year: null })).status, 204);
    const cleared = await request("GET", path);
    assert.equal(cleared.body.album, "Worker inherited album");
    assert.equal(cleared.body.year, 2025);
    assert.deepEqual(cleared.body.tags, { REVIEW_NOTE: "Custom metadata" });
    assert.equal((await request("GET", `/api/cds/2000000000/tracks/${added.body.id}/metadata`)).status, 404);
  } finally {
    await request("DELETE", `/api/cds/${cd.id}`);
  }
});

test("book summary primary author follows display order after reordering", async () => {
  const cleanup = [];
  try {
    const first = await create("/api/authors", { name: "Book secondary artist" }, cleanup);
    const second = await create("/api/authors", { name: "Book preferred artist" }, cleanup);
    const book = await create("/api/books/manual", { title: "Book author ordering", isbn: isbn() }, cleanup);
    const path = `/api/books/${book.id}/authors`;
    assert.equal((await request("POST", `${path}/${first.id}`)).status, 204);
    assert.equal((await request("POST", `${path}/${second.id}`)).status, 204);
    assert.equal((await request("PUT", `${path}/${first.id}`, { sort_order: 5 })).status, 204);
    const page = await request("GET", "/api/books?limit=100");
    assert.equal(page.status, 200);
    const summary = page.body.items.find(item => item.id === book.id);
    assert.equal(summary.primary_author_name, second.name);
    assert.deepEqual(summary.authors.map(author => author.id).sort((a,b) => a-b), [first.id, second.id]);
    assert.equal((await request("PUT", `${path}/${second.id}`, { sort_order: 9 })).status, 204);
    const reordered = (await request("GET", "/api/books?limit=100")).body.items.find(item => item.id === book.id);
    assert.equal(reordered.primary_author_name, first.name);
  } finally { await clean(cleanup); }
});
