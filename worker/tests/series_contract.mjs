import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";

async function request(method, path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
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
    headers: { "content-type": "application/json" },
    body: "{",
  });
  assert.equal(malformed.status, 400);
});
