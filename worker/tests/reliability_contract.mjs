import assert from "node:assert/strict";
import test from "node:test";
import { randomInt } from "node:crypto";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const token = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";
async function request(method, path, body) {
  const response = await fetch(baseUrl + path, {
    method, headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let data;
  try { data = text ? JSON.parse(text) : undefined; } catch { data = text; }
  return { status: response.status, body: data };
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

// The same HTTP contract also runs against the Native binary via WORKER_BASE_URL.
test("book patches retain omitted data and reject invalid relationships without partial writes", async () => {
  const cleanup = [];
  try {
    const series = await create("/api/series", { name: "Reliability series" }, cleanup);
    const group = await create("/api/grand-series", { name: "Reliability group" }, cleanup);
    const book = await create("/api/books/manual", { title: "Original book", isbn: isbn() }, cleanup);
    const path = `/api/books/${book.id}`;
    assert.equal((await request("PUT", path, {
      title: "Original book", jan: "4900000000001", series_title: "Keep catalog series",
      artist: "Keep artist", catalog_number: "CAT-1", isdn_useroption: "Keep option",
      grand_series_id: group.id,
    })).status, 204);
    assert.equal((await request("PUT", path, { title: "Renamed book" })).status, 204);
    let fetched = (await request("GET", path)).body;
    assert.equal(fetched.jan, "4900000000001");
    assert.equal(fetched.series_title, "Keep catalog series");
    assert.equal(fetched.artist, "Keep artist");
    assert.equal(fetched.catalog_number, "CAT-1");
    assert.equal(fetched.isdn_useroption, "Keep option");
    assert.equal((await request("PUT", path, { title: "Renamed book", jan: null, artist: null })).status, 204);
    fetched = (await request("GET", path)).body;
    assert.equal(fetched.jan, null);
    assert.equal(fetched.artist, null);
    assert.equal((await request("PUT", `${path}/series`, { series_id: series.id, series_number: 7 })).status, 204);
    assert.equal((await request("PUT", `${path}/series`, { series_id: null })).status, 204);
    fetched = (await request("GET", path)).body;
    assert.equal(fetched.series_id, null);
    assert.equal(fetched.series_number, null);
    assert.equal((await request("PUT", path, { title: "Must not commit", grand_series_id: 2_000_000_000 })).status, 400);
    assert.equal((await request("GET", path)).body.title, "Renamed book");
    let groups = await request("GET", "/api/grand-series");
    assert.equal(groups.status, 200);
    assert.ok(groups.body.find(g => g.id === group.id).items.some(i => i.item_type === "book" && i.item_id === book.id));
    assert.equal((await request("POST", `/api/grand-series/${group.id}/items`, { item_type: "cd", item_id: 2_000_000_000 })).status, 404);
    assert.equal((await request("DELETE", path)).status, 204);
    groups = await request("GET", "/api/grand-series");
    assert.equal(groups.status, 200);
    assert.deepEqual(groups.body.find(g => g.id === group.id).items, []);
    assert.equal((await request("PUT", "/api/books/2000000000/series", { series_id: null })).status, 404);
    assert.equal((await request("PUT", "/api/grand-series/2000000000", { name: "Missing" })).status, 404);
  } finally { await clean(cleanup); }
});

test("copy and borrower patches distinguish omitted notes from null, and lending preserves the UI status", async () => {
  const cleanup = [];
  try {
    const book = await create("/api/books/manual", { title: "Lending book", isbn: isbn() }, cleanup);
    const borrower = await create("/api/borrowers", { name: "Borrower", notes: "Keep borrower notes" }, cleanup);
    const createdCopy = await request("POST", `/api/books/${book.id}/copies`, { copy_type: "physical", location: "Shelf", notes: "Keep copy notes" });
    assert.equal(createdCopy.status, 201);
    const copy = createdCopy.body;
    assert.equal((await request("PUT", `/api/copies/${copy.id}`, { copy_type: "ebook" })).status, 204);
    let listed = (await request("GET", `/api/books/${book.id}/copies`)).body;
    let row = listed.find(c => c.id === copy.id);
    assert.equal(row.location, "Shelf"); assert.equal(row.notes, "Keep copy notes");
    assert.equal((await request("PUT", `/api/copies/${copy.id}`, { location: null, notes: null })).status, 204);
    row = (await request("GET", `/api/books/${book.id}/copies`)).body.find(c => c.id === copy.id);
    assert.equal(row.location, null); assert.equal(row.notes, null);
    assert.equal((await request("PUT", `/api/borrowers/${borrower.id}`, { name: "Renamed borrower" })).status, 204);
    let borrowers = (await request("GET", "/api/borrowers")).body;
    assert.equal(borrowers.find(b => b.id === borrower.id).notes, "Keep borrower notes");
    assert.equal((await request("PUT", `/api/borrowers/${borrower.id}`, { notes: null })).status, 204);
    borrowers = (await request("GET", "/api/borrowers")).body;
    assert.equal(borrowers.find(b => b.id === borrower.id).notes, null);
    const results = await Promise.all(Array.from({ length: 4 }, () => request("POST", `/api/copies/${copy.id}/lend`, { borrower_id: borrower.id })));
    assert.deepEqual(results.map(r => r.status).sort(), [204, 409, 409, 409]);
    listed = (await request("GET", `/api/books/${book.id}/copies`)).body.filter(c => c.id === copy.id);
    assert.equal(listed.length, 1);
    assert.equal(listed[0].lent_to, "Renamed borrower");
    assert.equal((await request("POST", `/api/copies/${copy.id}/return`)).status, 204);
    assert.equal((await request("POST", `/api/copies/${copy.id}/lend`, { borrower_id: borrower.id })).status, 204);
    const history = await request("GET", `/api/copies/${copy.id}/history`);
    assert.equal(history.status, 200);
    assert.equal(history.body.length, 2);
    assert.equal(history.body.filter(record => record.returned_date === null).length, 1);
    assert.equal((await request("POST", `/api/copies/${copy.id}/return`)).status, 204);
    assert.equal((await request("POST", `/api/books/${book.id}/copies`, { copy_type: "invalid" })).status, 400);
    assert.equal((await request("POST", "/api/books/2000000000/copies", { copy_type: "physical" })).status, 404);
  } finally { await clean(cleanup.reverse()); }
});

test("track metadata preserves omitted fields, clears null, and rolls back with invalid artists", async () => {
  const cleanup = [];
  try {
    const author = await create("/api/authors", { name: "Track author" }, cleanup);
    const cd = await create("/api/cds", { title: "Metadata album", tracks: [{ track_number: 1, title: "Original track" }] }, cleanup);
    const track = cd.tracks[0];
    const path = `/api/cds/${cd.id}/tracks/${track.id}/metadata`;
    const saved = await request("PUT", path, { title: "Keep title", lyrics: "Keep lyrics", comment: "Keep comment", artists: [author.id] });
    assert.ok([200, 204].includes(saved.status), JSON.stringify(saved));
    const failed = await request("PUT", path, { title: "Must not commit", artists: [2_000_000_000] });
    assert.equal(failed.status, 400);
    let fetched = (await request("GET", path)).body;
    assert.equal(fetched.title, "Keep title"); assert.equal(fetched.comment, "Keep comment");
    assert.deepEqual(fetched.artists.map(a => a.id), [author.id]);
    const cleared = await request("PUT", path, { lyrics: null });
    assert.ok([200, 204].includes(cleared.status));
    fetched = (await request("GET", path)).body;
    assert.equal(fetched.lyrics, null); assert.equal(fetched.comment, "Keep comment"); assert.equal(fetched.title, "Keep title");
    assert.equal((await request("PUT", path, { artists: "invalid", comment: "Must not commit" })).status, 400);
    assert.equal((await request("GET", path)).body.comment, "Keep comment");
    assert.equal((await request("GET", `/api/cds/2000000000/tracks/${track.id}/metadata`)).status, 404);
    assert.equal((await request("DELETE", `/api/cds/2000000000/tracks/${track.id}`)).status, 404);
    assert.equal((await request("PUT", `/api/books/2000000000/tracks/${track.id}`, { title: "Wrong parent" })).status, 404);
    const tracks = (await request("GET", `/api/cds/${cd.id}/tracks`)).body;
    assert.equal(tracks.find(t => t.id === track.id).title, "Original track");
  } finally { await clean(cleanup); }
});

test("artists-only metadata patches remain readable for newly added tracks", async () => {
  const cleanup = [];
  try {
    const author = await create("/api/authors", { name: "Artists-only author" }, cleanup);
    const cd = await create("/api/cds", { title: "Artists-only album" }, cleanup);
    const added = await request("POST", `/api/cds/${cd.id}/tracks`, { track_number: 1, title: "Fresh track" });
    assert.ok([200, 201].includes(added.status), JSON.stringify(added));
    const track = added.body;
    const path = `/api/cds/${cd.id}/tracks/${track.id}/metadata`;
    const saved = await request("PUT", path, { artists: [author.id] });
    assert.ok([200, 204].includes(saved.status), JSON.stringify(saved));
    const fetched = await request("GET", path);
    assert.equal(fetched.status, 200);
    assert.deepEqual(fetched.body.artists.map(artist => artist.id), [author.id]);
  } finally { await clean(cleanup); }
});

test("metadata patches preserve supported audio byte counts beyond signed 32 bits", async () => {
  const cleanup = [];
  try {
    const cd = await create("/api/cds", { title: "Large audio album", tracks: [{ track_number: 1, title: "Original track" }] }, cleanup);
    const path = `/api/cds/${cd.id}/tracks/${cd.tracks[0].id}/metadata`;
    const size = 2 ** 31;
    const saved = await request("PUT", path, { title: "Large audio metadata", raw_size_bytes: size });
    assert.ok([200, 204].includes(saved.status), JSON.stringify(saved));
    const fetched = await request("GET", path);
    assert.equal(fetched.status, 200);
    assert.equal(fetched.body.title, "Large audio metadata");
    assert.equal(fetched.body.raw_size_bytes, size);
  } finally { await clean(cleanup); }
});
