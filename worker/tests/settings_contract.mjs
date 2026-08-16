import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";

async function request(method, path, body) {
  const headers = { authorization: `Bearer ${apiToken}` };
  if (body !== undefined) headers["content-type"] = "application/json";
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let parsed;
  try {
    parsed = text.length === 0 ? undefined : JSON.parse(text);
  } catch {
    parsed = text;
  }
  return { status: response.status, body: parsed };
}

test("Worker settings keep data saver configuration available", async () => {
  const response = await request("PUT", "/api/settings", {
    "audio.data_saver.enabled": "true",
    "audio.data_saver.extensions": "wav,flac",
  });
  assert.equal(response.status, 200);
  assert.equal(response.body["audio.data_saver.enabled"], "true");
  assert.equal(response.body["audio.data_saver.extensions"], "wav,flac");
  assert.equal(response.body["upload.audio_max_mb"], undefined);
  assert.equal(response.body["backup.enabled"], undefined);
  assert.equal(response.body["media_sync.enabled"], undefined);
});

test("Worker settings reject native-only configuration", async () => {
  for (const key of ["upload.audio_max_mb", "backup.enabled", "media_sync.enabled"]) {
    const response = await request("PUT", "/api/settings", { [key]: "true" });
    assert.equal(response.status, 400);
    assert.match(response.body.error, /not configurable in Worker runtime/i);
  }
});

test("Worker media synchronization endpoint is unavailable", async () => {
  const response = await request("POST", "/api/media-sync/run");
  assert.equal(response.status, 409);
  assert.match(response.body.error, /not available in Worker runtime/i);
});
