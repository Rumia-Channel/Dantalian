import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";

async function request(path, token) {
  const headers = token === undefined ? undefined : { authorization: `Bearer ${token}` };
  const response = await fetch(`${baseUrl}${path}`, { headers });
  const text = await response.text();
  return {
    status: response.status,
    body: text.length === 0 ? undefined : JSON.parse(text),
  };
}

test("authentication protects API routes while keeping health public", async () => {
  if (process.env.WORKER_EXPECT_UNCONFIGURED === "true") {
    return;
  }
  const health = await request("/api/health");
  assert.equal(health.status, 200);
  assert.equal(typeof health.body.authentication_required, "boolean");
  if (process.env.WORKER_EXPECT_AUTH_REQUIRED !== undefined) {
    assert.equal(
      health.body.authentication_required,
      process.env.WORKER_EXPECT_AUTH_REQUIRED === "true",
    );
  }
  const missing = await request("/api/series");
  assert.equal(missing.status, 401);
  assert.equal(missing.body.code, "authentication_required");

  const wrong = await request("/api/series", "wrong-token");
  assert.equal(wrong.status, 401);
  assert.equal(wrong.body.code, "authentication_required");

  const correct = await request("/api/series", apiToken);
  assert.equal(correct.status, 200);
});

test("authentication fails closed when required configuration is missing", async () => {
  if (process.env.WORKER_EXPECT_UNCONFIGURED !== "true") {
    return;
  }
  const response = await request("/api/series");
  assert.equal(response.status, 500);
  assert.equal(response.body.code, "authentication_not_configured");
});
