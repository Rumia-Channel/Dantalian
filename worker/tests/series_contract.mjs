import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";

async function request(method, path, body) {
  const headers = { authorization: `Bearer ${apiToken}` };
  if (body !== undefined) {
    headers["content-type"] = "application/json";
  }
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  return {
    status: response.status,
    body: text.length === 0 ? undefined : JSON.parse(text),
  };
}

test("series HTTP contract", async () => {
  const name = `contract-${Date.now()}`;
  const created = await request("POST", "/api/series", { name });
  assert.equal(created.status, 201);
  assert.equal(created.body.name, name);
  const id = created.body.id;

  assert.equal((await request("GET", "/api/series")).status, 200);
  assert.equal((await request("POST", "/api/series", { name: "" })).status, 400);
  assert.equal((await request("PUT", `/api/series/${id}`, { name: `${name}-renamed` })).status, 204);
  assert.equal((await request("PUT", "/api/series/999999999", { name: "missing" })).status, 404);
  assert.equal((await request("PUT", "/api/series/not-an-id", { name: "invalid" })).status, 400);
  assert.equal((await request("DELETE", `/api/series/${id}`)).status, 204);
  assert.equal((await request("DELETE", `/api/series/${id}`)).status, 404);
  assert.equal((await request("DELETE", "/api/series/not-an-id")).status, 400);

  const malformed = await fetch(`${baseUrl}/api/series`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${apiToken}`,
    },
    body: "{",
  });
  assert.equal(malformed.status, 400);
});

// Audiobooks/CDs mirror books: they can be attached to a user series and carry
// a series volume, which the catalogue uses to order the series.
test("CD/audiobook series volume HTTP contract", async () => {
  const seriesName = `cd-series-${Date.now()}`;
  const series = await request("POST", "/api/series", { name: seriesName });
  assert.equal(series.status, 201);
  const seriesId = series.body.id;

  const created = await request("POST", "/api/cds", {
    title: `cd-series-contract-${Date.now()}`,
    media_type: "audiobook",
    series_id: seriesId,
    series_number: 2,
  });
  assert.equal(created.status, 201);
  const cdId = created.body.cd?.id ?? created.body.id;
  assert.ok(cdId, JSON.stringify(created.body));

  let list = await request("GET", "/api/cds");
  assert.equal(list.status, 200);
  let row = list.body.find((cd) => cd.id === cdId);
  assert.equal(row.series_id, seriesId);
  assert.equal(row.series_number, 2);

  // Volume-only update keeps the series assignment.
  assert.equal((await request("PUT", `/api/cds/${cdId}`, { title: row.title, series_number: 5 })).status, 204);
  list = await request("GET", "/api/cds");
  row = list.body.find((cd) => cd.id === cdId);
  assert.equal(row.series_id, seriesId);
  assert.equal(row.series_number, 5);

  // Clearing both values must round-trip as null.
  assert.equal(
    (await request("PUT", `/api/cds/${cdId}`, { title: row.title, series_id: null, series_number: null })).status,
    204,
  );
  list = await request("GET", "/api/cds");
  row = list.body.find((cd) => cd.id === cdId);
  assert.equal(row.series_id, null);
  assert.equal(row.series_number, null);

  await request("DELETE", `/api/cds/${cdId}`);
  await request("DELETE", `/api/series/${seriesId}`);
});
